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
        background: var(--bg-panel);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius);
        margin-bottom: 7px;
    }
    .header {
        display: flex;
        align-items: center;
        gap: 7px;
        padding: 7px 10px;
        border-bottom: 1px solid var(--border-soft);
        color: var(--text-muted);
    }
    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--text-dim);
    }
    .ic {
        display: inline-flex;
        color: var(--text-muted);
    }
    .header {
        transition: color 160ms var(--ease);
    }
    .active .ic,
    .active .title {
        color: var(--accent);
    }
    .active .dot {
        background: var(--accent);
        box-shadow: 0 0 5px var(--accent-glow);
    }
    .ic, .title {
        transition: color 160ms var(--ease);
    }
    .title {
        font-weight: 600;
        font-size: 11px;
        letter-spacing: 0.2px;
        color: var(--text);
    }
    .body {
        padding: 8px 10px 10px 10px;
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
</style>
