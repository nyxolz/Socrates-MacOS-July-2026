//! Crate entry — wires Tauri, the AppState, the command surface, and the
//! background overlay threads. Feature modules (ESP, aimbot, free cam, …)
//! plug in here as they land.

mod offsets;
mod math;
mod sdk;
mod overlay;
mod state;
mod commands;

use std::ptr;

use tauri::Manager;

use crate::overlay::{setup_window_overlay, start_mouse_tracker, start_window_tracker};
use crate::state::AppState;

/// `ptrace(PT_DENY_ATTACH)` — refuses lldb / debuggers from latching on.
fn anti_debug() {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::ptrace(libc::PT_DENY_ATTACH, 0, ptr::null_mut(), 0);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    anti_debug();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window missing");

            unsafe { setup_window_overlay(&window); }

            start_window_tracker(window.clone(), app.handle().clone());
            start_mouse_tracker(window, app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_roblox_bounds,
            commands::is_roblox_running,
            commands::set_interactive_zones,
            commands::panic_exit,
            commands::attach_sdk,
            commands::sdk_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
