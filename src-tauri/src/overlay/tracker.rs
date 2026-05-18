//! Background threads:
//!   - `start_window_tracker` keeps the overlay aligned with the Roblox window
//!     and emits `roblox-bounds-changed` / `roblox-closed`.
//!   - `start_mouse_tracker` toggles click-through every 16 ms based on
//!     whether the cursor sits inside an interactive zone the UI declared.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use cocoa::base::{id, nil, NO, YES};
use objc::{class, msg_send, sel, sel_impl};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use crate::state::{AppState, RobloxBounds};
use super::bounds::find_window_bounds;

/// Poll CGWindow every 500 ms. Show/hide the overlay when Roblox dis/appears,
/// move/resize when its bounds change, push the new bounds into AppState.
pub fn start_window_tracker(window: WebviewWindow, app_handle: tauri::AppHandle) {
    thread::spawn(move || {
        let candidates = ["RobloxPlayer", "Roblox", "RobloxStudio"];
        let mut last_bounds: Option<(f64, f64, f64, f64)> = None;
        loop {
            let mut found: Option<(f64, f64, f64, f64)> = None;
            for name in &candidates {
                if let Some(b) = find_window_bounds(name) { found = Some(b); break; }
            }
            { *app_handle.state::<AppState>().roblox_bounds.lock().unwrap() = found; }
            if found.is_some() {
                app_handle.state::<AppState>().roblox_ever_seen.store(true, Ordering::Relaxed);
            }
            if found != last_bounds {
                match (last_bounds.is_some(), found.is_some()) {
                    (false, true) => { let _ = window.show(); }
                    (true,  false) => { let _ = window.hide(); }
                    _ => {}
                }
                if let Some((x, y, w, h)) = found {
                    let _ = window.set_position(LogicalPosition::new(x, y));
                    let _ = window.set_size(LogicalSize::new(w, h));
                    let _ = app_handle.emit("roblox-bounds-changed", RobloxBounds { x, y, width: w, height: h });
                } else if last_bounds.is_some() {
                    let _ = app_handle.emit("roblox-closed", ());
                }
                last_bounds = found;
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

/// NSEvent.mouseLocation in top-left screen coords.
unsafe fn mouse_position_top_left() -> Option<(f64, f64)> {
    let ns_event_class = class!(NSEvent);
    let point: cocoa::foundation::NSPoint = msg_send![ns_event_class, mouseLocation];
    let main_screen: id = msg_send![class!(NSScreen), mainScreen];
    if main_screen == nil { return None; }
    let frame: cocoa::foundation::NSRect = msg_send![main_screen, frame];
    Some((point.x, frame.size.height - point.y))
}

/// 60 Hz cursor-vs-zones loop. Flips `setIgnoresMouseEvents` on the main
/// thread the moment the cursor crosses an interactive boundary.
///
/// Three cases:
///   - `bounds = Some` → interactive only inside the declared menu zones.
///   - `bounds = None`, never seen → keep interactive (license screen, etc.).
///   - `bounds = None`, was seen → force click-through (Roblox hidden, don't
///     trap clicks on whatever is below).
pub fn start_mouse_tracker(window: WebviewWindow, app_handle: tauri::AppHandle) {
    thread::spawn(move || {
        let mut current_interactive = false;

        loop {
            let mouse = unsafe { mouse_position_top_left() };
            let mouse = match mouse {
                Some(m) => m,
                None => { thread::sleep(Duration::from_millis(20)); continue; }
            };

            let win_bounds = app_handle.state::<AppState>().roblox_bounds.lock().unwrap().clone();
            let ever_seen = app_handle.state::<AppState>().roblox_ever_seen.load(Ordering::Relaxed);
            let in_zone = match win_bounds {
                Some((win_x, win_y, _, _)) => {
                    let local_x = mouse.0 - win_x;
                    let local_y = mouse.1 - win_y;
                    let zones = app_handle.state::<AppState>().interactive_zones.lock().unwrap().clone();
                    zones.iter().any(|z| {
                        local_x >= z.x && local_x < z.x + z.w &&
                        local_y >= z.y && local_y < z.y + z.h
                    })
                }
                None => !ever_seen,
            };

            if in_zone != current_interactive {
                current_interactive = in_zone;

                let w = window.clone();
                let w_inner = w.clone();
                let _ = w.run_on_main_thread(move || {
                    unsafe {
                        if let Ok(ns_ptr) = w_inner.ns_window() {
                            let ns_window = ns_ptr as id;
                            if ns_window != nil {
                                let ignore = if in_zone { NO } else { YES };
                                let _: () = msg_send![ns_window, setIgnoresMouseEvents: ignore];
                                if in_zone {
                                    let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
                                }
                            }
                        }
                    }
                });
            }
            thread::sleep(Duration::from_millis(16));
        }
    });
}
