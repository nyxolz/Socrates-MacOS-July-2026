//! Aimbot + trigger thread.
//!
//! 250 Hz loop. Reads `AppState.aimbot_cfg` every tick. When the keybind is
//! held it walks the char_caches, picks the best target according to the
//! selection mode (with min/max distance + FOV filters), optionally applies
//! linear velocity prediction + bullet drop, and projects to screen. In
//! Mouse mode it warps the OS cursor toward the projected target, lerped for
//! smoothing. In Camera mode it writes the camera orientation matrix only —
//! `CameraType` is never touched, so the character controller keeps working
//! and the player can move freely while the aimbot tracks.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};
use core_graphics::event::EventField;
use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
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
/// `warp_mouse_cursor_position` is the canonical macOS API for absolute cursor
/// placement — it actually relocates the visible cursor. After a warp macOS
/// dissociates the HID mouse from the cursor for ~250 ms, so we immediately
/// re-associate to keep movement responsive. Finally we post a `MouseMoved`
/// event at the same point so games that read the event stream (not just the
/// polled cursor) register the move.
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
fn move_mouse_relative(dx: f64, dy: f64) {
    if let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        // Crée un événement de mouvement à la position courante
        // (la position absolue sera ignorée par le jeu au profit des deltas)
        let pos = current_cursor().unwrap_or(CGPoint::new(0.0, 0.0));
        if let Ok(event) = CGEvent::new_mouse_event(
            src,
            CGEventType::MouseMoved,
            pos,
            CGMouseButton::Left,
        ) {
            // Définit les champs delta (entiers longs)
            event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as i64);
            event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as i64);
            event.post(CGEventTapLocation::HID);
        }
    }
}

/// Camera-follow mouse move (la version qui fait *tourner la caméra* comme
/// sur Windows). Le secret macOS : Roblox ne fait pivoter la caméra qu'en
/// lisant les **deltas relatifs** du périphérique HID — téléporter le curseur
/// (`warp_mouse_cursor_position`) n'en génère aucun, donc la caméra ne bouge
/// pas. Ici on :
///   1. désaccouple le curseur du mouvement HID (`associate(false)`) pour que
///      le système ne "réabsorbe" pas notre delta en recentrant le curseur,
///      exactement comme quand un FPS verrouille la souris ;
///   2. poste un `MouseMoved` portant `DELTA_X/DELTA_Y` — l'équivalent direct
///      de `SendInput(MOUSEEVENTF_MOVE)` côté Windows ;
///   3. recolle aussitôt le curseur pour rester réactif hors-jeu.
///
/// `pressed` indique si un bouton souris est tenu (aim tenu sur clic droit par
/// ex.) : on génère alors un `*MouseDragged` à la place de `MouseMoved`, sinon
/// certains jeux ignorent le delta tant qu'un bouton est enfoncé.
fn move_camera_delta(dx: f64, dy: f64, pressed: bool) {
    if dx > -0.01 && dx < 0.01 && dy > -0.01 && dy < 0.01 { return; }
    let src = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s, Err(_) => return,
    };
    let pos = current_cursor().unwrap_or(CGPoint::new(0.0, 0.0));
    let ev_type = if pressed { CGEventType::LeftMouseDragged } else { CGEventType::MouseMoved };
    let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(false);
    if let Ok(event) = CGEvent::new_mouse_event(src, ev_type, pos, CGMouseButton::Left) {
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as i64);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as i64);
        event.post(CGEventTapLocation::HID);
    }
    let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(true);
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
// literals never appear inline).
use security::resolve_camera;
use crate::offsets::CameraViewPort;

fn lerp(a: f64, b: f64, t: f64) -> f64 { a + (b - a) * t.clamp(0.0, 1.0) }

/// Classic Hermite smoothstep (ease-in-out) on `[0,1]`.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Effective smoothing for this tick. With `fov_scale` on, the smoothing is
/// scaled by how far the target sits inside the FOV: near the crosshair the
/// aim is snappier (less smoothing), at the edge it keeps the full configured
/// value — a human flicks toward a far target, then tracks gently up close.
fn effective_smoothing(base: f32, scr_d: f64, fov_radius: f32, fov_scale: bool) -> f32 {
    if !fov_scale || fov_radius <= 1.0 {
        return base;
    }
    let t = (scr_d as f32 / fov_radius).clamp(0.0, 1.0); // 0 = centre, 1 = edge
    (base * (0.2 + 0.8 * t)).clamp(0.0, 0.99)
}

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
    lerp_factor: f32, current: [[f32; 3]; 3],
) {
    aim_camera(sdk, camera_ptr, cam_pos, target, lerp_factor, current);
}

fn aim_camera(
    sdk: &SdkI,
    camera_ptr: u64,
    cam_pos: [f32; 3],
    target: [f32; 3],
    lerp_factor: f32,
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

    // `lerp_factor` is already this tick's interpolation amount (0 = no move,
    // 1 = snap straight to target). Computed time-based by the caller.
    let s = lerp_factor.clamp(0.0, 1.0);
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
fn move_mouse_physical(x: f64, y: f64) {
    if let Ok(src) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        // CGDisplayMoveCursor utilise la source HID réelle,
        // générant des événements delta comme un vrai périphérique.
        // Il faut le point cible en coordonnées globales.
        let target = CGPoint::new(x, y);
        if let Ok(event) = CGEvent::new_mouse_event(
            src,
            CGEventType::MouseMoved,
            target,
            CGMouseButton::Left,
        ) {
            event.post(CGEventTapLocation::HID);
        }
    }
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
        // Last tick time, for framerate-independent (time-based) smoothing.
        let mut last_aim_t = Instant::now();
        let mut burst_left: u32 = 0;
        let mut rng: u32 = 0x12345678;

        // ── Legit / humanizer persistent state ──────────────────────────
        // When the aim key was first held (reaction-time gate). None = not held.
        let mut aim_press_started: Option<Instant> = None;
        // Currently locked target (char_addr). With sticky aim it's set when
        // the aimbot engages and cleared on key release — a hard lock for the
        // whole activation, so it never switches to another player.
        let mut locked_target: Option<u64> = None;
        // Low-pass-filtered jitter offset — the humanizer wanders smoothly
        // toward fresh random goals instead of teleporting (white noise).
        let mut jitter = [0.0f32, 0.0f32];
        let mut jitter_goal = [0.0f32, 0.0f32];

        // Toggle-mode state for the aim keybind: `aim_toggled` is flipped on
        // each fresh press (rising edge tracked via `aim_key_prev`).
        let mut aim_toggled = false;
        let mut aim_key_prev = false;

        // Supervisor: a transient panic in the hot loop (e.g. a bad read right
        // after a place change on rejoin) must not kill the aimbot for the rest
        // of the session — the ESP thread survives such events, so the aimbot
        // should too. Run the loop under catch_unwind and restart it on panic;
        // the persistent state declared above is preserved across restarts.
        loop {
          let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            thread::sleep(Duration::from_millis(0));

            let state = app_handle.state::<AppState>();
            let cfg = state.aimbot_cfg.lock().unwrap().clone();
            let is_silent_aim = cfg.aim_type == "Silent Aim";
            if !cfg.enabled && !cfg.trigger_enabled { continue; }

            let sdk = match state.sdk.lock().unwrap().clone() {
                Some(s) => s, None => continue,
            };

            // Aim key state. In "Toggle" mode a fresh press flips a latch
            // (click to enable, click again to disable); otherwise the aimbot
            // is active only while the key is physically held ("Hold").
            let aim_key_down = is_held(&cfg.keybind);
            if cfg.keybind_mode == "Toggle" && cfg.enabled {
                if aim_key_down && !aim_key_prev { aim_toggled = !aim_toggled; }
            } else {
                aim_toggled = false; // clean latch in Hold mode or when disabled
            }
            aim_key_prev = aim_key_down;

            let aim_on = cfg.enabled
                && if cfg.keybind_mode == "Toggle" { aim_toggled } else { aim_key_down };
            let trig_on = cfg.trigger_enabled && is_held(&cfg.trigger_keybind);

            // Reaction-time gate: on a fresh key press, wait `reaction_time` ms
            // before the aim actually engages (human reflex). Released key resets.
            let react_ready = if aim_on {
                let started = *aim_press_started.get_or_insert_with(Instant::now);
                started.elapsed().as_millis() as f32 >= cfg.reaction_time
            } else {
                aim_press_started = None;
                false
            };
            if !aim_on { locked_target = None; }

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
            // Team check: characters sharing the local player's parent container
            // are treated as allies and never targeted.
            let local_parent = state.local_parent.load(std::sync::atomic::Ordering::Relaxed);
            let cx_center = win_x + win_w * 0.5;
            let cy_center = win_y + TITLEBAR_HEIGHT as f64 + render_h * 0.5;
            // In Mouse mode the user aims by moving the cursor — the FOV
            // circle + "closest" selection should both be measured from the
            // current cursor, not the geometric screen centre. In Camera
            // mode the centre is the only meaningful reference.
            let (ref_x, ref_y) = if cfg.aim_type == "Camera" || is_silent_aim {
                (cx_center, cy_center)
            } else {
                current_cursor()
                    .map(|c| (c.x, c.y))
                    .unwrap_or((cx_center, cy_center))
            };
            let now = Instant::now();
            // Real elapsed time since the previous tick — drives time-based
            // smoothing so the aim speed doesn't depend on the (uncapped) loop
            // rate. Clamped to avoid a huge jump after a pause.
            let dt = now.duration_since(last_aim_t).as_secs_f32().clamp(0.0001, 0.1);
            last_aim_t = now;
            let fov_sq = (cfg.fov_radius as f64).powi(2);

            // best = (score, screen_x, screen_y, screen_distance, world_target, char_addr)
            let mut best: Option<(f64, f64, f64, f64, [f32; 3], u64)> = None;
            // The candidate matching the currently-locked target, if still valid
            // and inside the FOV — used by sticky aim / target-switch delay.
            let mut locked_cand: Option<(f64, f64, f64, f64, [f32; 3], u64)> = None;

            for cache in caches.iter() {
                // Only enemies (relation 0) are targetable. Allies (1) and
                // neutrals (2) are skipped entirely. With Team check on,
                // same-parent players resolve to ally and are skipped here.
                let manual = relations.get(&cache.name).copied().unwrap_or(0);
                let relation = crate::players::effective_relation(
                    manual, cfg.team_check, cache.parent_addr, local_parent);
                if relation != 0 { continue; }
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

                let cand = (score, sx, sy, scr_d, tp, cache.char_addr);
                if best.map_or(true, |(s, ..)| score < s) {
                    best = Some(cand);
                }
                if Some(cache.char_addr) == locked_target {
                    locked_cand = Some(cand);
                }
            }

            // ── Force lock (always on — built into Socrate's aimbot) ────
            // The target is chosen when the aimbot first engages
            // (locked_target was None) and is kept for the WHOLE activation —
            // `locked_target` is cleared only on key release above. While
            // locked we ONLY aim that player: another player passing by or
            // entering the FOV is never focused. If the locked target is
            // momentarily not a valid candidate (out of FOV / occluded) we
            // simply don't aim, instead of switching to someone else.
            let chosen = match locked_target {
                Some(_) => locked_cand,
                None => {
                    if let Some(b) = best { locked_target = Some(b.5); }
                    best
                }
            };

            // Aim — Mouse warps the OS cursor; Camera writes the orientation
            // matrix directly so the in-game camera snaps onto the target
            // without ever touching `CameraType` (which would lock the
            // character controller and freeze player movement).
            if aim_on && react_ready {
                if let Some((_, sx, sy, scr_d, tp, _)) = chosen {
                    let miss = cfg.legit_enabled
                        && cfg.legit_miss > 0.0
                        && rand(&mut rng) < cfg.legit_miss;
                    // Smoothing for this tick — softened at the FOV edge when enabled.
                    let eff_smooth =
                        effective_smoothing(cfg.smoothing, scr_d, cfg.fov_radius, cfg.fov_scale_smoothing);
                    // Framerate-independent smoothing: map the slider to a time
                    // constant `tau` (higher smoothness → larger tau → slower,
                    // weaker aim) and turn it into this tick's interpolation
                    // factor via an exponential. 0 → near-instant snap, 1 → very
                    // weak. This is what makes "max smoothness" actually weak —
                    // a plain per-tick fraction would always snap on the
                    // uncapped loop.
                    let tau = lerp(0.01, 0.6, eff_smooth as f64) as f32;
                    let alpha = (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0);
                    if !miss {
                        match cfg.aim_type.as_str() {
                            "Camera" => {
                                aim_camera(&sdk, camera, cam_pos, tp, alpha, m);
                            }
                            "Silent Aim" => {
                                // Coordonnées de la cible dans la fenêtre de rendu (sans la barre de titre).
                                // Clampées dans [1, taille] : une cible au bord donnait un viewport
                                // négatif/nul -> le rayon de tir partait n'importe où (ratés).
                                let mut target_x = ((sx - win_x) as f32).clamp(1.0, win_w as f32);
                                let mut target_y = ((sy - win_y - TITLEBAR_HEIGHT as f64) as f32)
                                    .clamp(1.0, render_h as f32);

                                // Miss volontaire : avec une probabilité `silent_miss_rate`,
                                // on décale la cible de 120-280 px dans une direction aléatoire
                                // pour que le rayon de tir parte à côté exprès.
                                if cfg.silent_miss_rate > 0.0 && rand(&mut rng) < cfg.silent_miss_rate {
                                    let ang = rand(&mut rng) * std::f32::consts::TAU;
                                    let mag = 120.0 + rand(&mut rng) * 160.0;
                                    target_x = (target_x + ang.cos() * mag).clamp(1.0, win_w as f32);
                                    target_y = (target_y + ang.sin() * mag).clamp(1.0, render_h as f32);
                                }

                                // Pour que le centre du viewport tombe exactement sur la cible,
                                // on définit sa taille comme 2 * (coordonnées de la cible).
                                // Ainsi, le point (target_x, target_y) devient le centre du repère
                                // et tout tir depuis le centre de l'écran pointera vers la cible.
                                // Hyper-spam : le jeu réécrit le viewport chaque
                                // frame, et au moment du tir notre écriture pouvait
                                // déjà être écrasée -> silent aim "coupé". On martèle
                                // la valeur en rafale pour gagner la course au tir.
                                for _ in 0..8 {
                                    sdk.write::<f32>(camera + CameraViewPort::ViewPort as u64, target_x * 2.0);
                                    sdk.write::<f32>(camera + CameraViewPort::ViewPort as u64 + 4, target_y * 2.0);
                                }
                            }
                            _ => {
                                // Mode Mouse : on ne téléporte plus le curseur (ça ne
                                // fait pas tourner la caméra sur macOS). On envoie un
                                // mouvement RELATIF vers la cible — la caméra suit comme
                                // sous Windows. Le delta = (cible - curseur) lissé.
                                if let Some(cur) = current_cursor() {
                                    let raw_dx = sx - cur.x;
                                    let raw_dy = sy - cur.y;
                                    let mut step = alpha as f64;
                                    // smooth_path: ease-out — decelerate as the
                                    // cursor closes the last ~80 px on the target
                                    // so the lock settles instead of snapping.
                                    if cfg.smooth_path {
                                        let d = (raw_dx * raw_dx + raw_dy * raw_dy).sqrt();
                                        let ease = smoothstep((d / 80.0) as f32) as f64;
                                        step *= 0.25 + 0.75 * ease;
                                    }
                                    let mut dx = raw_dx * step;
                                    let mut dy = raw_dy * step;
                                    if cfg.humanize {
                                        // Advance the low-pass jitter toward a fresh
                                        // random goal once it has nearly arrived —
                                        // smooth organic wander, not per-tick noise.
                                        let amp = cfg.humanize_strength;
                                        if (jitter[0] - jitter_goal[0]).abs() < 0.1
                                            && (jitter[1] - jitter_goal[1]).abs() < 0.1
                                        {
                                            jitter_goal = [
                                                (rand(&mut rng) * 2.0 - 1.0) * amp,
                                                (rand(&mut rng) * 2.0 - 1.0) * amp,
                                            ];
                                        }
                                        jitter[0] += (jitter_goal[0] - jitter[0]) * 0.15;
                                        jitter[1] += (jitter_goal[1] - jitter[1]) * 0.15;
                                        dx += jitter[0] as f64;
                                        dy += jitter[1] as f64;
                                    }
                                    // Per-tick speed cap (px) — bounds how fast the
                                    // cursor can travel so flicks look human.
                                    if cfg.max_speed > 0.0 {
                                        let sp = (dx * dx + dy * dy).sqrt();
                                        let cap = cfg.max_speed as f64;
                                        if sp > cap {
                                            dx = dx / sp * cap;
                                            dy = dy / sp * cap;
                                        }
                                    }
                                    let pressed = cfg.keybind == "MouseLeft" && is_held("MouseLeft");
                                    move_camera_delta(dx, dy, pressed);
                                }
                            }
                        }
                    }
                }
            }

            // Trigger
            if trig_on {
                if let Some((_, _, _, scr_d, ..)) = best {
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
          }));
          // Reached only if the hot loop panicked. Pause briefly, then the
          // outer loop restarts it with the preserved state intact.
          if outcome.is_ok() { break; }
          thread::sleep(Duration::from_millis(200));
        }
    });
}
