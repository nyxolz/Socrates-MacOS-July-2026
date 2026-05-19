import { writable, type Writable } from "svelte/store";

export type TabId =
    | "Aiming"
    | "Visuals"
    | "Misc"
    | "Output"
    | "Options"
    | "Script View"
    | "Player List"
    | "Addons"
    | "Servers";

export const TABS: TabId[] = [
    "Aiming",
    "Visuals",
    "Misc",
    "Output",
    "Options",
    "Script View",
    "Player List",
    "Addons",
    "Servers",
];

export const activeTab: Writable<TabId> = writable("Aiming");

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

    // Text
    names: boolean;
    names_color: string;
    distance: boolean;
    distance_color: string;
    health: boolean;
    health_color: string;
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
    distance: false,
    distance_color: "#ffffff",
    health: false,
    health_color: "#ffffff",
    health_bar: false,
    health_bar_color: "#ffffff",

    team_check: true,
    show_self: false,
    max_distance: 1500,
});
