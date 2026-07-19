import { writable, get as getStore, type Writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type TabId =
    | "Aiming"
    | "Misc"
    | "Visuals"
    | "Player List"
    | "Explorer"
    | "Configs"
    | "Servers"
    | "Dev"
    | "Settings";

// Code-gated developer tab. Flip to `true` to expose the "Dev" tab in the
// sidebar; ship with `false`. Nothing else references this — toggling it here
// is the single switch to enable/disable the tab.
export const DEV_TAB_ENABLED = false;

export const TABS: TabId[] = [
    "Aiming",
    "Misc",
    "Visuals",
    "Player List",
    "Explorer",
    "Configs",
    "Servers",
    ...(DEV_TAB_ENABLED ? (["Dev"] as TabId[]) : []),
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

// Floating ESP preview window — toggled from the Visuals tab header.
export const previewWindowOpen: Writable<boolean> = writable(false);

// Cross-tab channel: hex address (e.g. "0x1234") the Explorer should reveal —
// expand the path to it, scroll it into view, and flash it. Set by the Player
// List's "View in explorer" button (alongside flipping `activeTab` to
// "Explorer"); the Explorer consumes and clears it once the tree has loaded.
export const explorerReveal: Writable<string | null> = writable(null);

// Dev tab live debug — full last esp_frame payload (camera matrix, players,
// health, etc). Set every frame from +page.svelte's esp_frame listener so
// the Dev tab can show exactly what the backend currently reads from memory.
export type DebugEspFrame = {
    camera_pos: [number, number, number];
    forward: [number, number, number];
    view_matrix_raw: number[][];
    fov: number;
    aspect: number;
    attached: boolean;
    player_count: number;
    players: {
        name: string;
        distance: number;
        health: number;
        relation: number;
        world: [number, number, number];
    }[];
};
export const debugEspFrame: Writable<DebugEspFrame | null> = writable(null);

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

// ─── localStorage-backed config stores ─────────────────────────────
// Aimbot / Visuals / MenuOptions all persist to localStorage so a fresh
// Socrate launch restores the user's last setup. The Configs tab reuses
// the same JSON shape for import/export.
const AIMBOT_KEY = "socrate.aimbot";
const VISUALS_KEY = "socrate.visuals";
const MENU_OPTS_KEY = "socrate.menuOptions";

// Shallow-merge with defaults so newly-introduced fields in a Socrate
// update fill in cleanly rather than appearing as `undefined`.
function loadStored<T>(key: string, defaults: T): T {
    if (typeof localStorage === "undefined") return defaults;
    const raw = localStorage.getItem(key);
    if (!raw) return defaults;
    try {
        const parsed = JSON.parse(raw);
        return { ...defaults, ...parsed } as T;
    } catch { return defaults; }
}

function persist<T>(key: string, store: Writable<T>) {
    store.subscribe((v) => {
        if (typeof localStorage !== "undefined") {
            localStorage.setItem(key, JSON.stringify(v));
        }
    });
}

export type SelectionMode = "Distance" | "Health" | "Crosshair";
export type AimType = "Mouse" | "Camera" | "Silent Aim";

export type AimbotCfg = {
    enabled: boolean;
    keybind: string;
    keybind_mode: "Hold" | "Toggle";
    aim_type: AimType;
    smoothing: number;
    silent_miss_rate: number;
    fov_radius: number;
    show_fov: boolean;
    fill_fov: boolean;
    fov_color: string;
    fov_fill_color: string;
    part: "Head" | "Torso" | "HumanoidRootPart";

    prediction_enabled: boolean;
    prediction_strength: number;
    prediction_y: number;
    velocity_prediction: boolean;
    projectile_speed: number;
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
    trigger_aim: boolean;

    legit_enabled: boolean;
    legit_miss: number;
    humanize: boolean;
    smooth_path: boolean;
    humanize_strength: number;
    reaction_time: number;
    sticky_aim: boolean;
    target_switch_delay: number;
    fov_scale_smoothing: boolean;
    max_speed: number;
};

export const AIMBOT_DEFAULTS: AimbotCfg = {
    enabled: false,
    keybind: "MouseRight",
    keybind_mode: "Hold",
    aim_type: "Camera",
    smoothing: 0,
    silent_miss_rate: 0.0,
    fov_radius: 150,
    show_fov: false,
    fill_fov: false,
    fov_color: "#ffde9d",
    fov_fill_color: "#ffde9d",
    part: "Head",

    prediction_enabled: false,
    prediction_strength: 1.0,
    prediction_y: 1.0,
    velocity_prediction: true,
    projectile_speed: 300,
    bullet_drop_comp: false,
    gravity_comp: 196.2,

    selection_mode: "Crosshair",
    wall_check: false,
    min_distance: 0,
    max_distance: 400,
    team_check: false,

    trigger_enabled: false,
    trigger_delay: 50,
    trigger_keybind: "MouseLeft",
    trigger_burst: false,
    trigger_aim: false,

    legit_enabled: false,
    legit_miss: 0.3,
    humanize: false,
    smooth_path: false,
    humanize_strength: 2.0,
    reaction_time: 0,
    sticky_aim: false,
    target_switch_delay: 0,
    fov_scale_smoothing: false,
    max_speed: 0,
};
// Team check is never restored from storage — every launch starts with it
// disabled, regardless of what was saved last session. It can only be
// turned on manually for the current session.
export const aimbot: Writable<AimbotCfg> = writable({
    ...loadStored(AIMBOT_KEY, AIMBOT_DEFAULTS),
    team_check: false,
});
persist(AIMBOT_KEY, aimbot);

export type TracerOrigin = "Top" | "Center" | "Bottom";
export type LabelAnchorX = "L" | "C" | "R";
export type LabelAnchorY = "T" | "C" | "B";
export type BoxStyle = "Full" | "Corner";

export type VisualsCfg = {
    // Global
    outline: boolean;

    // Skeleton
    skeleton: boolean;
    skeleton_color: string;
    skeleton_thickness: number;

    // Box 2D
    box2d: boolean;
    box2d_style: BoxStyle;
    box2d_color: string;
    box2d_fill: boolean;
    box2d_fill_color: string;
    box2d_thickness: number;

    // Box 3D
    box3d: boolean;
    box3d_style: BoxStyle;
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

    // Per-part box ESP — semi-transparent quad scaled to each part's real Size.
    part_esp: boolean;
    part_esp_color: string;
    part_esp_opacity: number;
    part_esp_border: boolean;
    part_esp_border_color: string;

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
    show_self: boolean;
    max_distance: number;
    /// Skip any character whose distance from the camera is below this —
    /// a belt+braces self-filter on top of the LocalPlayer.Name match so
    /// first-person games (camera inside the head) never ESP yourself.
    min_distance: number;

    // Radar — top-right circular minimap with the local player at the centre.
    radar: boolean;
    radar_size: number;          // diameter in px
    radar_range: number;         // world studs mapped to the radar radius
    radar_enemy_color: string;
    radar_ally_color: string;

    // Bullet Tracer — 3D line from local player to a hit target, fades over 4s.
    bullet_tracer: boolean;
    bullet_tracer_color: string;
    bullet_tracer_brightness: number;

    // Rotating crosshair drawn at screen center.
    crosshair: boolean;
    crosshair_color: string;
};

export const VISUALS_DEFAULTS: VisualsCfg = {
    outline: true,

    skeleton: false,
    skeleton_color: "#ffffff",
    skeleton_thickness: 1.5,

    box2d: true,
    box2d_style: "Full",
    box2d_color: "#ffffff",
    box2d_fill: false,
    box2d_fill_color: "#ffffff",
    box2d_thickness: 1.5,

    box3d: false,
    box3d_style: "Full",
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

    part_esp: false,
    part_esp_color: "#ffffff",
    part_esp_opacity: 0.25,
    part_esp_border: true,
    part_esp_border_color: "#ffffff",

    avatar_head: false,
    avatar_head_size: 28,
    avatar_head_border_color: "#ffde9d",
    avatar_head_border_thickness: 1.5,
    avatar_head_anchor_x: "C",
    avatar_head_anchor_y: "T",
    avatar_head_offset_x: 0,
    avatar_head_offset_y: -18,

    show_self: false,
    max_distance: 1500,
    min_distance: 3,

    radar: false,
    radar_size: 180,
    radar_range: 200,
    radar_enemy_color: "#ff4d4d",
    radar_ally_color: "#22e06a",

    bullet_tracer: false,
    bullet_tracer_color: "#ffffff",
    bullet_tracer_brightness: 1.0,

    crosshair: false,
    crosshair_color: "#ffffff",
};
export const visuals: Writable<VisualsCfg> = writable(loadStored(VISUALS_KEY, VISUALS_DEFAULTS));
persist(VISUALS_KEY, visuals);

export type MiscCfg = {
    hip_height_enabled: boolean;
    hip_height: number;
    jump_power_enabled: boolean;
    jump_power: number;
    walk_speed_enabled: boolean;
    walk_speed: number;
    display_distance_enabled: boolean;
    display_distance: number;
    fly_enabled: boolean;
    fly_keybind: string;
    fly_speed: number;
    noclip_enabled: boolean;
    tp_click_enabled: boolean;
    tp_click_keybind: string;
};

export const MISC_DEFAULTS: MiscCfg = {
    hip_height_enabled: false,
    hip_height: 2,
    jump_power_enabled: false,
    jump_power: 50,
    walk_speed_enabled: false,
    walk_speed: 16,
    display_distance_enabled: false,
    display_distance: 100,
    fly_enabled: false,
    fly_keybind: "",
    fly_speed: 5,
    noclip_enabled: false,
    tp_click_enabled: false,
    tp_click_keybind: "MouseRight",
};
// Misc tab settings are never persisted — always reset to defaults
// (disabled) on every Socrate restart.
export const misc: Writable<MiscCfg> = writable(MISC_DEFAULTS);

// Menu UI options — purely frontend, not synced to backend. `menu_visible`
// is flipped by a global keybind.
export type MenuOptionsCfg = {
    menu_visible: boolean;
    toggle_bind: string;
    show_binds: boolean;
    stream_proof: boolean;
};
export const MENU_OPTIONS_DEFAULTS: MenuOptionsCfg = {
    menu_visible: true,
    toggle_bind: "IntlBackslash",
    show_binds: true,
    stream_proof: true,
};
export const menuOptions: Writable<MenuOptionsCfg> = writable(
    // menu_visible is volatile — always start visible on launch, ignore the
    // persisted value for that one field so a hidden menu can't trap the user.
    { ...loadStored(MENU_OPTS_KEY, MENU_OPTIONS_DEFAULTS), menu_visible: true }
);
persist(MENU_OPTS_KEY, menuOptions);

// ─── KeyAuth key gate ────────────────────────────────────────────────
// The user pastes a KeyAuth license key; Socrate validates it server-side
// (bound to this machine's hwid) and stores the resulting expiry locally,
// re-prompting once it lapses.
const KEY_STORE = "socrate.key";
type StoredKey = { token: string; expiresAfter: number };

export function loadStoredKey(): StoredKey | null {
    if (typeof localStorage === "undefined") return null;
    const raw = localStorage.getItem(KEY_STORE);
    if (!raw) return null;
    try { return JSON.parse(raw) as StoredKey; } catch { return null; }
}

export function saveKey(token: string, expiresAfter: number) {
    if (typeof localStorage !== "undefined") {
        localStorage.setItem(KEY_STORE, JSON.stringify({ token, expiresAfter }));
    }
    // Hand the token to the backend license-watch thread so it can re-validate
    // it every few seconds and kick the user out live if it gets revoked.
    invoke("set_license_token", { token }).catch(() => {});
    keyUnlocked.set(true);
    checkKeyExpiry();
}

export function clearKey() {
    if (typeof localStorage !== "undefined") localStorage.removeItem(KEY_STORE);
    invoke("set_license_token", { token: null }).catch(() => {});
    keyUnlocked.set(false);
    keyTimeLeftMs.set(0);
}

/// True when a stored key exists and hasn't expired yet.
export function isKeyValid(): boolean {
    const k = loadStoredKey();
    // expiresAfter is Unix-millis from KeyAuth; 0 means "no expiry recorded" (lifetime).
    return !!k && (k.expiresAfter === 0 || k.expiresAfter > Date.now());
}

export const keyUnlocked: Writable<boolean> = writable(isKeyValid());

/// Milliseconds left on the stored key (0 once expired/absent). Single
/// source of truth for any UI that wants to show a countdown — derived from
/// the same expiry check that drives `keyUnlocked`, never duplicated.
export const keyTimeLeftMs: Writable<number> = writable(0);

/// Re-checks the stored key's expiry against the clock right now and
/// updates `keyUnlocked` / `keyTimeLeftMs` accordingly. Independent of any
/// mounted component — runs as long as the app process is alive, so the key
/// can never silently keep "working" past its expiry just because no UI
/// happened to be watching it.
function checkKeyExpiry() {
    const k = loadStoredKey();
    if (!k) {
        keyTimeLeftMs.set(0);
        keyUnlocked.set(false);
        return;
    }
    if (k.expiresAfter === 0) {
        // No expiry recorded — treat as valid indefinitely.
        keyTimeLeftMs.set(0);
        keyUnlocked.set(true);
        return;
    }
    const remaining = k.expiresAfter - Date.now();
    if (remaining <= 0) {
        clearKey(); // also zeroes keyTimeLeftMs and flips keyUnlocked false
        return;
    }
    keyTimeLeftMs.set(remaining);
    keyUnlocked.set(true);
}

type KeyauthResult = {
    valid: boolean;
    expires_after: number;
    network_error: boolean;
    version_blocked: boolean;
    latest_version: string;
};

/// Re-checks the stored key against the KeyAuth server itself, not just the
/// local clock — catches a key that's been deleted/blacklisted/hwid-reset on
/// the dashboard since it was last redeemed. A key that's simply unreachable
/// (offline) must NOT log the user out — `checkKeyExpiry` already owns the
/// "fails open on no connectivity" contract — so this only acts on an
/// explicit "invalid" verdict from the server.
///
/// On invalid: clears the stored key, which flips `keyUnlocked` false. That
/// alone closes the gate — `Menu.svelte` swaps the whole tab body for
/// `KeyGate` and `bridge.ts`'s kill switch disables every feature thread —
/// so the user is bounced straight back to the key-entry screen.
async function revalidateKeyWithServer() {
    const k = loadStoredKey();
    if (!k) return;
    try {
        const res = await invoke<KeyauthResult>("validate_keyauth_key", { token: k.token });
        if (res.network_error) return;
        if (res.version_blocked) {
            // This build was disabled server-side — show the message and quit.
            invoke("report_version_blocked", { latest: res.latest_version }).catch(() => {});
            return;
        }
        if (!res.valid) {
            clearKey();
            return;
        }
        // Still authoritatively valid — keep the cached expiry in sync in
        // case the subscription was extended/shortened remotely.
        saveKey(k.token, res.expires_after);
    } catch {
        /* couldn't reach Tauri/network — treat like offline, don't log out */
    }
}

if (typeof window !== "undefined") {
    checkKeyExpiry();
    setInterval(checkKeyExpiry, 1000);
    // Hand the stored token to the backend watcher on launch, then let its 5 s
    // loop own live re-validation. One immediate frontend check catches a key
    // already revoked before the app even opened.
    const boot = loadStoredKey();
    if (boot) invoke("set_license_token", { token: boot.token }).catch(() => {});
    revalidateKeyWithServer();
    // The backend `license_watch` thread re-checks every 5 s and emits this
    // when the key is revoked; bounce the user straight back to the key screen.
    listen("license_revoked", () => clearKey());
}

// ─── Bulk apply / export — used by the Configs tab ─────────────────
export type SocrateConfigFile = {
    version: number;
    aimbot: AimbotCfg;
    visuals: VisualsCfg;
    misc: MiscCfg;
    menuOptions: MenuOptionsCfg;
    theme: string;
};

export function exportCurrentConfig(): SocrateConfigFile {
    return {
        version: 1,
        aimbot: getStore(aimbot),
        visuals: getStore(visuals),
        misc: getStore(misc),
        menuOptions: getStore(menuOptions),
        theme: getStore(theme),
    };
}

export function applyImportedConfig(raw: unknown): { ok: boolean; reason?: string } {
    if (!raw || typeof raw !== "object") return { ok: false, reason: "not an object" };
    const cfg = raw as Partial<SocrateConfigFile>;
    if (cfg.aimbot)      aimbot.set({ ...AIMBOT_DEFAULTS, ...cfg.aimbot });
    if (cfg.visuals)     visuals.set({ ...VISUALS_DEFAULTS, ...cfg.visuals });
    if (cfg.misc)        misc.set({ ...MISC_DEFAULTS, ...cfg.misc });
    if (cfg.menuOptions) menuOptions.set({ ...MENU_OPTIONS_DEFAULTS, ...cfg.menuOptions, menu_visible: true });
    if (typeof cfg.theme === "string" && /^#[0-9a-fA-F]{6}$/.test(cfg.theme)) theme.set(cfg.theme);
    return { ok: true };
}

// ─── Saved configs (named JSON snapshots, kept in localStorage) ────
const SAVED_CONFIGS_KEY = "socrate.savedConfigs";

export type SavedConfig = { name: string; content: string; createdAt: string };

function loadSavedConfigs(): SavedConfig[] {
    if (typeof localStorage === "undefined") return [];
    try {
        const raw = localStorage.getItem(SAVED_CONFIGS_KEY);
        const parsed = raw ? JSON.parse(raw) : [];
        if (!Array.isArray(parsed)) return [];
        return parsed.map((c: Partial<SavedConfig>) => ({
            name: c.name ?? "",
            content: c.content ?? "",
            createdAt: c.createdAt ?? formatDate(new Date()),
        }));
    } catch { return []; }
}

export function formatDate(d: Date): string {
    const dd = String(d.getDate()).padStart(2, "0");
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    return `${dd}.${mm}.${d.getFullYear()}`;
}

export const savedConfigs = writable<SavedConfig[]>(loadSavedConfigs());
persist(SAVED_CONFIGS_KEY, savedConfigs);

export function addSavedConfig(name: string, content: string) {
    const createdAt = formatDate(new Date());
    savedConfigs.update((list) => [...list.filter((c) => c.name !== name), { name, content, createdAt }]);
}

export function deleteSavedConfig(name: string) {
    savedConfigs.update((list) => list.filter((c) => c.name !== name));
}

export function renameSavedConfig(oldName: string, newName: string) {
    savedConfigs.update((list) => list.map((c) => (c.name === oldName ? { ...c, name: newName } : c)));
}

export function loadSavedConfig(content: string): { ok: boolean; reason?: string } {
    try {
        return applyImportedConfig(JSON.parse(content));
    } catch (err) {
        return { ok: false, reason: String(err) };
    }
}

/// Restore aimbot, visuals, misc, menu options and theme to their factory defaults.
/// `menu_visible` is forced true so a reset can never trap the user.
export function resetToDefaults() {
    aimbot.set({ ...AIMBOT_DEFAULTS });
    visuals.set({ ...VISUALS_DEFAULTS });
    misc.set({ ...MISC_DEFAULTS });
    menuOptions.set({ ...MENU_OPTIONS_DEFAULTS, menu_visible: true });
    theme.set("#ffde9d");
}
