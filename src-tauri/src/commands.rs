//! Tauri command surface exposed to the SvelteKit frontend.
//!
//! Keep this thin — each command is just glue between the frontend and the
//! background threads / SDK. Feature work (aimbot config, ESP toggles, …)
//! gets its own command(s) added here as it lands.

use std::collections::HashSet;
use std::sync::Arc;

use obfstr::obfstr;
use serde::Serialize;
use tauri::State;

use crate::offsets;
use crate::overlay::InteractiveRect;
use crate::players::{find_child_fast, read_name_fast};
use crate::sdk::{self, Instance, SdkI};
use crate::security;
use crate::state::{AimbotCfg, AppState, RobloxBounds, VisualsCfg, ZoneInput};

#[tauri::command]
pub fn get_roblox_bounds(state: State<AppState>) -> Option<RobloxBounds> {
    state.roblox_bounds.lock().unwrap()
        .map(|(x, y, width, height)| RobloxBounds { x, y, width, height })
}

#[tauri::command]
pub fn is_roblox_running(state: State<AppState>) -> bool {
    state.roblox_bounds.lock().unwrap().is_some()
}

/// Frontend publishes its current interactive rects (window-local pixels) on
/// mount and on every resize / drag. The mouse tracker reads this on each tick.
#[tauri::command]
pub fn set_interactive_zones(state: State<AppState>, zones: Vec<ZoneInput>) {
    let mut z = state.interactive_zones.lock().unwrap();
    z.clear();
    for r in zones {
        z.push(InteractiveRect { x: r.x, y: r.y, w: r.w, h: r.h });
    }
}

#[tauri::command]
pub fn panic_exit() {
    crate::telemetry::mark_stopped();
    std::process::exit(0);
}

/// Current resident-set size of *this* process in bytes. Read directly from
/// `task_info(mach_task_self, MACH_TASK_BASIC_INFO)` so it matches what
/// Activity Monitor shows for "Memory" — no shell-out, no allocation.
#[tauri::command]
pub fn get_self_ram_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        // mach_task_basic_info layout (xnu osfmk/mach/task_info.h):
        //   virtual_size, resident_size, resident_size_max : u64
        //   user_time, system_time : time_value_t (2x i32)
        //   policy : i32
        //   suspend_count : i32
        // Count = struct size / sizeof(natural_t=u32) = 48/4 = 12.
        #[repr(C)]
        struct MachTaskBasicInfo {
            virtual_size: u64,
            resident_size: u64,
            resident_size_max: u64,
            user_time: [i32; 2],
            system_time: [i32; 2],
            policy: i32,
            suspend_count: i32,
        }
        unsafe extern "C" {
            fn mach_task_self() -> u32;
            fn task_info(target: u32, flavor: u32, info: *mut MachTaskBasicInfo, count: *mut u32) -> i32;
        }
        const MACH_TASK_BASIC_INFO: u32 = 20;
        let mut info = MachTaskBasicInfo {
            virtual_size: 0, resident_size: 0, resident_size_max: 0,
            user_time: [0, 0], system_time: [0, 0], policy: 0, suspend_count: 0,
        };
        let mut count: u32 = 12;
        if task_info(mach_task_self(), MACH_TASK_BASIC_INFO, &mut info, &mut count) == 0 {
            return info.resident_size;
        }
    }
    0
}

/// Attach to a running Roblox process. Stores the resulting SDK in AppState
/// for any later command / background thread to use. Returns the static base
/// address for sanity-checking from the UI.
#[tauri::command]
pub fn attach_sdk(state: State<AppState>, process_name: Option<String>) -> Result<String, String> {
    let name = process_name.unwrap_or_else(|| obfstr!("RobloxPlayer").to_string());
    let sdk = SdkI::new(&name)?;
    let base = sdk.base;
    *state.sdk.lock().unwrap() = Some(Arc::new(sdk));
    Ok(format!("0x{:X}", base))
}

#[tauri::command]
pub fn sdk_status(state: State<AppState>) -> Option<(String, u64, u64)> {
    let sdk = state.sdk.lock().unwrap().clone()?;
    let (ok, fail) = sdk.read_stats();
    Some((format!("0x{:X}", sdk.base), ok, fail))
}

#[tauri::command]
pub fn set_aimbot_config(state: State<AppState>, cfg: AimbotCfg) {
    *state.aimbot_cfg.lock().unwrap() = cfg;
}

#[tauri::command]
pub fn set_visuals_config(state: State<AppState>, cfg: VisualsCfg) {
    *state.visuals_cfg.lock().unwrap() = cfg;
}

#[tauri::command]
pub fn set_misc_config(state: State<AppState>, cfg: crate::state::MiscCfg) {
    *state.misc_cfg.lock().unwrap() = cfg;
}

#[tauri::command]
pub fn set_menu_toggle_bind(state: State<AppState>, bind: String) {
    *state.menu_toggle_bind.lock().unwrap() = bind;
}

/// Detected build architecture ("ARM" / "Intel"). The DataModel resolution
/// chain (1 vs 2 dereferences) is selected at compile time from this — the
/// menu surfaces it so the user can confirm the right binary is running.
#[tauri::command]
pub fn get_arch() -> &'static str { security::arch_name() }

/// Write `content` to `~/Desktop/<name>.json`. Used by the Configs tab's
/// Export button — keeps the file on the user's desktop instead of going
/// through the browser's download dialog. `name` is sanitised to a safe
/// filename (`[A-Za-z0-9_-]`) and `.json` is appended if missing. Returns
/// the absolute path written to on success.
#[tauri::command]
pub fn export_config_to_desktop(name: String, content: String) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "$HOME unset".to_string())?;
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let stem = if safe.is_empty() { "socrate-config".to_string() } else { safe };
    let filename = if stem.ends_with(".json") { stem } else { format!("{stem}.json") };
    let path = format!("{home}/Desktop/{filename}");
    std::fs::write(&path, content).map_err(|e| format!("write failed: {e}"))?;
    Ok(path)
}

// ─── Offset health check for the Output tab ─────────────────────────────

/// Returns `true` iff the offset table still matches this Roblox build. The
/// check is one pass: the DataModel chain must resolve, its `StarterGui`
/// service slot must read back the name "StarterGui", and a live character in
/// the cache must expose a child named "Humanoid".
#[tauri::command]
pub fn diag_check(state: State<AppState>) -> bool {
    let sdk = match state.sdk.lock().unwrap().clone() { Some(s) => s, None => return false };
    let dm = match security::resolve_data_model(&sdk) { Some(v) => v, None => return false };

    // StarterGui — fixed DataModel offset, validated by name.
    let sg = sdk.read::<u64>(dm + offsets::DataModel::StarterGui as u64);
    if !sdk.is_valid_ptr(sg) { return false; }
    if sdk::name_of(&sdk, sg) != obfstr!("StarterGui") { return false; }

    // Humanoid — name check off a live character in the cache.
    let caches = state.char_caches.lock().unwrap().clone();
    match caches.iter().find(|c| sdk.is_valid_ptr(c.humanoid_addr)) {
        Some(c) => sdk::name_of(&sdk, c.humanoid_addr) == obfstr!("Humanoid"),
        None => false,
    }
}

// ─── Player List + Spectate ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct PlayerEntry {
    pub char_addr: String,  // hex so JS can round-trip without losing precision
    pub name: String,
    pub distance: f32,
    pub health: f32,
    /// 0 = enemy (default), 1 = ally, 2 = neutral.
    pub relation: u8,
}

#[tauri::command]
pub fn list_players(state: State<AppState>) -> Vec<PlayerEntry> {
    let sdk = state.sdk.lock().unwrap().clone();
    let sdk = match sdk { Some(s) => s, None => return vec![] };

    // Compute distance from a fresh camera read — same chain as elsewhere.
    let cam_pos = resolve_cam_pos(&sdk).unwrap_or([0.0; 3]);

    let caches = state.char_caches.lock().unwrap().clone();
    let relations = state.player_relations.lock().unwrap().clone();
    let local_name = state.local_player_name.lock().unwrap().clone();
    // Team check (aimbot toggle): same-parent players show as ally in the list.
    let team_check = state.aimbot_cfg.lock().unwrap().team_check;
    let local_parent = state.local_parent.load(std::sync::atomic::Ordering::Relaxed);
    let mut list: Vec<PlayerEntry> = caches.iter().filter_map(|c| {
        // Exclude the local player (first registered in Players) from the list.
        if !local_name.is_empty() && c.name == local_name { return None; }
        // Use the first VALID part, not strictly parts[0] (the Head). Custom
        // rigs — e.g. a character that only owns a HumanoidRootPart and no Head
        // child — leave parts[0] empty, which would drop them from the list even
        // though ESP/radar still track them (they fall back to any valid part).
        let prim = c.parts.iter().find_map(|(p, v)| if *v { Some(*p) } else { None });
        let prim = match prim { Some(p) => p, None => return None };
        let pos = crate::players::read_pos_fast(&sdk, prim);
        if !pos[0].is_finite() { return None; }
        if pos[0] == 0.0 && pos[1] == 0.0 && pos[2] == 0.0 { return None; }
        let dx = pos[0] - cam_pos[0];
        let dy = pos[1] - cam_pos[1];
        let dz = pos[2] - cam_pos[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance < 0.5 { return None; } // skip self / overlap
        let health = if sdk.is_valid_ptr(c.humanoid_addr) {
            let h = sdk.read::<f32>(c.humanoid_addr + offsets::Humanoid::Health as u64);
            if h.is_finite() && (0.0..=1000.0).contains(&h) { h.clamp(0.0, 100.0) } else { -1.0 }
        } else { -1.0 };
        Some(PlayerEntry {
            char_addr: format!("0x{:X}", c.char_addr),
            name: c.name.clone(),
            distance,
            health,
            relation: crate::players::effective_relation(
                relations.get(&c.name).copied().unwrap_or(0),
                team_check, c.parent_addr, local_parent),
        })
    }).collect();
    list.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    list
}

fn resolve_cam_pos(sdk: &SdkI) -> Option<[f32; 3]> {
    let cam = security::resolve_camera(sdk)?;
    let off = offsets::Camera::CameraPosition as u64;
    Some([
        sdk.read::<f32>(cam + off),
        sdk.read::<f32>(cam + off + 4),
        sdk.read::<f32>(cam + off + 8),
    ])
}

/// `target` is the hex char_addr from `list_players` or `null` to stop.
#[tauri::command]
pub fn set_spectate_target(state: State<AppState>, target: Option<String>) {
    let val = target
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0);
    *state.spectate_target.lock().unwrap() = val;
}

#[tauri::command]
pub fn get_spectate_target(state: State<AppState>) -> Option<String> {
    let v = *state.spectate_target.lock().unwrap();
    if v == 0 { None } else { Some(format!("0x{:X}", v)) }
}

/// Set a player's relation by name. `relation`: 0 = enemy, 1 = ally,
/// 2 = neutral. Enemy is the default, so storing 0 just drops the entry to
/// keep the map small. Keyed by name (not char_addr) so it survives respawns.
#[tauri::command]
pub fn set_player_relation(state: State<AppState>, name: String, relation: u8) {
    if name.is_empty() { return; }
    let mut relations = state.player_relations.lock().unwrap();
    if relation == 0 {
        relations.remove(&name);
    } else {
        relations.insert(name, relation);
    }
}


// ─── Server / place name ─────────────────────────────────────────────────

/// DataModel's `Name` instance field is the place name in Roblox.
#[tauri::command]
pub fn get_place_name(state: State<AppState>) -> String {
    let sdk = state.sdk.lock().unwrap().clone();
    let sdk = match sdk { Some(s) => s, None => return "(not attached)".into() };
    let dm = match security::resolve_data_model(&sdk) {
        Some(v) => v, None => return "(no datamodel)".into(),
    };
    let name = sdk::name_of(&sdk, dm);
    if name.is_empty() { "(no place)".into() } else { name }
}

/// Live place identifiers read straight off the DataModel struct.
#[derive(Serialize, Clone)]
pub struct PlaceInfo {
    pub place_id: i64,
    pub game_id: i64,
    pub place_version: i32,
}

/// Read the current place id (`DataModel + 0x190`), game id (`+ 0x188`) and
/// place version (`+ 0x1ac`) from the attached Roblox process.
#[tauri::command]
pub fn get_place_info(state: State<AppState>) -> Option<PlaceInfo> {
    let sdk = state.sdk.lock().unwrap().clone()?;
    let dm = security::resolve_data_model(&sdk)?;
    Some(PlaceInfo {
        place_id: sdk.read::<i64>(dm + offsets::DataModel::PlaceId as u64),
        game_id: sdk.read::<i64>(dm + offsets::DataModel::GameId as u64),
        place_version: sdk.read::<i32>(dm + offsets::DataModel::PlaceVersion as u64),
    })
}

/// Toggle stream-proofing on the overlay window. When `enabled`, the window
/// is excluded from screen capture / sharing (OBS, QuickTime, Discord share,
/// …) so the cheat UI never shows up in a recording. Backed by the Settings
/// toggle; applied once on launch with the persisted value.
#[tauri::command]
pub fn set_stream_proof(enabled: bool, window: tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    unsafe { crate::overlay::set_window_sharing(&window, enabled); }
    #[cfg(not(target_os = "macos"))]
    let _ = (enabled, window);
}

/// "Reset ESP": ask the cache thread to drop everything and re-discover
/// players from scratch — same as a fresh Socrate launch. Doesn't touch any
/// visual settings.
#[tauri::command]
pub fn reset_esp(state: State<AppState>) {
    state.esp_reset_requested.store(true, std::sync::atomic::Ordering::Relaxed);
}

// ─── Dev tab ─────────────────────────────────────────────────────────────

/// Toggle the "ESP all game objects" dev overlay (3-D box + name on every
/// Workspace part). Heavy — capped backend-side.
#[tauri::command]
pub fn set_object_esp(state: State<AppState>, enabled: bool) {
    state.object_esp_enabled.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// List the name of every direct child of the `Players` service. Backs the
/// (code-gated) Dev tab — handy for eyeballing who the game registers.
#[tauri::command]
pub fn dev_players_children(state: State<AppState>) -> Vec<String> {
    let sdk = state.sdk.lock().unwrap().clone();
    let sdk = match sdk { Some(s) => s, None => return Vec::new() };
    let dm = match security::resolve_data_model(&sdk) {
        Some(v) => v, None => return Vec::new(),
    };
    let players = find_child_fast(&sdk, dm, obfstr!("Players").as_ref());
    if !sdk.is_valid_ptr(players) { return Vec::new(); }
    sdk::get_children(&sdk, players)
        .into_iter()
        .map(|c| sdk::name_of(&sdk, c))
        .filter(|n| !n.is_empty())
        .collect()
}

/// Launch the Roblox `roblox://placeId=…` URL via the macOS `open` command.
/// Frontend validates the input as a positive integer first.
#[tauri::command]
pub fn join_place(place_id: String) -> Result<(), String> {
    if place_id.is_empty() || !place_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("invalid place id".into());
    }
    let url = format!("roblox://placeId={}", place_id);
    std::process::Command::new("open")
        .arg(&url)
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Explorer — DataModel instance tree ─────────────────────────────────

#[derive(Serialize, Clone)]
pub struct InstanceNode {
    pub name: String,
    pub address: String,
    pub children: Vec<InstanceNode>,
}

/// Walk the DataModel recursively and return a capped tree. Caps keep the
/// payload sane (a real DataModel can hold tens of thousands of nodes).
///
/// `reveal` (optional hex address, e.g. the Player List's char_addr) makes the
/// walk "reveal-aware": the chain of ancestors from that node up to the
/// DataModel is force-included even past the depth / per-node caps, so the
/// target is guaranteed present in the returned tree. Without it, a player
/// nested deeper than `MAX_DEPTH` — or past a parent's child cap — wouldn't
/// appear, which is what made "View in explorer" land on nothing.
#[tauri::command]
pub fn get_instance_tree(state: State<AppState>, reveal: Option<String>) -> Result<InstanceNode, String> {
    let sdk = state.sdk.lock().unwrap().clone().ok_or_else(|| "not attached".to_string())?;
    let dm = security::resolve_data_model(&sdk).ok_or_else(|| "no datamodel".to_string())?;

    const MAX_DEPTH: u32 = 5;
    const MAX_CHILDREN_PER_NODE: usize = 200;
    const MAX_TOTAL_NODES: usize = 6000;

    // Ancestor chain of the reveal target (inclusive of target and DataModel),
    // found by walking `Instance::Parent` pointers upward. Empty when there's
    // nothing to reveal. Nodes in this set bypass the depth / child caps.
    let path: HashSet<u64> = match reveal
        .as_deref()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    {
        Some(target) if target != 0 => {
            let mut set = HashSet::new();
            let mut cur = target;
            let mut guard = 0;
            while sdk.is_valid_ptr(cur) && guard < 64 {
                set.insert(cur);
                if cur == dm { break; }
                cur = sdk.read::<u64>(cur + Instance::Parent as u64);
                guard += 1;
            }
            set
        }
        _ => HashSet::new(),
    };

    fn build(sdk: &SdkI, instance: u64, depth: u32, total: &mut usize, path: &HashSet<u64>) -> Option<InstanceNode> {
        if !sdk.is_valid_ptr(instance) { return None; }
        if *total >= MAX_TOTAL_NODES { return None; }
        *total += 1;
        let name = sdk::name_of(sdk, instance);
        let address = format!("0x{:X}", instance);
        // Descend past the depth cap only along the reveal path.
        let on_path = path.contains(&instance);
        let mut children = Vec::new();
        if depth < MAX_DEPTH || on_path {
            for (i, c) in sdk::get_children(sdk, instance).into_iter().enumerate() {
                if *total >= MAX_TOTAL_NODES { break; }
                // Honour the per-node cap unless this child is itself on the
                // reveal path (then keep it even past the cap).
                if i >= MAX_CHILDREN_PER_NODE && !path.contains(&c) { continue; }
                if let Some(n) = build(sdk, c, depth + 1, total, path) {
                    children.push(n);
                }
            }
        }
        Some(InstanceNode { name, address, children })
    }

    let mut total = 0usize;
    let root_children: Vec<InstanceNode> = sdk::get_children(&sdk, dm)
        .into_iter()
        .filter_map(|c| build(&sdk, c, 1, &mut total, &path))
        .collect();
    let root_name = sdk::name_of(&sdk, dm);
    Ok(InstanceNode {
        name: if root_name.is_empty() { "DataModel".to_string() } else { root_name },
        address: format!("0x{:X}", dm),
        children: root_children,
    })
}

/// Resolve a Roblox username to its 420×420 PNG headshot URL.
///
/// Has to run server-side because the Roblox JSON endpoints don't send
/// `Access-Control-Allow-Origin`, so the webview's own fetch is blocked.
/// The returned URL is on Roblox's CDN, which images load fine from any
/// origin — the frontend uses it directly as an `<img src>`.
#[tauri::command]
pub async fn get_avatar_url(username: String) -> Result<Option<String>, String> {
    if username.is_empty() { return Ok(None); }

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| e.to_string())?;

    let r1: serde_json::Value = client
        .post("https://users.roblox.com/v1/usernames/users")
        .json(&serde_json::json!({
            "usernames": [username],
            "excludeBannedUsers": false,
        }))
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let user_id = r1["data"].get(0).and_then(|d| d["id"].as_u64());
    let Some(user_id) = user_id else { return Ok(None); };

    let url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=420x420&format=Png",
        user_id
    );
    let r2: serde_json::Value = client
        .get(&url)
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(r2["data"]
        .get(0)
        .and_then(|d| d["imageUrl"].as_str())
        .map(|s| s.to_string()))
}

/// Resolve a Roblox **Universe ID** (the `GameId` we read off the DataModel)
/// to the public game title. Runs server-side for the same CORS reason as
/// `get_avatar_url` — the games endpoint sends no `Access-Control-Allow-Origin`.
///
/// `https://games.roblox.com/v1/games?universeIds={id}` → `data[0].name`.
/// No auth needed. Returns `None` if the id is invalid or the game is gated.
#[tauri::command]
pub async fn get_game_name(game_id: i64) -> Result<Option<String>, String> {
    if game_id <= 0 { return Ok(None); }

    let client = reqwest::Client::builder().build().map_err(|e| e.to_string())?;
    let url = format!("https://games.roblox.com/v1/games?universeIds={}", game_id);
    let r: serde_json::Value = client
        .get(&url)
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(r["data"]
        .get(0)
        .and_then(|d| d["name"].as_str())
        .map(|s| s.to_string()))
}

// ─── Versioning / self-update ────────────────────────────────────────────

/// Compile-time version hash baked in by `builder_sender.sh`. Cheap enough
/// the Settings tab calls it on every render.
#[tauri::command]
pub fn get_version() -> &'static str {
    crate::version::VERSION
}

/// Fetch the version currently published at usesocrate.xyz. Returns the
/// raw `socrate_version` string; the frontend compares against `get_version`
/// to decide if an update banner should show.
#[tauri::command]
pub async fn check_remote_version() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = client
        .get("https://usesocrate.xyz/status.json")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    body["Socrate"]["socrate_version"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "socrate_version missing".to_string())
}

// keep silenced warnings happy if these end up unused later
#[allow(dead_code)] fn _force_use(_a: u64) -> u64 { let _ = find_child_fast; let _ = read_name_fast; 0 }
