import { writable, type Writable } from "svelte/store";

export type TabId =
    | "Aiming"
    | "Visuals"
    | "Output"
    | "Options"
    | "Player List"
    | "Explorer"
    | "Servers"
    | "Settings";

export const TABS: TabId[] = [
    "Aiming",
    "Visuals",
    "Output",
    "Options",
    "Player List",
    "Explorer",
    "Servers",
    "Settings",
];

// Filled by the global poller in Menu.svelte (so the check runs regardless
// of which tab is active). Settings.svelte just displays them.
export const localVersion: Writable<string> = writable("…");
export const remoteVersion: Writable<string> = writable("…");
export const lastVersionCheck: Writable<number | null> = writable(null);

// Flipped to `true` by the global poller once the remote `socrate_version`
// differs from the compile-time `VERSION`. TabBar reads this to render a
// small red "i" badge on the Settings tab — visible from any other tab.
export const updateAvailable: Writable<boolean> = writable(false);

export const activeTab: Writable<TabId> = writable("Aiming");

// Cross-tab channel: hex address (e.g. "0x1234") the Explorer should reveal —
// expand the path to it, scroll it into view, and flash it. Set by the Player
// List's "View in explorer" button (alongside flipping `activeTab` to
// "Explorer"); the Explorer consumes and clears it once the tree has loaded.
export const explorerReveal: Writable<string | null> = writable(null);

// Theme accent — persisted in localStorage. Subscribe in Menu.svelte to
// reflect the colour into `--accent` etc. on the menu element.
const THEME_KEY = "socrate.theme";
const initialTheme = (typeof localStorage !== "undefined" && localStorage.getItem(THEME_KEY)) || "#ffde9d";
export const theme: Writable<string> = writable(initialTheme);
theme.subscribe((c) => {
    if (typeof localStorage !== "undefined") localStorage.setItem(THEME_KEY, c);
});

export const THEME_PRESETS: { name: string; value: string }[] = [
    { name: "Cream",  value: "#ffde9d" },
    { name: "Pink",   value: "#ffb3d9" },
    { name: "Red",    value: "#ff9a9e" },
    { name: "Blue",   value: "#a4d8ff" },
    { name: "Green",  value: "#b8e8c8" },
    { name: "Purple", value: "#d0a8ff" },
];

export type SelectionMode = "Distance" | "Health" | "Crosshair";
export type AimType = "Mouse" | "Camera";

export type AimbotCfg = {
    enabled: boolean;
    keybind: string;
    aim_type: AimType;
    smoothing: number;
    fov_radius: number;
    show_fov: boolean;
    fill_fov: boolean;
    fov_color: string;
    fov_fill_color: string;
    part: "Head" | "Torso" | "HumanoidRootPart";

    prediction_enabled: boolean;
    prediction_strength: number;
    velocity_prediction: boolean;
    bullet_drop_comp: boolean;
    gravity_comp: number;

    selection_mode: SelectionMode;
    wall_check: boolean;
    min_distance: number;
    max_distance: number;
    team_check: boolean;

    trigger_enabled: boolean;
    trigger_delay: number;
    trigger_keybind: string;
    trigger_burst: boolean;

    legit_enabled: boolean;
    legit_miss: number;
    humanize: boolean;
    smooth_path: boolean;
};

export const aimbot: Writable<AimbotCfg> = writable({
    enabled: false,
    keybind: "MouseRight",
    aim_type: "Mouse",
    smoothing: 0.85,
    fov_radius: 150,
    show_fov: false,
    fill_fov: false,
    fov_color: "#ffde9d",
    fov_fill_color: "#ffde9d",
    part: "Head",

    prediction_enabled: false,
    prediction_strength: 1.0,
    velocity_prediction: true,
    bullet_drop_comp: false,
    gravity_comp: 0,

    selection_mode: "Distance",
    wall_check: false,
    min_distance: 0,
    max_distance: 400,
    team_check: true,

    trigger_enabled: false,
    trigger_delay: 50,
    trigger_keybind: "MouseLeft",
    trigger_burst: false,

    legit_enabled: false,
    legit_miss: 0.3,
    humanize: false,
    smooth_path: false,
});

export type TracerOrigin = "Top" | "Center" | "Bottom";
export type LabelAnchorX = "L" | "C" | "R";
export type LabelAnchorY = "T" | "C" | "B";

export type VisualsCfg = {
    // Global
    outline: boolean;

    // Skeleton
    skeleton: boolean;
    skeleton_color: string;
    skeleton_thickness: number;

    // Box 2D
    box2d: boolean;
    box2d_color: string;
    box2d_fill: boolean;
    box2d_fill_color: string;
    box2d_thickness: number;

    // Box 3D
    box3d: boolean;
    box3d_color: string;
    box3d_fill: boolean;
    box3d_fill_color: string;
    box3d_thickness: number;

    // Tracers
    tracers: boolean;
    tracers_color: string;
    tracers_origin: TracerOrigin;
    tracers_thickness: number;

    // View direction
    view_dir: boolean;
    view_dir_color: string;
    view_dir_length: number;   // studs

    // Text — anchored to a 2-D box edge so labels stay box-relative
    // (e.g. "below the box") regardless of player distance. Anchor picks
    // the edge (L/C/R for X, T/C/B for Y); offset is pixel delta from it.
    names: boolean;
    names_color: string;
    names_anchor_x: LabelAnchorX;
    names_anchor_y: LabelAnchorY;
    names_offset_x: number;
    names_offset_y: number;
    distance: boolean;
    distance_color: string;
    distance_anchor_x: LabelAnchorX;
    distance_anchor_y: LabelAnchorY;
    distance_offset_x: number;
    distance_offset_y: number;
    health: boolean;
    health_color: string;
    health_anchor_x: LabelAnchorX;
    health_anchor_y: LabelAnchorY;
    health_offset_x: number;
    health_offset_y: number;
    health_bar: boolean;
    health_bar_color: string;

    // Avatar head — circular thumbnail anchored to the 2-D box just like
    // text labels. `size` is a fixed pixel diameter so the head stays the
    // same on-screen size regardless of how far the player is.
    avatar_head: boolean;
    avatar_head_size: number;
    avatar_head_border_color: string;
    avatar_head_border_thickness: number;
    avatar_head_anchor_x: LabelAnchorX;
    avatar_head_anchor_y: LabelAnchorY;
    avatar_head_offset_x: number;
    avatar_head_offset_y: number;

    // Filters
    team_check: boolean;
    show_self: boolean;
    max_distance: number;
    /// Skip any character whose distance from the camera is below this —
    /// a belt+braces self-filter on top of the LocalPlayer.Name match so
    /// first-person games (camera inside the head) never ESP yourself.
    min_distance: number;
};

export const visuals: Writable<VisualsCfg> = writable({
    outline: true,

    skeleton: false,
    skeleton_color: "#ffffff",
    skeleton_thickness: 1.5,

    box2d: true,
    box2d_color: "#ffffff",
    box2d_fill: true,
    box2d_fill_color: "#ffffff",
    box2d_thickness: 1.5,

    box3d: false,
    box3d_color: "#ffffff",
    box3d_fill: false,
    box3d_fill_color: "#ffffff",
    box3d_thickness: 1.5,

    tracers: false,
    tracers_color: "#ffffff",
    tracers_origin: "Bottom",
    tracers_thickness: 1.5,

    view_dir: false,
    view_dir_color: "#ffffff",
    view_dir_length: 6,

    names: false,
    names_color: "#ffffff",
    names_anchor_x: "C",
    names_anchor_y: "T",
    names_offset_x: 0,
    names_offset_y: -4,
    distance: false,
    distance_color: "#ffffff",
    distance_anchor_x: "C",
    distance_anchor_y: "B",
    distance_offset_x: 0,
    distance_offset_y: 4,
    health: false,
    health_color: "#ffffff",
    health_anchor_x: "C",
    health_anchor_y: "B",
    health_offset_x: 0,
    health_offset_y: 16,
    health_bar: false,
    health_bar_color: "#ffffff",

    avatar_head: false,
    avatar_head_size: 28,
    avatar_head_border_color: "#ffde9d",
    avatar_head_border_thickness: 1.5,
    avatar_head_anchor_x: "C",
    avatar_head_anchor_y: "T",
    avatar_head_offset_x: 0,
    avatar_head_offset_y: -18,

    team_check: true,
    show_self: false,
    max_distance: 1500,
    min_distance: 3,
});

// Menu UI options — purely frontend, not synced to backend. `menu_visible`
// is flipped by a global keybind; while visible, the overlay dims the game
// and rains theme-coloured snow flakes for fun.
export type MenuOptionsCfg = {
    menu_visible: boolean;
    toggle_bind: string;
    snow_enabled: boolean;
};
export const menuOptions: Writable<MenuOptionsCfg> = writable({
    menu_visible: true,
    toggle_bind: "Home",
    snow_enabled: true,
});
