<script lang="ts">
    import { scale } from "svelte/transition";
    import { toast } from "../toast";
    import Icon from "./Icon.svelte";

    export let label: string;
    export let checked: boolean = false;
    export let hint: string | undefined = undefined;

    // Fire a transient notification only on user-driven toggles (the DOM
    // `change` event doesn't fire for programmatic store updates, so loading a
    // config won't spam toasts).
    function onChange(e: Event) {
        const on = (e.currentTarget as HTMLInputElement).checked;
        toast(`${label}: ${on ? "ON" : "OFF"}`);
    }
</script>

<label class="toggle">
    <input type="checkbox" bind:checked on:change={onChange} />
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
    <span class="box" class:on={checked}>
        {#if checked}
            <span class="check" in:scale={{ duration: 140, start: 0.4, opacity: 0 }}>
                <Icon name="check" size={11} />
            </span>
        {/if}
    </span>
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
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 16px;
        height: 16px;
        margin-left: auto;
        border-radius: 4px;
        border: 1.5px solid var(--accent-off);
        background: var(--bg-input);
        color: #0a0a0d;
        flex-shrink: 0;
        transition:
            background 180ms var(--ease),
            border-color 180ms var(--ease),
            transform 120ms var(--ease);
    }
    .box.on {
        background: var(--accent-on);
        border-color: var(--accent-on);
    }
    .toggle:active .box {
        transform: scale(0.88);
    }
    .check {
        display: inline-flex;
        align-items: center;
        justify-content: center;
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
