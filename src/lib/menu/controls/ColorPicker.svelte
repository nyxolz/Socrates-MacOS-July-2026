<script lang="ts">
    export let value: string;
    export let title: string = "Pick colour";
</script>

<label class="swatch" style="background: {value}" {title}>
    <input type="color" bind:value />
</label>

<style>
    .swatch {
        display: inline-block;
        width: 24px;
        height: 18px;
        border: 1px solid var(--border);
        border-radius: 4px;
        cursor: pointer;
        margin-left: auto;
        position: relative;
        box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.4);
        flex-shrink: 0;
        transition: border-color 140ms var(--ease);
    }
    .swatch:hover { border-color: var(--accent); }
    input {
        position: absolute;
        inset: 0;
        opacity: 0;
        cursor: pointer;
        border: 0;
        padding: 0;
    }
</style>
