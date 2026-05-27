<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import { activeTab, explorerReveal } from "../stores";

    type PlayerEntry = {
        char_addr: string;
        name: string;
        distance: number;
        health: number;
        relation: number; // 0 = enemy, 1 = ally, 2 = neutral
    };

    // Relation options, in display order. Value matches the backend enum.
    const RELATIONS = [
        { value: 0, label: "Enemy" },
        { value: 1, label: "Ally" },
        { value: 2, label: "Neutral" },
    ];

    // Stable-order Map keyed by char_addr. New chars append at the end;
    // departed ones are removed; existing ones update in place (preserving
    // their slot). Map iteration order = insertion order in JS.
    let players = new Map<string, PlayerEntry>();
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
    }

    // Jump to the Explorer and reveal this player's instance. char_addr is the
    // same hex the Explorer prints as a node address, so the Explorer can match
    // and expand the path straight to it.
    function viewInExplorer(addr: string) {
        explorerReveal.set(addr);
        activeTab.set("Explorer");
    }

    async function setRelation(p: PlayerEntry, relation: number) {
        // Optimistic update so the segmented control reacts instantly; the
        // next refresh reconciles from the backend (keyed by name there).
        const cur = players.get(p.char_addr);
        if (cur) { cur.relation = relation; players = players; }
        try { await invoke("set_player_relation", { name: p.name, relation }); }
        catch { /* refresh will resync */ }
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
                <div class="relation" role="group" aria-label="relation">
                    {#each RELATIONS as r}
                        <button
                            class="rel rel-{r.value}"
                            class:active={(p.relation ?? 0) === r.value}
                            title={r.label}
                            on:click={() => setRelation(p, r.value)}
                        >{r.label[0]}</button>
                    {/each}
                </div>
                <button
                    class="spec"
                    title="Reveal in Explorer"
                    on:click={() => viewInExplorer(p.char_addr)}
                >View in explorer</button>
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
    .relation {
        display: inline-flex;
        gap: 2px;
        flex: none;
    }
    .rel {
        font: inherit;
        font-size: 10px;
        font-weight: 600;
        width: 20px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text-muted);
        padding: 3px 0;
        cursor: pointer;
        transition: border-color 140ms var(--ease), color 140ms var(--ease), background 140ms var(--ease);
    }
    .rel:hover { border-color: var(--text); color: var(--text); }
    .rel.active.rel-0 { background: rgba(239, 68, 68, 0.18); border-color: #ef4444; color: #ef6a6a; }
    .rel.active.rel-1 { background: rgba(34, 224, 106, 0.18); border-color: #22e06a; color: #4fe089; }
    .rel.active.rel-2 { background: rgba(148, 163, 184, 0.18); border-color: #94a3b8; color: #b3bccb; }
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
    .spec:hover { border-color: var(--accent); color: var(--accent); }
</style>
