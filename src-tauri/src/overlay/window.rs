//! NSWindow flags for the transparent overlay.

use cocoa::appkit::{NSColor, NSWindow, NSWindowCollectionBehavior};
use cocoa::base::{id, nil, NO, YES};
use objc::{msg_send, sel, sel_impl};
use tauri::WebviewWindow;

/// Make the Tauri window: transparent, no shadow, click-through by default,
/// floating above fullscreen apps, visible on every Space.
///
/// The mouse tracker disables click-through when the cursor enters an
/// `InteractiveRect` published by the Svelte side.
pub unsafe fn setup_window_overlay(window: &WebviewWindow) {
    let ns_window = window.ns_window().unwrap() as id;
    ns_window.setOpaque_(NO);
    ns_window.setBackgroundColor_(NSColor::clearColor(nil));
    ns_window.setHasShadow_(NO);
    ns_window.setIgnoresMouseEvents_(YES);

    // NSScreenSaverWindowLevel = 1000 — sits above Roblox in true fullscreen.
    let level: i64 = 1000;
    let _: () = msg_send![ns_window, setLevel: level];

    let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
    ns_window.setCollectionBehavior_(behavior);
}
