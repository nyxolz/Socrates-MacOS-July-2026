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
    /// Per-part oriented 3-D boxes. Each entry is the 8 projected corners in
    /// NDC (`[x, y]`), ordered by the sign bits of (sx, sy, sz) — index =
    /// 4·sx + 2·sy + 1·sz. The front draws the 12 edges and fills the 6 faces
    /// semi-transparently. One box per rig part, scaled to its real Size.
    pub part_boxes: Vec<[[f32; 2]; 8]>,
    /// 0 = enemy (full ESP), 1 = ally (only a green head dot is drawn).
    /// Neutrals are never emitted into the frame.
    pub relation: u8,
    /// World position (head/centre anchor) — used by the radar.
    pub world: [f32; 3],
}

/// One Workspace part for the dev "ESP all game objects" overlay.
#[derive(Serialize, Clone, Debug)]
pub struct EspObject {
    pub name: String,
    /// 8 projected NDC corners (same ordering as `EspPlayer::part_boxes`).
    pub box3d: [[f32; 2]; 8],
    /// Name anchor in NDC (part centre).
    pub label: [f32; 2],
}

#[derive(Serialize, Clone, Debug)]
pub struct EspFrame {
    pub markers: Vec<EspMarker>,
    pub players: Vec<EspPlayer>,
    pub objects: Vec<EspObject>,
    pub player_count: usize,
    pub camera_pos: [f32; 3],
    /// Camera look direction (world) — radar uses its X/Z to orient "up".
    pub forward: [f32; 3],
    /// Raw 3x3 CameraOrientation matrix as read from memory this frame — Dev tab live debug.
    pub view_matrix_raw: [[f32; 3]; 3],
    pub fov: f32,
    pub aspect: f32,
    pub attached: bool,
    pub cursor: Option<(f64, f64)>,
    /// Visuals "Bullet Tracer": [ox,oy,tx,ty,age,use_screen_origin] — re-projected each frame.
    pub bullet_vis_traces: Vec<[f32; 6]>,
    /// Total hits ever recorded (monotone). Frontend diffs against last value to play sounds.
    pub bullet_vis_hit_count: u64,
}

// Use the obfuscated chain resolver. Raw offset literals don't leak.
use security::resolve_camera;

/// Raw memory layout of a Roblox CFrame: a 3×3 rotation matrix (Right/Up/Back
/// columns) immediately followed by the Vec3 position — 48 contiguous bytes,
/// one `vm_read_overwrite` syscall instead of 12 separate float reads.
///
/// This exact layout is shared by every CFrame-holding struct we read:
/// Camera (`CameraOrientation` + `CameraPosition = CameraOrientation+0x24`)
/// AND every BasePart's Primitive (rotation at `+0xC8`, `Position = +0xC8+0x24
/// = 0xEC`) — same shape, different base offset. Reusable everywhere instead
/// of duplicating 12-read call sites.
///
/// Batching matters for two reasons:
///   1. **Correctness** — the camera is rewritten by Roblox's render thread
///      60+ times a second; spreading the read across 12 syscalls left a wide
///      window for a torn (part-old / part-new) read, which made every
///      projection fail for that tick and the whole ESP overlay blink off.
///   2. **Throughput** — Part ESP alone was doing ~13 syscalls per part
///      (1 pos read × 3 + 9 rotation floats + size); with up to 11 parts per
///      player this added up fast. One CFrame read replaces 12 of those.
#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct CFrameRaw {
    pub m: [[f32; 3]; 3],
    pub pos: [f32; 3],
}

fn read_camera(sdk: &SdkI, camera_ptr: u64) -> ([f32; 3], [[f32; 3]; 3]) {
    let mat_off = offsets::Camera::CameraOrientation as u64;
    let raw = sdk.read::<CFrameRaw>(camera_ptr + mat_off);
    (raw.pos, raw.m)
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
    // One read of the full rotation matrix (was 3 separate float reads for
    // just the Back column) — see `CFrameRaw`.
    let raw = sdk.read::<CFrameRaw>(primitive + 0xC8);
    let dir = [-raw.m[0][2], -raw.m[1][2], -raw.m[2][2]];
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if l < 1e-5 || !l.is_finite() { return [0.0, 0.0, 1.0]; }
    [dir[0] / l, dir[1] / l, dir[2] / l]
}

/// Project an oriented part box to its 8 NDC corners. Reads the part's world
/// Size (Primitive + 0x1B8), its CFrame basis (rotation columns at 0xC8,
/// Right/Up/Back) and position (0xEC), builds the 8 oriented corners and
/// projects each. Returns the corners ordered by the sign bits of (sx, sy, sz)
/// — index = 4·sx + 2·sy + 1·sz. `None` if any corner is behind the camera or
/// the part has a degenerate / absurd size (so a glitched read can't paint the
/// whole screen).
fn project_part_box(
    sdk: &SdkI,
    prim: u64,
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
) -> Option<[[f32; 2]; 8]> {
    // One read for rotation + position (was 1+9=10 separate float reads —
    // see `CFrameRaw`; Position lands at `+0xC8+0x24 == 0xEC`, matching
    // `Primitive::Position`, so this also replaces the old `read_pos_fast`
    // call), plus one more for Size — 2 syscalls total instead of 13.
    let cf = sdk.read::<CFrameRaw>(prim + 0xC8);
    let pos = cf.pos;
    if !pos[0].is_finite() { return None; }

    let size_off = offsets::Primitive::Size as u64;
    let size = sdk.read::<[f32; 3]>(prim + size_off);
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    if !(half[0].is_finite() && half[1].is_finite() && half[2].is_finite()) { return None; }
    if half[0] <= 0.0 && half[1] <= 0.0 && half[2] <= 0.0 { return None; }
    if half[0] > 200.0 || half[1] > 200.0 || half[2] > 200.0 { return None; }

    // CFrame rotation columns: Right (j=0), Up (j=1), Back (j=2).
    let right = [cf.m[0][0], cf.m[1][0], cf.m[2][0]];
    let up    = [cf.m[0][1], cf.m[1][1], cf.m[2][1]];
    let back  = [cf.m[0][2], cf.m[1][2], cf.m[2][2]];

    let mut corners = [[0.0_f32; 2]; 8];
    for bits in 0..8usize {
        let sx = if bits & 4 != 0 { half[0] } else { -half[0] };
        let sy = if bits & 2 != 0 { half[1] } else { -half[1] };
        let sz = if bits & 1 != 0 { half[2] } else { -half[2] };
        let world = [
            pos[0] + right[0] * sx + up[0] * sy + back[0] * sz,
            pos[1] + right[1] * sx + up[1] * sy + back[1] * sz,
            pos[2] + right[2] * sx + up[2] * sy + back[2] * sz,
        ];
        // All 8 corners must project, else the box is half behind the near
        // plane and a partial fill would look wrong — skip the whole part.
        let n = world_to_ndc(world, view, proj)?;
        corners[bits] = [n[0], n[1]];
    }
    Some(corners)
}

/// Project the oriented box of an arbitrary part (by its Primitive pointer),
/// returning the 8 NDC corners and the centre anchor. Mirrors `project_part_box`
/// but also hands back the projected centre for the name label.
fn project_object_box(
    sdk: &SdkI,
    prim: u64,
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
) -> Option<([[f32; 2]; 8], [f32; 2])> {
    // Same single-syscall batching as `project_part_box` — see `CFrameRaw`.
    let cf = sdk.read::<CFrameRaw>(prim + 0xC8);
    let pos = cf.pos;
    if !pos[0].is_finite() { return None; }

    let size_off = offsets::Primitive::Size as u64;
    let size = sdk.read::<[f32; 3]>(prim + size_off);
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    if !(half[0].is_finite() && half[1].is_finite() && half[2].is_finite()) { return None; }
    if half[0] <= 0.0 && half[1] <= 0.0 && half[2] <= 0.0 { return None; }
    if half[0] > 2000.0 || half[1] > 2000.0 || half[2] > 2000.0 { return None; }

    let right = [cf.m[0][0], cf.m[1][0], cf.m[2][0]];
    let up    = [cf.m[0][1], cf.m[1][1], cf.m[2][1]];
    let back  = [cf.m[0][2], cf.m[1][2], cf.m[2][2]];

    let center = world_to_ndc(pos, view, proj)?;
    let mut corners = [[0.0_f32; 2]; 8];
    for bits in 0..8usize {
        let sx = if bits & 4 != 0 { half[0] } else { -half[0] };
        let sy = if bits & 2 != 0 { half[1] } else { -half[1] };
        let sz = if bits & 1 != 0 { half[2] } else { -half[2] };
        let world = [
            pos[0] + right[0] * sx + up[0] * sy + back[0] * sz,
            pos[1] + right[1] * sx + up[1] * sy + back[1] * sz,
            pos[2] + right[2] * sx + up[2] * sy + back[2] * sz,
        ];
        let n = world_to_ndc(world, view, proj)?;
        corners[bits] = [n[0], n[1]];
    }
    Some((corners, [center[0], center[1]]))
}

/// Walk the Workspace tree and collect a 3-D box + name for every part, up to
/// a hard cap. Only on-screen parts are emitted (centre inside NDC) to keep
/// the payload and the front's draw cost bounded. BFS so shallow, visible
/// geometry is reached before deep map detail when the budget runs out.
fn collect_objects(
    sdk: &SdkI,
    workspace: u64,
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
) -> Vec<EspObject> {
    use std::collections::VecDeque;
    const MAX_OBJECTS: usize = 800;
    const MAX_NODES: usize = 8000;

    let mut out = Vec::new();
    let mut visited = 0usize;
    let mut queue: VecDeque<u64> = VecDeque::new();
    queue.push_back(workspace);

    while let Some(node) = queue.pop_front() {
        if visited >= MAX_NODES || out.len() >= MAX_OBJECTS { break; }
        for c in crate::sdk::get_children(sdk, node) {
            if visited >= MAX_NODES || out.len() >= MAX_OBJECTS { break; }
            visited += 1;

            let prim = sdk.read::<u64>(c + offsets::BasePart::Primitive as u64);
            if sdk.is_valid_ptr(prim) {
                if let Some((corners, label)) = project_object_box(sdk, prim, view, proj) {
                    // On-screen only.
                    if label[0] >= -1.0 && label[0] <= 1.0 && label[1] >= -1.0 && label[1] <= 1.0 {
                        out.push(EspObject {
                            name: crate::sdk::name_of(sdk, c),
                            box3d: corners,
                            label,
                        });
                    }
                }
            }
            queue.push_back(c);
        }
    }
    out
}

pub fn start_esp_thread(app_handle: AppHandle) {
    thread::spawn(move || {
        // Bullet Tracer (Visuals): HP tracking for aimbot locked target.
        let mut bvis_last_hp: f32 = -1.0;
        let mut bvis_last_target: u64 = 0;

        // Dev tab diagnostics — persists across ticks to detect anomalies
        // (camera teleports) between consecutive frames.
        let mut diag_prev_cam_pos: Option<[f32; 3]> = None;

        // Camera-resolve grace period: Roblox's FakeDataModel → DataModel →
        // Workspace → Camera chain can go transiently unresolvable for up to
        // a second or two (game-side reallocation, anti-cheat pointer
        // rotation, etc.) without the player actually having left. Blanking
        // the whole overlay the instant that happens makes EVERY player's
        // ESP vanish together for that whole window. Instead: while the
        // chain is down, simply stop emitting new frames (the frontend keeps
        // showing the last good one) for up to `CAMERA_GRACE`; only after
        // that do we actually clear the overlay.
        const CAMERA_GRACE: Duration = Duration::from_millis(2500);
        let mut camera_missing_since: Option<std::time::Instant> = None;

        loop {
            let state = app_handle.state::<AppState>();
            state.diag_esp_ticks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Work.ink key gate: do nothing — not even an SDK attach attempt —
            // until the frontend confirms a validated key. Emit a fully blank
            // frame so any stale overlay content clears immediately.
            if !state.key_unlocked.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = app_handle.emit("esp_frame", EspFrame {
                    markers: vec![],
                    players: vec![],
                    objects: vec![],
                    player_count: 0,
                    camera_pos: [0.0; 3],
                    forward: [0.0, 0.0, 1.0],
                    view_matrix_raw: [[0.0; 3]; 3],
                    fov: 70.0,
                    aspect: 16.0 / 9.0,
                    attached: false,
                    cursor: None,
                    bullet_vis_traces: vec![],
                    bullet_vis_hit_count: 0,
                });
                thread::sleep(Duration::from_millis(200));
                continue;
            }

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
                Some(c) => {
                    // Chain resolved again — clear the grace-period clock.
                    camera_missing_since = None;
                    c
                }
                None => {
                    let now = std::time::Instant::now();
                    let missing_since = *camera_missing_since.get_or_insert(now);
                    if now.duration_since(missing_since) < CAMERA_GRACE {
                        // Within the grace window: emit nothing and let the
                        // frontend keep showing its last received frame
                        // instead of flashing to blank. Poll fast so we
                        // recover the instant the chain comes back.
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    // Genuinely gone (game closed / place changed) — clear
                    // the overlay for real.
                    let _ = app_handle.emit("esp_frame", EspFrame {
                        markers: vec![],
                        players: vec![],
                        objects: vec![],
                        player_count: 0,
                        camera_pos: [0.0; 3],
                        forward: [0.0, 0.0, 1.0],
                        view_matrix_raw: [[0.0; 3]; 3],
                        fov: 70.0,
                        aspect: 16.0 / 9.0,
                        attached: true,
                        cursor: None,
                        bullet_vis_traces: vec![],
                        bullet_vis_hit_count: state.bullet_vis_hit_count.load(std::sync::atomic::Ordering::Relaxed),
                    });
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }
            };

            let (cam_pos, matrix) = read_camera(&sdk, camera_ptr);

            // Camera-jump detector: a torn (non-atomic) CFrame read makes
            // the position teleport for exactly one tick, then the NEXT
            // tick's clean read snaps it back — no legitimate camera pan
            // moves 200+ studs in 16 ms. Flag it so the Dev tab can show
            // whether this is actually happening (or rule it out).
            if let Some(prev) = diag_prev_cam_pos {
                let dx = cam_pos[0] - prev[0];
                let dy = cam_pos[1] - prev[1];
                let dz = cam_pos[2] - prev[2];
                let jump = (dx * dx + dy * dy + dz * dz).sqrt();
                if jump > 200.0 {
                    state.diag_esp_camera_jumps.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            diag_prev_cam_pos = Some(cam_pos);

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
            let local_name = state.local_player_name.lock().unwrap().clone();
            // Team check: same-parent characters are treated as allies. Driven
            // by the single Team check box in Aiming > Targeting (aimbot cfg).
            let team_check = state.aimbot_cfg.lock().unwrap().team_check;
            let local_parent = state.local_parent.load(std::sync::atomic::Ordering::Relaxed);
            let max_dist = visuals.max_distance;
            let min_dist = visuals.min_distance.max(0.0);

            let mut players: Vec<EspPlayer> = Vec::with_capacity(caches.len());
            for cache in caches.iter() {
                // Exclude the local player (first registered in Players) from ESP.
                if !local_name.is_empty() && cache.name == local_name { continue; }
                // Relation gate: neutrals (2) are never drawn; allies (1) get
                // only a green head dot (handled below); enemies (0) get the
                // full ESP build.
                let manual = relations.get(&cache.name).copied().unwrap_or(0);
                let relation = crate::players::effective_relation(
                    manual, team_check, cache.parent_addr, local_parent);
                if relation == 2 { continue; }

                // Fast distance cull — read ONE part (head, or the first valid
                // slot) to compute the character's distance to the camera
                // before doing anything else. If they're out of range we can
                // skip the other ~10 part reads entirely. This alone cuts the
                // SDK read rate by ~90% on populated servers where most
                // players are far outside `max_distance`.
                let anchor_pos = 'anchor: {
                    for (prim, valid) in cache.parts.iter() {
                        if !*valid { continue; }
                        let p = read_pos_fast(&sdk, *prim);
                        if !p[0].is_finite() { continue; }
                        if p[0] == 0.0 && p[1] == 0.0 && p[2] == 0.0 { continue; }
                        break 'anchor Some(p);
                    }
                    None
                };
                let anchor_pos = match anchor_pos {
                    Some(a) => a,
                    None => {
                        // Every valid part came back NaN/zero this tick — this
                        // player's ESP visibly blinks off for this one frame.
                        state.diag_esp_players_culled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                };

                let dx = anchor_pos[0] - cam_pos[0];
                let dy = anchor_pos[1] - cam_pos[1];
                let dz = anchor_pos[2] - cam_pos[2];
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                // Enemies (relation 0) beyond `max_distance` are dropped
                // before we pay for the rest of their parts. Allies (relation
                // 1) fall through — they get the range-independent green head
                // dot handled below.
                if relation == 0 && (distance < min_dist || distance > max_dist) {
                    continue;
                }

                // Now that we know they're worth drawing, read every remaining
                // valid part (skeleton + box2d bounds + centre choice).
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

                let anchor = head_world.or(center_world).unwrap_or(anchor_pos);

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
                            part_boxes: Vec::new(),
                            relation: 1,
                            world: anchor,
                        });
                    }
                    continue;
                }

                // Distance filter already applied above for enemies before the
                // parts read — no need to check again here.

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

                // Per-part boxes — a quad per valid part scaled to its real Size.
                let mut part_boxes: Vec<[[f32; 2]; 8]> = Vec::new();
                // Compute the boxes if either the fill OR the border is on, so
                // the border can be drawn even with the fill disabled.
                if visuals.part_esp || visuals.part_esp_border {
                    for (prim, valid) in cache.parts.iter() {
                        if !*valid { continue; }
                        if let Some(rect) = project_part_box(&sdk, *prim, view, proj) {
                            part_boxes.push(rect);
                        }
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
                    part_boxes,
                    relation: 0,
                    world: anchor,
                });
            }

            // Bullet Tracer: emit tracer + sound when locked target HP drops > 4.
            let bvis_on = visuals.bullet_tracer;
            if bvis_on {
                let target_addr = state.aimbot_locked_target.load(std::sync::atomic::Ordering::Relaxed);
                if target_addr != 0 {
                    // Reset baseline if target changed.
                    if target_addr != bvis_last_target {
                        bvis_last_target = target_addr;
                        bvis_last_hp = -1.0;
                    }
                    let caches = state.char_caches.lock().unwrap().clone();
                    if let Some(cache) = caches.iter().find(|c| c.char_addr == target_addr) {
                        let cur_hp = if sdk.is_valid_ptr(cache.humanoid_addr) {
                            let h = sdk.read::<f32>(cache.humanoid_addr + offsets::Humanoid::Health as u64);
                            if h.is_finite() && (0.0..=1000.0).contains(&h) { h.clamp(0.0, 100.0) } else { -1.0 }
                        } else { -1.0 };

                        if cur_hp >= 0.0 {
                            let drop = bvis_last_hp - cur_hp;
                            if bvis_last_hp >= 0.0 && drop > 4.0 {
                                if let Some(&(prim, true)) = cache.parts.first() {
                                    let po = offsets::Primitive::Position as u64;
                                    let tgt = [
                                        sdk.read::<f32>(prim + po),
                                        sdk.read::<f32>(prim + po + 4),
                                        sdk.read::<f32>(prim + po + 8),
                                    ];
                                    if tgt[0].is_finite() {
                                        let mut traces = state.bullet_vis_traces.lock().unwrap();
                                        traces.push((std::time::Instant::now(), [cam_pos, tgt]));
                                        let over = traces.len().saturating_sub(50);
                                        if over > 0 { traces.drain(0..over); }
                                        state.bullet_vis_hit_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                            }
                            bvis_last_hp = cur_hp;
                        }
                    }
                } else {
                    bvis_last_target = 0;
                    bvis_last_hp = -1.0;
                }
            } else {
                bvis_last_target = 0;
                bvis_last_hp = -1.0;
                state.bullet_vis_traces.lock().unwrap().clear();
            }

            const TRACER_LIFETIME: f64 = 4.0;
            let bullet_vis_ndc: Vec<[f32; 6]> = if bvis_on {
                let mut traces = state.bullet_vis_traces.lock().unwrap();
                traces.retain(|(t, _)| t.elapsed().as_secs_f64() < TRACER_LIFETIME);
                traces.iter().filter_map(|(t, [orig, tgt])| {
                    let age = (t.elapsed().as_secs_f64() / TRACER_LIFETIME) as f32;
                    let nb = world_to_ndc(*tgt, view, proj)?;
                    let (ox, oy, screen_origin) = match world_to_ndc(*orig, view, proj) {
                        Some(na) => (na[0], na[1], 0.0f32),
                        None     => (0.0, -1.0, 1.0f32),
                    };
                    Some([ox, oy, nb[0], nb[1], age, screen_origin])
                }).collect()
            } else {
                Vec::new()
            };

            let cursor_local = state.roblox_bounds.lock().unwrap()
                .and_then(|(bx, by, _, _)| current_cursor().map(|c| (c.x - bx, c.y - by)));

            // Camera look direction (world). Roblox CFrame stores -lookVector
            // in the third column of the rotation matrix; negate to get the
            // forward the radar orients "up" to.
            let forward = [-matrix[0][2], -matrix[1][2], -matrix[2][2]];

            // Dev "ESP all game objects": walk the Workspace and box every part.
            let objects = if state.object_esp_enabled.load(std::sync::atomic::Ordering::Relaxed) {
                match security::resolve_workspace(&sdk) {
                    Some(ws) => collect_objects(&sdk, ws, view, proj),
                    None => Vec::new(),
                }
            } else {
                Vec::new()
            };

            let _ = app_handle.emit("esp_frame", EspFrame {
                markers: vec![origin],
                player_count: players.len(),
                players,
                objects,
                camera_pos: cam_pos,
                forward,
                view_matrix_raw: matrix,
                fov,
                aspect,
                attached: true,
                cursor: cursor_local,
                bullet_vis_traces: bullet_vis_ndc,
                bullet_vis_hit_count: state.bullet_vis_hit_count.load(std::sync::atomic::Ordering::Relaxed),
            });

            thread::sleep(Duration::from_millis(16));
        }
    });
}
