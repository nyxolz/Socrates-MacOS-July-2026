<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { scale } from "svelte/transition";
    import Button from "../controls/Button.svelte";
    import Icon from "../controls/Icon.svelte";
    import {
        exportCurrentConfig,
        savedConfigs, addSavedConfig, deleteSavedConfig, renameSavedConfig, loadSavedConfig,
        formatDate,
        type SavedConfig,
    } from "../stores";
    import { toast } from "../toast";

    let status = "";
    let showAddPanel = false;
    let newConfigName = "";
    let newConfigContent = "";
    let renamingName: string | null = null;
    let renameValue = "";
    let view: "configs" | "browse" = "configs";
    let sortDesc = true;
    let fileInput: HTMLInputElement;

    const PIXEL_COLS = 14;
    const PIXEL_ROWS = 7;
    const PIXEL_COLORS = ["#87878f", "#a8a8b0", "#d4d4d8", "#e8e8ec", "#ffffff"];
    const PIXEL_GRID = Array.from({ length: PIXEL_COLS * PIXEL_ROWS }, () => ({
        c: PIXEL_COLORS[Math.floor(Math.random() * PIXEL_COLORS.length)],
        d: Math.random() * 3,
        dur: 0.6 + Math.random() * 1.8,
    }));

    function parseDate(s: string): number {
        const [d, m, y] = s.split(".").map(Number);
        return new Date(y || 0, (m || 1) - 1, d || 1).getTime();
    }

    $: sortedConfigs = [...$savedConfigs].sort((a, b) =>
        sortDesc ? parseDate(b.createdAt) - parseDate(a.createdAt) : parseDate(a.createdAt) - parseDate(b.createdAt)
    );

    function confirmAdd() {
        const n = newConfigName.trim();
        if (!n) { status = "enter a config name"; return; }
        if (!newConfigContent.trim()) { status = "paste config text first"; return; }
        addSavedConfig(n, newConfigContent.trim());
        status = `saved "${n}"`;
        toast(`Config saved: ${n}`);
        newConfigName = "";
        newConfigContent = "";
        showAddPanel = false;
    }

    function doLoad(cfg: SavedConfig) {
        const r = loadSavedConfig(cfg.content);
        status = r.ok ? `loaded "${cfg.name}"` : `load failed: ${r.reason ?? "unknown"}`;
        toast(r.ok ? `Config loaded: ${cfg.name}` : "Config load failed");
    }

    function doDelete(cfg: SavedConfig) {
        deleteSavedConfig(cfg.name);
        toast(`Deleted: ${cfg.name}`);
    }

    function toggleRename(cfg: SavedConfig) {
        if (renamingName === cfg.name) {
            renamingName = null;
        } else {
            renamingName = cfg.name;
            renameValue = cfg.name;
        }
    }

    function confirmRename(cfg: SavedConfig) {
        const n = renameValue.trim();
        if (!n) return;
        renameSavedConfig(cfg.name, n);
        renamingName = null;
    }

    async function doExport() {
        const cfg = exportCurrentConfig();
        const content = JSON.stringify(cfg, null, 2);
        try {
            const path = await invoke<string>("export_config_to_desktop", {
                name: "socrate-config",
                content,
            });
            status = `saved → ${path}`;
            toast("Config exported");
        } catch (e) {
            status = `export failed: ${e}`;
            toast("Export failed");
        }
    }

    async function doCopy() {
        const cfg = exportCurrentConfig();
        const content = JSON.stringify(cfg, null, 2);
        try {
            await navigator.clipboard.writeText(content);
            status = "config copied to clipboard";
            toast("Config copied");
        } catch (e) {
            status = `copy failed: ${e}`;
            toast("Copy failed");
        }
    }

    function pickImport() {
        fileInput?.click();
    }

    async function onFile(e: Event) {
        const target = e.target as HTMLInputElement;
        const f = target.files?.[0];
        if (!f) return;
        try {
            const text = await f.text();
            const n = f.name.replace(/\.json$/i, "");
            addSavedConfig(n, text);
            status = `imported "${n}"`;
            toast(`Config imported: ${n}`);
            view = "configs";
        } catch (err) {
            status = `import failed: ${err}`;
            toast("Import failed");
        } finally {
            target.value = "";
        }
    }
</script>

<div class="page">
    <div class="toprow">
        <div class="tabs">
            <button class="tab" class:active={view === "configs"} on:click={() => (view = "configs")}>Configs</button>
            <button class="tab" class:active={view === "browse"} on:click={() => (view = "browse")}>Browse</button>
        </div>
        <div class="actions">
            <Button label="Copy my configs" icon="copy" onclick={doCopy} />
            <Button label="By created date" icon="calendar" onclick={() => (sortDesc = !sortDesc)} />
            <Button label="Create new" icon="plus" primary onclick={() => (showAddPanel = !showAddPanel)} />
        </div>
    </div>

    {#if view === "configs"}
        <div class="grid3">
            {#if sortedConfigs.length === 0}
                <div class="hint">No saved configs yet.</div>
            {/if}
            {#each sortedConfigs as cfg (cfg.name)}
                <div class="config-card">
                    <span class="pixels">
                        {#each PIXEL_GRID as p, idx (idx)}
                            <span class="px" style="--c: {p.c}; --d: {p.d}s; --dur: {p.dur}s;"></span>
                        {/each}
                        <span class="fade"></span>
                    </span>
                    <div class="card-top">
                        <span class="file-icon"><Icon name="file" size={16} /></span>
                        <div class="card-info">
                            <span class="card-name">{cfg.name}</span>
                            <span class="card-date">created {cfg.createdAt}</span>
                        </div>
                    </div>
                    <div class="card-actions">
                        <Button icon="load" onclick={() => doLoad(cfg)} />
                        <Button icon="trash" danger onclick={() => doDelete(cfg)} />
                        <Button icon="edit" onclick={() => toggleRename(cfg)} />
                    </div>
                    {#if renamingName === cfg.name}
                        <div class="rename-box">
                            <input class="txt" type="text" bind:value={renameValue} placeholder="New name" />
                            <Button label="Rename" icon="edit" onclick={() => confirmRename(cfg)} />
                        </div>
                    {/if}
                </div>
            {/each}
        </div>
    {:else}
        <div class="browse">
            <Button label="Browse files" icon="file" primary onclick={pickImport} />
            <input
                bind:this={fileInput}
                type="file"
                accept=".json,application/json"
                style="display:none"
                on:change={onFile}
            />
            <div class="hint">Pick a Socrate config (.json) to add it to your saved configs.</div>
        </div>
    {/if}

    {#if status}
        <div class="status">{status}</div>
    {/if}

    {#if showAddPanel}
        <div class="add-panel-wrap">
            <div class="add-panel" transition:scale={{ duration: 160, start: 0.92, opacity: 0 }}>
                <div class="add-header">
                    <div class="add-title">Add Configs</div>
                    <Button icon="x" onclick={() => (showAddPanel = false)} />
                </div>
                <input class="txt" type="text" bind:value={newConfigName} placeholder="Config name" />
                <textarea
                    class="paste"
                    bind:value={newConfigContent}
                    spellcheck="false"
                    placeholder="Paste a Socrate config (JSON) here…"
                ></textarea>
                <Button icon="plus" primary onclick={confirmAdd} />
            </div>
        </div>
    {/if}
</div>

<style>
    .page {
        position: relative;
        min-height: 100%;
        padding: 10px;
        box-sizing: border-box;
    }
    .toprow {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 10px;
        padding: 4px;
    }
    .tabs {
        display: flex;
        gap: 4px;
    }
    .tab {
        background: var(--bg-panel);
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        color: var(--text-muted);
        font: inherit;
        font-size: 12px;
        font-weight: 600;
        padding: 8px 14px;
        cursor: pointer;
        transition: color 140ms var(--ease), background 140ms var(--ease);
    }
    .tab:hover {
        color: var(--text);
    }
    .tab.active {
        color: var(--text);
        background: var(--bg-tab-active);
    }
    .actions {
        display: flex;
        gap: 8px;
        margin-left: auto;
    }
    .actions :global(button) {
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        font-weight: 600;
        font-size: 12px;
        padding: 8px 14px;
    }
    .add-panel-wrap {
        position: absolute;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        z-index: 10;
    }
    .add-panel {
        width: 280px;
        padding: 14px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        background: var(--bg-sidebar);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    }
    .add-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
    }
    .add-title {
        font-size: 12px;
        font-weight: 700;
        color: var(--text);
    }
    .add-panel :global(button) {
        align-self: flex-start;
        width: 28px;
        height: 28px;
        padding: 0;
        border-color: rgba(255, 255, 255, 0.16);
    }
    .grid3 {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 8px;
    }
    .config-card {
        position: relative;
        box-sizing: border-box;
        padding: 10px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        background: var(--bg-sidebar);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
    }
    .pixels {
        position: absolute;
        inset: 0;
        left: 50%;
        display: grid;
        grid-template-columns: repeat(14, 1fr);
        grid-template-rows: repeat(7, 1fr);
        gap: 1px;
        opacity: 0;
        pointer-events: none;
        z-index: 0;
        overflow: hidden;
        border-top-right-radius: var(--radius);
        border-bottom-right-radius: var(--radius);
        transition: opacity 200ms var(--ease);
    }
    .config-card:hover .pixels {
        opacity: 0.5;
    }
    .px {
        background: var(--c);
        border-radius: 0.5px;
        opacity: 0.12;
        transform: skewX(-20deg);
        animation: px-glitch var(--dur) ease-in-out var(--d) infinite;
    }
    @keyframes px-glitch {
        0%, 100% { opacity: 0.12; }
        50% { opacity: 0.65; }
    }
    .fade {
        position: absolute;
        inset: 0;
        background: linear-gradient(to right, var(--bg-sidebar) 0%, transparent 55%);
        pointer-events: none;
    }
    .card-top {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: center;
        gap: 8px;
        min-width: 0;
    }
    .file-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        flex-shrink: 0;
        background: #0e0e12;
        border-radius: var(--radius-small);
        color: var(--text);
    }
    .card-info {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
    }
    .card-name {
        font-size: 11px;
        font-weight: 600;
        color: var(--text);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .card-date {
        font-size: 9px;
        color: var(--text-muted);
    }
    .card-actions {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: center;
        gap: 4px;
    }
    .card-actions :global(button) {
        width: 26px;
        height: 26px;
        padding: 0;
        border-color: rgba(255, 255, 255, 0.16);
    }
    .browse {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: 8px;
    }
    .browse :global(button) {
        border-color: rgba(255, 255, 255, 0.16);
    }
    .rename-box {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .txt {
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 8px;
        outline: none;
    }
    .txt:focus { border-color: var(--accent); }
    .paste {
        width: 100%;
        box-sizing: border-box;
        min-height: 100px;
        resize: vertical;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font-family: ui-monospace, Menlo, monospace;
        font-size: 10px;
        line-height: 1.4;
        padding: 6px 8px;
        outline: none;
    }
    .paste:focus { border-color: var(--accent); }
    .hint {
        font-size: 10px;
        color: var(--text-muted);
        line-height: 1.4;
    }
    .status {
        font-size: 10px;
        color: var(--accent);
        margin-top: 6px;
        font-family: ui-monospace, Menlo, monospace;
        word-break: break-all;
    }
</style>
