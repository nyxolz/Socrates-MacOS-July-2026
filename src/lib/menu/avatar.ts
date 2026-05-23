import { invoke } from "@tauri-apps/api/core";

// Username → avatar headshot URL. Goes through a Rust command rather than
// fetching the Roblox JSON endpoints directly — they don't send CORS
// headers, so the webview blocks them. Cached at module scope so repeated
// lookups are free; misses are cached as `null` too, no point retrying a
// username that came back empty.
const cache = new Map<string, Promise<string | null>>();

export function getAvatar(username: string): Promise<string | null> {
    if (!username) return Promise.resolve(null);
    const hit = cache.get(username);
    if (hit) return hit;
    const p = invoke<string | null>("get_avatar_url", { username }).catch(() => null);
    cache.set(username, p);
    return p;
}
