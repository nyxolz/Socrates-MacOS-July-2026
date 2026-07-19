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

use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use obfstr::obfstr;
use tauri::{AppHandle, Manager};

use crate::offsets;
use crate::sdk::{get_children, Instance, SdkI};
use crate::security;
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
    /// Parent container (instance addr) this character is parented to. Used by
    /// Team check to compare against the local player's parent container.
    pub parent_addr: u64,
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
//
// `find_child_fast` and `read_name_fast` are on the per-tick discovery hot
// path, so they bypass `sdk::read_string` and inline the byte-level reads.
// The ABI of the Name field differs per arch, so we keep two impls:
//   • ARM   : Name is a raw `char*` — one deref to the chars.
//   • Intel : Name is a pointer to a 24-byte libc++ `std::string` —
//             SSO when `byte0 & 1 == 0` (chars at strobj+1), else long-form
//             (size at strobj+8, char* at strobj+16). Same logic as
//             `sdk::instance::read_string`, just inlined to avoid the call.

/// Intel-only helper: given the *strobj pointer the Name field holds, return
/// the address of the first character and the string length, or None if the
/// std::string header doesn't look clean.
#[cfg(target_arch = "x86_64")]
fn intel_name_locate(sdk: &SdkI, strobj: u64) -> Option<(u64, u64)> {
    if !sdk.is_valid_ptr(strobj) { return None; }
    let byte0 = sdk.read::<u8>(strobj);
    if byte0 & 1 == 0 {
        let size = (byte0 >> 1) as u64;
        if size == 0 || size > 22 { return None; }
        Some((strobj + 1, size))
    } else {
        let size = sdk.read::<u64>(strobj + 8);
        if size == 0 || size > 4096 { return None; }
        let data_ptr = sdk.read::<u64>(strobj + 16);
        if !sdk.is_valid_ptr(data_ptr) { return None; }
        Some((data_ptr, size.min(256)))
    }
}

/// Like `find_first_child` but compares names byte-by-byte without ever
/// allocating a `String` — significantly cheaper inside the cache rebuild.
#[cfg(target_arch = "aarch64")]
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
        // One bulk read of `target_len + 1` bytes instead of up to 33
        // individual 1-byte syscalls per candidate child.
        let buf = sdk.vm_read_bulk(name_ptr, target_len + 1);
        if buf.len() == target_len + 1 && &buf[..target_len] == target_bytes && buf[target_len] == 0 {
            return child;
        }
    }
    0
}

#[cfg(target_arch = "x86_64")]
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
        let strobj = sdk.read::<u64>(child + Instance::Name as u64);
        let (data_addr, len) = match intel_name_locate(sdk, strobj) {
            Some(v) => v, None => continue,
        };
        if len as usize != target_len { continue; }
        // One bulk read instead of up to 32 individual 1-byte syscalls.
        let buf = sdk.vm_read_bulk(data_addr, target_len);
        if buf.len() == target_len && buf.as_slice() == target_bytes {
            return child;
        }
    }
    0
}

#[cfg(target_arch = "aarch64")]
pub fn read_name_fast(sdk: &SdkI, instance: u64) -> String {
    let name_ptr = sdk.read::<u64>(instance + Instance::Name as u64);
    if !sdk.is_valid_ptr(name_ptr) { return String::new(); }
    // One bulk read instead of up to 64 individual 1-byte syscalls.
    let bytes = sdk.vm_read_bulk(name_ptr, 64);
    bytes.iter().take_while(|&&b| b != 0 && b >= 32 && b <= 126)
        .map(|&b| b as char).collect()
}

#[cfg(target_arch = "x86_64")]
pub fn read_name_fast(sdk: &SdkI, instance: u64) -> String {
    let strobj = sdk.read::<u64>(instance + Instance::Name as u64);
    let (data_addr, len) = match intel_name_locate(sdk, strobj) {
        Some(v) => v, None => return String::new(),
    };
    // One bulk read instead of up to `len` individual 1-byte syscalls.
    let bytes = sdk.vm_read_bulk(data_addr, len.min(256) as usize);
    bytes.iter().take_while(|&&b| b != 0 && (32..=126).contains(&b))
        .map(|&b| b as char).collect()
}

/// Read a `Vec3 f32` at `primitive_ptr + Primitive::Position` in a single
/// `vm_read_overwrite` syscall (was 3 separate ones). This is the single
/// hottest read in the whole SDK — called for every part of every player,
/// every tick, in ESP, aimbot targeting, and cache building — so cutting it
/// 3x cuts overall SDK syscall volume by a wide margin.
pub fn read_pos_fast(sdk: &SdkI, primitive: u64) -> [f32; 3] {
    let off = offsets::Primitive::Position as u64;
    sdk.read::<[f32; 3]>(primitive + off)
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
        parent_addr: sdk.read::<u64>(char_addr + Instance::Parent as u64),
        last_validated: Instant::now(),
    })
}

/// Per-container child cap, shared by the full walk and the shallow rescan.
const MAX_CHILDREN_PER_NODE: usize = 256;

/// BETA hard-code: training-dummy rigs own a `Humanoid` / `HumanoidRootPart`
/// just like a real character, so the marker check treats them as players.
/// They aren't — skip them by exact name so they never reach ESP / aimbot /
/// the player list. Acts as if the node simply weren't a character.
fn is_ignored_character(name: &str) -> bool {
    matches!(name, "Dummy" | "DPS Dummy")
}

/// `false` once a character's `Health` drops to (or below) 0 — a dead
/// character is dropped from the cache so ESP/aimbot stop tracking it.
/// Unknown humanoid (`0`, e.g. HRP+Head/Health-only rigs) is treated as alive.
fn is_alive(sdk: &SdkI, humanoid_addr: u64) -> bool {
    if !sdk.is_valid_ptr(humanoid_addr) { return true; }
    sdk.read::<f32>(humanoid_addr + offsets::Humanoid::Health as u64) > 0.0
}

/// `(instance, humanoid_ptr)` if `instance` is a character, else `None`.
///
/// A node is a character when it owns a `Humanoid`, or — for custom rigs that
/// lack one — when it owns **both** a `HumanoidRootPart` *and* a `Head`, or
/// when it owns a `Health` child (some custom rigs expose Health directly on
/// the model instead of via a `Humanoid`).
/// Requiring the `Head` alongside the HRP filters out stray models/parts that
/// happen to carry a lone `HumanoidRootPart` without being a real character.
/// `humanoid_ptr` is 0 when only the HRP+Head pair or the `Health` child was
/// found — `build_char_cache` then reports health as unknown.
fn character_marker(
    sdk: &SdkI,
    instance: u64,
    humanoid_name: &str,
    hrp_name: &str,
    head_name: &str,
    health_name: &str,
) -> Option<u64> {
    let humanoid = find_child_fast(sdk, instance, humanoid_name);
    if sdk.is_valid_ptr(humanoid) { return Some(humanoid); }
    if sdk.is_valid_ptr(find_child_fast(sdk, instance, hrp_name))
        && sdk.is_valid_ptr(find_child_fast(sdk, instance, head_name))
    {
        return Some(0);
    }
    if sdk.is_valid_ptr(find_child_fast(sdk, instance, health_name)) {
        return Some(0);
    }
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
/// Traversal is **breadth-first** on purpose: character containers (a `Players`
/// folder, a `Characters` folder, `Living`, a map model, …) sit near the top
/// of the tree, while the bulk of a Workspace is deep static geometry. BFS
/// visits the shallow levels first, so those containers are reached before the
/// per-walk node budget gets spent diving into deep map parts — a plain DFS
/// would plunge into the first big model and could exhaust the budget before
/// ever touching the `Players → Characters` branch.
///
/// Bounded by depth / per-node / total-node caps so a huge Workspace can't
/// stall the cache thread.
pub fn discover_characters(sdk: &SdkI, workspace: u64, local_name: &str, player_names: &std::collections::HashSet<String>) -> (Vec<CharCache>, Vec<u64>, u64, u64) {
    const MAX_DEPTH: u32 = 8;
    const MAX_TOTAL_NODES: usize = 6000;

    let humanoid_name = obfstr!("Humanoid").to_string();
    let hrp_name = obfstr!("HumanoidRootPart").to_string();
    let head_name = obfstr!("Head").to_string();
    let health_name = obfstr!("Health").to_string();

    let mut caches = Vec::new();
    let mut containers: Vec<u64> = Vec::new();
    // Parent container holding the local player's character (for Team check).
    let mut local_parent: u64 = 0;
    // Humanoid instance addr of our own character (for the Misc tab).
    let mut local_humanoid: u64 = 0;
    let mut total = 0usize;
    // Iterative BFS over `(node, depth)`: shallow levels first.
    let mut queue: VecDeque<(u64, u32)> = VecDeque::new();
    queue.push_back((workspace, 0));

    while let Some((node, depth)) = queue.pop_front() {
        if total >= MAX_TOTAL_NODES { break; }
        for c in get_children(sdk, node).into_iter().take(MAX_CHILDREN_PER_NODE) {
            if total >= MAX_TOTAL_NODES { break; }
            total += 1;

            if let Some(humanoid) = character_marker(sdk, c, &humanoid_name, &hrp_name, &head_name, &health_name) {
                let cname = read_name_fast(sdk, c);
                // Real-player gate: when we know the Players roster, a rig whose
                // name isn't on it is an NPC / dummy / mob — skip it entirely
                // (don't even remember its container).
                let is_real_player = player_names.is_empty() || player_names.contains(&cname);
                if !is_ignored_character(&cname) && is_real_player {
                    // Remember the parent path even if the only character here is
                    // ourselves — it's still where players will appear.
                    if !containers.contains(&node) { containers.push(node); }
                    if !local_name.is_empty() && cname == local_name {
                        // Found ourselves — remember our parent container so
                        // Team check can flag same-parent characters as allies,
                        // and our Humanoid addr for the Misc tab.
                        local_parent = node;
                        local_humanoid = humanoid;
                    } else if local_name.is_empty() || cname != local_name {
                        if is_alive(sdk, humanoid) {
                            if let Some(cache) = build_char_cache(sdk, c, humanoid) {
                                caches.push(cache);
                            }
                        }
                    }
                }
                // A character's children are its body parts (a dummy's too) —
                // never recurse in.
                continue;
            }

            if depth + 1 < MAX_DEPTH {
                queue.push_back((c, depth + 1));
            }
        }
    }
    (caches, containers, local_parent, local_humanoid)
}

/// Team-check relation resolver. When `team_check` is on, any character sharing
/// the local player's parent container is treated as an ally (1); otherwise the
/// manually-set `manual` relation is returned unchanged (0 = enemy default).
pub fn effective_relation(manual: u8, team_check: bool, cache_parent: u64, local_parent: u64) -> u8 {
    if team_check && local_parent != 0 && cache_parent == local_parent {
        1
    } else {
        manual
    }
}

/// Cheap rescan of already-known parent containers: only their direct children
/// are checked for the character marker — no tree walk. Stale pointers (e.g.
/// after a new game) are skipped via `is_valid_ptr` and simply yield nothing,
/// which signals the caller to fall back to a full `discover_characters`.
pub fn discover_in_containers(sdk: &SdkI, containers: &[u64], local_name: &str, player_names: &std::collections::HashSet<String>) -> Vec<CharCache> {
    let humanoid_name = obfstr!("Humanoid").to_string();
    let hrp_name = obfstr!("HumanoidRootPart").to_string();
    let head_name = obfstr!("Head").to_string();
    let health_name = obfstr!("Health").to_string();

    let mut caches = Vec::new();
    for &container in containers {
        if !sdk.is_valid_ptr(container) { continue; }
        for c in get_children(sdk, container).into_iter().take(MAX_CHILDREN_PER_NODE) {
            if let Some(humanoid) = character_marker(sdk, c, &humanoid_name, &hrp_name, &head_name, &health_name) {
                let cname = read_name_fast(sdk, c);
                if is_ignored_character(&cname) { continue; }
                // Real-player gate (see discover_characters): reject rigs that
                // aren't in the Players roster when we know it.
                if !player_names.is_empty() && !player_names.contains(&cname) { continue; }
                if !local_name.is_empty() && cname == local_name { continue; }
                if !is_alive(sdk, humanoid) { continue; }
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
    let dm = match security::resolve_data_model(sdk) {
        Some(v) => v, None => return String::new(),
    };
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

/// Names of every child of the `Players` service — i.e. the set of real
/// players currently in the server (the `Player` instances Roblox owns, not
/// their Workspace characters). Character discovery uses this to reject any
/// humanoid rig whose name isn't a real player (NPCs, dummies, mob models):
/// if a character's `Name` isn't in this set, it isn't a player, so it never
/// reaches ESP / aimbot.
///
/// Returns an empty set when `Players` can't be resolved; callers treat an
/// empty set as "unknown" and skip the filter rather than hiding everyone.
pub fn resolve_player_names(sdk: &SdkI) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let dm = match security::resolve_data_model(sdk) {
        Some(v) => v, None => return out,
    };
    let players = find_child_fast(sdk, dm, obfstr!("Players").as_ref());
    if !sdk.is_valid_ptr(players) { return out; }
    for child in get_children(sdk, players) {
        let name = read_name_fast(sdk, child);
        if !name.is_empty() { out.insert(name); }
    }
    out
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
            state.diag_cache_ticks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            state.diag_cache_known_containers.store(known_containers.len(), std::sync::atomic::Ordering::Relaxed);

            // "Reset ESP" requested: forget remembered containers and the live
            // cache, then fall through to a full cold-start walk this tick.
            if state.esp_reset_requested.swap(false, std::sync::atomic::Ordering::Relaxed) {
                known_containers.clear();
                last_full_walk = Instant::now() - Duration::from_secs(60);
                *state.char_caches.lock().unwrap() = Arc::new(Vec::new());
                state.local_parent.store(0, std::sync::atomic::Ordering::Relaxed);
                state.local_humanoid_addr.store(0, std::sync::atomic::Ordering::Relaxed);
            }

            let sdk_opt = state.sdk.lock().unwrap().clone();
            if let Some(sdk) = sdk_opt {
                if let Some(workspace) = resolve_workspace(&sdk) {
                    let local_name = resolve_local_player_name(&sdk);
                    *state.local_player_name.lock().unwrap() = local_name.clone();

                    // Roster of real players (children of the Players service):
                    // any humanoid rig whose name isn't here is an NPC/dummy and
                    // is rejected from the cache (and thus from ESP/aimbot).
                    let player_names = resolve_player_names(&sdk);

                    // Cheap path first: rescan remembered containers.
                    let mut caches = if known_containers.is_empty() {
                        Vec::new()
                    } else {
                        discover_in_containers(&sdk, &known_containers, &local_name, &player_names)
                    };

                    // Full walk when the cheap path found nothing (first run,
                    // new game, or stale paths) or on a periodic refresh so
                    // newly-added containers are still discovered.
                    let refresh_due = last_full_walk.elapsed() >= Duration::from_secs(4);
                    if caches.is_empty() || refresh_due {
                        state.diag_cache_full_walks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let (full, containers, local_parent, local_humanoid) = discover_characters(&sdk, workspace, &local_name, &player_names);
                        last_full_walk = Instant::now();
                        if local_parent != 0 {
                            state.local_parent.store(local_parent, std::sync::atomic::Ordering::Relaxed);
                        }
                        if local_humanoid != 0 {
                            state.local_humanoid_addr.store(local_humanoid, std::sync::atomic::Ordering::Relaxed);
                        } else if !sdk.is_valid_ptr(state.local_humanoid_addr.load(std::sync::atomic::Ordering::Relaxed)) {
                            state.local_humanoid_addr.store(0, std::sync::atomic::Ordering::Relaxed);
                        }
                        if !containers.is_empty() { known_containers = containers; }
                        // Keep whichever found more (refresh may run while the
                        // cheap path already had results).
                        if full.len() >= caches.len() { caches = full; }
                    }

                    // BETA sticky-target : une passe de découverte rate parfois
                    // un joueur (occlusion dans le walk, conteneur qui change) ->
                    // il disparaissait de l'ESP/aimbot ~1 s puis revenait. On
                    // fusionne : on garde les joueurs déjà connus TOUJOURS vivants
                    // (Humanoid valide + Health > 0) que cette passe-ci n'a pas
                    // retrouvés. Un joueur n'est lâché qu'à sa mort / pointeurs KO.
                    {
                        let prev = state.char_caches.lock().unwrap().clone();
                        for old in prev.iter() {
                            if caches.iter().any(|c| c.char_addr == old.char_addr) { continue; }
                            // Never resurrect the local player: an early tick (before
                            // `local_name` resolved) may have captured ourselves, and
                            // the sticky-merge would otherwise keep us alive forever.
                            if !local_name.is_empty() && old.name == local_name { continue; }
                            if !sdk.is_valid_ptr(old.char_addr) || !sdk.is_valid_ptr(old.humanoid_addr) { continue; }
                            // Validation STRUCTURELLE (pas de timer) : on garde le
                            // joueur EN CONTINU tant qu'il est réellement vivant ->
                            // zéro flicker, retour instantané. Les liens cassent net
                            // à la mort / respawn / déco (mémoire libérée), donc la
                            // box fantôme disparaît immédiatement :
                            //   - le Humanoid est toujours enfant du perso,
                            //   - le perso est toujours rattaché à l'arbre,
                            //   - Health > 0.
                            if sdk.read::<u64>(old.humanoid_addr + Instance::Parent as u64) != old.char_addr { continue; }
                            if !sdk.is_valid_ptr(sdk.read::<u64>(old.char_addr + Instance::Parent as u64)) { continue; }
                            let hp = sdk.read::<f32>(old.humanoid_addr + crate::offsets::Humanoid::Health as u64);
                            if hp.is_finite() && hp > 0.0 {
                                caches.push(old.clone());
                            }
                        }
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

/// Misc tab — continuously re-applies the enabled overrides
/// (HipHeight / JumpPower / DisplayDistance / Fly) to our own
/// `Humanoid`. Re-writing every tick (rather than once) keeps the value
/// stuck even when Roblox's own scripts reset it (e.g. respawn, sprint
/// scripts toggling values back).
/// Force CanCollide = false on every part of the local character. The
/// character is the parent of the Humanoid; each BasePart child exposes its
/// Primitive at `+0x138`, and CanCollide is the u8 at `Primitive + 0x1B6`.
/// A finite, plausible `Size` read off the Primitive is used to tell real
/// parts from non-part children (Humanoid, scripts, …) so we never poke a
/// stray byte.
fn apply_noclip(sdk: &SdkI, humanoid: u64) {
    let character = sdk.read::<u64>(humanoid + Instance::Parent as u64);
    if !sdk.is_valid_ptr(character) { return; }
    for child in get_children(sdk, character) {
        let prim = sdk.read::<u64>(child + offsets::BasePart::Primitive as u64);
        if !sdk.is_valid_ptr(prim) { continue; }
        // Sanity-gate on Size so we only touch genuine parts.
        let sx = sdk.read::<f32>(prim + offsets::Primitive::Size as u64);
        if !sx.is_finite() || sx <= 0.0 || sx > 1000.0 { continue; }
        sdk.write::<u8>(prim + offsets::Primitive::CanCollide as u64, 0);
    }
}

/// Resolve the local character's HumanoidRootPart Primitive pointer
/// (humanoid's parent is the character; the HRP is its child).
fn local_hrp_primitive(sdk: &SdkI, humanoid: u64) -> Option<u64> {
    let character = sdk.read::<u64>(humanoid + Instance::Parent as u64);
    if !sdk.is_valid_ptr(character) { return None; }
    let hrp = find_child_fast(sdk, character, obfstr!("HumanoidRootPart").as_ref());
    if !sdk.is_valid_ptr(hrp) { return None; }
    let prim = sdk.read::<u64>(hrp + offsets::BasePart::Primitive as u64);
    if sdk.is_valid_ptr(prim) { Some(prim) } else { None }
}

/// Compute the ground point under the cursor: camera ray ∩ the horizontal
/// plane at the character's current height. Returns the world target, or
/// `None` when there's no camera / the cursor points above the horizon / a
/// read was invalid.
fn resolve_click_target(sdk: &SdkI, state: &AppState, humanoid: u64) -> Option<[f32; 3]> {
    let prim = local_hrp_primitive(sdk, humanoid)?;
    let cur = sdk.read::<[f32; 3]>(prim + offsets::Primitive::Position as u64);
    if !cur[0].is_finite() || !cur[1].is_finite() || !cur[2].is_finite() { return None; }

    let camera = security::resolve_camera(sdk)?;
    let cmo = offsets::Camera::CameraOrientation as u64;
    let raw = sdk.read::<crate::esp::CFrameRaw>(camera + cmo);
    let cam_pos = raw.pos;
    let m = raw.m;
    // Columns of the CFrame 3×3 are Right / Up / Back (see `aim_camera`).
    let right = [m[0][0], m[1][0], m[2][0]];
    let up = [m[0][1], m[1][1], m[2][1]];
    let forward = [-m[0][2], -m[1][2], -m[2][2]];

    let bounds = state.roblox_bounds.lock().unwrap().clone();
    let (win_x, win_y, win_w, win_h) = bounds.unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    let render_h = (win_h - crate::overlay::TITLEBAR_HEIGHT as f64).max(1.0);
    let aspect = (win_w / render_h) as f32;

    let cur_pt = crate::aimbot::current_cursor()?;
    // Cursor (global points) → NDC in [-1, +1].
    let nx = ((cur_pt.x - win_x) / win_w * 2.0 - 1.0) as f32;
    let ny = (1.0 - (cur_pt.y - win_y - crate::overlay::TITLEBAR_HEIGHT as f64) / render_h * 2.0) as f32;

    let zoom = sdk.read::<f32>(camera + offsets::Camera::Fov as u64 + 8);
    let fov_rad = crate::math::fov_from_zoom(zoom).to_radians();
    let tan_y = (fov_rad / 2.0).tan();
    let tan_x = aspect * tan_y;

    // Ray direction through the cursor, in world space.
    let mut dir = [
        forward[0] + nx * tan_x * right[0] + ny * tan_y * up[0],
        forward[1] + nx * tan_x * right[1] + ny * tan_y * up[1],
        forward[2] + nx * tan_x * right[2] + ny * tan_y * up[2],
    ];
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if !len.is_finite() || len < 1e-4 { return None; }
    dir = [dir[0] / len, dir[1] / len, dir[2] / len];

    // Intersect with the horizontal plane at the character's current height.
    let plane_y = cur[1];
    if dir[1].abs() < 1e-4 { return None; } // ray parallel to the ground
    let t = (plane_y - cam_pos[1]) / dir[1];
    if !t.is_finite() || t <= 0.0 { return None; } // point is behind / above the horizon

    let hit = [cam_pos[0] + dir[0] * t, plane_y, cam_pos[2] + dir[2] * t];
    if !hit[0].is_finite() || !hit[2].is_finite() { return None; }
    Some(hit)
}

/// Teleport the whole character to `target` by shifting every body part by the
/// same delta (target − current HRP position). Moving the entire assembly (not
/// just the HRP) keeps it rigid, so the physics solver has nothing to snap back
/// to. Self-correcting: called every tick during the hold window, so if
/// physics/replication drags the character part-way home the delta grows again
/// and we re-assert. Returns false if the character couldn't be resolved.
fn teleport_assembly(sdk: &SdkI, humanoid: u64, target: [f32; 3]) -> bool {
    let character = sdk.read::<u64>(humanoid + Instance::Parent as u64);
    if !sdk.is_valid_ptr(character) { return false; }
    let hrp_prim = match local_hrp_primitive(sdk, humanoid) { Some(p) => p, None => return false };

    let pos_off = offsets::Primitive::Position as u64;
    let cur = sdk.read::<[f32; 3]>(hrp_prim + pos_off);
    if !cur[0].is_finite() || !cur[1].is_finite() || !cur[2].is_finite() { return false; }

    let delta = [target[0] - cur[0], target[1] - cur[1], target[2] - cur[2]];
    // Already at the target (within 0.05 studs) — nothing to do.
    if delta[0].abs() < 0.05 && delta[1].abs() < 0.05 && delta[2].abs() < 0.05 { return true; }

    for child in get_children(sdk, character) {
        let prim = sdk.read::<u64>(child + offsets::BasePart::Primitive as u64);
        if !sdk.is_valid_ptr(prim) { continue; }
        // Sanity-gate on Size so we only shift genuine parts (skip Humanoid, scripts…).
        let sx = sdk.read::<f32>(prim + offsets::Primitive::Size as u64);
        if !sx.is_finite() || sx <= 0.0 || sx > 1000.0 { continue; }
        let p = sdk.read::<[f32; 3]>(prim + pos_off);
        if !p[0].is_finite() || !p[1].is_finite() || !p[2].is_finite() { continue; }
        sdk.write::<[f32; 3]>(prim + pos_off, [p[0] + delta[0], p[1] + delta[1], p[2] + delta[2]]);
    }
    true
}

pub fn start_misc_thread(app_handle: AppHandle) {
    let mut last_tick = Instant::now();
    // Extra HipHeight accumulated while the fly bind is held — climbs while
    // held, resets to 0 (back to `cfg.hip_height`) once released.
    let mut fly_offset: f32 = 0.0;
    // Previous click-TP key state, so we teleport once per press (rising edge)
    // instead of every tick the key is held.
    let mut tp_key_prev = false;
    // Active teleport: target world position + the instant to hold it until.
    // A single write gets overwritten by Roblox physics on the next frame and
    // the character snaps back, so we re-assert the target for a short window.
    let mut tp_target: Option<[f32; 3]> = None;
    let mut tp_hold_until = Instant::now();
    thread::spawn(move || loop {
        let dt = last_tick.elapsed().as_secs_f32();
        last_tick = Instant::now();
        let state = app_handle.state::<AppState>();
        let unlocked = state.key_unlocked.load(std::sync::atomic::Ordering::Relaxed);
        let sdk_opt = if unlocked { state.sdk.lock().unwrap().clone() } else { None };
        if let Some(sdk) = sdk_opt {
            let humanoid = state.local_humanoid_addr.load(std::sync::atomic::Ordering::Relaxed);
            if sdk.is_valid_ptr(humanoid) {
                let cfg = state.misc_cfg.lock().unwrap().clone();
                if cfg.jump_power_enabled {
                    sdk.write::<f32>(humanoid + offsets::Humanoid::JumpPower as u64, cfg.jump_power);
                }
                if cfg.walk_speed_enabled {
                    sdk.write::<f32>(humanoid + offsets::Humanoid::WalkSpeed as u64, cfg.walk_speed);
                }
                if cfg.display_distance_enabled {
                    sdk.write::<f32>(humanoid + offsets::Humanoid::DisplayDistance as u64, cfg.display_distance);
                }

                if cfg.fly_enabled && crate::input::is_held(&cfg.fly_keybind) {
                    fly_offset += cfg.fly_speed * dt;
                } else {
                    fly_offset = 0.0;
                }

                if cfg.hip_height_enabled || fly_offset > 0.0 {
                    sdk.write::<f32>(humanoid + offsets::Humanoid::HipHeight as u64, cfg.hip_height + fly_offset);
                }

                if cfg.noclip_enabled {
                    apply_noclip(&sdk, humanoid);
                }

                // Click teleport — on the key's rising edge, resolve the ground
                // point under the cursor and hold the character there for a
                // short window so physics/replication can't pull it back.
                if cfg.tp_click_enabled && !cfg.tp_click_keybind.is_empty() {
                    let down = crate::input::is_held(&cfg.tp_click_keybind);
                    if down && !tp_key_prev {
                        if let Some(target) = resolve_click_target(&sdk, state.inner(), humanoid) {
                            tp_target = Some(target);
                            tp_hold_until = Instant::now() + Duration::from_millis(500);
                        }
                    }
                    tp_key_prev = down;
                } else {
                    tp_key_prev = false;
                    tp_target = None;
                }

                // Re-assert the teleport target every tick until the hold expires.
                if let Some(target) = tp_target {
                    if Instant::now() < tp_hold_until {
                        teleport_assembly(&sdk, humanoid, target);
                    } else {
                        tp_target = None;
                    }
                }
            }
        }
        // Tick fast while a teleport is being held so we re-assert the position
        // on every physics frame; idle at 16 ms otherwise.
        let sleep_ms = if tp_target.is_some() { 3 } else { 16 };
        thread::sleep(Duration::from_millis(sleep_ms));
    });
}
