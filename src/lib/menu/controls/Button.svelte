<script lang="ts">
    import Icon from "./Icon.svelte";

    export let label: string = "";
    export let icon: string | undefined = undefined;
    export let danger: boolean = false;
    export let primary: boolean = false;
    export let onclick: () => void = () => {};
</script>

<button class:danger class:primary class:icon-only={!label} on:click={onclick}>
    {#if icon}<Icon name={icon} size={12} />{/if}
    {#if label}<span>{label}</span>{/if}
</button>

<style>
    button {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 6px;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 6px 8px;
        cursor: pointer;
        text-align: center;
        transition: background 140ms var(--ease), border-color 140ms var(--ease), color 140ms var(--ease);
    }
    button:hover {
        background: var(--bg-tab-active);
        border-color: var(--accent-off);
    }
    .icon-only {
        padding: 6px;
    }
    .danger {
        color: #ff8a8a;
        border-color: rgba(255, 90, 90, 0.35);
    }
    .danger:hover {
        background: rgba(255, 90, 90, 0.08);
    }
    .primary {
        background: var(--text);
        border-color: var(--text);
        color: var(--bg-window);
        font-weight: 600;
    }
    .primary:hover {
        background: var(--text-muted);
        border-color: var(--text-muted);
    }
</style>
