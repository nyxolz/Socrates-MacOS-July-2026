//! Permissionless held-key / held-button state for every bind in use.
//!
//! macOS won't let us observe other apps' keys without granting
//! Accessibility / Input Monitoring. Two narrow APIs work without prompts:
//!   • `RegisterEventHotKey` (Carbon) — system-routes a chosen key to
//!     our app. Trade-off: the bound key is consumed, the focused app
//!     does NOT receive it. Picked via the `global-hotkey` crate.
//!   • `+[NSEvent pressedMouseButtons]` — a bitmask of currently held
//!     mouse buttons. Observation only, no consumption, no permission.
//!
//! `is_held(bind)` is the single entry point; every consumer (aimbot,
//! trigger bot, menu toggle) goes through it. The background coordinator
//! re-registers global hotkeys whenever the configured set of binds
//! changes (menu toggle + aimbot bind + trigger bind), and updates a
//! shared `HashMap` from `GlobalHotKeyEvent` press / release events.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use objc::{class, msg_send, sel, sel_impl};
use once_cell::sync::Lazy;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Held-state for keyboard binds (mouse binds are queried live).
static KEY_STATE: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn mouse_button(s: &str) -> Option<u32> {
    Some(match s {
        "MouseLeft" => 0, "MouseRight" => 1, "MouseMiddle" => 2,
        "Mouse4" => 3, "Mouse5" => 4,
        _ => return None,
    })
}

/// True iff the bound key / mouse button is currently held. Cheap — safe
/// to call from a tight per-tick loop (aimbot calls this at 250 Hz).
pub fn is_held(bind: &str) -> bool {
    if bind.is_empty() { return false; }
    if let Some(b) = mouse_button(bind) {
        let mask: u64 = unsafe { msg_send![class!(NSEvent), pressedMouseButtons] };
        return mask & (1u64 << b) != 0;
    }
    KEY_STATE.lock().unwrap().get(bind).copied().unwrap_or(false)
}

/// Read every bind name that should be watched, from app state.
fn wanted_binds(app: &AppHandle) -> HashSet<String> {
    let state = app.state::<AppState>();
    let aimbot = state.aimbot_cfg.lock().unwrap();
    let menu_bind = state.menu_toggle_bind.lock().unwrap();
    let misc = state.misc_cfg.lock().unwrap();
    let mut set = HashSet::new();
    for b in [menu_bind.as_str(), aimbot.keybind.as_str(), aimbot.trigger_keybind.as_str(), misc.fly_keybind.as_str()] {
        // Only keyboard binds need to be registered — mouse buttons are
        // polled from NSEvent on the fly.
        if !b.is_empty() && mouse_button(b).is_none() {
            set.insert(b.to_string());
        }
    }
    set
}

pub fn start_input_thread(app: AppHandle) {
    thread::spawn(move || {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[input] GlobalHotKeyManager init failed: {e:?}");
                return;
            }
        };

        // bind string → currently-registered HotKey
        let mut registered: HashMap<String, HotKey> = HashMap::new();

        loop {
            let wanted = wanted_binds(&app);

            // Drop binds that are no longer wanted.
            let current: Vec<String> = registered.keys().cloned().collect();
            for b in current {
                if !wanted.contains(&b) {
                    if let Some(hk) = registered.remove(&b) {
                        let _ = manager.unregister(hk);
                    }
                    KEY_STATE.lock().unwrap().remove(&b);
                }
            }

            // Register the new ones.
            for b in wanted {
                if registered.contains_key(&b) { continue; }
                let code = match Code::from_str(&b) { Ok(c) => c, Err(_) => continue };
                let hk = HotKey::new(Some(Modifiers::empty()), code);
                if manager.register(hk).is_ok() {
                    registered.insert(b.clone(), hk);
                    KEY_STATE.lock().unwrap().insert(b, false);
                }
            }

            // Drain hotkey events into the held-state map.
            while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                let held = event.state() == HotKeyState::Pressed;
                let id = event.id();
                if let Some(bind) = registered.iter().find(|(_, hk)| hk.id() == id).map(|(b, _)| b.clone()) {
                    KEY_STATE.lock().unwrap().insert(bind, held);
                }
            }

            thread::sleep(Duration::from_millis(20));
        }
    });
}
