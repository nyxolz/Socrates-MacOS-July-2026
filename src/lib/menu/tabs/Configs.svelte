<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import Button from "../controls/Button.svelte";
    import { exportCurrentConfig, applyImportedConfig, resetToDefaults } from "../stores";
    import { toast } from "../toast";

    function doReset() {
        resetToDefaults();
        status = "reset to defaults";
        toast("Reset to default");
    }

    let name = "socrate-config";
    let status = "";
    let fileInput: HTMLInputElement;
    let pasteText = "";

    function applyPasted() {
        const text = pasteText.trim();
        if (!text) { status = "paste a config first"; return; }
        try {
            const parsed = JSON.parse(text);
            const r = applyImportedConfig(parsed);
            status = r.ok ? "config applied" : `import failed: ${r.reason ?? "unknown"}`;
            toast(r.ok ? "Config loaded" : "Config load failed");
        } catch (err) {
            status = `import failed: ${err}`;
            toast("Config load failed");
        }
    }

    function copyCurrent() {
        const content = JSON.stringify(exportCurrentConfig(), null, 2);
        pasteText = content;
        try {
            navigator.clipboard?.writeText(content);
            status = "current config copied to clipboard";
            toast("Config copied");
        } catch {
            status = "current config loaded below — select & copy";
        }
    }

    async function doExport() {
        const cfg = exportCurrentConfig();
        const content = JSON.stringify(cfg, null, 2);
        try {
            const path = await invoke<string>("export_config_to_desktop", {
                name,
                content,
            });
            status = `saved → ${path}`;
            toast("Config exported");
        } catch (e) {
            status = `export failed: ${e}`;
            toast("Export failed");
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
            toast(r.ok ? `Config loaded: ${f.name}` : "Config load failed");
        } catch (err) {
            status = `import failed: ${err}`;
            toast("Config load failed");
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
        <Section title="Reset" icon="terminal" active={false}>
            <Button label="Reset to Default" onclick={doReset} />
            <div class="hint">Restores aimbot, visuals, menu options and theme to factory defaults.</div>
        </Section>
    </div>
    <div class="col wide">
        <Section title="Paste config" icon="terminal" active={!!pasteText.trim()}>
            <textarea
                class="paste"
                bind:value={pasteText}
                spellcheck="false"
                placeholder={'Paste a Socrate config (JSON) here…'}
            ></textarea>
            <div class="btnrow">
                <Button label="Apply pasted" onclick={applyPasted} />
                <Button label="Copy current" onclick={copyCurrent} />
            </div>
            <div class="hint">
                Paste a shared config and hit <code>Apply pasted</code>, or hit
                <code>Copy current</code> to dump yours here (also copied to clipboard) to share.
            </div>
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
    .wide { grid-column: 1 / -1; }
    .paste {
        width: 100%;
        box-sizing: border-box;
        min-height: 130px;
        resize: vertical;
        background: var(--bg-input);
        border: 3px solid var(--border);
        border-radius: 10px;
        color: var(--text);
        font-family: ui-monospace, Menlo, monospace;
        font-size: 10px;
        line-height: 1.4;
        padding: 6px 8px;
        outline: none;
    }
    .paste:focus { border-color: var(--accent); }
    .btnrow { display: flex; gap: 6px; margin-top: 6px; }
    .btnrow :global(button) { flex: 1; }
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
