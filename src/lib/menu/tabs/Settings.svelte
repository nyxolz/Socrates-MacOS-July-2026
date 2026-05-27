<script lang="ts">
    import Section from "../controls/Section.svelte";
    import { localVersion, remoteVersion, lastVersionCheck, updateAvailable } from "../stores";

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
</script>

<div class="grid">
    <Section title="Socrate Update" icon="download" active={$updateAvailable}>
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

<style>
    .grid { padding: 6px; }
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
    .muted {
        font-size: 10px;
        color: var(--text-muted);
        margin-top: 4px;
    }
</style>
