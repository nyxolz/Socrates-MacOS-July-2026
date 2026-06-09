<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { aimbot, menuOptions, theme } from "./stores";
    import { setZone } from "./zones";

    function hexToRgba(hex: string, alpha: number): string {
        const h = hex.replace("#", "");
        const r = parseInt(h.slice(0, 2), 16) || 0;
        const g = parseInt(h.slice(2, 4), 16) || 0;
        const b = parseInt(h.slice(4, 6), 16) || 0;
        return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }
    $: themeStyle =
        `--accent: ${$theme};` +
        `--accent-glow: ${hexToRgba($theme, 0.45)};`;

    // Friendly key labels (mirrors KeybindInput's pretty()).
    const PRETTY: Record<string, string> = {
        IntlBackslash: ">", Period: ".", Comma: ",", Slash: "/", Backslash: "\\",
        Minus: "-", Equal: "=", Semicolon: ";", Quote: "'", Backquote: "`",
        BracketLeft: "[", BracketRight: "]",
        MouseLeft: "M1", MouseRight: "M2", MouseMiddle: "M3",
    };
    function pretty(v: string): string {
        if (!v) return "—";
        if (PRETTY[v]) return PRETTY[v];
        return v.replace(/^Key/, "").replace(/^Digit/, "").replace(/^Mouse/, "M");
    }

    // Active binds, derived from the live config. Aimbot/Trigger only appear
    // when their feature is enabled; the menu toggle is always listed.
    type Bind = { name: string; key: string; tag?: string };
    $: binds = ((): Bind[] => {
        const out: Bind[] = [];
        out.push({ name: "Menu", key: pretty($menuOptions.toggle_bind) });
        if ($aimbot.enabled) {
            out.push({ name: "Aimbot", key: pretty($aimbot.keybind), tag: $aimbot.keybind_mode });
        }
        if ($aimbot.trigger_enabled) {
            out.push({ name: "Trigger", key: pretty($aimbot.trigger_keybind) });
        }
        return out;
    })();

    // ─── Position (persisted) + drag ────────────────────────────────────
    const POS_KEY = "socrate.bindsHud.pos";
    let pos = { x: 24, y: 0 };
    if (typeof localStorage !== "undefined") {
        try {
            const raw = localStorage.getItem(POS_KEY);
            if (raw) pos = JSON.parse(raw);
        } catch { /* keep default */ }
    }
    // Default to lower-left if no saved Y yet.
    onMount(() => {
        if (pos.y === 0) pos.y = Math.max(40, window.innerHeight - 160);
        pos = pos;
        push();
        resizeObs = new ResizeObserver(push);
        if (hudEl) resizeObs.observe(hudEl);
        window.addEventListener("resize", onResize);
    });

    let hudEl: HTMLDivElement;
    let resizeObs: ResizeObserver | undefined;
    let dragging = false;
    let dragOffset = { x: 0, y: 0 };
    let activePointerId = -1;

    function savePos() {
        if (typeof localStorage !== "undefined") {
            localStorage.setItem(POS_KEY, JSON.stringify(pos));
        }
    }

    function push() {
        if (!hudEl || !$menuOptions.show_binds) { setZone("binds", null); return; }
        const r = hudEl.getBoundingClientRect();
        setZone("binds", { x: r.left, y: r.top, w: r.width, h: r.height });
    }

    function clampToViewport() {
        if (!hudEl) return;
        const r = hudEl.getBoundingClientRect();
        pos.x = Math.max(0, Math.min(pos.x, window.innerWidth - r.width));
        pos.y = Math.max(0, Math.min(pos.y, window.innerHeight - r.height));
        pos = pos;
    }
    function onResize() { clampToViewport(); push(); }

    function onPointerDown(e: PointerEvent) {
        if (e.button !== 0) return;
        const r = hudEl.getBoundingClientRect();
        dragOffset.x = e.clientX - r.left;
        dragOffset.y = e.clientY - r.top;
        dragging = true;
        activePointerId = e.pointerId;
        hudEl.setPointerCapture(e.pointerId);
        e.preventDefault();
    }
    function onPointerMove(e: PointerEvent) {
        if (!dragging || e.pointerId !== activePointerId) return;
        pos.x = e.clientX - dragOffset.x;
        pos.y = e.clientY - dragOffset.y;
        clampToViewport();
        push();
    }
    function onPointerUp(e: PointerEvent) {
        if (e.pointerId !== activePointerId) return;
        dragging = false;
        activePointerId = -1;
        try { hudEl.releasePointerCapture(e.pointerId); } catch {}
        savePos();
        push();
    }

    // Re-push (or clear) the zone whenever visibility flips.
    $: if (typeof window !== "undefined") { void $menuOptions.show_binds; push(); }

    onDestroy(() => {
        resizeObs?.disconnect();
        setZone("binds", null);
        if (typeof window !== "undefined") window.removeEventListener("resize", onResize);
    });
</script>

{#if $menuOptions.show_binds}
    <div
        bind:this={hudEl}
        class="binds-hud"
        class:dragging
        style="left: {pos.x}px; top: {pos.y}px; {themeStyle}"
        on:pointerdown={onPointerDown}
        on:pointermove={onPointerMove}
        on:pointerup={onPointerUp}
        on:pointercancel={onPointerUp}
    >
        <div class="title">Binds</div>
        {#each binds as b}
            <div class="row">
                <span class="name">{b.name}{#if b.tag}<span class="tag">{b.tag}</span>{/if}</span>
                <span class="key">{b.key}</span>
            </div>
        {/each}
    </div>
{/if}

<style>
    .binds-hud {
        position: fixed;
        min-width: 120px;
        background: rgba(20, 20, 24, 0.92);
        border: 1px solid var(--border, rgba(255, 255, 255, 0.18));
        border-radius: 8px;
        box-shadow: 0 4px 18px rgba(0, 0, 0, 0.5);
        padding: 7px 9px 8px;
        pointer-events: auto;
        cursor: grab;
        touch-action: none;
        -webkit-user-select: none;
        user-select: none;
        z-index: 9998;
        will-change: left, top;
    }
    .binds-hud.dragging { cursor: grabbing; }
    .title {
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.5px;
        text-transform: uppercase;
        color: var(--accent, #ffde9d);
        margin-bottom: 5px;
        padding-bottom: 4px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }
    .row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        padding: 2px 0;
    }
    .name {
        font-size: 11px;
        color: #e8e8e8;
        display: flex;
        align-items: center;
        gap: 5px;
    }
    .tag {
        font-size: 8px;
        font-weight: 700;
        text-transform: uppercase;
        color: var(--accent, #ffde9d);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 3px;
        padding: 0 3px;
        opacity: 0.85;
    }
    .key {
        font-size: 11px;
        font-weight: 700;
        font-variant-numeric: tabular-nums;
        color: #fff;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        padding: 1px 7px;
        min-width: 22px;
        text-align: center;
    }
</style>
