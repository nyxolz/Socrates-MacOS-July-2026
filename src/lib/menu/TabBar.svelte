<script lang="ts">
    import { activeTab, TABS, type TabId } from "./stores";

    function set(t: TabId) {
        activeTab.set(t);
    }
</script>

<div class="tabs">
    {#each TABS as t}
        <button
            class="tab"
            class:active={$activeTab === t}
            on:click={() => set(t)}
            title={t}
        >
            <span class="label">{t}</span>
        </button>
    {/each}
</div>

<style>
    .tabs {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        padding: 8px;
        background: var(--bg-tab);
        border-bottom: 1px solid var(--border);
    }
    .tab {
        /* Full-pill buttons: 14 px ≈ half the tab height. */
        flex: 0 1 auto;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: 14px;
        color: var(--text-muted);
        font: inherit;
        font-size: 11px;
        font-weight: 500;
        padding: 5px 13px;
        cursor: pointer;
        white-space: nowrap;
        transition:
            background 140ms var(--ease),
            border-color 140ms var(--ease),
            color 140ms var(--ease);
    }
    .tab:hover {
        background: var(--bg-tab-hover);
        border-color: rgba(255, 222, 157, 0.30);
        color: var(--text);
    }
    .tab.active {
        background: rgba(255, 222, 157, 0.10);
        border-color: var(--accent);
        color: var(--accent);
    }
</style>
