<script lang="ts">
    import { visuals } from "../stores";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import Slider from "../controls/Slider.svelte";
    import Dropdown from "../controls/Dropdown.svelte";
    import ColorPicker from "../controls/ColorPicker.svelte";
    import Preview from "../Preview.svelte";

    const ORIGINS = ["Top", "Center", "Bottom"] as const;
</script>

<div class="grid">
    <div class="col">
        <Section title="Skeleton" icon="eye" active={$visuals.skeleton}>
            <div class="row">
                <Toggle bind:checked={$visuals.skeleton} label="Enabled" />
                <ColorPicker bind:value={$visuals.skeleton_color} />
            </div>
            <Slider label="Thickness" bind:value={$visuals.skeleton_thickness}
                min={0.5} max={5} step={0.1} format={(v) => v.toFixed(1)} />
        </Section>

        <Section title="Box 2D" icon="filter" active={$visuals.box2d}>
            <div class="row">
                <Toggle bind:checked={$visuals.box2d} label="Enabled" />
                <ColorPicker bind:value={$visuals.box2d_color} />
            </div>
            <div class="row">
                <Toggle bind:checked={$visuals.box2d_fill} label="Fill" />
                <ColorPicker bind:value={$visuals.box2d_fill_color} />
            </div>
            <Slider label="Thickness" bind:value={$visuals.box2d_thickness}
                min={0.5} max={5} step={0.1} format={(v) => v.toFixed(1)} />
        </Section>

        <Section title="Box 3D" icon="package" active={$visuals.box3d}>
            <div class="row">
                <Toggle bind:checked={$visuals.box3d} label="Enabled" />
                <ColorPicker bind:value={$visuals.box3d_color} />
            </div>
            <div class="row">
                <Toggle bind:checked={$visuals.box3d_fill} label="Fill" />
                <ColorPicker bind:value={$visuals.box3d_fill_color} />
            </div>
            <Slider label="Thickness" bind:value={$visuals.box3d_thickness}
                min={0.5} max={5} step={0.1} format={(v) => v.toFixed(1)} />
        </Section>

        <Section title="View direction" icon="compass" active={$visuals.view_dir}>
            <div class="row">
                <Toggle bind:checked={$visuals.view_dir} label="Enabled" />
                <ColorPicker bind:value={$visuals.view_dir_color} />
            </div>
            <Slider label="Length" bind:value={$visuals.view_dir_length}
                min={1} max={30} step={0.5} format={(v) => v.toFixed(1) + " s"} />
        </Section>

        <Section title="Tracers" icon="activity" active={$visuals.tracers}>
            <div class="row">
                <Toggle bind:checked={$visuals.tracers} label="Enabled" />
                <ColorPicker bind:value={$visuals.tracers_color} />
            </div>
            <Dropdown label="Origin" bind:value={$visuals.tracers_origin} options={ORIGINS} />
            <Slider label="Thickness" bind:value={$visuals.tracers_thickness}
                min={0.5} max={5} step={0.1} format={(v) => v.toFixed(1)} />
        </Section>
    </div>

    <div class="col">
        <Section title="Name" icon="text" active={$visuals.names}>
            <div class="row">
                <Toggle bind:checked={$visuals.names} label="Enabled" />
                <ColorPicker bind:value={$visuals.names_color} />
            </div>
        </Section>

        <Section title="Distance" icon="crosshair" active={$visuals.distance}>
            <div class="row">
                <Toggle bind:checked={$visuals.distance} label="Enabled" />
                <ColorPicker bind:value={$visuals.distance_color} />
            </div>
        </Section>

        <Section title="Avatar head" icon="user" active={$visuals.avatar_head}>
            <div class="row">
                <Toggle bind:checked={$visuals.avatar_head} label="Enabled" />
                <ColorPicker bind:value={$visuals.avatar_head_border_color} />
            </div>
            <Slider label="Size" bind:value={$visuals.avatar_head_size}
                min={8} max={80} step={1} format={(v) => v.toFixed(0) + " px"} />
            <Slider label="Border" bind:value={$visuals.avatar_head_border_thickness}
                min={0} max={5} step={0.1} format={(v) => v.toFixed(1)} />
        </Section>

        <Section title="Health" icon="activity" active={$visuals.health}>
            <div class="row">
                <Toggle bind:checked={$visuals.health} label="Numeric" />
                <ColorPicker bind:value={$visuals.health_color} />
            </div>
            <div class="row">
                <Toggle bind:checked={$visuals.health_bar} label="Bar" />
                <ColorPicker bind:value={$visuals.health_bar_color} />
            </div>
        </Section>

        <Section title="Filters" icon="filter">
            <Toggle bind:checked={$visuals.team_check} label="Team check" />
            <Toggle bind:checked={$visuals.show_self} label="Show local player" />
            <Slider label="Min distance" bind:value={$visuals.min_distance}
                min={0} max={20} step={0.5} format={(v) => v.toFixed(1) + " s"} />
            <Slider label="Max distance" bind:value={$visuals.max_distance}
                min={50} max={5000} step={10} format={(v) => v.toFixed(0) + " s"} />
        </Section>

        <Section title="Global" icon="sliders">
            <Toggle bind:checked={$visuals.outline} label="Black outline" />
        </Section>

        <Section title="Preview" icon="eye">
            <Preview />
            <p class="hint">drag the labels to reposition — applies to real ESP</p>
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
    .row { display: flex; align-items: center; gap: 8px; }
    .row :global(.toggle) { flex: 1; }
    .hint {
        margin: 6px 0 0 0;
        font-size: 9px;
        color: var(--text-muted);
        text-align: center;
    }
</style>
