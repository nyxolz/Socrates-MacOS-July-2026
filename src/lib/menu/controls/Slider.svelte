<script lang="ts">
    export let label: string;
    export let value: number;
    export let min: number = 0;
    export let max: number = 100;
    export let step: number = 1;
    export let format: (v: number) => string = (v) => v.toFixed(0);

    $: pct = max === min ? 0 : ((value - min) / (max - min)) * 100;

    const KNOB_PX = 10;
</script>

<div class="slider">
    <div class="row">
        <span class="label">{label}</span>
        <span class="value">{format(value)}/{format(max)}</span>
    </div>
    <div class="track" style="--pct: {pct}%; --knob: {KNOB_PX}px;">
        <div class="fill"></div>
        <div class="knob"></div>
        <input type="range" {min} {max} {step} bind:value />
    </div>
</div>

<style>
    .slider {
        display: flex;
        flex-direction: column;
        gap: 5px;
        padding: 3px 0;
    }
    .row {
        display: flex;
        justify-content: space-between;
        align-items: baseline;
    }
    .label {
        font-size: 11px;
        color: var(--text);
    }
    .value {
        font-size: 10px;
        color: var(--text-muted);
        font-variant-numeric: tabular-nums;
    }
    .track {
        position: relative;
        height: 5px;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: 3px;
        overflow: visible;
    }
    /* Flat fill — small transition keeps drag feel responsive while still
       smoothing out programmatic value changes (config loads etc). */
    .fill {
        position: absolute;
        top: -1px; left: -1px; bottom: -1px;
        width: calc((100% - var(--knob)) * var(--pct) / 100% + var(--knob) / 2);
        max-width: 100%;
        background: var(--accent);
        border-radius: 3px;
        transition: width 80ms var(--ease);
    }
    /* Small flat knob, centred on the fill edge. */
    .knob {
        position: absolute;
        top: 50%;
        width: var(--knob);
        height: var(--knob);
        left: calc((100% - var(--knob)) * var(--pct) / 100%);
        transform: translateY(-50%);
        background: #ffffff;
        border-radius: 50%;
        pointer-events: none;
        transition: left 80ms var(--ease);
        box-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
    }
    input {
        position: absolute;
        top: -4px;
        left: 0;
        width: 100%;
        height: 24px;
        margin: 0;
        opacity: 0;
        cursor: pointer;
    }
</style>
