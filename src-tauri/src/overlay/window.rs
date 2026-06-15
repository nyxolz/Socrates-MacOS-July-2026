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

    // Stream-proof by default: exclude the overlay from screen capture /
    // sharing so it's invisible in recordings, OBS, Discord share, etc.
    // The Settings toggle flips this at runtime via `set_stream_proof`.
    set_window_sharing(window, true);
}

/// Toggle whether the window is captured by screen recording / sharing.
/// `proof = true`  → `NSWindowSharingNone` (0): hidden from any capture.
/// `proof = false` → `NSWindowSharingReadOnly` (1): the macOS default, visible.
pub unsafe fn set_window_sharing(window: &WebviewWindow, proof: bool) {
    let ns_window = window.ns_window().unwrap() as id;
    // NSWindowSharingType: 0 = None, 1 = ReadOnly.
    let sharing: u64 = if proof { 0 } else { 1 };
    let _: () = msg_send![ns_window, setSharingType: sharing];
}
