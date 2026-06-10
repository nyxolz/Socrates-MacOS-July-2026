//! Application state — everything the background threads and Tauri commands
//! share. One `AppState` instance is managed by Tauri (`app.manage(...)`).
//!
//! Keep this module dumb on purpose: just mutexes and atomics, no behavior.
//! Behavior lives in the modules that mutate this state (overlay::tracker,
//! commands, future esp / aimbot loops).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::overlay::InteractiveRect;
use crate::players::CharCache;
use crate::sdk::SdkI;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AimbotCfg {
    // Core
    pub enabled: bool,
    pub keybind: String,
    /// "Hold" = active while the key is held; "Toggle" = press to enable, press
    /// again to disable.
    pub keybind_mode: String,
    pub aim_type: String,
    pub smoothing: f32,
    /// Silent-aim-only: probability [0,1] that a given shot is deliberately
    /// shoved off-target so the bullet whiffs on purpose.
    pub silent_miss_rate: f32,
    pub fov_radius: f32,
    pub show_fov: bool,
    pub fill_fov: bool,
    pub fov_color: String,
    pub fov_fill_color: String,
    pub part: String,

    // Prediction
    pub prediction_enabled: bool,
    pub prediction_strength: f32,
    pub velocity_prediction: bool,
    pub bullet_drop_comp: bool,
    pub gravity_comp: f32,

    // Targeting filters
    pub selection_mode: String,
    pub wall_check: bool,
    pub min_distance: f32,
    pub max_distance: f32,
    pub team_check: bool,

    // Trigger bot
    pub trigger_enabled: bool,
    pub trigger_delay: f32,
    pub trigger_keybind: String,
    pub trigger_burst: bool,

    // Legit
    pub legit_enabled: bool,
    pub legit_miss: f32,
    pub humanize: bool,
    pub smooth_path: bool,
    /// Jitter amplitude (px) added by the humanizer — smoothed wander, not white noise.
    pub humanize_strength: f32,
    /// Delay (ms) between pressing the aim key and the aimbot engaging — human reaction time.
    pub reaction_time: f32,
    /// Aim Assist: hard-lock onto the target picked when the aimbot engages and
    /// keep it for the whole activation (never switches to another player).
    pub sticky_aim: bool,
    /// Minimum time (ms) a target must be held before the aimbot may switch to another.
    pub target_switch_delay: f32,
    /// Scale smoothing by the target's distance inside the FOV: snappier near the
    /// crosshair, softer at the edge — mimics a human flick-then-track.
    pub fov_scale_smoothing: bool,
    /// Cap on per-tick cursor movement (px) in Mouse mode. 0 = uncapped.
    pub max_speed: f32,

}

impl Default for AimbotCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            keybind: "MouseRight".into(),
            keybind_mode: "Hold".into(),
            aim_type: "Camera".into(),
            smoothing: 0.85,
            silent_miss_rate: 0.0,
            fov_radius: 150.0,
            show_fov: false,
            fill_fov: false,
            fov_color: "#ffde9d".into(),
            fov_fill_color: "#ffde9d".into(),
            part: "Head".into(),

            prediction_enabled: false,
            prediction_strength: 1.0,
            velocity_prediction: true,
            bullet_drop_comp: false,
            gravity_comp: 0.0,

            selection_mode: "Crosshair".into(),
            wall_check: false,
            min_distance: 0.0,
            max_distance: 400.0,
            team_check: false,

            trigger_enabled: false,
            trigger_delay: 50.0,
            trigger_keybind: "MouseLeft".into(),
            trigger_burst: false,

            legit_enabled: false,
            legit_miss: 0.3,
            humanize: false,
            smooth_path: false,
            humanize_strength: 2.0,
            reaction_time: 0.0,
            sticky_aim: true,
            target_switch_delay: 0.0,
            fov_scale_smoothing: false,
            max_speed: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct VisualsCfg {
    pub outline: bool,

    pub skeleton: bool,
    pub skeleton_color: String,
    pub skeleton_thickness: f32,

    pub box2d: bool,
    pub box2d_color: String,
    pub box2d_fill: bool,
    pub box2d_fill_color: String,
    pub box2d_thickness: f32,

    pub box3d: bool,
    pub box3d_color: String,
    pub box3d_fill: bool,
    pub box3d_fill_color: String,
    pub box3d_thickness: f32,

    pub tracers: bool,
    pub tracers_color: String,
    pub tracers_origin: String,
    pub tracers_thickness: f32,

    pub view_dir: bool,
    pub view_dir_color: String,
    pub view_dir_length: f32,

    pub names: bool,
    pub names_color: String,
    pub names_anchor_x: String,
    pub names_anchor_y: String,
    pub names_offset_x: f32,
    pub names_offset_y: f32,
    pub distance: bool,
    pub distance_color: String,
    pub distance_anchor_x: String,
    pub distance_anchor_y: String,
    pub distance_offset_x: f32,
    pub distance_offset_y: f32,
    pub health: bool,
    pub health_color: String,
    pub health_anchor_x: String,
    pub health_anchor_y: String,
    pub health_offset_x: f32,
    pub health_offset_y: f32,
    pub health_bar: bool,
    pub health_bar_color: String,

    pub team_check: bool,
    pub show_self: bool,
    pub max_distance: f32,
    pub min_distance: f32,
}

impl Default for VisualsCfg {
    fn default() -> Self {
        Self {
            outline: true,
            skeleton: false,
            skeleton_color: "#ffffff".into(),
            skeleton_thickness: 1.5,
            box2d: true,
            box2d_color: "#ffffff".into(),
            box2d_fill: false,
            box2d_fill_color: "#ffffff".into(),
            box2d_thickness: 1.5,
            box3d: false,
            box3d_color: "#ffffff".into(),
            box3d_fill: false,
            box3d_fill_color: "#ffffff".into(),
            box3d_thickness: 1.5,
            tracers: false,
            tracers_color: "#ffffff".into(),
            tracers_origin: "Bottom".into(),
            tracers_thickness: 1.5,
            view_dir: false,
            view_dir_color: "#ffffff".into(),
            view_dir_length: 6.0,
            names: false,
            names_color: "#ffffff".into(),
            names_anchor_x: "C".into(),
            names_anchor_y: "T".into(),
            names_offset_x: 0.0,
            names_offset_y: -4.0,
            distance: false,
            distance_color: "#ffffff".into(),
            distance_anchor_x: "C".into(),
            distance_anchor_y: "B".into(),
            distance_offset_x: 0.0,
            distance_offset_y: 4.0,
            health: false,
            health_color: "#ffffff".into(),
            health_anchor_x: "C".into(),
            health_anchor_y: "B".into(),
            health_offset_x: 0.0,
            health_offset_y: 16.0,
            health_bar: false,
            health_bar_color: "#ffffff".into(),
            team_check: false,
            show_self: false,
            max_distance: 1500.0,
            min_distance: 3.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct MiscCfg {
    pub walk_speed_enabled: bool,
    pub walk_speed: f32,
    pub hip_height_enabled: bool,
    pub hip_height: f32,
    pub display_distance_enabled: bool,
    pub display_distance: f32,
    pub max_slope_enabled: bool,
    pub max_slope: f32,
}

impl Default for MiscCfg {
    fn default() -> Self {
        Self {
            walk_speed_enabled: false,
            walk_speed: 16.0,
            hip_height_enabled: false,
            hip_height: 2.0,
            display_distance_enabled: false,
            display_distance: 100.0,
            max_slope_enabled: false,
            max_slope: 89.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RobloxBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZoneInput {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Pure-frontend menu UI options that the backend only needs to *read* (so
/// it can poll the global hotkey that toggles menu visibility).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct MenuOptionsCfg {
    pub menu_visible: bool,
    pub toggle_bind: String,
    pub snow_enabled: bool,
}

pub struct AppState {
    /// `Some((x, y, w, h))` when Roblox is currently visible on screen.
    pub roblox_bounds: Mutex<Option<(f64, f64, f64, f64)>>,

    /// Set once when Roblox is first detected; never reset. Used by the
    /// click-through logic to tell "not launched yet" from "minimized".
    pub roblox_ever_seen: AtomicBool,

    /// Window-local pixel rects published by the Svelte UI. Mouse tracker
    /// flips click-through when the cursor crosses one of these.
    pub interactive_zones: Mutex<Vec<InteractiveRect>>,

    /// Lazy: filled when `attach_sdk` succeeds.
    pub sdk: Mutex<Option<Arc<SdkI>>>,

    /// Built off-thread by `players::start_char_cache_thread`. The ESP loop
    /// only clones this Arc each frame (atomic refcount bump).
    pub char_caches: Mutex<Arc<Vec<CharCache>>>,

    pub aimbot_cfg: Mutex<AimbotCfg>,
    pub visuals_cfg: Mutex<VisualsCfg>,
    pub menu_toggle_bind: Mutex<String>,

    /// LocalPlayer.Name — resolved every cache rebuild from
    /// `DataModel → Players → first child`. Used to filter self out of the
    /// ESP / aimbot / player-list pipelines. Empty string until resolved.
    pub local_player_name: Mutex<String>,

    /// Parent container (instance addr) that holds the LocalPlayer's character,
    /// resolved on each full character walk. Used by Team check: any character
    /// sharing this parent is treated as a teammate. 0 until resolved.
    pub local_parent: AtomicU64,

    /// Humanoid instance addr of the LocalPlayer's character, resolved on
    /// each full character walk. Used by the Misc tab to write WalkSpeed,
    /// HipHeight, DisplayDistance and MaxSlope to our own character. 0 until
    /// resolved or while dead/respawning.
    pub local_humanoid_addr: AtomicU64,

    pub misc_cfg: Mutex<MiscCfg>,

    /// Char_addr of the player currently being spectated. 0 = off.
    pub spectate_target: Mutex<u64>,

    /// Per-player relation, keyed by player name (stable across respawns,
    /// unlike char_addr). `0` = enemy (default — any name absent from the map
    /// is treated as enemy), `1` = ally, `2` = neutral. Enemies get aimbot +
    /// full ESP; allies get only a green head dot; neutrals are ignored.
    pub player_relations: Mutex<HashMap<String, u8>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            roblox_bounds: Mutex::new(None),
            roblox_ever_seen: AtomicBool::new(false),
            interactive_zones: Mutex::new(Vec::new()),
            sdk: Mutex::new(None),
            char_caches: Mutex::new(Arc::new(Vec::new())),
            aimbot_cfg: Mutex::new(AimbotCfg::default()),
            visuals_cfg: Mutex::new(VisualsCfg::default()),
            menu_toggle_bind: Mutex::new("IntlBackslash".into()),
            local_player_name: Mutex::new(String::new()),
            local_parent: AtomicU64::new(0),
            local_humanoid_addr: AtomicU64::new(0),
            misc_cfg: Mutex::new(MiscCfg::default()),
            spectate_target: Mutex::new(0),
            player_relations: Mutex::new(HashMap::new()),
        }
    }
}
