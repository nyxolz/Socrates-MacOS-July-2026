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

/// Limb-tip extensions: `(from, to, factor)`. R15 has no dedicated hand/foot
/// part, so its arm/leg bones stop at the *lower* part's centre (mid-forearm
/// / mid-shin). Each entry draws an extra bone starting at `to` and reaching
/// `factor × |to − from|` further along the (from → to) direction, so the
/// skeleton covers the whole arm down to the hand and the whole leg down to
/// the foot.
pub const SKELETON_TIPS_R15: [(usize, usize, f32); 4] = [
    (3, 4, 0.85),   // left forearm → hand
    (5, 6, 0.85),   // right forearm → hand
    (7, 8, 0.95),   // left shin → foot
    (9, 10, 0.95),  // right shin → foot
];

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

    /// Extra limb-tip bones extending arms/legs to hands/feet. Only R15
    /// needs them (R6 limbs are single parts, HRP has no limbs).
    pub fn tips(&self) -> &'static [(usize, usize, f32)] {
        match self.rig_type {
            0 => &SKELETON_TIPS_R15,
            _ => &[],
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

/// Per-container child cap, shared by the full walk and the shallow rescan.
const MAX_CHILDREN_PER_NODE: usize = 256;

/// `(instance, humanoid_ptr)` if `instance` is a character (owns a `Humanoid`
/// or a `HumanoidRootPart` child), else `None`. `humanoid_ptr` is 0 when only
/// an HRP was found — `build_char_cache` then reports health as unknown.
fn character_marker(sdk: &SdkI, instance: u64, humanoid_name: &str, hrp_name: &str) -> Option<u64> {
    let humanoid = find_child_fast(sdk, instance, humanoid_name);
    if sdk.is_valid_ptr(humanoid) { return Some(humanoid); }
    if sdk.is_valid_ptr(find_child_fast(sdk, instance, hrp_name)) { return Some(0); }
    None
}

/// Recursively walk the Workspace subtree and keep every node that owns a
/// `Humanoid` *or* a `HumanoidRootPart` child — that node is a character and
/// its `Name` is the owning player's name. Walking the whole tree (not just
/// Workspace's direct children) makes discovery work in games that nest
/// characters under folders ("Players", "Living", a map model, …) instead of
/// parenting them straight to Workspace.
///
/// Returns the characters **and** the de-duplicated set of parent containers
/// that held them, so the cache thread can remember those paths and rescan
/// them cheaply next time instead of re-walking the whole tree.
///
/// Once a node is identified as a character we do *not* descend into it — its
/// own rig parts would otherwise be mistaken for sub-characters. `local_name`
/// (the local player's username) is filtered out here, the single point of
/// truth for both ESP and aimbot.
///
/// Bounded by depth / per-node / total-node caps so a huge Workspace can't
/// stall the cache thread.
pub fn discover_characters(sdk: &SdkI, workspace: u64, local_name: &str) -> (Vec<CharCache>, Vec<u64>) {
    const MAX_DEPTH: u32 = 6;
    const MAX_TOTAL_NODES: usize = 3000;

    let humanoid_name = obfstr!("Humanoid").to_string();
    let hrp_name = obfstr!("HumanoidRootPart").to_string();

    let mut caches = Vec::new();
    let mut containers: Vec<u64> = Vec::new();
    let mut total = 0usize;
    // Iterative DFS over `(node, depth)` to avoid deep recursion on the stack.
    let mut stack: Vec<(u64, u32)> = vec![(workspace, 0)];

    while let Some((node, depth)) = stack.pop() {
        if total >= MAX_TOTAL_NODES { break; }
        for c in get_children(sdk, node).into_iter().take(MAX_CHILDREN_PER_NODE) {
            if total >= MAX_TOTAL_NODES { break; }
            total += 1;

            if let Some(humanoid) = character_marker(sdk, c, &humanoid_name, &hrp_name) {
                // Remember the parent path even if the only character here is
                // ourselves — it's still where players will appear.
                if !containers.contains(&node) { containers.push(node); }
                if local_name.is_empty() || read_name_fast(sdk, c) != local_name {
                    if let Some(cache) = build_char_cache(sdk, c, humanoid) {
                        caches.push(cache);
                    }
                }
                // A character's children are its body parts — never recurse in.
                continue;
            }

            if depth + 1 < MAX_DEPTH {
                stack.push((c, depth + 1));
            }
        }
    }
    (caches, containers)
}

/// Cheap rescan of already-known parent containers: only their direct children
/// are checked for the character marker — no tree walk. Stale pointers (e.g.
/// after a new game) are skipped via `is_valid_ptr` and simply yield nothing,
/// which signals the caller to fall back to a full `discover_characters`.
pub fn discover_in_containers(sdk: &SdkI, containers: &[u64], local_name: &str) -> Vec<CharCache> {
    let humanoid_name = obfstr!("Humanoid").to_string();
    let hrp_name = obfstr!("HumanoidRootPart").to_string();

    let mut caches = Vec::new();
    for &container in containers {
        if !sdk.is_valid_ptr(container) { continue; }
        for c in get_children(sdk, container).into_iter().take(MAX_CHILDREN_PER_NODE) {
            if let Some(humanoid) = character_marker(sdk, c, &humanoid_name, &hrp_name) {
                if !local_name.is_empty() && read_name_fast(sdk, c) == local_name { continue; }
                if let Some(cache) = build_char_cache(sdk, c, humanoid) {
                    caches.push(cache);
                }
            }
        }
    }
    caches
}

/// First child of `Players` is the LocalPlayer (Roblox convention — the
/// local player is inserted first on join). Its `Name` is the local user's
/// username, which we then strip from the char cache.
pub fn resolve_local_player_name(sdk: &SdkI) -> String {
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return String::new(); }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return String::new(); }
    let players = find_child_fast(sdk, dm, obfstr!("Players").as_ref());
    if !sdk.is_valid_ptr(players) { return String::new(); }
    let container = sdk.read::<u64>(players + Instance::ChildrenList as u64);
    if !sdk.is_valid_ptr(container) { return String::new(); }
    let array_begin = sdk.read::<u64>(container);
    let array_end = sdk.read::<u64>(container + 8);
    if !sdk.is_valid_ptr(array_begin) || array_end <= array_begin { return String::new(); }
    let count = ((array_end - array_begin) / 8).min(2000);
    for i in 0..count {
        let child = sdk.read::<u64>(array_begin + i * 8);
        if !sdk.is_valid_ptr(child) { continue; }
        if sdk.read::<u64>(child + Instance::Parent as u64) != players { continue; }
        let name = read_name_fast(sdk, child);
        if !name.is_empty() { return name; }
    }
    String::new()
}

// ─── Off-thread cache rebuild ─────────────────────────────────────────

/// Refreshes `AppState.char_caches`. The ESP loop only ever clones the
/// `Arc<Vec<CharCache>>` (atomic refcount bump) so it never pays for the walk.
///
/// To stay fast *and* survive a new game (which empties the list when the old
/// containers are freed), it works in two modes:
///   * **Remembered path** — once a full walk locates the parent container(s)
///     that hold characters, later ticks just rescan those containers' direct
///     children (`discover_in_containers`), skipping the whole-tree walk.
///   * **Recovery** — if the rescan comes back empty (new game, stale paths),
///     fall back to a full `discover_characters`, relearn the containers, and
///     poll every 500 ms until players reappear.
/// A periodic full walk also runs every few seconds so containers added later
/// (e.g. a second character folder) are still picked up.
pub fn start_char_cache_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        let mut known_containers: Vec<u64> = Vec::new();
        let mut last_full_walk = Instant::now() - Duration::from_secs(60);
        loop {
            let mut empty = true;
            let state = app_handle.state::<AppState>();
            let sdk_opt = state.sdk.lock().unwrap().clone();
            if let Some(sdk) = sdk_opt {
                if let Some(workspace) = resolve_workspace(&sdk) {
                    let local_name = resolve_local_player_name(&sdk);
                    *state.local_player_name.lock().unwrap() = local_name.clone();

                    // Cheap path first: rescan remembered containers.
                    let mut caches = if known_containers.is_empty() {
                        Vec::new()
                    } else {
                        discover_in_containers(&sdk, &known_containers, &local_name)
                    };

                    // Full walk when the cheap path found nothing (first run,
                    // new game, or stale paths) or on a periodic refresh so
                    // newly-added containers are still discovered.
                    let refresh_due = last_full_walk.elapsed() >= Duration::from_secs(4);
                    if caches.is_empty() || refresh_due {
                        let (full, containers) = discover_characters(&sdk, workspace, &local_name);
                        last_full_walk = Instant::now();
                        if !containers.is_empty() { known_containers = containers; }
                        // Keep whichever found more (refresh may run while the
                        // cheap path already had results).
                        if full.len() >= caches.len() { caches = full; }
                    }

                    empty = caches.is_empty();
                    *state.char_caches.lock().unwrap() = Arc::new(caches);
                }
            }
            // Poll fast (0.5 s) while the list is empty so a fresh game recovers
            // quickly; settle to 700 ms once players are visible.
            thread::sleep(Duration::from_millis(if empty { 500 } else { 700 }));
        }
    });
}

fn resolve_workspace(sdk: &SdkI) -> Option<u64> {
    crate::security::resolve_workspace(sdk)
}
