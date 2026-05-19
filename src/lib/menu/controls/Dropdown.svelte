<script lang="ts">
    type T = $$Generic<string>;
    export let label: string | undefined = undefined;
    export let value: T;
    export let options: readonly T[];
</script>

<div class="dropdown">
    {#if label}<span class="label">{label}</span>{/if}
    <div class="select">
        <select bind:value>
            {#each options as opt}
                <option value={opt}>{opt}</option>
            {/each}
        </select>
        <span class="caret">▾</span>
    </div>
</div>

<style>
    .dropdown {
        display: flex;
        flex-direction: column;
        gap: 3px;
        padding: 2px 0;
    }
    .label {
        font-size: 11px;
        color: var(--text);
    }
    .select {
        position: relative;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
    }
    select {
        width: 100%;
        appearance: none;
        background: transparent;
        border: 0;
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 22px 5px 8px;
        outline: none;
        cursor: pointer;
    }
    .caret {
        position: absolute;
        right: 6px;
        top: 50%;
        transform: translateY(-50%);
        color: var(--text-muted);
        pointer-events: none;
        font-size: 10px;
    }
    option {
        background: var(--bg-window);
        color: var(--text);
    }
</style>
