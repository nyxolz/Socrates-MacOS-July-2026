<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";

    let children: string[] = [];
    let error = "";
    let timer = 0;

    let objectEsp = false;
    $: invoke("set_object_esp", { enabled: objectEsp }).catch(() => {});

    async function refresh() {
        try {
            children = await invoke<string[]>("dev_players_children");
            error = "";
        } catch (e) {
            children = [];
            error = String(e);
        }
    }

    onMount(() => {
        refresh();
        timer = window.setInterval(refresh, 1000);
    });
    onDestroy(() => {
        clearInterval(timer);
        // Don't leave the heavy object walk running once the tab is closed.
        invoke("set_object_esp", { enabled: false }).catch(() => {});
    });
</script>

<div class="wrap">
    <Section title="Players — children" icon="users" active={children.length > 0}>
        {#if error}
            <div class="err">{error}</div>
        {:else if !children.length}
            <div class="empty">no children (not attached?)</div>
        {:else}
            <div class="count">{children.length} child{children.length === 1 ? "" : "ren"}</div>
            {#each children as name, i (i)}
                <div class="row">
                    <span class="idx mono">{i}</span>
                    <span class="name">{name}</span>
                </div>
            {/each}
        {/if}
    </Section>

    <Section title="ESP all game objects" icon="package" active={objectEsp}>
        <Toggle bind:checked={objectEsp} label="ESP all game objects" />
        <div class="hint">3D box + name on every Workspace part (capped, heavy).</div>
    </Section>
</div>

<style>
    .wrap { padding: 6px; }
    .count {
        font-size: 9px;
        color: var(--text-muted);
        margin-bottom: 4px;
    }
    .row {
        display: flex;
        align-items: baseline;
        gap: 8px;
        padding: 3px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        margin-bottom: 3px;
    }
    .idx {
        font-size: 9px;
        color: var(--text-muted);
        min-width: 18px;
        text-align: right;
    }
    .name {
        font-size: 11px;
        font-weight: 600;
        color: var(--text);
        word-break: break-all;
    }
    .mono { font-variant-numeric: tabular-nums; }
    .err {
        font-size: 10px;
        color: #e88;
    }
    .empty {
        font-size: 11px;
        color: var(--text-muted);
        text-align: center;
        padding: 8px 0;
    }
    .hint {
        font-size: 9px;
        color: var(--text-muted);
        margin-top: 4px;
    }
</style>
