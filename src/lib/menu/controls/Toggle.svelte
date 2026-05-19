<script lang="ts">
    export let label: string;
    export let checked: boolean = false;
    export let hint: string | undefined = undefined;
</script>

<label class="toggle">
    <input type="checkbox" bind:checked />
    <span class="box" class:on={checked}></span>
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
</label>

<style>
    .toggle {
        display: flex;
        align-items: center;
        gap: 8px;
        cursor: pointer;
        user-select: none;
        padding: 2px 0;
    }
    input {
        position: absolute;
        opacity: 0;
        pointer-events: none;
    }
    .box {
        width: 16px;
        height: 16px;
        /* Slight square-rounded shape — not a full circle, max 3-4px of corner */
        border-radius: 4px;
        border: 1.5px solid var(--accent-off);
        background: var(--bg-input);
        flex-shrink: 0;
        transition:
            background 180ms var(--ease),
            border-color 180ms var(--ease),
            box-shadow 180ms var(--ease),
            transform 120ms var(--ease);
    }
    .box.on {
        background: var(--accent-on);
        border-color: var(--accent-on);
        box-shadow: 0 0 8px var(--accent-glow), inset 0 0 6px rgba(255, 255, 255, 0.15);
    }
    .toggle:active .box {
        transform: scale(0.92);
    }
    .label {
        font-size: 11px;
        color: var(--text);
    }
    .hint {
        margin-left: auto;
        font-size: 10px;
        color: var(--text-muted);
        font-variant-numeric: tabular-nums;
    }
</style>
