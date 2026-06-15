<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";

    let ok = false;
    let arch = "";
    let timer = 0;

    async function refresh() {
        try { ok = await invoke<boolean>("diag_check"); }
        catch { ok = false; }
    }
    onMount(async () => {
        try { arch = await invoke<string>("get_arch"); } catch { arch = "?"; }
        refresh();
        timer = window.setInterval(refresh, 1000);
    });
    onDestroy(() => clearInterval(timer));
</script>

<div class="grid">
    <div class="col">
        <Section title="Status" icon="terminal" active={ok}>
            <div class="line" class:ok class:fail={!ok}>
                {ok ? "✓ Offset Updated" : "✗ Offset out of date"}
            </div>
            <div class="sub">build: <span class="arch">{arch || "…"}</span></div>
        </Section>
    </div>
    <div class="col"></div>
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
    .line {
        font-size: 11px;
        font-weight: 600;
        font-family: ui-monospace, Menlo, monospace;
    }
    .ok { color: #6fdb8c; }
    .fail { color: #ff6e6e; }
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
