//! Aimbot + trigger thread.
//!
//! 250 Hz loop. Reads `AppState.aimbot_cfg` every tick. When the keybind is
//! held it walks the char_caches, picks the best target according to the
//! selection mode (with min/max distance + FOV filters), optionally applies
//! linear velocity prediction + bullet drop, projects to screen, and warps
//! the cursor with smoothing. Trigger bot fires `LeftMouseDown/Up` when a
//! target sits within ~6 px of the crosshair (delay + burst respected).

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use tauri::{AppHandle, Manager};

use crate::math::{build_projection_matrix, build_view_matrix, world_to_ndc};
use crate::offsets;
use crate::overlay::TITLEBAR_HEIGHT;
use crate::players::read_pos_fast;
use crate::sdk::SdkI;
use crate::state::AppState;

unsafe extern "C" {
    fn CGEventSourceKeyState(state_id: i32, keycode: u16) -> bool;
    fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: i32) -> i32;
}
const HID: i32 = 1;

fn mouse_button(s: &str) -> Option<u32> {
    Some(match s {
        "MouseLeft" => 0, "MouseRight" => 1, "MouseMiddle" => 2,
        "Mouse4" => 3, "Mouse5" => 4,
        _ => return None,
    })
}

/// e.code → macOS virtual keycode. Covers the keys a normal aimbot keybind
/// would use; the rest fall back to "no match" (the keybind is then dead
/// until the user re-binds with one of these).
fn key_code(s: &str) -> Option<u16> {
    Some(match s {
        "KeyA" => 0, "KeyS" => 1, "KeyD" => 2, "KeyF" => 3,
        "KeyH" => 4, "KeyG" => 5, "KeyZ" => 6, "KeyX" => 7,
        "KeyC" => 8, "KeyV" => 9, "KeyB" => 11, "KeyQ" => 12,
        "KeyW" => 13, "KeyE" => 14, "KeyR" => 15, "KeyY" => 16,
        "KeyT" => 17, "KeyO" => 31, "KeyU" => 32, "KeyI" => 34,
        "KeyP" => 35, "KeyL" => 37, "KeyJ" => 38, "KeyK" => 40,
        "KeyN" => 45, "KeyM" => 46,
        "Space" => 49, "Tab" => 48, "Enter" => 36, "Escape" => 53,
        "ShiftLeft" => 56, "ControlLeft" => 59, "AltLeft" => 58, "MetaLeft" => 55,
        "ShiftRight" => 60, "ControlRight" => 62, "AltRight" => 61,
        _ => return None,
    })
}

fn is_held(bind: &str) -> bool {
    if let Some(b) = mouse_button(bind) {
        unsafe { CGEventSourceButtonState(HID, b) }
    } else if let Some(c) = key_code(bind) {
        unsafe { CGEventSourceKeyState(HID, c) }
    } else { false }
}

fn warp(x: f64, y: f64) {
    unsafe {
        CGWarpMouseCursorPosition(CGPoint::new(x, y));
        // Without this, the OS keeps suppressing tiny relative deltas for
        // ~250 ms after a warp — kills smooth aim.
        CGAssociateMouseAndMouseCursorPosition(1);
    }
}

fn click_left() {
    let src = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s, Err(_) => return,
    };
    let pos = CGEvent::new(src.clone())
        .map(|e| e.location())
        .unwrap_or(CGPoint::new(0.0, 0.0));
    for ty in [CGEventType::LeftMouseDown, CGEventType::LeftMouseUp] {
        if let Ok(e) = CGEvent::new_mouse_event(src.clone(), ty, pos, CGMouseButton::Left) {
            e.post(CGEventTapLocation::HID);
        }
    }
}

pub fn current_cursor() -> Option<CGPoint> {
    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let e = CGEvent::new(src).ok()?;
    Some(e.location())
}

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

fn lerp(a: f64, b: f64, t: f64) -> f64 { a + (b - a) * t.clamp(0.0, 1.0) }

// ─── Camera-write aim (alternative to mouse-warp) ───────────────────────
// Computes the look-at orientation matrix that points the camera at
// `target` from `cam_pos`, lerps it with the camera's current matrix, then
// writes the 9 f32s back to `camera + CameraOrientation`. Roblox CFrame
// stores Right / Up / Back (= -Forward) as the three *columns* of the 3×3.
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm3(v: [f32; 3]) -> Option<[f32; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if !l.is_finite() || l < 1e-5 { return None; }
    Some([v[0] / l, v[1] / l, v[2] / l])
}

/// Public wrapper so `spectate.rs` (and any future module) can reuse the
/// look-at + matrix-write logic without duplicating it.
pub fn aim_camera_pub(
    sdk: &SdkI, camera_ptr: u64, cam_pos: [f32; 3], target: [f32; 3],
    smoothing: f32, current: [[f32; 3]; 3],
) {
    aim_camera(sdk, camera_ptr, cam_pos, target, smoothing, current);
}

fn aim_camera(
    sdk: &SdkI,
    camera_ptr: u64,
    cam_pos: [f32; 3],
    target: [f32; 3],
    smoothing: f32,
    current: [[f32; 3]; 3],
) {
    let forward = match norm3(sub3(target, cam_pos)) {
        Some(v) => v, None => return,
    };
    // If forward is (near-)parallel to world up, the cross collapses. Use
    // world X as the right vector in that degenerate case.
    let world_up = [0.0_f32, 1.0, 0.0];
    let right = norm3(cross3(forward, world_up)).unwrap_or([1.0, 0.0, 0.0]);
    let up = match norm3(cross3(right, forward)) {
        Some(v) => v, None => return,
    };

    // Columns: 0 = Right, 1 = Up, 2 = Back (= -Forward)
    let target_m: [[f32; 3]; 3] = [
        [right[0], up[0], -forward[0]],
        [right[1], up[1], -forward[1]],
        [right[2], up[2], -forward[2]],
    ];

    // smoothing slider semantics: 0 = no smoothing (snap), 1 = max smoothing
    // (slow lerp). Convert to lerp factor by inverting.
    let s = (1.0 - smoothing.clamp(0.0, 0.99)).max(0.01);
    let mut new_m = [[0.0_f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            new_m[i][j] = current[i][j] + (target_m[i][j] - current[i][j]) * s;
        }
    }

    // Write the 9 floats at `camera + CameraOrientation`. Offset for row i,
    // col j is `i * 12 + j * 4` (matches the read layout in `esp.rs`).
    let base = camera_ptr + offsets::Camera::CameraOrientation as u64;
    for i in 0..3u64 {
        for j in 0..3u64 {
            sdk.write::<f32>(base + i * 12 + j * 4, new_m[i as usize][j as usize]);
        }
    }
}

/// xorshift32 — cheap PRNG for legit miss roll.
fn rand(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32) / (u32::MAX as f32)
}

/// Map `cfg.part` → slot index inside `CharCache.parts`. Cherry's rig table
/// puts Head at 0, UpperTorso/Torso at 1, and the HRP fallback at slot 1.
fn part_slot(part: &str, rig_type: u8) -> usize {
    match part {
        "Torso" => 1,
        "HumanoidRootPart" => if rig_type == 2 { 1 } else { 0 },
        _ => 0, // Head
    }
}

pub fn start_aimbot_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        // Per-character last (pos, time) for velocity prediction.
        let mut prev: HashMap<u64, ([f32; 3], Instant)> = HashMap::new();
        let mut last_shot = Instant::now() - Duration::from_secs(10);
        let mut burst_left: u32 = 0;
        let mut rng: u32 = 0x12345678;

        loop {
            thread::sleep(Duration::from_millis(4));

            let state = app_handle.state::<AppState>();
            let cfg = state.aimbot_cfg.lock().unwrap().clone();
            if !cfg.enabled && !cfg.trigger_enabled { continue; }

            let sdk = match state.sdk.lock().unwrap().clone() {
                Some(s) => s, None => continue,
            };

            let aim_on = cfg.enabled && is_held(&cfg.keybind);
            let trig_on = cfg.trigger_enabled && is_held(&cfg.trigger_keybind);
            if !aim_on && !trig_on { continue; }

            let camera = match resolve_camera(&sdk) { Some(c) => c, None => continue };
            let cpo = offsets::Camera::CameraPosition as u64;
            let cmo = offsets::Camera::CameraOrientation as u64;
            let cam_pos = [
                sdk.read::<f32>(camera + cpo),
                sdk.read::<f32>(camera + cpo + 4),
                sdk.read::<f32>(camera + cpo + 8),
            ];
            let m = [
                [sdk.read::<f32>(camera + cmo),
                 sdk.read::<f32>(camera + cmo + 4),
                 sdk.read::<f32>(camera + cmo + 8)],
                [sdk.read::<f32>(camera + cmo + 12),
                 sdk.read::<f32>(camera + cmo + 16),
                 sdk.read::<f32>(camera + cmo + 20)],
                [sdk.read::<f32>(camera + cmo + 24),
                 sdk.read::<f32>(camera + cmo + 28),
                 sdk.read::<f32>(camera + cmo + 32)],
            ];

            let bounds = state.roblox_bounds.lock().unwrap().clone();
            let (win_x, win_y, win_w, win_h) =
                bounds.unwrap_or((0.0, 0.0, 1920.0, 1080.0));
            let render_h = (win_h - TITLEBAR_HEIGHT as f64).max(1.0);
            let aspect = (win_w / render_h) as f32;
            let view = build_view_matrix(cam_pos, m);
            let proj = build_projection_matrix(70.0, aspect, 0.5, 400.0);

            let caches = state.char_caches.lock().unwrap().clone();
            let cx_center = win_x + win_w * 0.5;
            let cy_center = win_y + TITLEBAR_HEIGHT as f64 + render_h * 0.5;
            // In Mouse mode the user aims by moving the cursor — the FOV
            // circle + "closest" selection should both be measured from the
            // current cursor, not the geometric screen centre. In Camera
            // mode the centre is the only meaningful reference.
            let (ref_x, ref_y) = if cfg.aim_type == "Camera" {
                (cx_center, cy_center)
            } else {
                current_cursor()
                    .map(|c| (c.x, c.y))
                    .unwrap_or((cx_center, cy_center))
            };
            let now = Instant::now();
            let fov_sq = (cfg.fov_radius as f64).powi(2);

            // best = (score, screen_x, screen_y, screen_distance, world_target)
            let mut best: Option<(f64, f64, f64, f64, [f32; 3])> = None;

            for cache in caches.iter() {
                let slot = part_slot(&cfg.part, cache.rig_type).min(cache.parts.len() - 1);
                let (prim, valid) = cache.parts[slot];
                if !valid { continue; }
                let pos = read_pos_fast(&sdk, prim);
                if !pos[0].is_finite() { continue; }
                if pos[0] == 0.0 && pos[1] == 0.0 && pos[2] == 0.0 { continue; }

                let dx = pos[0] - cam_pos[0];
                let dy = pos[1] - cam_pos[1];
                let dz = pos[2] - cam_pos[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                // Hardcoded self/overlap filter — anything sitting on top of
                // the local camera is either ourselves (first-person) or a
                // glitched read; we never want the aimbot to lock onto it.
                if dist < 0.5 { continue; }
                if dist < cfg.min_distance || dist > cfg.max_distance { continue; }

                // Prediction
                let mut tp = pos;
                if cfg.prediction_enabled {
                    if cfg.velocity_prediction {
                        if let Some((pp, pt)) = prev.get(&cache.char_addr).copied() {
                            let dt = (now - pt).as_secs_f32().max(0.001);
                            // Travel time estimate: dist / projectile_speed (300 studs/s default).
                            let k = cfg.prediction_strength * (dist / 300.0);
                            tp[0] += (pos[0] - pp[0]) / dt * k;
                            tp[1] += (pos[1] - pp[1]) / dt * k;
                            tp[2] += (pos[2] - pp[2]) / dt * k;
                        }
                    }
                    if cfg.bullet_drop_comp {
                        let travel = dist / 300.0;
                        tp[1] += 0.5 * cfg.gravity_comp * travel * travel;
                    }
                }
                prev.insert(cache.char_addr, (pos, now));

                let ndc = match world_to_ndc(tp, view, proj) {
                    Some(n) => n, None => continue,
                };
                let sx = win_x + (ndc[0] + 1.0) as f64 * 0.5 * win_w;
                let sy = win_y + TITLEBAR_HEIGHT as f64
                    + (1.0 - ndc[1] as f64) * 0.5 * render_h;
                let ssx = sx - ref_x;
                let ssy = sy - ref_y;
                let scr_d_sq = ssx * ssx + ssy * ssy;
                if scr_d_sq > fov_sq { continue; }
                let scr_d = scr_d_sq.sqrt();

                // In Mouse mode, *default* to cursor-distance scoring so the
                // aimbot picks whichever target is closest to where the user
                // is already aiming — even if the dropdown is on Distance.
                let score = if cfg.aim_type == "Mouse" {
                    scr_d
                } else {
                    match cfg.selection_mode.as_str() {
                        "Crosshair" => scr_d,
                        _ => dist as f64,
                    }
                };

                if best.map_or(true, |(s, _, _, _, _)| score < s) {
                    best = Some((score, sx, sy, scr_d, tp));
                }
            }

            // Aim — Mouse warps the OS cursor; Camera writes the orientation
            // matrix directly so the in-game camera snaps onto the target
            // independent of any user input.
            if aim_on {
                if let Some((_, sx, sy, _, tp)) = best {
                    let miss = cfg.legit_enabled
                        && cfg.legit_miss > 0.0
                        && rand(&mut rng) < cfg.legit_miss;
                    if !miss {
                        match cfg.aim_type.as_str() {
                            "Camera" => {
                                aim_camera(&sdk, camera, cam_pos, tp, cfg.smoothing, m);
                            }
                            _ => {
                                if let Some(cur) = current_cursor() {
                                    // Invert: 0 = snap, 1 = very smooth (small lerp factor)
                                    let smooth = (1.0 - cfg.smoothing as f64).max(0.01);
                                    let (mut nx, mut ny) = (lerp(cur.x, sx, smooth), lerp(cur.y, sy, smooth));
                                    if cfg.humanize {
                                        let jx = (rand(&mut rng) - 0.5) as f64;
                                        let jy = (rand(&mut rng) - 0.5) as f64;
                                        nx += jx;
                                        ny += jy;
                                    }
                                    warp(nx, ny);
                                }
                            }
                        }
                    }
                }
            }

            // Trigger
            if trig_on {
                if let Some((_, _, _, scr_d, _)) = best {
                    if scr_d < 6.0 {
                        let delay = cfg.trigger_delay as u128;
                        let since = now.duration_since(last_shot).as_millis();
                        if since >= delay {
                            click_left();
                            last_shot = now;
                            if cfg.trigger_burst { burst_left = 2; }
                        } else if burst_left > 0 && since >= 30 {
                            click_left();
                            last_shot = now;
                            burst_left -= 1;
                        }
                    }
                }
            }
        }
    });
}
