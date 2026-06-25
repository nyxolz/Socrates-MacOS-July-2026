//! Native-style spectate via `Camera.CameraSubject`.
//!
//! Previous implementation wrote `CameraPosition` + `CameraOrientation`
//! every 16 ms, fighting Roblox's CameraScript every frame — jittery and
//! brittle. This version sets `Camera.CameraSubject` to the target's
//! Humanoid instead. Roblox's own Custom-camera scripts then follow that
//! target with full lissage + mouse-orbit control: same UX as a regular
//! Roblox spectate, zero polling war.
//!
//! `Camera.CameraSubject` is at a build-specific offset we don't ship in
//! `offsets.rs`. We discover it once at runtime: by default
//! `CameraSubject == LocalPlayer.Character.Humanoid`, so we scan the
//! Camera struct for that exact pointer value. First match (8-byte
//! aligned, generous range) wins; the offset is cached in an atomic for
//! the rest of the session.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use obfstr::obfstr;
use tauri::{AppHandle, Manager};

use crate::players::find_child_fast;
use crate::sdk::SdkI;
use crate::security::{self, resolve_camera};
use crate::state::AppState;

/// 0 sentinel = not discovered yet. Real offsets are small (< 0x400).
static CAMERA_SUBJECT_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Walk DataModel → Workspace → child named like the local player →
/// `Humanoid`. Returns 0 on any failure (process not attached, respawn in
/// progress, etc.). Cheap enough to call from the spectate tick.
fn resolve_local_humanoid(sdk: &SdkI, local_name: &str) -> u64 {
    if local_name.is_empty() { return 0; }
    let ws = match security::resolve_workspace(sdk) {
        Some(v) => v, None => return 0,
    };
    let char_addr = find_child_fast(sdk, ws, local_name);
    if !sdk.is_valid_ptr(char_addr) { return 0; }
    let humanoid = find_child_fast(sdk, char_addr, obfstr!("Humanoid").as_ref());
    if !sdk.is_valid_ptr(humanoid) { return 0; }
    humanoid
}

/// Scan Camera memory 8-byte aligned for a u64 equal to `target`. The
/// match is unique enough in practice — a precise Humanoid pointer almost
/// never collides with random struct bytes.
fn discover_subject_offset(sdk: &SdkI, camera_ptr: u64, target: u64) -> Option<u64> {
    let mut off = 0x100u64;
    while off + 8 <= 0x400 {
        if sdk.read::<u64>(camera_ptr + off) == target {
            return Some(off);
        }
        off += 8;
    }
    None
}

/// Ensure the subject offset is known, returning it. Lazy: tries each
/// tick until the local Humanoid is resolvable and a match is found.
fn ensure_offset(sdk: &SdkI, state: &AppState, camera_ptr: u64) -> Option<u64> {
    let cached = CAMERA_SUBJECT_OFFSET.load(Ordering::Relaxed);
    if cached != 0 { return Some(cached); }
    let local_name = state.local_player_name.lock().unwrap().clone();
    let local_humanoid = resolve_local_humanoid(sdk, &local_name);
    if local_humanoid == 0 { return None; }
    let off = discover_subject_offset(sdk, camera_ptr, local_humanoid)?;
    CAMERA_SUBJECT_OFFSET.store(off, Ordering::Relaxed);
    Some(off)
}

pub fn start_spectate_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        // `original_subject != 0` ↔ we are currently spectating and have
        // saved the user's original CameraSubject so we can restore it.
        let mut original_subject: u64 = 0;

        loop {
            thread::sleep(Duration::from_millis(100));

            let state = app_handle.state::<AppState>();
            if !state.key_unlocked.load(std::sync::atomic::Ordering::Relaxed) { continue; }
            let target = *state.spectate_target.lock().unwrap();

            let sdk = match state.sdk.lock().unwrap().clone() {
                Some(s) => s, None => continue,
            };
            let camera = match resolve_camera(&sdk) { Some(c) => c, None => continue };

            let subject_offset = match ensure_offset(&sdk, &state, camera) {
                Some(o) => o, None => continue,
            };

            // Resolve target's Humanoid via the existing char cache. If the
            // target left / died, the cache no longer has it → we'll bail
            // out below and restore the user's original subject.
            let target_humanoid: u64 = if target != 0 {
                state.char_caches.lock().unwrap().iter()
                    .find(|c| c.char_addr == target)
                    .map(|c| c.humanoid_addr)
                    .filter(|h| sdk.is_valid_ptr(*h))
                    .unwrap_or(0)
            } else { 0 };

            match (original_subject != 0, target_humanoid != 0) {
                (false, true) => {
                    // Idle → spectating. Save original, redirect subject.
                    let orig = sdk.read::<u64>(camera + subject_offset);
                    if sdk.is_valid_ptr(orig) {
                        original_subject = orig;
                        sdk.write::<u64>(camera + subject_offset, target_humanoid);
                    }
                }
                (true, true) => {
                    // Spectating — keep the subject pinned (Roblox can
                    // reset it across respawns / mode changes).
                    let current = sdk.read::<u64>(camera + subject_offset);
                    if current != target_humanoid {
                        sdk.write::<u64>(camera + subject_offset, target_humanoid);
                    }
                }
                (true, false) => {
                    // Spectating → no valid target (cleared by user, or
                    // target gone). Restore and clear.
                    sdk.write::<u64>(camera + subject_offset, original_subject);
                    original_subject = 0;
                    if target != 0 {
                        *state.spectate_target.lock().unwrap() = 0;
                    }
                }
                (false, false) => {} // idle, nothing to do
            }
        }
    });
}
