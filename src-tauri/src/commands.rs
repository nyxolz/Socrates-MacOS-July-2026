//! Tauri command surface exposed to the SvelteKit frontend.
//!
//! Keep this thin — each command is just glue between the frontend and the
//! background threads / SDK. Feature work (aimbot config, ESP toggles, …)
//! gets its own command(s) added here as it lands.

use std::sync::Arc;

use tauri::State;

use crate::overlay::InteractiveRect;
use crate::sdk::SdkI;
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
    let name = process_name.unwrap_or_else(|| "RobloxPlayer".to_string());
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
