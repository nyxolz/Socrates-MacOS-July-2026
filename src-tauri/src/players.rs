//! Player discovery + character cache.
//!
//! The ESP loop runs at 60 Hz and can't afford to re-walk the Workspace
//! every frame. So a separate "cache" thread builds `CharCache` entries
//! every ~700 ms — each entry pins the resolved Primitive pointer for
//! every part of the rig. The ESP loop only does `read::<f32>(prim + off)`.
//!
//! Three rig templates are tried per character; the one with the most
//! matched parts wins (so an R6 rig isn't mis-tagged as R15 just because
//! "Head" alone matches the R15 table).

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use obfstr::obfstr;
use tauri::{AppHandle, Manager};

use crate::offsets;
use crate::sdk::{get_children, Instance, SdkI};
use crate::state::AppState;

// ─── Rig tables ────────────────────────────────────────────────────────

pub const PART_NAMES_R15: [&str; 11] = [
    "Head", "UpperTorso", "LowerTorso",
    "LeftUpperArm", "LeftLowerArm",
    "RightUpperArm", "RightLowerArm",
    "LeftUpperLeg", "LeftLowerLeg",
    "RightUpperLeg", "RightLowerLeg",
];

pub const PART_NAMES_R6: [&str; 6] = [
    "Head", "Torso",
    "Left Arm", "Right Arm",
    "Left Leg", "Right Leg",
];

/// Fallback used when neither R15 nor R6 templates match (custom rigs).
pub const PART_NAMES_HRP: [&str; 2] = ["Head", "HumanoidRootPart"];

/// Edges drawn for an R15 rig (indices into `CharCache.parts`).
///   0 head, 1 upper_torso, 2 lower_torso,
///   3 LU_arm, 4 LL_arm, 5 RU_arm, 6 RL_arm,
///   7 LU_leg, 8 LL_leg, 9 RU_leg, 10 RL_leg
pub const SKELETON_SEGMENTS_R15: [(usize, usize); 10] = [
    (0, 1), (1, 2),
    (1, 3), (3, 4),
    (1, 5), (5, 6),
    (2, 7), (7, 8),
    (2, 9), (9, 10),
];

/// Edges drawn for an R6 rig:
///   0 head, 1 torso, 2 left_arm, 3 right_arm, 4 left_leg, 5 right_leg
pub const SKELETON_SEGMENTS_R6: [(usize, usize); 5] = [
    (0, 1),
    (1, 2), (1, 3),
    (1, 4), (1, 5),
];

/// Degenerate fallback — just a line between Head and HumanoidRootPart.
pub const SKELETON_SEGMENTS_HRP: [(usize, usize); 1] = [(0, 1)];

// ─── Cache entry ───────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CharCache {
    pub char_addr: u64,
    pub name: String,
    /// `(primitive_ptr, valid)` per slot. Slot order matches the rig table.
    pub parts: [(u64, bool); 11],
    /// 0 = R15, 1 = R6, 2 = HRP fallback.
    pub rig_type: u8,
    pub humanoid_addr: u64,
    /// HumanoidRootPart primitive — the only part whose LookVector reliably
    /// matches the character's facing. Spectate / camera-aim need this; the
    /// head's LookVector can pivot independently and gives wrong results.
    /// 0 if HRP wasn't found.
    pub hrp_primitive: u64,
    pub last_validated: Instant,
}

impl CharCache {
    pub fn segments(&self) -> &'static [(usize, usize)] {
        match self.rig_type {
            0 => &SKELETON_SEGMENTS_R15,
            1 => &SKELETON_SEGMENTS_R6,
            _ => &SKELETON_SEGMENTS_HRP,
        }
    }
}

// ─── Fast readers ──────────────────────────────────────────────────────

/// Like `find_first_child` but compares names byte-by-byte without ever
/// allocating a `String` — significantly cheaper inside the cache rebuild.
pub fn find_child_fast(sdk: &SdkI, instance: u64, target: &str) -> u64 {
    let container = sdk.read::<u64>(instance + Instance::ChildrenList as u64);
    if !sdk.is_valid_ptr(container) { return 0; }
    let array_begin = sdk.read::<u64>(container);
    let array_end   = sdk.read::<u64>(container + 8);
    if !sdk.is_valid_ptr(array_begin) || array_end <= array_begin { return 0; }
    let count = ((array_end - array_begin) / 8).min(2000);
    let target_bytes = target.as_bytes();
    let target_len = target_bytes.len();
    if target_len > 32 { return 0; }
    for i in 0..count {
        let child = sdk.read::<u64>(array_begin + i * 8);
        if !sdk.is_valid_ptr(child) { continue; }
        if sdk.read::<u64>(child + Instance::Parent as u64) != instance { continue; }
        let name_ptr = sdk.read::<u64>(child + Instance::Name as u64);
        if !sdk.is_valid_ptr(name_ptr) { continue; }
        let mut buf = [0u8; 33];
        for j in 0..(target_len + 1) {
            buf[j] = sdk.read::<u8>(name_ptr + j as u64);
        }
        if &buf[..target_len] == target_bytes && buf[target_len] == 0 {
            return child;
        }
    }
    0
}

pub fn read_name_fast(sdk: &SdkI, instance: u64) -> String {
    let name_ptr = sdk.read::<u64>(instance + Instance::Name as u64);
    if !sdk.is_valid_ptr(name_ptr) { return String::new(); }
    let mut bytes = [0u8; 64];
    for i in 0..64u64 {
        bytes[i as usize] = sdk.read::<u8>(name_ptr + i);
        if bytes[i as usize] == 0 { break; }
    }
    bytes.iter().take_while(|&&b| b != 0 && b >= 32 && b <= 126)
        .map(|&b| b as char).collect()
}

/// Read a `Vec3 f32` at `primitive_ptr + Primitive::Position` (current
/// offset = 0x134 per the table in `offsets.rs`).
pub fn read_pos_fast(sdk: &SdkI, primitive: u64) -> [f32; 3] {
    let off = offsets::Primitive::Position as u64;
    [
        sdk.read::<f32>(primitive + off),
        sdk.read::<f32>(primitive + off + 4),
        sdk.read::<f32>(primitive + off + 8),
    ]
}

// ─── Cache build ───────────────────────────────────────────────────────

/// Resolve `parts[i]` for every name in `names`. Returns the number of
/// matched slots so callers can compare templates.
fn try_parts_count(
    sdk: &SdkI,
    char_addr: u64,
    names: &[&str],
    parts: &mut [(u64, bool); 11],
) -> usize {
    let mut count = 0;
    for (i, name) in names.iter().enumerate() {
        if i >= 11 { break; }
        let part_addr = find_child_fast(sdk, char_addr, name);
        if part_addr > 0 {
            let prim = sdk.read::<u64>(part_addr + offsets::BasePart::Primitive as u64);
            if sdk.is_valid_ptr(prim) {
                parts[i] = (prim, true);
                count += 1;
            }
        }
    }
    count
}

pub fn build_char_cache(sdk: &SdkI, char_addr: u64, humanoid_addr: u64) -> Option<CharCache> {
    let mut best_rig: u8 = u8::MAX;
    let mut best_score: usize = 0;
    let mut best_parts = [(0u64, false); 11];

    // R15 — need ≥3 R15-specific parts so an R6 rig that only matches "Head"
    // doesn't win this template.
    let mut r15_parts = [(0u64, false); 11];
    let r15_score = try_parts_count(sdk, char_addr, &PART_NAMES_R15, &mut r15_parts);
    if r15_score >= 3 && r15_score > best_score {
        best_rig = 0; best_score = r15_score; best_parts = r15_parts;
    }

    let mut r6_parts = [(0u64, false); 11];
    let r6_score = try_parts_count(sdk, char_addr, &PART_NAMES_R6, &mut r6_parts);
    if r6_score >= 2 && r6_score > best_score {
        best_rig = 1; best_score = r6_score; best_parts = r6_parts;
    }

    let mut hrp_parts = [(0u64, false); 11];
    let hrp_score = try_parts_count(sdk, char_addr, &PART_NAMES_HRP, &mut hrp_parts);
    if hrp_score >= 1 && hrp_score > best_score {
        best_rig = 2; best_score = hrp_score; best_parts = hrp_parts;
    }

    if best_rig == u8::MAX || best_score == 0 { return None; }

    // HRP independent of the rig template — used by spectate for a stable
    // "behind the character" direction. Name kept obfuscated.
    let hrp_addr = find_child_fast(sdk, char_addr, obfstr!("HumanoidRootPart").as_ref());
    let hrp_primitive = if sdk.is_valid_ptr(hrp_addr) {
        let p = sdk.read::<u64>(hrp_addr + offsets::BasePart::Primitive as u64);
        if sdk.is_valid_ptr(p) { p } else { 0 }
    } else { 0 };

    Some(CharCache {
        char_addr,
        name: read_name_fast(sdk, char_addr),
        parts: best_parts,
        rig_type: best_rig,
        humanoid_addr,
        hrp_primitive,
        last_validated: Instant::now(),
    })
}

/// Walk Workspace direct children and keep the ones that have a `Humanoid`.
pub fn discover_characters(sdk: &SdkI, workspace: u64) -> Vec<CharCache> {
    let mut caches = Vec::new();
    let workspace_children = get_children(sdk, workspace);
    let humanoid_name = obfstr!("Humanoid").to_string();
    for c in workspace_children.into_iter().take(200) {
        let humanoid = find_child_fast(sdk, c, &humanoid_name);
        if !sdk.is_valid_ptr(humanoid) { continue; }
        if let Some(cache) = build_char_cache(sdk, c, humanoid) {
            caches.push(cache);
        }
    }
    caches
}

// ─── Off-thread cache rebuild ─────────────────────────────────────────

/// Refreshes `AppState.char_caches` every 700 ms. The ESP loop only ever
/// clones the `Arc<Vec<CharCache>>` (atomic refcount bump) so it never
/// pays for the Workspace walk.
pub fn start_char_cache_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        loop {
            let state = app_handle.state::<AppState>();
            let sdk_opt = state.sdk.lock().unwrap().clone();
            if let Some(sdk) = sdk_opt {
                if let Some(workspace) = resolve_workspace(&sdk) {
                    let caches = discover_characters(&sdk, workspace);
                    *state.char_caches.lock().unwrap() = Arc::new(caches);
                }
            }
            thread::sleep(Duration::from_millis(700));
        }
    });
}

fn resolve_workspace(sdk: &SdkI) -> Option<u64> {
    crate::security::resolve_workspace(sdk)
}
