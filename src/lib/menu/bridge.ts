import { invoke } from "@tauri-apps/api/core";
import { aimbot, visuals, misc, menuOptions, keyUnlocked, type AimbotCfg, type VisualsCfg, type MiscCfg, type MenuOptionsCfg } from "./stores";

/** Wire each store to its corresponding Tauri command. Called once at mount. */
export function startConfigBridge() {
    // Work.ink key gate — pushed FIRST and on every change, independent of
    // whatever tab/screen is currently rendered. This is the backend's
    // authoritative kill switch: every feature thread (ESP, aimbot, misc,
    // spectate) checks it before doing anything, so a stale "enabled" config
    // below can never run while the key is unverified.
    const unsubKey = keyUnlocked.subscribe((unlocked: boolean) => {
        invoke("set_key_unlocked", { unlocked }).catch(() => {});
    });
    const unsubA = aimbot.subscribe((cfg: AimbotCfg) => {
        invoke("set_aimbot_config", { cfg }).catch(() => {});
    });
    const unsubV = visuals.subscribe((cfg: VisualsCfg) => {
        invoke("set_visuals_config", { cfg }).catch(() => {});
    });
    const unsubMisc = misc.subscribe((cfg: MiscCfg) => {
        invoke("set_misc_config", { cfg }).catch(() => {});
    });
    // Only the toggle bind needs to flow to Rust — the rest of `menuOptions`
    // is purely UI state and lives in the frontend.
    let lastBind = "";
    const unsubM = menuOptions.subscribe((cfg: MenuOptionsCfg) => {
        if (cfg.toggle_bind !== lastBind) {
            lastBind = cfg.toggle_bind;
            invoke("set_menu_toggle_bind", { bind: cfg.toggle_bind }).catch(() => {});
        }
    });
    return () => {
        unsubKey();
        unsubA();
        unsubV();
        unsubMisc();
        unsubM();
    };
}
