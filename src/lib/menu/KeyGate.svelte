<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { openUrl } from "@tauri-apps/plugin-opener";
    import { saveKey } from "./stores";

    type KeyResult = {
        valid: boolean;
        expires_after: number;
        network_error: boolean;
        version_blocked: boolean;
        latest_version: string;
    };

    const BUY_LIFETIME_URL = "https://cherrylovers.mykomerza.com/product?id=socrate-lifetime";

    let token = "";
    let busy = false;
    let error = "";

    function buyLifetime() {
        void openUrl(BUY_LIFETIME_URL);
    }

    async function activate() {
        error = "";
        const t = token.trim();
        if (!t) { error = "Enter your key."; return; }
        busy = true;
        try {
            const res = await invoke<KeyResult>("validate_keyauth_key", { token: t });
            if (res.network_error) {
                error = "Couldn't reach the key server — try again.";
            } else if (res.version_blocked) {
                // Disabled build — the backend shows the message and quits.
                await invoke("report_version_blocked", { latest: res.latest_version });
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
        <div class="sub">Paste your KeyAuth license key below to activate.</div>

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

        <div class="or">or</div>
        <button class="btn lifetime" on:click={buyLifetime}>
            <span class="lt-main">Buy lifetime</span>
            <span class="lt-price">$5.00</span>
        </button>
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
    .or {
        text-align: center;
        font-size: 10px;
        color: var(--text-muted);
        margin: 4px 0 0 0;
        text-transform: uppercase;
        letter-spacing: 1px;
    }
    .btn.lifetime {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: #fff;
        font-weight: 700;
        border: 0;
        background: linear-gradient(135deg, #ffae3d, #ff7a1f, #ff4a2a, #ff3d6e, #d81b60);
        background-size: 200% 200%;
        background-position: 0% 0%;
        box-shadow: 0 0 14px rgba(255, 122, 31, 0.40);
        transition: background-position 360ms var(--ease), box-shadow 220ms var(--ease), transform 120ms var(--ease);
    }
    .btn.lifetime:hover {
        background-position: 100% 100%;
        box-shadow: 0 0 22px rgba(255, 61, 110, 0.55);
        transform: translateY(-1px);
    }
    .lt-main { font-size: 12px; }
    .lt-price {
        font-variant-numeric: tabular-nums;
        font-size: 11px;
        padding: 2px 7px;
        border-radius: 4px;
        background: rgba(0, 0, 0, 0.25);
    }
</style>
