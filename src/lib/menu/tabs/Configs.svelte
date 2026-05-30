<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import Button from "../controls/Button.svelte";
    import { exportCurrentConfig, applyImportedConfig } from "../stores";

    let name = "socrate-config";
    let status = "";
    let fileInput: HTMLInputElement;

    async function doExport() {
        const cfg = exportCurrentConfig();
        const content = JSON.stringify(cfg, null, 2);
        try {
            const path = await invoke<string>("export_config_to_desktop", {
                name,
                content,
            });
            status = `saved → ${path}`;
        } catch (e) {
            status = `export failed: ${e}`;
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
            const parsed = JSON.parse(text);
            const r = applyImportedConfig(parsed);
            status = r.ok ? `imported ${f.name}` : `import failed: ${r.reason ?? "unknown"}`;
        } catch (err) {
            status = `import failed: ${err}`;
        } finally {
            target.value = "";
        }
    }
</script>

<div class="grid">
    <div class="col">
        <Section title="Export" icon="terminal" active={!!name}>
            <div class="field">
                <label for="cfgname" class="lab">Filename</label>
                <input id="cfgname" class="txt" type="text" bind:value={name} placeholder="socrate-config" />
            </div>
            <Button label="Export to Desktop" onclick={doExport} />
            <div class="hint">Writes to <code>~/Desktop/&lt;name&gt;.json</code>.</div>
        </Section>
    </div>
    <div class="col">
        <Section title="Import" icon="terminal" active={false}>
            <Button label="Import .json" onclick={pickImport} />
            <input
                bind:this={fileInput}
                type="file"
                accept=".json,application/json"
                style="display:none"
                on:change={onFile}
            />
            <div class="hint">Replaces aimbot, visuals, menu options and theme with the file contents.</div>
        </Section>
    </div>
    {#if status}
        <div class="status">{status}</div>
    {/if}
</div>

<style>
    .grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 6px;
        padding: 6px;
        align-items: start;
    }
    .col { display: flex; flex-direction: column; min-width: 0; gap: 6px; }
    .field { display: flex; flex-direction: column; gap: 3px; margin-bottom: 6px; }
    .lab {
        font-size: 11px;
        color: var(--text);
    }
    .txt {
        background: var(--bg-input);
        border: 3px solid var(--border);
        border-radius: 10px;
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 8px;
        outline: none;
    }
    .txt:focus { border-color: var(--accent); }
    .hint {
        font-size: 10px;
        color: var(--text-muted);
        margin-top: 4px;
        line-height: 1.4;
    }
    .hint code {
        font-family: ui-monospace, Menlo, monospace;
        font-size: 10px;
        background: var(--bg-input);
        padding: 1px 4px;
        border-radius: 3px;
    }
    .status {
        grid-column: 1 / -1;
        font-size: 10px;
        color: var(--accent);
        margin-top: 2px;
        padding: 0 4px;
        font-family: ui-monospace, Menlo, monospace;
        word-break: break-all;
    }
</style>
