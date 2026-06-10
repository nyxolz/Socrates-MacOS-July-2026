<script lang="ts">
    import { misc } from "../stores";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import Slider from "../controls/Slider.svelte";
</script>

<div class="grid">
    <Section title="Player" icon="user">
        <Toggle bind:checked={$misc.hip_height_enabled} label="Hip Height" />
        <Slider label="Hip Height value" bind:value={$misc.hip_height}
            min={0} max={20} step={0.1} format={(v) => v.toFixed(1)} />
    </Section>
</div>

<style>
    .grid {
        padding: 6px;
        max-width: 280px;
    }
</style>
