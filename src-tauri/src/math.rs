//! Projection math: build view + projection matrices and project a world
//! point down to NDC. The pipeline stays in this file so the rest of the
//! crate only deals with `[f32; 3]` → `Option<[f32; 2]>`.
//!
//! See `ESP_PIPELINE.md` in the Cherry repo for the derivation; this is the
//! same code, kept here for parity.

pub fn build_view_matrix(cam_pos: [f32; 3], cam_rot_3x3: [[f32; 3]; 3]) -> [[f32; 4]; 4] {
    let r00 = cam_rot_3x3[0][0]; let r01 = cam_rot_3x3[1][0]; let r02 = cam_rot_3x3[2][0];
    let r10 = cam_rot_3x3[0][1]; let r11 = cam_rot_3x3[1][1]; let r12 = cam_rot_3x3[2][1];
    let r20 = cam_rot_3x3[0][2]; let r21 = cam_rot_3x3[1][2]; let r22 = cam_rot_3x3[2][2];
    let tx = -(r00 * cam_pos[0] + r01 * cam_pos[1] + r02 * cam_pos[2]);
    let ty = -(r10 * cam_pos[0] + r11 * cam_pos[1] + r12 * cam_pos[2]);
    let tz = -(r20 * cam_pos[0] + r21 * cam_pos[1] + r22 * cam_pos[2]);
    [
        [r00, r01, r02, tx],
        [r10, r11, r12, ty],
        [r20, r21, r22, tz],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn build_projection_matrix(fov_deg: f32, aspect_ratio: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let fov_rad = fov_deg.to_radians();
    let f = 1.0 / (fov_rad / 2.0).tan();
    [
        [f / aspect_ratio, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, (far + near) / (near - far), (2.0 * far * near) / (near - far)],
        [0.0, 0.0, -1.0, 0.0],
    ]
}

/// World point → Normalized Device Coordinates in `[-1, +1]`. Returns `None`
/// when the point is behind the near plane (the Svelte side then knows it
/// must skip this marker or hand it to the off-screen radar).
pub fn world_to_ndc(
    world_pos: [f32; 3],
    view_matrix: [[f32; 4]; 4],
    proj_matrix: [[f32; 4]; 4],
) -> Option<[f32; 2]> {
    let view_x = view_matrix[0][0] * world_pos[0] + view_matrix[0][1] * world_pos[1] + view_matrix[0][2] * world_pos[2] + view_matrix[0][3];
    let view_y = view_matrix[1][0] * world_pos[0] + view_matrix[1][1] * world_pos[1] + view_matrix[1][2] * world_pos[2] + view_matrix[1][3];
    let view_z = view_matrix[2][0] * world_pos[0] + view_matrix[2][1] * world_pos[1] + view_matrix[2][2] * world_pos[2] + view_matrix[2][3];
    let view_w = view_matrix[3][0] * world_pos[0] + view_matrix[3][1] * world_pos[1] + view_matrix[3][2] * world_pos[2] + view_matrix[3][3];

    let clip_x = proj_matrix[0][0] * view_x + proj_matrix[0][1] * view_y + proj_matrix[0][2] * view_z + proj_matrix[0][3] * view_w;
    let clip_y = proj_matrix[1][0] * view_x + proj_matrix[1][1] * view_y + proj_matrix[1][2] * view_z + proj_matrix[1][3] * view_w;
    let clip_w = proj_matrix[3][0] * view_x + proj_matrix[3][1] * view_y + proj_matrix[3][2] * view_z + proj_matrix[3][3] * view_w;

    if clip_w < 0.1 { return None; }
    Some([clip_x / clip_w, clip_y / clip_w])
}

pub fn world_to_screen(
    world_pos: [f32; 3],
    view_matrix: [[f32; 4]; 4],
    proj_matrix: [[f32; 4]; 4],
    width: f32,
    height: f32,
) -> Option<[f32; 2]> {
    let ndc = world_to_ndc(world_pos, view_matrix, proj_matrix)?;
    let screen_x = (ndc[0] + 1.0) * 0.5 * width;
    let screen_y = (1.0 - ndc[1]) * 0.5 * height;
    Some([screen_x, screen_y])
}

/// For points off-screen / behind the camera. Returns the NDC direction the
/// off-screen indicator should point in. `None` means the point is on screen.
pub fn world_to_offscreen_dir(
    world_pos: [f32; 3],
    view_matrix: [[f32; 4]; 4],
    proj_matrix: [[f32; 4]; 4],
) -> Option<(f32, f32)> {
    let view_x = view_matrix[0][0]*world_pos[0] + view_matrix[0][1]*world_pos[1] + view_matrix[0][2]*world_pos[2] + view_matrix[0][3];
    let view_y = view_matrix[1][0]*world_pos[0] + view_matrix[1][1]*world_pos[1] + view_matrix[1][2]*world_pos[2] + view_matrix[1][3];
    let view_z = view_matrix[2][0]*world_pos[0] + view_matrix[2][1]*world_pos[1] + view_matrix[2][2]*world_pos[2] + view_matrix[2][3];
    let view_w = view_matrix[3][0]*world_pos[0] + view_matrix[3][1]*world_pos[1] + view_matrix[3][2]*world_pos[2] + view_matrix[3][3];

    let clip_x = proj_matrix[0][0]*view_x + proj_matrix[0][1]*view_y + proj_matrix[0][2]*view_z + proj_matrix[0][3]*view_w;
    let clip_y = proj_matrix[1][0]*view_x + proj_matrix[1][1]*view_y + proj_matrix[1][2]*view_z + proj_matrix[1][3]*view_w;
    let clip_w = proj_matrix[3][0]*view_x + proj_matrix[3][1]*view_y + proj_matrix[3][2]*view_z + proj_matrix[3][3]*view_w;

    let abs_w = clip_w.abs().max(0.001);
    let nx = clip_x / abs_w;
    let ny = clip_y / abs_w;
    let in_front = clip_w > 0.0;
    let on_screen = in_front && nx >= -1.0 && nx <= 1.0 && ny >= -1.0 && ny <= 1.0;
    if on_screen { return None; }
    if in_front { Some((nx, ny)) } else { Some((-nx, -ny)) }
}

/// Project a 3D segment, clipping at the near plane so a line with one
/// endpoint behind the camera doesn't explode to infinity.
pub fn segment_to_ndc(
    a: [f32; 3],
    b: [f32; 3],
    view_matrix: [[f32; 4]; 4],
    proj_matrix: [[f32; 4]; 4],
) -> Option<([f32; 2], [f32; 2])> {
    fn to_clip(p: [f32; 3], v: [[f32; 4]; 4], pr: [[f32; 4]; 4]) -> [f32; 4] {
        let vx = v[0][0]*p[0] + v[0][1]*p[1] + v[0][2]*p[2] + v[0][3];
        let vy = v[1][0]*p[0] + v[1][1]*p[1] + v[1][2]*p[2] + v[1][3];
        let vz = v[2][0]*p[0] + v[2][1]*p[1] + v[2][2]*p[2] + v[2][3];
        let vw = v[3][0]*p[0] + v[3][1]*p[1] + v[3][2]*p[2] + v[3][3];
        [
            pr[0][0]*vx + pr[0][1]*vy + pr[0][2]*vz + pr[0][3]*vw,
            pr[1][0]*vx + pr[1][1]*vy + pr[1][2]*vz + pr[1][3]*vw,
            pr[2][0]*vx + pr[2][1]*vy + pr[2][2]*vz + pr[2][3]*vw,
            pr[3][0]*vx + pr[3][1]*vy + pr[3][2]*vz + pr[3][3]*vw,
        ]
    }

    const NEAR_W: f32 = 0.1;
    let mut ca = to_clip(a, view_matrix, proj_matrix);
    let mut cb = to_clip(b, view_matrix, proj_matrix);

    if ca[3] < NEAR_W && cb[3] < NEAR_W { return None; }

    if ca[3] < NEAR_W {
        let t = (NEAR_W - ca[3]) / (cb[3] - ca[3]);
        ca = [
            ca[0] + (cb[0] - ca[0]) * t,
            ca[1] + (cb[1] - ca[1]) * t,
            ca[2] + (cb[2] - ca[2]) * t,
            NEAR_W,
        ];
    }
    if cb[3] < NEAR_W {
        let t = (NEAR_W - cb[3]) / (ca[3] - cb[3]);
        cb = [
            cb[0] + (ca[0] - cb[0]) * t,
            cb[1] + (ca[1] - cb[1]) * t,
            cb[2] + (ca[2] - cb[2]) * t,
            NEAR_W,
        ];
    }

    Some(([ca[0]/ca[3], ca[1]/ca[3]], [cb[0]/cb[3], cb[1]/cb[3]]))
}
