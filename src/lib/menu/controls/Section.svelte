<script lang="ts">
    import Icon from "./Icon.svelte";

    export let title: string;
    export let active: boolean = false;
    export let icon: string | undefined = undefined;
</script>

<div class="section">
    <div class="header" class:active>
        {#if icon}
            <span class="ic"><Icon name={icon} size={13} /></span>
        {:else}
            <span class="dot"></span>
        {/if}
        <span class="title">{title}</span>
    </div>
    <div class="body">
        <slot />
    </div>
</div>

<style>
    .section {
        background: var(--bg-sidebar);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius);
        margin-bottom: 8px;
        overflow: hidden;
    }
    .header {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 9px 12px;
        border-bottom: 1px solid var(--border-soft);
        color: var(--text);
    }
    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--text-dim);
    }
    .ic {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        border-radius: var(--radius-small);
        background: var(--bg-input);
        color: var(--text-muted);
    }
    .header {
        transition: color 160ms var(--ease);
    }
    .active .ic {
        color: var(--accent);
    }
    .active .dot {
        background: var(--accent);
    }
    .ic, .title {
        transition: color 160ms var(--ease);
    }
    .title {
        font-weight: 600;
        font-size: 11.5px;
        letter-spacing: 0.2px;
        color: var(--text);
    }
    .body {
        padding: 9px 12px 11px 12px;
        display: flex;
        flex-direction: column;
        gap: 7px;
    }
</style>
