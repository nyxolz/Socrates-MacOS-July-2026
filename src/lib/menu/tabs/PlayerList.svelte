<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";

    type PlayerEntry = {
        char_addr: string;
        name: string;
        distance: number;
        health: number;
    };

    // Stable-order Map keyed by char_addr. New chars append at the end;
    // departed ones are removed; existing ones update in place (preserving
    // their slot). Map iteration order = insertion order in JS.
    let players = new Map<string, PlayerEntry>();
    let spectating: string | null = null;
    let timer = 0;

    async function refresh() {
        let list: PlayerEntry[];
        try { list = await invoke<PlayerEntry[]>("list_players"); }
        catch { list = []; }
        const seen = new Set(list.map(p => p.char_addr));
        for (const addr of Array.from(players.keys())) {
            if (!seen.has(addr)) players.delete(addr);
        }
        for (const p of list) players.set(p.char_addr, p); // update or append
        players = players; // trigger reactivity

        try { spectating = await invoke<string | null>("get_spectate_target"); }
        catch { /* keep */ }
    }

    async function spectate(addr: string) {
        const tgt = spectating === addr ? null : addr;
        await invoke("set_spectate_target", { target: tgt });
        spectating = tgt;
    }

    onMount(() => { refresh(); timer = window.setInterval(refresh, 1000); });
    onDestroy(() => clearInterval(timer));

    $: list = Array.from(players.values());
</script>

<div class="stack">
    <Section title="Players" icon="users" active={list.length > 0}>
        {#if !list.length}
            <div class="empty">no players visible</div>
        {/if}
        {#each list as p (p.char_addr)}
            <div class="row">
                <span class="name">{p.name || "?"}</span>
                <span class="meta">{p.distance.toFixed(0)}m · {p.health.toFixed(0)} hp</span>
                <button
                    class="spec"
                    class:active={spectating === p.char_addr}
                    on:click={() => spectate(p.char_addr)}
                >{spectating === p.char_addr ? "Stop" : "Spectate"}</button>
            </div>
        {/each}
    </Section>
</div>

<style>
    .stack { padding: 6px; }
    .empty {
        font-size: 11px;
        color: var(--text-muted);
        text-align: center;
        padding: 12px 0;
    }
    .row {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 4px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: 4px;
        margin-bottom: 4px;
    }
    .name {
        font-size: 11px;
        font-weight: 600;
        color: var(--text);
        flex: 1;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .meta {
        font-size: 9px;
        color: var(--text-muted);
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
    }
    .spec {
        font: inherit;
        font-size: 10px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text);
        padding: 3px 8px;
        cursor: pointer;
        transition: border-color 140ms var(--ease), color 140ms var(--ease), background 140ms var(--ease);
    }
    .spec:hover { border-color: var(--accent); }
    .spec.active {
        background: rgba(0, 0, 0, 0.3);
        border-color: var(--accent);
        color: var(--accent);
    }
</style>
