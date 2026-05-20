//! Crate entry — wires Tauri, the AppState, the command surface, and the
//! background overlay threads. Feature modules (ESP, aimbot, free cam, …)
//! plug in here as they land.

mod offsets;
mod math;
mod sdk;
mod overlay;
mod state;
mod commands;
mod players;
mod esp;
mod aimbot;
mod spectate;
mod security;
mod input;
mod menu_hotkey;

use std::ptr;

use tauri::Manager;

use crate::aimbot::start_aimbot_thread;
use crate::esp::start_esp_thread;
use crate::input::start_input_thread;
use crate::menu_hotkey::start_menu_hotkey_thread;
use crate::overlay::bounds::find_window_bounds;
use crate::overlay::{setup_window_overlay, start_mouse_tracker, start_window_tracker};
use crate::players::start_char_cache_thread;
use crate::security::start_security_thread;
use crate::spectate::start_spectate_thread;
use crate::state::AppState;

/// Apply an iOS-style "squircle" corner mask (~22.4 % of the icon side)
/// so the Dock icon has the same rounded look as Google / Roblox / etc.
/// 1-px feathered alpha along the curve to avoid aliasing.
fn rounded_icon_png(src: &[u8]) -> Vec<u8> {
    use image::{ImageReader, Rgba};
    use std::io::Cursor;

    let decoded = match ImageReader::new(Cursor::new(src))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.decode().ok())
    {
        Some(i) => i.to_rgba8(),
        None => return src.to_vec(),
    };
    let (w, h) = decoded.dimensions();
    let r = (w.min(h) as f32) * 0.224;
    let (wf, hf) = (w as f32, h as f32);
    let mut img = decoded;

    for y in 0..h {
        for x in 0..w {
            let xf = x as f32 + 0.5;
            let yf = y as f32 + 0.5;
            // Distance from the nearest *outer* corner-zone reference point.
            let dx = if xf < r { r - xf } else if xf > wf - r { xf - (wf - r) } else { 0.0 };
            let dy = if yf < r { r - yf } else if yf > hf - r { yf - (hf - r) } else { 0.0 };
            if dx == 0.0 || dy == 0.0 { continue; }
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist <= r - 1.0 { 1.0 }
                        else if dist >= r + 1.0 { 0.0 }
                        else { 0.5 - (dist - r) / 2.0 };
            if alpha < 1.0 {
                let p = img.get_pixel(x, y);
                let na = (p[3] as f32 * alpha.max(0.0)).clamp(0.0, 255.0) as u8;
                img.put_pixel(x, y, Rgba([p[0], p[1], p[2], na]));
            }
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    if img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png).is_err() {
        return src.to_vec();
    }
    buf
}

/// In `tauri dev` there is no .app bundle, so macOS falls back to the
/// generic Tauri icon in the Dock. Push the bundled PNG (with rounded
/// corners applied) into `NSApplication.setApplicationIconImage` at
/// startup so dev mode looks the same as a production build.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    const ICON: &[u8] = include_bytes!("../icons/icon.png");
    let rounded = rounded_icon_png(ICON);
    unsafe {
        let data: id = msg_send![
            class!(NSData),
            dataWithBytes: rounded.as_ptr() as *const std::ffi::c_void
            length: rounded.len() as u64
        ];
        if data == nil { return; }
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: data];
        if image == nil { return; }
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setApplicationIconImage: image];
    }
}

/// `ptrace(PT_DENY_ATTACH)` — refuses lldb / debuggers from latching on.
fn anti_debug() {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::ptrace(libc::PT_DENY_ATTACH, 0, ptr::null_mut(), 0);
    }
}

/// Generic error dialog (rfd → NSAlert on macOS). Buttons read "Retry" /
/// "Quit". Returns `true` if the user picked Retry, `false` for Quit.
fn show_roblox_missing_dialog() -> bool {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    let res = MessageDialog::new()
        .set_title("Error")
        .set_description("Roblox must be opened first.")
        .set_level(MessageLevel::Warning)
        .set_buttons(MessageButtons::OkCancelCustom("Retry".into(), "Quit".into()))
        .show();
    matches!(&res, MessageDialogResult::Custom(s) if s == "Retry")
        || matches!(&res, MessageDialogResult::Ok)
}

fn roblox_running() -> bool {
    for name in &["RobloxPlayer", "Roblox", "RobloxStudio"] {
        if find_window_bounds(name).is_some() { return true; }
    }
    false
}

/// Blocks until Roblox is found. Returns `false` if the user clicks Quit
/// — caller should `exit(0)` in that case (no overlay should ever appear).
fn gate_on_roblox() -> bool {
    while !roblox_running() {
        if !show_roblox_missing_dialog() { return false; }
    }
    true
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

            #[cfg(target_os = "macos")]
            set_dock_icon();

            // Gate the whole startup on Roblox being open. Native error
            // dialog (Retry / Quit), shown BEFORE any overlay window
            // appears or any background thread runs. If the user quits,
            // we exit immediately so nothing of Socrate is ever drawn.
            if !gate_on_roblox() { std::process::exit(0); }

            unsafe { setup_window_overlay(&window); }

            start_window_tracker(window.clone(), app.handle().clone());
            start_mouse_tracker(window, app.handle().clone());
            start_char_cache_thread(app.handle().clone());
            start_esp_thread(app.handle().clone());
            start_input_thread(app.handle().clone());
            start_aimbot_thread(app.handle().clone());
            start_spectate_thread(app.handle().clone());
            start_menu_hotkey_thread(app.handle().clone());

            // Background anti-RE watcher: Frida / lldb / sysctl P_TRACED.
            #[cfg(target_os = "macos")]
            start_security_thread();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_roblox_bounds,
            commands::is_roblox_running,
            commands::set_interactive_zones,
            commands::panic_exit,
            commands::attach_sdk,
            commands::sdk_status,
            commands::set_aimbot_config,
            commands::set_visuals_config,
            commands::set_menu_toggle_bind,
            commands::diag_check,
            commands::list_players,
            commands::set_spectate_target,
            commands::get_spectate_target,
            commands::get_place_name,
            commands::get_instance_tree,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
