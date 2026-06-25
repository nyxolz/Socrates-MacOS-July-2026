<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { openUrl } from "@tauri-apps/plugin-opener";
    import { WORKINK_URL, saveKey } from "./stores";

    type KeyResult = { valid: boolean; expires_after: number; network_error: boolean };

    let token = "";
    let busy = false;
    let error = "";
    let copied = false;

    async function getKey() {
        try { await openUrl(WORKINK_URL); }
        catch { /* opener unavailable — ignore */ }
    }

    async function copyLink() {
        try {
            await navigator.clipboard.writeText(WORKINK_URL);
            copied = true;
            setTimeout(() => { copied = false; }, 1500);
        } catch { /* clipboard unavailable — ignore */ }
    }

    async function activate() {
        error = "";
        const t = token.trim();
        if (!t) { error = "Enter your key."; return; }
        busy = true;
        try {
            const res = await invoke<KeyResult>("validate_workink_key", { token: t });
            if (res.network_error) {
                error = "Couldn't reach work.ink — try again.";
            } else if (!res.valid) {
                error = "Invalid or already-used key.";
            } else {
                saveKey(t, res.expires_after);
            }
        } catch {
            error = "Validation failed — try again.";
        } finally {
            busy = false;
        }
    }

    function onKeydown(e: KeyboardEvent) {
        if (e.key === "Enter" && !busy) activate();
    }
</script>

<div class="keypanel">
    <div class="card">
        <div class="title">Activate Socrate</div>
        <div class="sub">Complete the steps to get your key, then paste it below.</div>

        <input
            class="field"
            type="text"
            placeholder="Paste your key…"
            bind:value={token}
            on:keydown={onKeydown}
            spellcheck="false"
            autocomplete="off"
        />

        {#if error}<div class="error">{error}</div>{/if}

        <button class="btn primary" on:click={activate} disabled={busy}>
            {busy ? "Checking…" : "Activate"}
        </button>
        <div class="btn-row">
            <button class="btn ghost" on:click={getKey}>Get a key</button>
            <button class="btn ghost" on:click={copyLink}>{copied ? "Copied!" : "Copy link"}</button>
        </div>
    </div>
</div>

<style>
    .keypanel {
        flex: 1;
        min-height: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        background: var(--bg-window);
    }
    .card {
        width: 300px;
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    .title { font-size: 16px; font-weight: 800; color: var(--text); }
    .sub { font-size: 11px; color: var(--text-muted); margin-bottom: 10px; line-height: 1.4; }
    .field {
        width: 100%;
        box-sizing: border-box;
        padding: 9px 11px;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font: inherit;
        font-size: 12px;
        outline: none;
        -webkit-user-select: text;
        user-select: text;
    }
    .field:focus { border-color: var(--accent); }
    .error {
        width: 100%;
        font-size: 10px;
        color: #ff6b6b;
        text-align: left;
    }
    .btn {
        width: 100%;
        padding: 9px;
        border-radius: var(--radius);
        border: 1px solid transparent;
        font: inherit;
        font-size: 12px;
        font-weight: 600;
        cursor: pointer;
        transition: background 140ms var(--ease), opacity 140ms var(--ease);
    }
    .btn.primary {
        background: var(--accent);
        color: #fff;
        margin-top: 4px;
    }
    .btn.primary:hover { opacity: 0.9; }
    .btn.primary:disabled { opacity: 0.5; cursor: default; }
    .btn-row {
        display: flex;
        gap: 6px;
    }
    .btn-row .btn { flex: 1; }
    .btn.ghost {
        background: transparent;
        border-color: var(--border);
        color: var(--text-muted);
    }
    .btn.ghost:hover { color: var(--text); background: var(--bg-tab-hover); }
</style>
