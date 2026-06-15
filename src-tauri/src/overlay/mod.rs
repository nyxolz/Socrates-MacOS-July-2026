//! Overlay infrastructure — everything that makes the Tauri window behave like
//! a transparent always-on-top click-through overlay sitting on top of
//! Roblox.
//!
//! - `window`  : NSWindow flags (transparent, alwaysOnTop, NSScreenSaver level).
//! - `bounds`  : CGWindow polling — find the live Roblox window rect.
//! - `tracker` : Two background threads. One keeps the overlay glued to the
//!               Roblox window. The other toggles click-through depending on
//!               whether the mouse is over an interactive UI zone.

pub mod window;
pub mod bounds;
pub mod tracker;

pub use window::setup_window_overlay;
pub use window::set_window_sharing;
pub use tracker::{start_window_tracker, start_mouse_tracker};

/// macOS titlebar adjustment used everywhere (NDC → pixel conversion).
pub const TITLEBAR_HEIGHT: i32 = 28;

/// Pixel rectangle, **window-local** coordinates (top-left = 0,0). The Svelte
/// side publishes one of these per interactive area (the menu, modals, etc.).
#[derive(Clone, Copy, Default, Debug)]
pub struct InteractiveRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
