<script lang="ts">
    import { theme, THEME_PRESETS, menuOptions } from "../stores";
    import Section from "../controls/Section.svelte";
    import ColorPicker from "../controls/ColorPicker.svelte";
    import KeybindInput from "../controls/KeybindInput.svelte";
    import Toggle from "../controls/Toggle.svelte";

    function pick(c: string) { theme.set(c); }
</script>

<div class="grid">
    <div class="col">
        <Section title="Theme" icon="sliders" active>
            <div class="presets">
                {#each THEME_PRESETS as p}
                    <button
                        class="preset"
                        class:active={$theme === p.value}
                        style="background: {p.value};"
                        title={p.name}
                        on:click={() => pick(p.value)}
                    ></button>
                {/each}
            </div>
            <div class="row">
                <span class="lbl">Custom</span>
                <ColorPicker bind:value={$theme} />
            </div>
            <div class="row">
                <span class="lbl">Active</span>
                <span class="hex">{$theme}</span>
            </div>
        </Section>
    </div>
    <div class="col">
        <Section title="Menu" icon="sliders" active>
            <KeybindInput label="Hide menu" noMouse bind:value={$menuOptions.toggle_bind} />
            <Toggle label="Show binds HUD" bind:checked={$menuOptions.show_binds} />
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
    .presets {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 5px;
        margin-bottom: 6px;
    }
    .preset {
        height: 22px;
        border-radius: 5px;
        border: 1px solid var(--border);
        cursor: pointer;
        transition: transform 120ms var(--ease), border-color 120ms var(--ease);
        box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.4);
    }
    .preset:hover { transform: scale(1.05); }
    .preset.active {
        border-color: #ffffff;
        box-shadow: 0 0 0 2px rgba(255,255,255,0.15), inset 0 0 0 1px rgba(0,0,0,0.4);
    }
    .row {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 3px 0;
    }
    .lbl { flex: 1; font-size: 11px; color: var(--text); }
    .hex {
        margin-left: auto;
        font-family: ui-monospace, Menlo, monospace;
        font-size: 10px;
        color: var(--text-muted);
        text-transform: uppercase;
    }
</style>
