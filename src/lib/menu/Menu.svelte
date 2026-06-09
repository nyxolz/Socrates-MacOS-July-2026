<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { activeTab, theme, menuOptions, localVersion, remoteVersion, lastVersionCheck, updateAvailable } from "./stores";
    import { get } from "svelte/store";
    import { startConfigBridge } from "./bridge";
    import { setZone } from "./zones";
    import TabBar from "./TabBar.svelte";
    import Aiming from "./tabs/Aiming.svelte";
    import Visuals from "./tabs/Visuals.svelte";
    import Output from "./tabs/Output.svelte";
    import Options from "./tabs/Options.svelte";
    import PlayerList from "./tabs/PlayerList.svelte";
    import Explorer from "./tabs/Explorer.svelte";
    import Configs from "./tabs/Configs.svelte";
    import Servers from "./tabs/Servers.svelte";
    import Settings from "./tabs/Settings.svelte";
    import Icon from "./controls/Icon.svelte";
    import "./tokens.css";

    function hexToRgba(hex: string, alpha: number): string {
        const h = hex.replace("#", "");
        const r = parseInt(h.slice(0, 2), 16) || 0;
        const g = parseInt(h.slice(2, 4), 16) || 0;
        const b = parseInt(h.slice(4, 6), 16) || 0;
        return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }

    // Theme overrides — applied inline on the root menu element so they win
    // against the scoped `.socrate-menu` defaults in tokens.css.
    $: themeStyle =
        `--accent: ${$theme};` +
        `--accent-on: ${$theme};` +
        `--accent-soft: ${hexToRgba($theme, 0.55)};` +
        `--accent-glow: ${hexToRgba($theme, 0.45)};`;

    let menuEl: HTMLDivElement;
    let headerEl: HTMLDivElement;
    let resizeObs: ResizeObserver | undefined;
    let stopBridge: (() => void) | undefined;
    let socrateVersion = "…";
    let versionTimer = 0;

    async function pollRemoteVersion() {
        try {
            const r = await invoke<string>("check_remote_version");
            remoteVersion.set(r);
            lastVersionCheck.set(Date.now());
        } catch {
            remoteVersion.set("offline");
        }
        const local = get(localVersion);
        const remote = get(remoteVersion);
        updateAvailable.set(
            local !== "…" && local !== "unknown" &&
            remote !== "…" && remote !== "offline" &&
            remote !== local
        );
    }

    // Portrait shape — wide enough for a 2-column body, tall enough to fill
    // most of the Roblox window. Width is fixed; height is `85vh` clamped.
    let pos = { x: 24, y: 24 };
    const MENU_W = 490;
    let menuH = 0;

    let dragging = false;
    let dragOffset = { x: 0, y: 0 };
    let activePointerId = -1;
    let minimized = false;

    function toggleMinimize() {
        minimized = !minimized;
        // The ResizeObserver picks up the new geometry and re-pushes the
        // interactive zone; no explicit pushZone() needed here.
    }

    function pushZone() {
        if (!menuEl || !$menuOptions.menu_visible) {
            // Clear our click-through zone so the overlay stops eating clicks
            // behind the (hidden) menu. Other windows keep their own zones.
            setZone("menu", null);
            return;
        }
        const r = menuEl.getBoundingClientRect();
        menuH = r.height;
        setZone("menu", { x: r.left, y: r.top, w: r.width, h: r.height });
    }

    // Re-push (or clear) the zone whenever visibility flips.
    $: if (typeof window !== "undefined") { void $menuOptions.menu_visible; pushZone(); }

    function clampToViewport() {
        const w = window.innerWidth;
        const h = window.innerHeight;
        pos.x = Math.max(0, Math.min(pos.x, w - MENU_W));
        pos.y = Math.max(0, Math.min(pos.y, h - menuH));
        pos = pos;
    }

    function onPointerDown(e: PointerEvent) {
        if (e.button !== 0) return;
        const r = menuEl.getBoundingClientRect();
        dragOffset.x = e.clientX - r.left;
        dragOffset.y = e.clientY - r.top;
        dragging = true;
        activePointerId = e.pointerId;
        // Capture so the drag survives the cursor briefly leaving the header
        // even though the rest of the window is click-through.
        headerEl.setPointerCapture(e.pointerId);
        e.preventDefault();
    }

    function onPointerMove(e: PointerEvent) {
        if (!dragging || e.pointerId !== activePointerId) return;
        pos.x = e.clientX - dragOffset.x;
        pos.y = e.clientY - dragOffset.y;
        clampToViewport();
        pushZone();
    }

    function onPointerUp(e: PointerEvent) {
        if (e.pointerId !== activePointerId) return;
        dragging = false;
        activePointerId = -1;
        try { headerEl.releasePointerCapture(e.pointerId); } catch {}
        pushZone();
    }

    onMount(() => {
        pushZone();
        resizeObs = new ResizeObserver(pushZone);
        resizeObs.observe(menuEl);
        window.addEventListener("resize", () => { clampToViewport(); pushZone(); });
        stopBridge = startConfigBridge();
        // Compile-time hash baked by builder_sender.sh into version.rs.
        invoke<string>("get_version")
            .then(v => {
                socrateVersion = `Version Hash ${v}`;
                localVersion.set(v);
                // First poll right after we know the local version so the
                // badge can flip on within the first 4-second window.
                pollRemoteVersion();
            })
            .catch(() => {
                socrateVersion = "Version Hash unknown";
                localVersion.set("unknown");
            });
        // Keep polling regardless of which tab is active — that's the whole
        // point of running this in Menu instead of Settings.
        versionTimer = window.setInterval(pollRemoteVersion, 4000);
    });

    onDestroy(() => {
        resizeObs?.disconnect();
        stopBridge?.();
        clearInterval(versionTimer);
    });
</script>

<div
    bind:this={menuEl}
    class="socrate-menu window"
    class:hidden={!$menuOptions.menu_visible}
    class:minimized
    style="left: {pos.x}px; top: {pos.y}px; width: {MENU_W}px; {themeStyle}"
>
    <div
        bind:this={headerEl}
        class="header"
        class:dragging
        on:pointerdown={onPointerDown}
        on:pointermove={onPointerMove}
        on:pointerup={onPointerUp}
        on:pointercancel={onPointerUp}
    >
        <span class="brand">Socrate</span>
        <span class="version">{socrateVersion} · https://usesocrate.xyz/</span>
        <span class="spacer"></span>
        <button class="iconbtn" title={minimized ? "Restore" : "Minimize"}
                on:click|stopPropagation={toggleMinimize}
                on:pointerdown|stopPropagation>
            <Icon name={minimized ? "plus" : "minus"} size={11} />
        </button>
        <button class="iconbtn" title="Close" on:click|stopPropagation={() => invoke("panic_exit")}>
            <Icon name="x" size={11} />
        </button>
    </div>

    {#if !minimized}
        <TabBar />

        <div class="body">
            {#if $activeTab === "Aiming"}
                <Aiming />
            {:else if $activeTab === "Visuals"}
                <Visuals />
            {:else if $activeTab === "Output"}
                <Output />
            {:else if $activeTab === "Options"}
                <Options />
            {:else if $activeTab === "Player List"}
                <PlayerList />
            {:else if $activeTab === "Explorer"}
                <Explorer />
            {:else if $activeTab === "Configs"}
                <Configs />
            {:else if $activeTab === "Servers"}
                <Servers />
            {:else if $activeTab === "Settings"}
                <Settings />
            {/if}
        </div>
    {/if}
</div>

<style>
    .window {
        position: fixed;
        height: 85vh;
        min-height: 420px;
        max-height: 900px;
        background: var(--bg-window);
        border: 1px solid var(--border);
        border-radius: 6px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.55);
        display: flex;
        flex-direction: column;
        overflow: hidden;
        pointer-events: auto;
        will-change: left, top;
        /* Lock down text selection / copy across the whole menu UI. Inputs
         * inside still work because the global rule below re-enables them. */
        -webkit-user-select: none;
        user-select: none;
        -webkit-user-drag: none;
    }
    .window :global(input),
    .window :global(textarea) {
        -webkit-user-select: text;
        user-select: text;
    }
    .window.hidden {
        display: none;
    }
    /* Minimized: only the header is visible, height collapses to fit. */
    .window.minimized {
        height: auto;
        min-height: 0;
        max-height: none;
    }
    .header {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 8px 6px 10px;
        background: var(--bg-window-edge);
        border-bottom: 1px solid var(--border);
        cursor: grab;
        touch-action: none;
        user-select: none;
    }
    .header.dragging { cursor: grabbing; }
    .brand {
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 0.3px;
        color: var(--text);
    }
    .version {
        font-size: 9px;
        color: var(--text-muted);
        font-variant-numeric: tabular-nums;
    }
    .spacer { flex: 1; }
    .iconbtn {
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
    .iconbtn:hover {
        background: var(--bg-tab-active);
        color: var(--text);
        border-color: var(--border);
    }
    .body {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        background: var(--bg-window);
    }
    .body::-webkit-scrollbar { width: 6px; }
    .body::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
</style>
