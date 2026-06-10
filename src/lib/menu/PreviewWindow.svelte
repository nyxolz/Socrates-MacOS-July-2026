<script lang="ts">
    import { onDestroy } from "svelte";
    import { previewWindowOpen } from "./stores";
    import { setZone } from "./zones";
    import Preview from "./Preview.svelte";
    import Icon from "./controls/Icon.svelte";
    import "./tokens.css";

    export let left: number;
    export let top: number;

    let panelEl: HTMLDivElement;
    let resizeObs: ResizeObserver | undefined;

    function push() {
        if (!panelEl || !$previewWindowOpen) { setZone("preview", null); return; }
        const r = panelEl.getBoundingClientRect();
        setZone("preview", { x: r.left, y: r.top, w: r.width, h: r.height });
    }

    function bindPanel(el: HTMLDivElement) {
        panelEl = el;
        resizeObs = new ResizeObserver(push);
        resizeObs.observe(panelEl);
        push();
        return {
            destroy() {
                resizeObs?.disconnect();
            },
        };
    }

    $: if (typeof window !== "undefined") { void $previewWindowOpen; void left; void top; push(); }

    onDestroy(() => setZone("preview", null));
</script>

{#if $previewWindowOpen}
    <div class="socrate-menu preview-window" use:bindPanel style="left: {left}px; top: {top}px;">
        <div class="pw-header">
            <span class="ic"><Icon name="eye" size={13} /></span>
            <span class="pw-title">Preview</span>
            <button class="pw-close" on:click={() => previewWindowOpen.set(false)}>
                <Icon name="x" size={11} />
            </button>
        </div>
        <div class="pw-body">
            <Preview />
            <p class="hint">drag the labels to reposition — applies to real ESP</p>
        </div>
    </div>
{/if}

<style>
    .preview-window {
        position: fixed;
        background: var(--bg-window, #1e1e24);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius);
        overflow: hidden;
        pointer-events: auto;
        z-index: 9997;
        -webkit-user-select: none;
        user-select: none;
    }
    .pw-header {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 9px 12px;
        border-bottom: 1px solid var(--border-soft);
        color: var(--text);
    }
    .ic {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        border-radius: var(--radius-small);
        background: var(--bg-input);
        color: var(--text-muted);
    }
    .pw-title {
        font-weight: 600;
        font-size: 11.5px;
        letter-spacing: 0.2px;
        color: var(--text);
        flex: 1;
    }
    .pw-close {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 18px;
        height: 18px;
        background: transparent;
        border: 1px solid transparent;
        border-radius: var(--radius-small);
        color: var(--text-muted);
        cursor: pointer;
    }
    .pw-close:hover {
        background: var(--bg-tab-active);
        color: var(--text);
        border-color: var(--border);
    }
    .pw-body {
        padding: 9px 12px 11px 12px;
        display: flex;
        flex-direction: column;
        gap: 7px;
    }
    .hint {
        margin: 0;
        font-size: 9px;
        color: var(--text-muted);
        text-align: center;
    }
</style>
