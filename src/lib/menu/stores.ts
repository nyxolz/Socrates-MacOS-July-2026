import { writable, type Writable } from "svelte/store";

export type TabId =
    | "Aiming"
    | "Visuals"
    | "Output"
    | "Options"
    | "Player List"
    | "Servers";

export const TABS: TabId[] = [
    "Aiming",
    "Visuals",
    "Output",
    "Options",
    "Player List",
    "Servers",
];

export const activeTab: Writable<TabId> = writable("Aiming");

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

    // Text — offsets are pixel deltas from the head anchor; dragging in
    // the Preview rewrites these values live.
    names: boolean;
    names_color: string;
    names_offset_x: number;
    names_offset_y: number;
    distance: boolean;
    distance_color: string;
    distance_offset_x: number;
    distance_offset_y: number;
    health: boolean;
    health_color: string;
    health_offset_x: number;
    health_offset_y: number;
    health_bar: boolean;
    health_bar_color: string;

    // Filters
    team_check: boolean;
    show_self: boolean;
    max_distance: number;
};

export const visuals: Writable<VisualsCfg> = writable({
    outline: true,

    skeleton: true,
    skeleton_color: "#ffffff",
    skeleton_thickness: 1.5,

    box2d: false,
    box2d_color: "#ffffff",
    box2d_fill: false,
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
    names_offset_x: 0,
    names_offset_y: -14,
    distance: false,
    distance_color: "#ffffff",
    distance_offset_x: 0,
    distance_offset_y: 22,
    health: false,
    health_color: "#ffffff",
    health_offset_x: 0,
    health_offset_y: 34,
    health_bar: false,
    health_bar_color: "#ffffff",

    team_check: true,
    show_self: false,
    max_distance: 1500,
});
