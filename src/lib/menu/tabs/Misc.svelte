<script lang="ts">
    import { misc } from "../stores";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import Slider from "../controls/Slider.svelte";
</script>

<div class="grid">
    <Section title="Player" icon="user">
        <Toggle bind:checked={$misc.walk_speed_enabled} label="Walk Speed" />
        <Slider label="Walk Speed value" bind:value={$misc.walk_speed}
            min={0} max={200} step={1} />

        <Toggle bind:checked={$misc.hip_height_enabled} label="Hip Height" />
        <Slider label="Hip Height value" bind:value={$misc.hip_height}
            min={0} max={20} step={0.1} format={(v) => v.toFixed(1)} />

        <Toggle bind:checked={$misc.display_distance_enabled} label="Display Distance" />
        <Slider label="Display Distance value" bind:value={$misc.display_distance}
            min={0} max={5000} step={10} />

        <Toggle bind:checked={$misc.max_slope_enabled} label="Max Slope" />
        <Slider label="Max Slope value" bind:value={$misc.max_slope}
            min={0} max={180} step={1} format={(v) => v.toFixed(0) + "°"} />
    </Section>
</div>

<style>
    .grid {
        padding: 6px;
    }
</style>
