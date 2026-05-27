//! Aimbot + trigger thread.
//!
//! 250 Hz loop. Reads `AppState.aimbot_cfg` every tick. When the keybind is
//! held it walks the char_caches, picks the best target according to the
//! selection mode (with min/max distance + FOV filters), optionally applies
//! linear velocity prediction + bullet drop, and projects to screen. In
//! Mouse mode it moves the OS cursor with a synthetic CGEvent (mouse-moved,
//! posted to the HID tap) toward the projected target, lerped for smoothing.
//! In Camera mode it writes the camera orientation matrix. Trigger bot fires
//! `LeftMouseDown/Up` when a
//! target sits within ~6 px of the crosshair (delay + burst respected).

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use obfstr::obfstr;
use tauri::{AppHandle, Manager};

use crate::math::{build_projection_matrix, build_view_matrix, fov_from_zoom, world_to_ndc};
use crate::offsets;
use crate::overlay::TITLEBAR_HEIGHT;
use crate::players::read_pos_fast;
use crate::sdk::SdkI;
use crate::security;
use crate::state::AppState;

use crate::input::is_held;

/// Move the OS cursor to `(x, y)` in global screen points.
///
/// `CGWarpMouseCursorPosition` is the canonical macOS API for absolute cursor
/// placement — it actually relocates the visible cursor (a bare `MouseMoved`
/// CGEvent often doesn't). After a warp macOS dissociates the HID mouse from
/// the cursor for ~250 ms, so we immediately re-associate to keep movement
/// responsive. Finally we post a `MouseMoved` event at the same point so games
/// that read the event stream (not just the polled cursor) register the move.
fn move_mouse_to(x: f64, y: f64) {
    let pt = CGPoint::new(x, y);
    let _ = CGDisplay::warp_mouse_cursor_position(pt);
    let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(true);
    if let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        if let Ok(e) = CGEvent::new_mouse_event(
            src, CGEventType::MouseMoved, pt, CGMouseButton::Left,
        ) {
            e.post(CGEventTapLocation::HID);
        }
    }
}

/// Simulate a *relative* mouse-look movement of `(dx, dy)` — the exact input a
/// player makes with the mouse to rotate the camera.
///
/// The key point: we keep the cursor where it is and carry the motion purely
/// in the `kCGMouseEventDeltaX/Y` fields. A mouse-look game reads those raw
/// deltas to turn the view. Moving the *absolute* position instead (as
/// `move_mouse_to` does) would be treated as cursor positioning, which doesn't
/// rotate the camera. We post `MouseMoved` to the HID tap so it enters the
/// same input stream as a physical mouse.
fn nudge_mouse(dx: f64, dy: f64) {
    let src = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s, Err(_) => return,
    };
    // Event location = current cursor, unchanged. Only the deltas move.
    let cur = CGEvent::new(src.clone())
        .map(|e| e.location())
        .unwrap_or(CGPoint::new(0.0, 0.0));
    if let Ok(e) = CGEvent::new_mouse_event(src, CGEventType::MouseMoved, cur, CGMouseButton::Left) {
        e.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as i64);
        e.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as i64);
        e.post(CGEventTapLocation::HID);
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

// `resolve_camera` lives in `security` now (uses XOR'd offsets so the raw
// literals never appear inline). Re-export for the rest of this module.
use security::resolve_camera;

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

/// Returns the orientation matrix it actually wrote, so the caller can compare
/// it against the camera state next tick and detect when the game's camera
/// module reverts our write (default camera) versus keeps it (scriptable).
fn aim_camera(
    sdk: &SdkI,
    camera_ptr: u64,
    cam_pos: [f32; 3],
    target: [f32; 3],
    smoothing: f32,
    current: [[f32; 3]; 3],
) -> [[f32; 3]; 3] {
    let forward = match norm3(sub3(target, cam_pos)) {
        Some(v) => v, None => return current,
    };
    // If forward is (near-)parallel to world up, the cross collapses. Use
    // world X as the right vector in that degenerate case.
    let world_up = [0.0_f32, 1.0, 0.0];
    let right = norm3(cross3(forward, world_up)).unwrap_or([1.0, 0.0, 0.0]);
    let up = match norm3(cross3(right, forward)) {
        Some(v) => v, None => return current,
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

    new_m
}

/// Force the camera into Scriptable (6) so Roblox's camera module stops
/// driving `CameraOrientation` and our matrix write sticks. Saves the previous
/// CameraType into `saved` (usually 5/Custom) so it can be restored later.
/// No-op if we've already forced it (keeps the original saved value).
fn force_scriptable(sdk: &SdkI, camera_ptr: u64, saved: &mut Option<i32>) {
    let off = camera_ptr + offsets::Camera::CameraType as u64;
    // Save the original value once, on the first lock tick.
    if saved.is_none() {
        *saved = Some(sdk.read::<i32>(off));
    }
    // Re-assert Scriptable *every* tick — some games' own camera scripts
    // reset CameraType back to Custom each frame, so writing it once isn't
    // enough; we have to keep overwriting it while we hold the lock.
    sdk.write::<i32>(off, 4);
}

/// Restore the CameraType saved by `force_scriptable`, handing control back to
/// the camera module (the player can look around again). No-op if we never
/// forced it.
fn restore_cam_type(sdk: &SdkI, camera_ptr: u64, saved: &mut Option<i32>) {
    if let Some(prev) = saved.take() {
        sdk.write::<i32>(camera_ptr + offsets::Camera::CameraType as u64, prev);
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
    if part == obfstr!("Torso") { 1 }
    else if part == obfstr!("HumanoidRootPart") { if rig_type == 2 { 1 } else { 0 } }
    else { 0 } // Head
}

pub fn start_aimbot_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        // Per-character last (pos, time) for velocity prediction.
        let mut prev: HashMap<u64, ([f32; 3], Instant)> = HashMap::new();
        let mut last_shot = Instant::now() - Duration::from_secs(10);
        let mut burst_left: u32 = 0;
        let mut rng: u32 = 0x12345678;
        // Camera mode: while we're locked onto a target we force the camera to
        // Scriptable so its module stops reverting our orientation write.
        // `Some(prev)` = we forced it and must restore `prev` (usually 5/Custom)
        // when the lock ends; `None` = camera untouched.
        let mut saved_cam_type: Option<i32> = None;
        // TEMP diagnostics: track what we wrote last tick + a throttle counter.
        let mut dbg_tick: u32 = 0;
        let mut dbg_last: Option<[[f32; 3]; 3]> = None;
        // BETA diag: cursor-vibration phase (Camera mode, locked on target).
        // Cycles right → back → up → back to test whether the game's camera
        // reacts to relative mouse-look deltas.
        let mut vib_phase: u32 = 0;

        loop {
            thread::sleep(Duration::from_millis(1));

            let state = app_handle.state::<AppState>();
            let cfg = state.aimbot_cfg.lock().unwrap().clone();
            if !cfg.enabled && !cfg.trigger_enabled { continue; }

            let sdk = match state.sdk.lock().unwrap().clone() {
                Some(s) => s, None => continue,
            };

            // BETA diag: the moment Camera mode is enabled — no key held, no
            // target needed — nudge the camera continuously with a tiny
            // relative mouse-look delta. The pattern is symmetric (right →
            // back → up → back) so it oscillates without ever drifting away,
            // which keeps it unnoticeable while still proving in game that the
            // camera reacts to injected mouse movement. One phase per tick at
            // this loop rate, so it's effectively non-stop.
            if cfg.enabled && cfg.aim_type == "Camera" {
                let (vx, vy) = match vib_phase % 4 {
                    0 => (4.0, 0.0),   // right
                    1 => (-4.0, 0.0),  // back to centre
                    2 => (0.0, -4.0),  // up
                    _ => (0.0, 4.0),   // back to centre
                };
                vib_phase = vib_phase.wrapping_add(1);
                nudge_mouse(vx, vy);
            }

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
            let zoom = sdk.read::<f32>(camera + offsets::Camera::Fov as u64 + 8);
            let proj = build_projection_matrix(fov_from_zoom(zoom), aspect, 0.5, 400.0);

            let caches = state.char_caches.lock().unwrap().clone();
            let relations = state.player_relations.lock().unwrap().clone();
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
                // Only enemies (relation 0) are targetable. Allies (1) and
                // neutrals (2) are skipped entirely.
                if relations.get(&cache.name).copied().unwrap_or(0) != 0 { continue; }
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

            // Aim — Camera mode forces the camera to Scriptable and writes the
            // orientation matrix straight at the target (the only thing that
            // works in an FPS, where shots come from the camera, not the
            // cursor). Mouse mode writes Roblox's internal cursor-position Vec3
            // (no OS input, lands on macOS) for top-down / Mouse.Hit games.
            if aim_on {
                let mut locked = false;
                if let Some((_, sx, sy, _, tp)) = best {
                    let miss = cfg.legit_enabled
                        && cfg.legit_miss > 0.0
                        && rand(&mut rng) < cfg.legit_miss;
                    if !miss {
                        if cfg.aim_type == "Camera" {
                            // Halt the camera module so our write sticks, then
                            // point the camera at the target.
                            // How far did the camera drift from what we wrote
                            // last tick? ≈0 = our write held; large = something
                            // overwrote the orientation between ticks.
                            let drift = dbg_last.map(|lw| {
                                (m[0][2] - lw[0][2]).abs()
                                    + (m[1][2] - lw[1][2]).abs()
                                    + (m[2][2] - lw[2][2]).abs()
                            });
                            let ct_off = camera + offsets::Camera::CameraType as u64;
                            let ct_before = sdk.read::<i32>(ct_off);
                            force_scriptable(&sdk, camera, &mut saved_cam_type);
                            let ct_after = sdk.read::<i32>(ct_off);
                            let written = aim_camera(&sdk, camera, cam_pos, tp, cfg.smoothing, m);
                            dbg_last = Some(written);
                            dbg_tick = dbg_tick.wrapping_add(1);
                            if dbg_tick % 50 == 0 {
                                eprintln!(
                                    "[cam] CameraType before={ct_before} after_write={ct_after} (want 6) | orient_drift={:.4} (~0=our write held, big=overwritten)",
                                    drift.unwrap_or(-1.0)
                                );
                            }
                            locked = true;
                        } else {
                            // Mouse mode: drive the OS cursor toward the
                            // projected target via CGEvent. `sx/sy` are already
                            // global screen coordinates — the same space
                            // `current_cursor()` reports — so lerp from the
                            // current cursor for smoothing + optional jitter.
                            let (cx, cy) = current_cursor()
                                .map(|c| (c.x, c.y))
                                .unwrap_or((sx, sy));
                            let s = (1.0 - cfg.smoothing.clamp(0.0, 0.99)).max(0.01) as f64;
                            let (jx, jy) = if cfg.humanize {
                                ((rand(&mut rng) - 0.5) as f64, (rand(&mut rng) - 0.5) as f64)
                            } else { (0.0, 0.0) };
                            let nx = cx + (sx - cx) * s + jx;
                            let ny = cy + (sy - cy) * s + jy;
                            move_mouse_to(nx, ny);
                        }
                    }
                }
                // No target locked this tick (none in FOV, or a legit miss) →
                // hand the camera back so the player isn't frozen on nothing.
                if !locked {
                    restore_cam_type(&sdk, camera, &mut saved_cam_type);
                }
            } else {
                // Aim key released — restore camera control.
                restore_cam_type(&sdk, camera, &mut saved_cam_type);
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
