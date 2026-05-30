<script lang="ts">
    import { aimbot } from "../stores";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import Slider from "../controls/Slider.svelte";
    import Dropdown from "../controls/Dropdown.svelte";
    import KeybindInput from "../controls/KeybindInput.svelte";
    import ColorPicker from "../controls/ColorPicker.svelte";

    const PARTS = ["Head", "Torso", "HumanoidRootPart"] as const;
    const SELECT_MODES = ["Distance", "Health", "Crosshair"] as const;
    const AIM_TYPES = ["Mouse", "Camera", "Silent Aim"] as const;
</script>

<div class="grid">
    <div class="col">
        <Section title="Aimbot" icon="target" active={$aimbot.enabled}>
            <div class="row">
                <Toggle bind:checked={$aimbot.enabled} label="Enabled" />
                <KeybindInput inline bind:value={$aimbot.keybind} />
            </div>
            <Dropdown label="Aim type" bind:value={$aimbot.aim_type} options={AIM_TYPES} />
            <Dropdown label="Target part" bind:value={$aimbot.part} options={PARTS} />
            <Slider label="FOV radius" bind:value={$aimbot.fov_radius}
                min={10} max={600} step={1} />
            <div class="row">
                <Toggle bind:checked={$aimbot.show_fov} label="Show FOV" />
                <ColorPicker bind:value={$aimbot.fov_color} title="Border colour" />
            </div>
            <div class="row">
                <Toggle bind:checked={$aimbot.fill_fov} label="Fill FOV" />
                <ColorPicker bind:value={$aimbot.fov_fill_color} title="Fill colour" />
            </div>
            <Slider label="Smoothing" bind:value={$aimbot.smoothing}
                min={0.01} max={1} step={0.01} format={(v) => v.toFixed(2)} />
        </Section>

        <Section title="Prediction" icon="zap" active={$aimbot.prediction_enabled}>
            <Toggle bind:checked={$aimbot.prediction_enabled} label="Enabled" />
            <Slider label="Strength" bind:value={$aimbot.prediction_strength}
                min={0} max={2} step={0.01} format={(v) => v.toFixed(2) + "x"} />
            <Toggle bind:checked={$aimbot.velocity_prediction} label="Velocity-based" />
            <Toggle bind:checked={$aimbot.bullet_drop_comp} label="Bullet drop" />
            <Slider label="Gravity comp" bind:value={$aimbot.gravity_comp}
                min={0} max={200} step={1} />
        </Section>

        <Section title="Trigger" icon="mouse" active={$aimbot.trigger_enabled}>
            <div class="row">
                <Toggle bind:checked={$aimbot.trigger_enabled} label="Auto trigger" />
                <KeybindInput inline bind:value={$aimbot.trigger_keybind} />
            </div>
            <Slider label="Delay" bind:value={$aimbot.trigger_delay}
                min={0} max={500} step={1} format={(v) => v.toFixed(0) + " ms"} />
            <Toggle bind:checked={$aimbot.trigger_burst} label="Burst mode" />
        </Section>
    </div>

    <div class="col">
        <Section title="Targeting" icon="crosshair">
            <Dropdown label="Selection mode" bind:value={$aimbot.selection_mode} options={SELECT_MODES} />
            <Toggle bind:checked={$aimbot.team_check} label="Team check" />
            <Toggle bind:checked={$aimbot.wall_check} label="Wall check" />
            <Slider label="Min distance" bind:value={$aimbot.min_distance}
                min={0} max={500} step={1} format={(v) => v.toFixed(0) + " s"} />
            <Slider label="Max distance" bind:value={$aimbot.max_distance}
                min={50} max={2000} step={10} format={(v) => v.toFixed(0) + " s"} />
        </Section>

        <Section title="Legit" icon="shield" active={$aimbot.legit_enabled}>
            <Toggle bind:checked={$aimbot.legit_enabled} label="Legit mode" />
            <Slider label="Miss rate" bind:value={$aimbot.legit_miss}
                min={0} max={0.9} step={0.01} format={(v) => (v * 100).toFixed(0) + "%"} />
            <Toggle bind:checked={$aimbot.humanize} label="Humanize aim" />
            <Toggle bind:checked={$aimbot.smooth_path} label="Smooth path" />
        </Section>
    </div>
</div>

<style>
    .grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 6px;
        padding: 6px;
        align-items: start;
    }
    .col { display: flex; flex-direction: column; min-width: 0; }
    .row {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .row :global(.toggle) {
        flex: 1;
    }
</style>
