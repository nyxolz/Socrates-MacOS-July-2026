// Tiny transient-notification system. `toast("…")` drops a message that the
// <Toasts/> overlay shows for ~2s then auto-dismisses. Used to confirm actions
// like flipping a checkbox or loading a config.

import { writable, type Writable } from "svelte/store";

export type Toast = { id: number; text: string; ms: number };

export const toasts: Writable<Toast[]> = writable([]);

let nextId = 1;

export function toast(text: string, ms = 2000) {
    const id = nextId++;
    toasts.update((t) => [...t, { id, text, ms }]);
    setTimeout(() => {
        toasts.update((t) => t.filter((x) => x.id !== id));
    }, ms);
}
