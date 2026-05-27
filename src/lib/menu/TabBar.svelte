<script lang="ts">
    import { activeTab, TABS, updateAvailable, type TabId } from "./stores";

    function set(t: TabId) {
        activeTab.set(t);
    }
</script>

<div class="tabs">
    {#each TABS as t}
        <button
            class="tab"
            class:active={$activeTab === t}
            class:alert={t === "Settings" && $updateAvailable}
            on:click={() => set(t)}
            title={t === "Settings" && $updateAvailable ? "Update available" : t}
        >
            <span class="label">{t}</span>
            {#if t === "Settings" && $updateAvailable}
                <span class="dot" aria-label="update available">i</span>
            {/if}
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
    .tab.alert {
        border-color: #ff5b5b;
        box-shadow: 0 0 6px rgba(255, 91, 91, 0.45);
    }
    .dot {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 12px;
        height: 12px;
        margin-left: 6px;
        border-radius: 50%;
        background: #ff5b5b;
        color: #fff;
        font-size: 9px;
        font-weight: 700;
        font-style: italic;
        line-height: 1;
    }
</style>
