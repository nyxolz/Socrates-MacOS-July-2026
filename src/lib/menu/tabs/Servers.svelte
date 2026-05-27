<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";

    type CachedPlace = { id: string; name: string; lastJoined: number };

    const CACHE_KEY = "socrate.places.cached";
    const LAST_KEY = "socrate.places.lastJoined";

    let placeName = "...";
    let lastJoinedId: string | null = null;
    let cached: CachedPlace[] = [];
    let inputId = "";
    let joinError = "";
    let joining = false;
    let timer = 0;
    let ramBytes = 0;

    function loadCache() {
        try {
            const raw = localStorage.getItem(CACHE_KEY);
            cached = raw ? JSON.parse(raw) : [];
        } catch { cached = []; }
        lastJoinedId = localStorage.getItem(LAST_KEY);
    }

    function saveCache() {
        localStorage.setItem(CACHE_KEY, JSON.stringify(cached));
    }

    async function refresh() {
        try { placeName = await invoke<string>("get_place_name"); }
        catch { placeName = "(not attached)"; }
        try { ramBytes = await invoke<number>("get_self_ram_bytes"); }
        catch { ramBytes = 0; }

        // If we know the last joined id and the place name updated to a real
        // value, refresh that entry's display name so the cache stays useful.
        if (lastJoinedId && placeName && !placeName.startsWith("(")) {
            const idx = cached.findIndex(c => c.id === lastJoinedId);
            if (idx >= 0 && cached[idx].name !== placeName) {
                cached[idx].name = placeName;
                cached = cached;
                saveCache();
            }
        }
    }

    async function join(id: string) {
        const trimmed = id.trim();
        if (!/^\d+$/.test(trimmed)) {
            joinError = "place id must be digits only";
            return;
        }
        joinError = "";
        joining = true;
        try {
            await invoke("join_place", { placeId: trimmed });
            lastJoinedId = trimmed;
            localStorage.setItem(LAST_KEY, trimmed);

            const existing = cached.findIndex(c => c.id === trimmed);
            const entry: CachedPlace = {
                id: trimmed,
                // Reuse the known name if we have one, else placeholder until
                // the next refresh tick pulls it from the attached Roblox.
                name: existing >= 0 ? cached[existing].name : "(joining…)",
                lastJoined: Date.now(),
            };
            if (existing >= 0) cached.splice(existing, 1);
            cached.unshift(entry);
            cached = cached.slice(0, 12); // keep the list bounded
            saveCache();
            inputId = "";
        } catch (e) {
            joinError = String(e);
        } finally {
            joining = false;
        }
    }

    function forget(id: string) {
        cached = cached.filter(c => c.id !== id);
        saveCache();
        if (lastJoinedId === id) {
            lastJoinedId = null;
            localStorage.removeItem(LAST_KEY);
        }
    }

    function onKey(e: KeyboardEvent) {
        if (e.key === "Enter") join(inputId);
    }

    function fmtRam(bytes: number): string {
        if (!bytes) return "—";
        const mb = bytes / (1024 * 1024);
        return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
    }

    onMount(() => {
        loadCache();
        refresh();
        timer = window.setInterval(refresh, 2000);
    });
    onDestroy(() => clearInterval(timer));
</script>

<div class="grid">
    <div class="col">
        <Section title="Current Place" icon="server" active={placeName !== "..." && !placeName.startsWith("(")}>
            <div class="row">
                <span class="lbl">Name</span>
                <span class="val">{placeName}</span>
            </div>
            <div class="row">
                <span class="lbl">Place ID</span>
                <span class="val mono">{lastJoinedId ?? "—"}</span>
            </div>
            <div class="ram mono">socrate ram: {fmtRam(ramBytes)}</div>
        </Section>

        <Section title="Join Place ID" icon="compass" active={inputId.length > 0}>
            <div class="join">
                <input
                    type="text"
                    inputmode="numeric"
                    placeholder="e.g. 920587237"
                    bind:value={inputId}
                    on:keydown={onKey}
                    disabled={joining}
                />
                <button class="btn" disabled={joining || !inputId.trim()} on:click={() => join(inputId)}>
                    {joining ? "…" : "Join"}
                </button>
            </div>
            {#if joinError}
                <div class="err">{joinError}</div>
            {/if}
        </Section>
    </div>

    <div class="col">
        <Section title="Cached Places" icon="list" active={cached.length > 0}>
            {#if !cached.length}
                <div class="empty">no places joined yet</div>
            {/if}
            {#each cached as p (p.id)}
                <div class="cache-row" class:current={p.id === lastJoinedId}>
                    <div class="cache-text">
                        <span class="cache-name">{p.name}</span>
                        <span class="cache-id mono">{p.id}</span>
                    </div>
                    <button class="mini" title="Rejoin" on:click={() => join(p.id)}>Join</button>
                    <button class="mini ghost" title="Forget" on:click={() => forget(p.id)}>×</button>
                </div>
            {/each}
        </Section>

    </div>
</div>

<style>
    .grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 6px;
        padding: 6px;
        align-items: start;
    }
    .col { display: flex; flex-direction: column; min-width: 0; }
    .row {
        display: flex;
        align-items: baseline;
        gap: 6px;
        padding: 2px 0;
    }
    .lbl {
        font-size: 10px;
        color: var(--text-muted);
        min-width: 56px;
    }
    .val {
        font-size: 11px;
        font-weight: 600;
        color: var(--accent);
        word-break: break-all;
    }
    .mono { font-variant-numeric: tabular-nums; }
    .ram {
        font-size: 9px;
        color: var(--text-muted);
        margin-top: 4px;
        text-align: right;
        opacity: 0.7;
    }

    .join { display: flex; gap: 6px; align-items: stretch; }
    .join input {
        flex: 1;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 8px;
        outline: none;
        min-width: 0;
    }
    .join input:focus { border-color: var(--accent); }
    .join input:disabled { opacity: 0.6; }
    .btn {
        font: inherit;
        font-size: 11px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        padding: 4px 12px;
        cursor: pointer;
        transition: border-color 140ms var(--ease), color 140ms var(--ease);
    }
    .btn:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
    .btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .err {
        font-size: 10px;
        color: #e88;
        margin-top: 2px;
    }

    .cache-row {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 4px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        margin-bottom: 4px;
    }
    .cache-row.current { border-color: var(--accent); }
    .cache-text {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 1px;
    }
    .cache-name {
        font-size: 11px;
        font-weight: 600;
        color: var(--text);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .cache-id {
        font-size: 9px;
        color: var(--text-muted);
    }
    .mini {
        font: inherit;
        font-size: 10px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text);
        padding: 3px 7px;
        cursor: pointer;
        transition: border-color 140ms var(--ease), color 140ms var(--ease);
    }
    .mini:hover { border-color: var(--accent); color: var(--accent); }
    .mini.ghost {
        padding: 3px 6px;
        color: var(--text-muted);
    }
    .empty {
        font-size: 11px;
        color: var(--text-muted);
        text-align: center;
        padding: 8px 0;
    }
</style>
