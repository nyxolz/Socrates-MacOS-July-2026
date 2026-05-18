//! Application state — everything the background threads and Tauri commands
//! share. One `AppState` instance is managed by Tauri (`app.manage(...)`).
//!
//! Keep this module dumb on purpose: just mutexes and atomics, no behavior.
//! Behavior lives in the modules that mutate this state (overlay::tracker,
//! commands, future esp / aimbot loops).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::overlay::InteractiveRect;
use crate::sdk::SdkI;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RobloxBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZoneInput {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct AppState {
    /// `Some((x, y, w, h))` when Roblox is currently visible on screen.
    pub roblox_bounds: Mutex<Option<(f64, f64, f64, f64)>>,

    /// Set once when Roblox is first detected; never reset. Used by the
    /// click-through logic to tell "not launched yet" from "minimized".
    pub roblox_ever_seen: AtomicBool,

    /// Window-local pixel rects published by the Svelte UI. Mouse tracker
    /// flips click-through when the cursor crosses one of these.
    pub interactive_zones: Mutex<Vec<InteractiveRect>>,

    /// Lazy: filled when `attach_sdk` succeeds.
    pub sdk: Mutex<Option<Arc<SdkI>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            roblox_bounds: Mutex::new(None),
            roblox_ever_seen: AtomicBool::new(false),
            interactive_zones: Mutex::new(Vec::new()),
            sdk: Mutex::new(None),
        }
    }
}
