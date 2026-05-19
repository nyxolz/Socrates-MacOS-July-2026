<script lang="ts">
    export let label: string | undefined = undefined;
    export let value: string = "";
    export let placeholder: string = "";
</script>

<div class="input">
    {#if label}<span class="label">{label}</span>{/if}
    <input type="text" bind:value {placeholder} />
</div>

<style>
    .input {
        display: flex;
        flex-direction: column;
        gap: 3px;
        padding: 2px 0;
    }
    .label {
        font-size: 11px;
        color: var(--text);
    }
    input {
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 8px;
        outline: none;
    }
    input:focus {
        border-color: var(--accent);
    }
</style>
