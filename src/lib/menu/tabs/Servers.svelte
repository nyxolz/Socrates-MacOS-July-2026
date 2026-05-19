<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";

    let placeName = "...";
    let timer = 0;

    async function refresh() {
        try { placeName = await invoke<string>("get_place_name"); }
        catch { placeName = "(not attached)"; }
    }
    onMount(() => { refresh(); timer = window.setInterval(refresh, 2000); });
    onDestroy(() => clearInterval(timer));
</script>

<div class="grid">
    <div class="col">
        <Section title="Server" icon="server" active>
            <div class="row">
                <span class="lbl">Place</span>
                <span class="val">{placeName}</span>
            </div>
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
    .row {
        display: flex;
        align-items: baseline;
        gap: 6px;
        padding: 2px 0;
    }
    .lbl { font-size: 10px; color: var(--text-muted); }
    .val {
        font-size: 11px;
        font-weight: 600;
        color: var(--accent);
        word-break: break-all;
    }
</style>
