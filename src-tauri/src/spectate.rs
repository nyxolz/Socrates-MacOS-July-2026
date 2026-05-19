//! Spectate thread.
//!
//! When `AppState.spectate_target` is non-zero, every ~16 ms (60 Hz) we
//! locate the target's head, place the camera ~10 studs behind it (along
//! the target's look direction, +3 studs up), and lock the camera matrix
//! to point at the head. Roblox's default Custom camera may fight us each
//! frame; the high cadence keeps the rendered view mostly on our side.

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::aimbot;
use crate::offsets;
use crate::players::read_pos_fast;
use crate::sdk::SdkI;
use crate::state::AppState;

fn resolve_camera(sdk: &SdkI) -> Option<u64> {
    let fdm = sdk.read::<u64>(sdk.base + offsets::Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return None; }
    let dm = sdk.read::<u64>(fdm + offsets::FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return None; }
    let ws = sdk.read::<u64>(dm + offsets::DataModel::WorkSpace as u64);
    if !sdk.is_valid_ptr(ws) { return None; }
    let cam = sdk.read::<u64>(ws + offsets::Workspace::Camera as u64);
    if !sdk.is_valid_ptr(cam) { return None; }
    Some(cam)
}

fn read_camera_matrix(sdk: &SdkI, camera_ptr: u64) -> [[f32; 3]; 3] {
    let mo = offsets::Camera::CameraOrientation as u64;
    [
        [sdk.read::<f32>(camera_ptr + mo),
         sdk.read::<f32>(camera_ptr + mo + 4),
         sdk.read::<f32>(camera_ptr + mo + 8)],
        [sdk.read::<f32>(camera_ptr + mo + 12),
         sdk.read::<f32>(camera_ptr + mo + 16),
         sdk.read::<f32>(camera_ptr + mo + 20)],
        [sdk.read::<f32>(camera_ptr + mo + 24),
         sdk.read::<f32>(camera_ptr + mo + 28),
         sdk.read::<f32>(camera_ptr + mo + 32)],
    ]
}

fn read_look_dir(sdk: &SdkI, primitive: u64) -> [f32; 3] {
    let d = [
        -sdk.read::<f32>(primitive + 0xC8),
        -sdk.read::<f32>(primitive + 0xD4),
        -sdk.read::<f32>(primitive + 0xE0),
    ];
    let l = (d[0]*d[0] + d[1]*d[1] + d[2]*d[2]).sqrt();
    if l < 1e-5 || !l.is_finite() { [0.0, 0.0, 1.0] } else { [d[0]/l, d[1]/l, d[2]/l] }
}

pub fn start_spectate_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        const BACK: f32 = 10.0;
        const UP: f32 = 3.0;
        loop {
            thread::sleep(Duration::from_millis(16));
            let state = app_handle.state::<AppState>();
            let target = *state.spectate_target.lock().unwrap();
            if target == 0 { continue; }

            let sdk = match state.sdk.lock().unwrap().clone() {
                Some(s) => s, None => continue,
            };

            let caches = state.char_caches.lock().unwrap().clone();
            let cache = match caches.iter().find(|c| c.char_addr == target) {
                Some(c) => c, None => continue,
            };
            let (head_prim, valid) = cache.parts[0];
            if !valid { continue; }
            let head_pos = read_pos_fast(&sdk, head_prim);
            if !head_pos[0].is_finite() { continue; }

            // Prefer HRP look-dir (matches body facing); fall back to head
            // primitive only when HRP isn't cached. Without this, the head's
            // independent pivot makes the trailing camera jump around.
            let look_prim = if cache.hrp_primitive != 0 { cache.hrp_primitive } else { head_prim };
            let look = read_look_dir(&sdk, look_prim);
            // Zero the vertical component so the camera stays at a stable
            // height instead of tilting through the floor when the target
            // crouches or looks up/down.
            let mut look_flat = [look[0], 0.0_f32, look[2]];
            let l = (look_flat[0]*look_flat[0] + look_flat[2]*look_flat[2]).sqrt();
            if l > 1e-3 {
                look_flat[0] /= l;
                look_flat[2] /= l;
            } else {
                look_flat = [0.0, 0.0, 1.0]; // arbitrary fallback
            }
            let new_cam = [
                head_pos[0] - look_flat[0] * BACK,
                head_pos[1] + UP,
                head_pos[2] - look_flat[2] * BACK,
            ];

            let camera = match resolve_camera(&sdk) { Some(c) => c, None => continue };
            let pos_off = offsets::Camera::CameraPosition as u64;
            sdk.write::<f32>(camera + pos_off,     new_cam[0]);
            sdk.write::<f32>(camera + pos_off + 4, new_cam[1]);
            sdk.write::<f32>(camera + pos_off + 8, new_cam[2]);

            let current = read_camera_matrix(&sdk, camera);
            // Snap orientation onto the target — smoothing 0 = snap.
            aimbot::aim_camera_pub(&sdk, camera, new_cam, head_pos, 0.0, current);
        }
    });
}
