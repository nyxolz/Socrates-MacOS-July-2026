//! ESP frame producer.
//!
//! Resolves Camera, builds view/projection, walks `char_caches`, and emits a
//! per-player payload with everything the front needs to render: skeleton
//! segments, 2-D bbox, 3-D wireframe, view-direction line, head anchor for
//! text, name / distance / health.
//!
//! All geometry is in NDC ([-1, +1]). The front converts to pixels via the
//! shared `ndcToScreen` helper.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use obfstr::obfstr;

use crate::aimbot::current_cursor;
use crate::math::{build_projection_matrix, build_view_matrix, fov_from_zoom, segment_to_ndc, world_to_ndc, world_to_offscreen_dir};
use crate::offsets;
use crate::overlay::TITLEBAR_HEIGHT;
use crate::players::read_pos_fast;
use crate::sdk::SdkI;
use crate::security;
use crate::state::AppState;

#[derive(Serialize, Clone, Debug)]
pub struct EspMarker {
    pub name: String,
    pub ndc_x: f32,
    pub ndc_y: f32,
    pub on_screen: bool,
    pub distance: f32,
    pub world: [f32; 3],
}

/// Per-character payload. All segments are `[ax, ay, bx, by]` in NDC.
#[derive(Serialize, Clone, Debug)]
pub struct EspPlayer {
    pub name: String,
    pub distance: f32,
    pub health: f32,                    // -1 if unknown
    pub head: Option<[f32; 2]>,         // NDC anchor for text/tracer
    pub box2d: Option<[f32; 4]>,        // [xmin, ymin, xmax, ymax] in NDC
    pub box3d: Vec<[f32; 4]>,           // up to 12 edges
    pub view_dir: Option<[f32; 4]>,     // 1 segment
    pub skeleton: Vec<[f32; 4]>,        // segments
    /// 0 = enemy (full ESP), 1 = ally (only a green head dot is drawn).
    /// Neutrals are never emitted into the frame.
    pub relation: u8,
    /// World position (head/centre anchor) — used by the radar.
    pub world: [f32; 3],
}

#[derive(Serialize, Clone, Debug)]
pub struct EspFrame {
    pub markers: Vec<EspMarker>,
    pub players: Vec<EspPlayer>,
    pub player_count: usize,
    pub camera_pos: [f32; 3],
    /// Camera look direction (world) — radar uses its X/Z to orient "up".
    pub forward: [f32; 3],
    pub fov: f32,
    pub aspect: f32,
    pub attached: bool,
    pub cursor: Option<(f64, f64)>,
}

// Use the obfuscated chain resolver. Raw offset literals don't leak.
use security::resolve_camera;

fn read_camera(sdk: &SdkI, camera_ptr: u64) -> ([f32; 3], [[f32; 3]; 3]) {
    let pos_off = offsets::Camera::CameraPosition as u64;
    let mat_off = offsets::Camera::CameraOrientation as u64;
    let cam_pos = [
        sdk.read::<f32>(camera_ptr + pos_off),
        sdk.read::<f32>(camera_ptr + pos_off + 4),
        sdk.read::<f32>(camera_ptr + pos_off + 8),
    ];
    let matrix = [
        [sdk.read::<f32>(camera_ptr + mat_off),
         sdk.read::<f32>(camera_ptr + mat_off + 4),
         sdk.read::<f32>(camera_ptr + mat_off + 8)],
        [sdk.read::<f32>(camera_ptr + mat_off + 12),
         sdk.read::<f32>(camera_ptr + mat_off + 16),
         sdk.read::<f32>(camera_ptr + mat_off + 20)],
        [sdk.read::<f32>(camera_ptr + mat_off + 24),
         sdk.read::<f32>(camera_ptr + mat_off + 28),
         sdk.read::<f32>(camera_ptr + mat_off + 32)],
    ];
    (cam_pos, matrix)
}

fn project_marker(
    name: &str,
    world: [f32; 3],
    cam_pos: [f32; 3],
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
) -> EspMarker {
    let dx = world[0] - cam_pos[0];
    let dy = world[1] - cam_pos[1];
    let dz = world[2] - cam_pos[2];
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if let Some(ndc) = world_to_ndc(world, view, proj) {
        return EspMarker {
            name: name.to_string(),
            ndc_x: ndc[0], ndc_y: ndc[1],
            on_screen: ndc[0] >= -1.0 && ndc[0] <= 1.0 && ndc[1] >= -1.0 && ndc[1] <= 1.0,
            distance, world,
        };
    }
    let (dx_n, dy_n) = world_to_offscreen_dir(world, view, proj).unwrap_or((0.0, 0.0));
    EspMarker {
        name: name.to_string(),
        ndc_x: dx_n, ndc_y: dy_n,
        on_screen: false, distance, world,
    }
}

/// Read the LookVector from a primitive. CFrame rotation = 9 floats just
/// before Position (0xEC), starting at 0xC8 (layout i*12 + j*4, columns
/// Right/Up/Back). (0xC8,0xD4,0xE0) is the Right column → trait latéral.
/// LookVector = -(Back column) = -(m[i][2]) = -(0xD0, 0xDC, 0xE8).
fn read_look_dir(sdk: &SdkI, primitive: u64) -> [f32; 3] {
    let dir = [
        -sdk.read::<f32>(primitive + 0xD0),
        -sdk.read::<f32>(primitive + 0xDC),
        -sdk.read::<f32>(primitive + 0xE8),
    ];
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if l < 1e-5 || !l.is_finite() { return [0.0, 0.0, 1.0]; }
    [dir[0] / l, dir[1] / l, dir[2] / l]
}

pub fn start_esp_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        loop {
            let state = app_handle.state::<AppState>();
            let sdk_opt = state.sdk.lock().unwrap().clone();

            let sdk = match sdk_opt {
                Some(s) => s,
                None => {
                    // Process name kept off-binary by obfstr — `strings` on
                    // the release Mach-O won't show "RobloxPlayer".
                    let pname = obfstr!("RobloxPlayer").to_string();
                    if let Ok(new_sdk) = SdkI::new(&pname) {
                        *state.sdk.lock().unwrap() = Some(Arc::new(new_sdk));
                    }
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };

            let camera_ptr = match resolve_camera(&sdk) {
                Some(c) => c,
                None => {
                    let _ = app_handle.emit("esp_frame", EspFrame {
                        markers: vec![],
                        players: vec![],
                        player_count: 0,
                        camera_pos: [0.0; 3],
                        forward: [0.0, 0.0, 1.0],
                        fov: 70.0,
                        aspect: 16.0 / 9.0,
                        attached: true,
                        cursor: None,
                    });
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }
            };

            let (cam_pos, matrix) = read_camera(&sdk, camera_ptr);

            let bounds = state.roblox_bounds.lock().unwrap().clone();
            let (win_w, win_h) = bounds.map(|(_, _, w, h)| (w, h)).unwrap_or((1920.0, 1080.0));
            let render_h = (win_h - TITLEBAR_HEIGHT as f64).max(1.0);
            let aspect = (win_w / render_h) as f32;
            // FOV is derived from the camera's zoom factor `c` (third component
            // of the viewport Vec3 at `Camera + Fov`), not hardcoded.
            let zoom = sdk.read::<f32>(camera_ptr + offsets::Camera::Fov as u64 + 8);
            let fov = fov_from_zoom(zoom);
            let view = build_view_matrix(cam_pos, matrix);
            let proj = build_projection_matrix(fov, aspect, 0.5, 400.0);

            let origin = project_marker("origin", [0.0, 0.0, 0.0], cam_pos, view, proj);

            let caches = state.char_caches.lock().unwrap().clone();
            let visuals = state.visuals_cfg.lock().unwrap().clone();
            let relations = state.player_relations.lock().unwrap().clone();
            let max_dist = visuals.max_distance;
            let min_dist = visuals.min_distance.max(0.0);

            let mut players: Vec<EspPlayer> = Vec::with_capacity(caches.len());
            for cache in caches.iter() {
                // Relation gate: neutrals (2) are never drawn; allies (1) get
                // only a green head dot (handled below); enemies (0) get the
                // full ESP build.
                let relation = relations.get(&cache.name).copied().unwrap_or(0);
                if relation == 2 { continue; }

                // Read every valid part once; reuse for skeleton + box2d
                // bounding + box centre choice.
                let mut world_positions: [Option<[f32; 3]>; 11] = [None; 11];
                let mut head_world: Option<[f32; 3]> = None;
                let mut center_world: Option<[f32; 3]> = None;
                for (i, (prim, valid)) in cache.parts.iter().enumerate() {
                    if !*valid { continue; }
                    let pos = read_pos_fast(&sdk, *prim);
                    if !pos[0].is_finite() { continue; }
                    if pos[0] == 0.0 && pos[1] == 0.0 && pos[2] == 0.0 { continue; }
                    world_positions[i] = Some(pos);
                    if i == 0 { head_world = Some(pos); }
                    if center_world.is_none() { center_world = Some(pos); }
                }

                let anchor = match head_world.or(center_world) { Some(a) => a, None => continue };

                // Distance + max-distance filter
                let dx = anchor[0] - cam_pos[0];
                let dy = anchor[1] - cam_pos[1];
                let dz = anchor[2] - cam_pos[2];
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                // Allies: emit a stripped payload carrying only the head
                // anchor (for the green dot) — no boxes / skeleton / text, and
                // independent of the ESP distance filters so teammates stay
                // marked at any range.
                if relation == 1 {
                    let head_ndc = head_world.and_then(|h| world_to_ndc(h, view, proj))
                        .or_else(|| world_to_ndc(anchor, view, proj));
                    if head_ndc.is_some() {
                        players.push(EspPlayer {
                            name: cache.name.clone(),
                            distance,
                            health: -1.0,
                            head: head_ndc,
                            box2d: None,
                            box3d: Vec::new(),
                            view_dir: None,
                            skeleton: Vec::new(),
                            relation: 1,
                            world: anchor,
                        });
                    }
                    continue;
                }

                if distance < min_dist { continue; }
                if distance > max_dist { continue; }

                // Health (Humanoid + 0x184). -1 sentinel if read garbage.
                let mut health = -1.0_f32;
                if sdk.is_valid_ptr(cache.humanoid_addr) {
                    let h = sdk.read::<f32>(cache.humanoid_addr + offsets::Humanoid::Health as u64);
                    if h.is_finite() && (0.0..=1000.0).contains(&h) {
                        health = h.clamp(0.0, 100.0);
                    }
                }

                // Skeleton segments (NDC, near-plane clipped)
                let mut skeleton: Vec<[f32; 4]> = Vec::with_capacity(10);
                let mut ndcs: Vec<[f32; 2]> = Vec::with_capacity(11);
                for (i, p) in world_positions.iter().enumerate() {
                    if let Some(pos) = p {
                        if let Some(n) = world_to_ndc(*pos, view, proj) {
                            ndcs.push(n);
                            // tag slot index into the ndc list isn't needed —
                            // we project pairs via cache.segments() below.
                            let _ = i;
                        }
                    }
                }
                for &(a, b) in cache.segments() {
                    let (pa, pb) = match (world_positions[a], world_positions[b]) {
                        (Some(pa), Some(pb)) => (pa, pb),
                        _ => continue,
                    };
                    if let Some((na, nb)) = segment_to_ndc(pa, pb, view, proj) {
                        skeleton.push([na[0], na[1], nb[0], nb[1]]);
                    }
                }

                // Extend arms/legs to the hands/feet (R15 has no part there),
                // so the skeleton covers the full limb. The extra world tips
                // also feed the box bounds below.
                let mut limb_tips: Vec<[f32; 3]> = Vec::with_capacity(4);
                for &(from, to, factor) in cache.tips() {
                    let (pf, pt) = match (world_positions[from], world_positions[to]) {
                        (Some(pf), Some(pt)) => (pf, pt),
                        _ => continue,
                    };
                    let tip = [
                        pt[0] + (pt[0] - pf[0]) * factor,
                        pt[1] + (pt[1] - pf[1]) * factor,
                        pt[2] + (pt[2] - pf[2]) * factor,
                    ];
                    if let Some((na, nb)) = segment_to_ndc(pt, tip, view, proj) {
                        skeleton.push([na[0], na[1], nb[0], nb[1]]);
                    }
                    limb_tips.push(tip);
                }

                // Head NDC (text anchor) — falls back to centre projection.
                let head_ndc = head_world.and_then(|h| world_to_ndc(h, view, proj))
                    .or_else(|| world_to_ndc(anchor, view, proj));

                // Box 2D — bbox of the projected NDC ends of every skeleton
                // segment (already clipped to the near plane).
                let mut box2d = None;
                if !skeleton.is_empty() {
                    let (mut xmin, mut ymin) = (f32::INFINITY, f32::INFINITY);
                    let (mut xmax, mut ymax) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                    for seg in &skeleton {
                        for &v in &[seg[0], seg[2]] { xmin = xmin.min(v); xmax = xmax.max(v); }
                        for &v in &[seg[1], seg[3]] { ymin = ymin.min(v); ymax = ymax.max(v); }
                    }
                    // Padding so the box doesn't hug the body. A bit roomier
                    // now that limb tips push the bounds out to hands/feet.
                    let px = (xmax - xmin) * 0.25;
                    let py = (ymax - ymin) * 0.10;
                    box2d = Some([xmin - px, ymin - py, xmax + px, ymax + py]);
                }

                // Box 3D — dynamic AABB of every read part position. The
                // cube grows/shrinks with the character's pose (arms out =
                // wider, crouch = shorter, etc.) instead of being a fixed
                // cube hovering around the head/text area.
                let mut box3d: Vec<[f32; 4]> = Vec::with_capacity(12);
                {
                    let mut count = 0usize;
                    let mut xmin = f32::INFINITY;
                    let mut xmax = f32::NEG_INFINITY;
                    let mut ymin = f32::INFINITY;
                    let mut ymax = f32::NEG_INFINITY;
                    let mut zmin = f32::INFINITY;
                    let mut zmax = f32::NEG_INFINITY;
                    for p in world_positions.iter().filter_map(|p| *p).chain(limb_tips.iter().copied()) {
                        xmin = xmin.min(p[0]); xmax = xmax.max(p[0]);
                        ymin = ymin.min(p[1]); ymax = ymax.max(p[1]);
                        zmin = zmin.min(p[2]); zmax = zmax.max(p[2]);
                        count += 1;
                    }
                    if count >= 2 {
                        // Pad for limb thickness on horizontal axes; feet
                        // sit roughly 1 stud below the lowest cached joint
                        // (lower legs), and the head top is ~0.5 above the
                        // head joint.
                        let pad_xz = 0.5_f32;
                        xmin -= pad_xz; xmax += pad_xz;
                        zmin -= pad_xz; zmax += pad_xz;
                        ymin -= 1.0;
                        ymax += 0.5;

                        let corners = [
                            [xmin, ymin, zmin], [xmax, ymin, zmin],
                            [xmin, ymax, zmin], [xmax, ymax, zmin],
                            [xmin, ymin, zmax], [xmax, ymin, zmax],
                            [xmin, ymax, zmax], [xmax, ymax, zmax],
                        ];
                        const EDGES: [(usize, usize); 12] = [
                            (0,1),(2,3),(4,5),(6,7),  // X
                            (0,2),(1,3),(4,6),(5,7),  // Y
                            (0,4),(1,5),(2,6),(3,7),  // Z
                        ];
                        for &(a, b) in &EDGES {
                            if let Some((na, nb)) = segment_to_ndc(corners[a], corners[b], view, proj) {
                                box3d.push([na[0], na[1], nb[0], nb[1]]);
                            }
                        }
                    }
                }

                // View direction — segment from head world along LookVector
                // for `view_dir_length` studs. Uses head primitive's look.
                let mut view_dir = None;
                if let (Some(head_w), Some((head_prim, true))) = (head_world, cache.parts.get(0).copied()) {
                    let dir = read_look_dir(&sdk, head_prim);
                    let end = [
                        head_w[0] + dir[0] * visuals.view_dir_length,
                        head_w[1] + dir[1] * visuals.view_dir_length,
                        head_w[2] + dir[2] * visuals.view_dir_length,
                    ];
                    if let Some((a, b)) = segment_to_ndc(head_w, end, view, proj) {
                        view_dir = Some([a[0], a[1], b[0], b[1]]);
                    }
                }

                players.push(EspPlayer {
                    name: cache.name.clone(),
                    distance,
                    health,
                    head: head_ndc,
                    box2d,
                    box3d,
                    view_dir,
                    skeleton,
                    relation: 0,
                    world: anchor,
                });
            }

            let cursor_local = state.roblox_bounds.lock().unwrap()
                .and_then(|(bx, by, _, _)| current_cursor().map(|c| (c.x - bx, c.y - by)));

            // Camera look direction (world). Roblox CFrame stores -lookVector
            // in the third column of the rotation matrix; negate to get the
            // forward the radar orients "up" to.
            let forward = [-matrix[0][2], -matrix[1][2], -matrix[2][2]];

            let _ = app_handle.emit("esp_frame", EspFrame {
                markers: vec![origin],
                player_count: players.len(),
                players,
                camera_pos: cam_pos,
                forward,
                fov,
                aspect,
                attached: true,
                cursor: cursor_local,
            });

            thread::sleep(Duration::from_millis(16));
        }
    });
}
