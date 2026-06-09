// Shared interactive-zone registry.
//
// The backend `set_interactive_zones` command REPLACES the whole list on every
// call, so multiple floating windows (the main Menu, the binds HUD, …) can't
// each call it directly — they'd clobber each other. Instead every window
// registers its rect here under a stable id; we merge them and push the union.

import { invoke } from "@tauri-apps/api/core";

export type Zone = { x: number; y: number; w: number; h: number };

const zones = new Map<string, Zone>();

function push() {
    invoke("set_interactive_zones", { zones: [...zones.values()] }).catch(() => {});
}

/// Register (or update) a named interactive rect. Pass `null` to remove it
/// (e.g. when a window is hidden) so the overlay stops eating clicks there.
export function setZone(id: string, zone: Zone | null) {
    if (zone) zones.set(id, zone);
    else zones.delete(id);
    push();
}
