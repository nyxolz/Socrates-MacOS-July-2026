import { invoke } from "@tauri-apps/api/core";
import { aimbot, visuals, menuOptions, type AimbotCfg, type VisualsCfg, type MenuOptionsCfg } from "./stores";

/** Wire each store to its corresponding Tauri command. Called once at mount. */
export function startConfigBridge() {
    const unsubA = aimbot.subscribe((cfg: AimbotCfg) => {
        invoke("set_aimbot_config", { cfg }).catch(() => {});
    });
    const unsubV = visuals.subscribe((cfg: VisualsCfg) => {
        invoke("set_visuals_config", { cfg }).catch(() => {});
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
        unsubA();
        unsubV();
        unsubM();
    };
}
