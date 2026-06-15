<script lang="ts">
    import { misc } from "../stores";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import Slider from "../controls/Slider.svelte";
    import KeybindInput from "../controls/KeybindInput.svelte";
</script>

<div class="grid">
    <Section title="Player" icon="user">
        <Toggle bind:checked={$misc.hip_height_enabled} label="Hip Height" />
        <Slider label="Hip Height value" bind:value={$misc.hip_height}
            min={0} max={20} step={0.1} format={(v) => v.toFixed(1)} />

        <Toggle bind:checked={$misc.jump_power_enabled} label="Jump Power" />
        <Slider label="Jump Power value" bind:value={$misc.jump_power}
            min={0} max={1000} step={10} />

        <div class="row">
            <Toggle bind:checked={$misc.fly_enabled} label="Fly" />
            <KeybindInput inline bind:value={$misc.fly_keybind} />
        </div>
        <Slider label="Fly speed" bind:value={$misc.fly_speed}
            min={1} max={50} step={1} format={(v) => v.toFixed(0) + " /s"} />

        <Toggle bind:checked={$misc.noclip_enabled} label="No Clip" />
    </Section>
</div>

<style>
    .grid {
        padding: 6px;
        max-width: 280px;
    }
    .row { display: flex; align-items: center; gap: 8px; }
    .row :global(.toggle) { flex: 1; }
</style>
