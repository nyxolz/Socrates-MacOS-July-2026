<script lang="ts">
    export let label: string;
    export let value: number;
    export let min: number = 0;
    export let max: number = 100;
    export let step: number = 1;
    export let format: (v: number) => string = (v) => v.toFixed(0);

    $: pct = max === min ? 0 : ((value - min) / (max - min)) * 100;

    const KNOB_PX = 12;
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
        height: 16px;
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: 8px;
        overflow: hidden;
    }
    /* Fill width = knob's right edge + 2 px. The yellow band runs past the
       centre of the knob and visually wraps the right side of the white ball.
       No transition — the user wants the drag to feel instant. */
    .fill {
        position: absolute;
        top: 0; left: 0; bottom: 0;
        width: calc((100% - var(--knob)) * var(--pct) / 100% + var(--knob) + 2px);
        max-width: 100%;
        background: linear-gradient(90deg, var(--accent-soft) 0%, var(--accent) 100%);
        border-radius: 8px;
        box-shadow: inset 0 1px 2px rgba(255, 255, 255, 0.08);
    }
    /* Knob centre travels from knob_w/2 to (W - knob_w/2) so the whole ball
       stays inside the track. */
    .knob {
        position: absolute;
        top: 50%;
        width: var(--knob);
        height: var(--knob);
        left: calc((100% - var(--knob)) * var(--pct) / 100%);
        transform: translateY(-50%);
        background: #ffffff;
        border-radius: 50%;
        box-shadow:
            0 0 5px var(--accent-glow),
            0 1px 2px rgba(0, 0, 0, 0.45),
            inset 0 -1px 1px rgba(0, 0, 0, 0.08);
        pointer-events: none;
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
