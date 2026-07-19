<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import ColorPicker from "../controls/ColorPicker.svelte";
    import KeybindInput from "../controls/KeybindInput.svelte";
    import { localVersion, remoteVersion, lastVersionCheck, updateAvailable, menuOptions, theme, THEME_PRESETS, clearKey, keyTimeLeftMs } from "../stores";

    function logoutKey() {
        clearKey();
    }

    function formatKeyTime(ms: number): string {
        if (ms <= 0) return "—";
        const totalSec = Math.floor(ms / 1000);
        const h = Math.floor(totalSec / 3600);
        const m = Math.floor((totalSec % 3600) / 60);
        const s = totalSec % 60;
        if (h > 0) return `${h}h ${m}m`;
        if (m > 0) return `${m}m ${s}s`;
        return `${s}s`;
    }

    const UPDATE_CMD = "curl -fsSL usesocrate.xyz/luncher.sh | sh";
    let copied = false;
    let copyTimer = 0;

    async function copyCmd() {
        try {
            await navigator.clipboard.writeText(UPDATE_CMD);
            copied = true;
            clearTimeout(copyTimer);
            copyTimer = window.setTimeout(() => { copied = false; }, 1500);
        } catch {}
    }

    function pick(c: string) { theme.set(c); }

    // Status (was the "Output" tab)
    let diagOk = false;
    let arch = "";
    let diagTimer = 0;
    async function refreshDiag() {
        try { diagOk = await invoke<boolean>("diag_check"); }
        catch { diagOk = false; }
    }
    onMount(async () => {
        try { arch = await invoke<string>("get_arch"); } catch { arch = "?"; }
        refreshDiag();
        diagTimer = window.setInterval(refreshDiag, 1000);
    });
    onDestroy(() => clearInterval(diagTimer));
</script>

<div class="grid">
    <div class="cell">
    <Section title="Theme" icon="sliders" active>
        <div class="presets">
            {#each THEME_PRESETS as p}
                <button
                    class="preset"
                    class:active={$theme === p.value}
                    style="background: {p.value};"
                    title={p.name}
                    on:click={() => pick(p.value)}
                ></button>
            {/each}
        </div>
        <div class="row">
            <span class="lbl">Custom</span>
            <ColorPicker bind:value={$theme} />
        </div>
        <div class="row">
            <span class="lbl">Active</span>
            <span class="hex">{$theme}</span>
        </div>
    </Section>
    </div>

    <div class="cell">
    <Section title="Menu" icon="sliders" active>
        <KeybindInput label="Hide menu" noMouse bind:value={$menuOptions.toggle_bind} />
        <Toggle label="Show binds HUD" bind:checked={$menuOptions.show_binds} />
    </Section>
    </div>

    <div class="cell">
    <Section title="Status" icon="terminal" active={diagOk}>
        <div class="line" class:ok={diagOk} class:fail={!diagOk}>
            {diagOk ? "✓ Offset Updated" : "✗ Offset out of date"}
        </div>
        <div class="sub">build: <span class="arch">{arch || "…"}</span></div>
    </Section>
    <Section title="Key" icon="lock" active>
        <div class="row">
            <span class="lbl">Time left</span>
            <span class="val mono">{formatKeyTime($keyTimeLeftMs)}</span>
        </div>
        <div class="cmd-row">
            <button class="btn logout" on:click={logoutKey}>Disconnect key</button>
        </div>
        <div class="muted">Clears the saved key and returns to the unlock screen.</div>
    </Section>
    <Section title="Privacy" icon="eye" active={$menuOptions.stream_proof}>
        <div class="row">
            <span class="sq" class:sq-ok={$menuOptions.stream_proof} class:sq-off={!$menuOptions.stream_proof}></span>
            <span class="lbl">Stream proof</span>
            <span class="val small">{$menuOptions.stream_proof ? "On" : "Off"}</span>
        </div>
        <Toggle bind:checked={$menuOptions.stream_proof} label="Stream proof" />
        <div class="muted">
            Hides the overlay from screen capture &amp; sharing (OBS, QuickTime,
            Discord share). Doesn't hide it from your own eyes.
        </div>
    </Section>
    </div>

    <div class="cell">
    <Section title="Socrate Update" icon="download" active={$updateAvailable}>
        <div class="row">
            <span class="sq" class:sq-bad={$updateAvailable} class:sq-ok={!$updateAvailable}></span>
            <span class="lbl">Status</span>
            <span class="val small">{$updateAvailable ? "Update available" : "Up to date"}</span>
        </div>
        <div class="row">
            <span class="lbl">Installed</span>
            <span class="val mono">{$localVersion}</span>
        </div>
        <div class="row">
            <span class="lbl">Remote</span>
            <span class="val mono">{$remoteVersion}</span>
        </div>
        <div class="row">
            <span class="lbl">Last check</span>
            <span class="val mono small">{$lastVersionCheck ? new Date($lastVersionCheck).toLocaleTimeString() : "—"}</span>
        </div>

        {#if $updateAvailable}
            <div class="banner">A new version is available.</div>
            <ol class="steps">
                <li>Close Roblox.</li>
                <li>Close Socrate.</li>
                <li>Run this in Terminal:</li>
            </ol>
            <div class="cmd-row">
                <code class="cmd mono">{UPDATE_CMD}</code>
                <button class="btn" on:click={copyCmd}>{copied ? "Copied" : "Copy"}</button>
            </div>
        {:else if $remoteVersion === "offline"}
            <div class="muted">Couldn't reach usesocrate.xyz.</div>
        {:else if $localVersion !== "…" && $remoteVersion !== "…"}
            <div class="muted">You're on the latest build.</div>
        {/if}
    </Section>
    </div>
</div>

<style>
    .grid {
        padding: 6px;
        display: grid;
        grid-template-columns: 1fr 1fr;
        grid-auto-rows: auto;
        gap: 6px;
    }
    .cell { display: flex; flex-direction: column; }
    .row {
        display: flex;
        align-items: baseline;
        gap: 6px;
        padding: 2px 0;
    }
    .lbl {
        font-size: 10px;
        color: var(--text-muted);
        min-width: 70px;
    }
    .val {
        font-size: 11px;
        font-weight: 600;
        color: var(--accent);
        word-break: break-all;
    }
    .val.small { font-size: 10px; font-weight: 500; color: var(--text-muted); }
    .mono { font-variant-numeric: tabular-nums; }
    .banner {
        font-size: 11px;
        color: #ff8a8a;
        background: rgba(255, 91, 91, 0.10);
        border: 1px solid rgba(255, 91, 91, 0.35);
        border-radius: var(--radius-small);
        padding: 6px 8px;
        margin-top: 6px;
    }
    .steps {
        margin: 6px 0 4px 0;
        padding-left: 18px;
        font-size: 11px;
        color: var(--text);
        line-height: 1.5;
    }
    .steps li { padding: 1px 0; }
    .cmd-row {
        display: flex;
        align-items: center;
        gap: 6px;
        margin-top: 4px;
    }
    .cmd {
        flex: 1;
        font-size: 11px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        padding: 6px 8px;
        color: var(--accent);
        word-break: break-all;
        user-select: text;
        -webkit-user-select: text;
    }
    .btn {
        font: inherit;
        font-size: 11px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        padding: 5px 12px;
        cursor: pointer;
        transition: border-color 140ms var(--ease), color 140ms var(--ease);
    }
    .btn:hover { border-color: var(--accent); color: var(--accent); }
    .btn.logout { border-color: rgba(255, 91, 91, 0.55); color: #ff8a8a; }
    .btn.logout:hover { border-color: #ff5b5b; color: #ff5b5b; background: rgba(255, 91, 91, 0.08); }
    .muted {
        font-size: 10px;
        color: var(--text-muted);
        margin-top: 4px;
    }
    .presets {
        display: grid;
        grid-template-columns: repeat(6, 1fr);
        gap: 4px;
        margin-bottom: 6px;
    }
    .preset {
        height: 14px;
        border-radius: 3px;
        border: 1px solid var(--border);
        cursor: pointer;
        transition: transform 120ms var(--ease), border-color 120ms var(--ease);
        box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.4);
    }
    .preset:hover { transform: scale(1.1); }
    .preset.active {
        border-color: #ffffff;
        box-shadow: 0 0 0 2px rgba(255,255,255,0.15), inset 0 0 0 1px rgba(0,0,0,0.4);
    }
    .sq {
        width: 8px;
        height: 8px;
        flex-shrink: 0;
        border-radius: 2px;
        box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.4);
    }
    .sq-ok { background: #6fdb8c; }
    .sq-bad { background: #ff8a8a; }
    .sq-off { background: var(--text-dim); }
    .hex {
        margin-left: auto;
        font-family: ui-monospace, Menlo, monospace;
        font-size: 10px;
        color: var(--text-muted);
        text-transform: uppercase;
    }
    .line {
        font-size: 11px;
        font-weight: 600;
        font-family: ui-monospace, Menlo, monospace;
    }
    .line.ok { color: #6fdb8c; }
    .line.fail { color: #ff6e6e; }
    .sub {
        margin-top: 4px;
        font-size: 10px;
        color: var(--text-muted);
        font-family: ui-monospace, Menlo, monospace;
    }
    .arch {
        color: var(--text);
        font-weight: 600;
    }
</style>
