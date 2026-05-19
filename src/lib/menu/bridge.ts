import { invoke } from "@tauri-apps/api/core";
import { aimbot, visuals, type AimbotCfg, type VisualsCfg } from "./stores";

/** Wire each store to its corresponding Tauri command. Called once at mount. */
export function startConfigBridge() {
    const unsubA = aimbot.subscribe((cfg: AimbotCfg) => {
        invoke("set_aimbot_config", { cfg }).catch(() => {});
    });
    const unsubV = visuals.subscribe((cfg: VisualsCfg) => {
        invoke("set_visuals_config", { cfg }).catch(() => {});
    });
    return () => {
        unsubA();
        unsubV();
    };
}
