<script lang="ts">
    export let label: string;
    export let danger: boolean = false;
    export let onclick: () => void = () => {};
</script>

<button class:danger on:click={onclick}>{label}</button>

<style>
    button {
        background: var(--bg-input);
        border: 3px solid var(--border);
        border-radius: 10px;
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 6px 8px;
        cursor: pointer;
        text-align: center;
    }
    button:hover {
        background: var(--bg-tab-active);
    }
    button {
        transition: background 140ms var(--ease), border-color 140ms var(--ease);
    }
    .danger {
        color: var(--accent);
        border-color: rgba(255, 222, 157, 0.35);
    }
    .danger:hover {
        background: rgba(255, 222, 157, 0.08);
    }
</style>
