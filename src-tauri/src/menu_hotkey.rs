//! Rising-edge detector on the menu-toggle bind. The actual key/button
//! state lives in `crate::input` (see that module for the permissionless
//! detection trick).

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::input::is_held;
use crate::state::AppState;

pub fn start_menu_hotkey_thread(app: AppHandle) {
    thread::spawn(move || {
        let mut prev = false;
        loop {
            let bind = {
                let state = app.state::<AppState>();
                let guard = state.menu_toggle_bind.lock().unwrap();
                guard.clone()
            };
            let held = is_held(&bind);
            if held && !prev {
                let _ = app.emit("menu_toggle", ());
            }
            prev = held;
            thread::sleep(Duration::from_millis(25));
        }
    });
}
