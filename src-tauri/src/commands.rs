//! Tauri command surface exposed to the SvelteKit frontend.
//!
//! Keep this thin — each command is just glue between the frontend and the
//! background threads / SDK. Feature work (aimbot config, ESP toggles, …)
//! gets its own command(s) added here as it lands.

use std::sync::Arc;

use obfstr::obfstr;
use serde::Serialize;
use tauri::State;

use crate::offsets;
use crate::overlay::InteractiveRect;
use crate::players::{find_child_fast, read_name_fast};
use crate::sdk::{self, SdkI};
use crate::state::{AimbotCfg, AppState, RobloxBounds, VisualsCfg, ZoneInput};

#[tauri::command]
pub fn get_roblox_bounds(state: State<AppState>) -> Option<RobloxBounds> {
    state.roblox_bounds.lock().unwrap()
        .map(|(x, y, width, height)| RobloxBounds { x, y, width, height })
}

#[tauri::command]
pub fn is_roblox_running(state: State<AppState>) -> bool {
    state.roblox_bounds.lock().unwrap().is_some()
}

/// Frontend publishes its current interactive rects (window-local pixels) on
/// mount and on every resize / drag. The mouse tracker reads this on each tick.
#[tauri::command]
pub fn set_interactive_zones(state: State<AppState>, zones: Vec<ZoneInput>) {
    let mut z = state.interactive_zones.lock().unwrap();
    z.clear();
    for r in zones {
        z.push(InteractiveRect { x: r.x, y: r.y, w: r.w, h: r.h });
    }
}

#[tauri::command]
pub fn panic_exit() {
    std::process::exit(0);
}

/// Attach to a running Roblox process. Stores the resulting SDK in AppState
/// for any later command / background thread to use. Returns the static base
/// address for sanity-checking from the UI.
#[tauri::command]
pub fn attach_sdk(state: State<AppState>, process_name: Option<String>) -> Result<String, String> {
    let name = process_name.unwrap_or_else(|| obfstr!("RobloxPlayer").to_string());
    let sdk = SdkI::new(&name)?;
    let base = sdk.base;
    *state.sdk.lock().unwrap() = Some(Arc::new(sdk));
    Ok(format!("0x{:X}", base))
}

#[tauri::command]
pub fn sdk_status(state: State<AppState>) -> Option<(String, u64, u64)> {
    let sdk = state.sdk.lock().unwrap().clone()?;
    let (ok, fail) = sdk.read_stats();
    Some((format!("0x{:X}", sdk.base), ok, fail))
}

#[tauri::command]
pub fn set_aimbot_config(state: State<AppState>, cfg: AimbotCfg) {
    *state.aimbot_cfg.lock().unwrap() = cfg;
}

#[tauri::command]
pub fn set_visuals_config(state: State<AppState>, cfg: VisualsCfg) {
    *state.visuals_cfg.lock().unwrap() = cfg;
}

#[tauri::command]
pub fn set_menu_toggle_bind(state: State<AppState>, bind: String) {
    *state.menu_toggle_bind.lock().unwrap() = bind;
}

// ─── Offset health check for the Output tab ─────────────────────────────

/// Returns `true` iff we can chain `Roblox.base → FakeDataModel → DataModel
/// → Workspace` and the resulting instance reads back the name "Workspace"
/// (which proves the offset table is still valid for this Roblox build).
#[tauri::command]
pub fn diag_check(state: State<AppState>) -> bool {
    let sdk = match state.sdk.lock().unwrap().clone() { Some(s) => s, None => return false };
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return false; }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return false; }
    let ws = sdk.read::<u64>(dm + offsets::DataModel::WorkSpace as u64);
    if !sdk.is_valid_ptr(ws) { return false; }
    sdk::name_of(&sdk, ws) == obfstr!("Workspace")
}

// ─── Player List + Spectate ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct PlayerEntry {
    pub char_addr: String,  // hex so JS can round-trip without losing precision
    pub name: String,
    pub distance: f32,
    pub health: f32,
}

#[tauri::command]
pub fn list_players(state: State<AppState>) -> Vec<PlayerEntry> {
    let sdk = state.sdk.lock().unwrap().clone();
    let sdk = match sdk { Some(s) => s, None => return vec![] };

    let camera_pos = state.aimbot_cfg.lock().unwrap();
    drop(camera_pos);
    // Compute distance from a fresh camera read — same chain as elsewhere.
    let cam_pos = resolve_cam_pos(&sdk).unwrap_or([0.0; 3]);

    let caches = state.char_caches.lock().unwrap().clone();
    let mut list: Vec<PlayerEntry> = caches.iter().filter_map(|c| {
        let (prim, valid) = c.parts[0];
        if !valid { return None; }
        let pos = crate::players::read_pos_fast(&sdk, prim);
        if !pos[0].is_finite() { return None; }
        if pos[0] == 0.0 && pos[1] == 0.0 && pos[2] == 0.0 { return None; }
        let dx = pos[0] - cam_pos[0];
        let dy = pos[1] - cam_pos[1];
        let dz = pos[2] - cam_pos[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance < 0.5 { return None; } // skip self / overlap
        let health = if sdk.is_valid_ptr(c.humanoid_addr) {
            let h = sdk.read::<f32>(c.humanoid_addr + offsets::Humanoid::Health as u64);
            if h.is_finite() && (0.0..=1000.0).contains(&h) { h.clamp(0.0, 100.0) } else { -1.0 }
        } else { -1.0 };
        Some(PlayerEntry {
            char_addr: format!("0x{:X}", c.char_addr),
            name: c.name.clone(),
            distance,
            health,
        })
    }).collect();
    list.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    list
}

fn resolve_cam_pos(sdk: &SdkI) -> Option<[f32; 3]> {
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return None; }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return None; }
    let ws = sdk.read::<u64>(dm + offsets::DataModel::WorkSpace as u64);
    if !sdk.is_valid_ptr(ws) { return None; }
    let cam = sdk.read::<u64>(ws + offsets::Workspace::Camera as u64);
    if !sdk.is_valid_ptr(cam) { return None; }
    let off = offsets::Camera::CameraPosition as u64;
    Some([
        sdk.read::<f32>(cam + off),
        sdk.read::<f32>(cam + off + 4),
        sdk.read::<f32>(cam + off + 8),
    ])
}

/// `target` is the hex char_addr from `list_players` or `null` to stop.
#[tauri::command]
pub fn set_spectate_target(state: State<AppState>, target: Option<String>) {
    let val = target
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0);
    *state.spectate_target.lock().unwrap() = val;
}

#[tauri::command]
pub fn get_spectate_target(state: State<AppState>) -> Option<String> {
    let v = *state.spectate_target.lock().unwrap();
    if v == 0 { None } else { Some(format!("0x{:X}", v)) }
}


// ─── Server / place name ─────────────────────────────────────────────────

/// DataModel's `Name` instance field is the place name in Roblox.
#[tauri::command]
pub fn get_place_name(state: State<AppState>) -> String {
    let sdk = state.sdk.lock().unwrap().clone();
    let sdk = match sdk { Some(s) => s, None => return "(not attached)".into() };
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return "(no fake datamodel)".into(); }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return "(no datamodel)".into(); }
    let name = sdk::name_of(&sdk, dm);
    if name.is_empty() { "(no place)".into() } else { name }
}

/// Launch the Roblox `roblox://placeId=…` URL via the macOS `open` command.
/// Frontend validates the input as a positive integer first.
#[tauri::command]
pub fn join_place(place_id: String) -> Result<(), String> {
    if place_id.is_empty() || !place_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("invalid place id".into());
    }
    let url = format!("roblox://placeId={}", place_id);
    std::process::Command::new("open")
        .arg(&url)
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Explorer — DataModel instance tree ─────────────────────────────────

#[derive(Serialize, Clone)]
pub struct InstanceNode {
    pub name: String,
    pub address: String,
    pub children: Vec<InstanceNode>,
}

/// Walk the DataModel recursively and return a capped tree. Caps keep the
/// payload sane (a real DataModel can hold tens of thousands of nodes).
#[tauri::command]
pub fn get_instance_tree(state: State<AppState>) -> Result<InstanceNode, String> {
    let sdk = state.sdk.lock().unwrap().clone().ok_or_else(|| "not attached".to_string())?;
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return Err("no fake datamodel".into()); }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return Err("no datamodel".into()); }

    const MAX_DEPTH: u32 = 5;
    const MAX_CHILDREN_PER_NODE: usize = 200;
    const MAX_TOTAL_NODES: usize = 4000;

    fn build(sdk: &SdkI, instance: u64, depth: u32, total: &mut usize) -> Option<InstanceNode> {
        if !sdk.is_valid_ptr(instance) { return None; }
        if *total >= MAX_TOTAL_NODES { return None; }
        *total += 1;
        let name = sdk::name_of(sdk, instance);
        let address = format!("0x{:X}", instance);
        let mut children = Vec::new();
        if depth < MAX_DEPTH {
            for c in sdk::get_children(sdk, instance).into_iter().take(MAX_CHILDREN_PER_NODE) {
                if *total >= MAX_TOTAL_NODES { break; }
                if let Some(n) = build(sdk, c, depth + 1, total) {
                    children.push(n);
                }
            }
        }
        Some(InstanceNode { name, address, children })
    }

    let mut total = 0usize;
    let root_children: Vec<InstanceNode> = sdk::get_children(&sdk, dm)
        .into_iter()
        .filter_map(|c| build(&sdk, c, 1, &mut total))
        .collect();
    let root_name = sdk::name_of(&sdk, dm);
    Ok(InstanceNode {
        name: if root_name.is_empty() { "DataModel".to_string() } else { root_name },
        address: format!("0x{:X}", dm),
        children: root_children,
    })
}

// keep silenced warnings happy if these end up unused later
#[allow(dead_code)] fn _force_use(_a: u64) -> u64 { let _ = find_child_fast; let _ = read_name_fast; 0 }
