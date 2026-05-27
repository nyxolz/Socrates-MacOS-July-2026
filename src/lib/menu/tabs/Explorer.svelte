<script lang="ts">
    import { onMount } from "svelte";
    import { get } from "svelte/store";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import { explorerReveal } from "../stores";
    import TreeNode, { type Node } from "./TreeNode.svelte";

    let root: Node | null = null;
    let error = "";
    let loading = false;
    // Address the Player List asked us to reveal. Consumed (and the store
    // cleared) on mount so navigating back to the Explorer later doesn't
    // re-trigger an old jump. Passed down the tree; the matching node expands
    // its ancestors, scrolls into view and flashes.
    let reveal: string | null = null;

    async function refresh() {
        loading = true;
        error = "";
        try {
            root = await invoke<Node>("get_instance_tree", { reveal });
        } catch (e) {
            error = String(e);
            root = null;
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        reveal = get(explorerReveal);
        explorerReveal.set(null);
        refresh();
    });
</script>

<div class="stack">
    <Section title="DataModel" icon="package" active={!!root}>
        <div class="bar">
            <button class="refresh" on:click={refresh} disabled={loading}>
                {loading ? "loading..." : "refresh"}
            </button>
            {#if error}<span class="err">{error}</span>{/if}
        </div>
        <div class="tree">
            {#if root}
                <TreeNode node={root} {reveal} />
            {:else if !loading && !error}
                <div class="empty">no datamodel yet</div>
            {/if}
        </div>
    </Section>
</div>

<style>
    .stack { padding: 6px; }
    .bar {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 6px;
    }
    .refresh {
        font: inherit;
        font-size: 10px;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: 4px;
        color: var(--text);
        padding: 3px 8px;
        cursor: pointer;
    }
    .refresh:hover { border-color: var(--accent); color: var(--accent); }
    .refresh:disabled { opacity: 0.5; cursor: wait; }
    .err {
        font-size: 10px;
        color: #ff8a8a;
    }
    .tree {
        max-height: 60vh;
        overflow: auto;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: 4px;
        padding: 4px;
    }
    .tree::-webkit-scrollbar { width: 6px; }
    .tree::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
    .empty {
        font-size: 10px;
        color: var(--text-muted);
        text-align: center;
        padding: 10px 0;
    }
</style>
