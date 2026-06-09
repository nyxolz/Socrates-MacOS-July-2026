<script lang="ts">
    import { fly, fade } from "svelte/transition";
    import { flip } from "svelte/animate";
    import { theme } from "./stores";
    import { toasts } from "./toast";

    $: accentStyle = `--toast-accent: ${$theme};`;
</script>

<div class="toast-stack" style={accentStyle}>
    {#each $toasts as t (t.id)}
        <div
            class="toast"
            animate:flip={{ duration: 180 }}
            in:fly={{ x: 20, duration: 180 }}
            out:fade={{ duration: 160 }}
        >
            <div class="content">
                <span class="dot"></span>
                <span class="text">{t.text}</span>
            </div>
            <div class="timebar" style="animation-duration: {t.ms}ms"></div>
        </div>
    {/each}
</div>

<style>
    .toast-stack {
        position: fixed;
        bottom: 18px;
        right: 18px;
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 6px;
        pointer-events: none;
        z-index: 9999;
    }
    .toast {
        position: relative;
        min-width: 200px;
        padding: 8px 14px 9px;
        background: rgba(20, 20, 24, 0.92);
        border: 1px solid var(--border, rgba(255, 255, 255, 0.18));
        border-radius: 8px;
        box-shadow: 0 4px 18px rgba(0, 0, 0, 0.5);
        color: #f2f2f2;
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.2px;
        white-space: nowrap;
        overflow: hidden;
        -webkit-user-select: none;
        user-select: none;
    }
    .content {
        display: flex;
        align-items: center;
        gap: 8px;
    }
    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--toast-accent, #ffde9d);
        flex-shrink: 0;
    }
    /* Countdown bar — a colored line pinned to the bottom edge that retracts
     * from full width to zero over the toast's lifetime. */
    .timebar {
        position: absolute;
        left: 0;
        bottom: 0;
        height: 2px;
        width: 100%;
        background: var(--toast-accent, #ffde9d);
        transform-origin: left center;
        animation-name: shrink;
        animation-timing-function: linear;
        animation-fill-mode: forwards;
    }
    @keyframes shrink {
        from { transform: scaleX(1); }
        to   { transform: scaleX(0); }
    }
</style>
