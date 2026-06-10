<script lang="ts">
    import { onMount, tick } from "svelte";
    import { activeTab, TABS, updateAvailable, type TabId } from "./stores";
    import Icon from "./controls/Icon.svelte";

    function set(t: TabId) {
        activeTab.set(t);
    }

    // Icon per tab — reuses the existing icon set.
    const TAB_ICONS: Record<TabId, string> = {
        "Aiming": "target",
        "Misc": "sliders",
        "Visuals": "eye",
        "Output": "terminal",
        "Options": "filter",
        "Player List": "users",
        "Explorer": "code",
        "Configs": "package",
        "Servers": "server",
        "Settings": "gear",
    };

    // Dense grid of tiny grayscale pixels filling the active button — each
    // one flickers between shades on its own random timer for a "hacker
    // terminal" glitch effect, while staying within the theme's greys.
    const PIXEL_COLS = 25;
    const PIXEL_ROWS = 12;
    const PIXEL_COLORS = ["#87878f", "#a8a8b0", "#d4d4d8", "#e8e8ec", "#ffffff"];
    const PIXEL_GRID = Array.from({ length: PIXEL_COLS * PIXEL_ROWS }, () => ({
        c: PIXEL_COLORS[Math.floor(Math.random() * PIXEL_COLORS.length)],
        d: Math.random() * 3,
        dur: 0.6 + Math.random() * 1.8,
    }));

    let itemEls: HTMLButtonElement[] = [];
    let indicatorTop = 0;
    let indicatorHeight = 0;

    function updateIndicator() {
        const idx = TABS.indexOf($activeTab);
        const el = itemEls[idx];
        if (el) {
            indicatorTop = el.offsetTop;
            indicatorHeight = el.offsetHeight;
        }
    }

    // Re-measure whenever the active category changes (after DOM updates) so
    // the highlight slides smoothly to the new item.
    $: if (typeof window !== "undefined") { void $activeTab; tick().then(updateIndicator); }

    onMount(updateIndicator);
</script>

<div class="sidebar">
    <div class="brand">
        <span class="logo"><img src="/logo.png" alt="" draggable="false" /></span>
        <div class="brand-text">
            <span class="name">Socrate</span>
            <span class="sub">for Roblox</span>
        </div>
        <button class="gear" title="Settings" on:click={() => set("Settings")}>
            <Icon name="gear" size={14} />
        </button>
    </div>

    <span class="cat-label">Category</span>

    <nav class="nav">
        <div class="indicator" style="top: {indicatorTop}px; height: {indicatorHeight}px;"></div>
        {#each TABS as t, i}
            <button
                bind:this={itemEls[i]}
                class="item"
                class:active={$activeTab === t}
                class:alert={t === "Settings" && $updateAvailable}
                on:click={() => set(t)}
                title={t === "Settings" && $updateAvailable ? "Update available" : t}
            >
                {#if $activeTab === t}
                    <span class="pixels">
                        {#each PIXEL_GRID as p, idx (idx)}
                            <span class="px" style="--c: {p.c}; --d: {p.d}s; --dur: {p.dur}s;"></span>
                        {/each}
                        <span class="fade"></span>
                    </span>
                {/if}
                <span class="content">
                    <Icon name={TAB_ICONS[t]} size={13} />
                    <span class="label">{t}</span>
                    {#if t === "Settings" && $updateAvailable}
                        <span class="dot" aria-label="update available">!</span>
                    {/if}
                </span>
            </button>
        {/each}
    </nav>

    <div class="footer">
        <div class="status" class:bad={$updateAvailable}>
            <span class="status-icon">
                <Icon name={$updateAvailable ? "x" : "check"} size={14} />
            </span>
            <span class="status-text">
                {$updateAvailable ? "Please update Socrate" : "Socrate is up to date"}
            </span>
        </div>
    </div>
</div>

<style>
    .sidebar {
        display: flex;
        flex-direction: column;
        width: 200px;
        flex-shrink: 0;
        background: var(--bg-sidebar);
        border-right: 1px solid var(--border);
        padding: 12px 10px;
        gap: 10px;
    }
    .brand {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 10px 12px;
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        transition: transform 200ms var(--ease);
    }
    .brand:hover {
        transform: scale(1.04);
    }
    .logo {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 26px;
        height: 26px;
        border-radius: var(--radius-small);
        background: var(--bg-input);
        color: var(--text);
        font-weight: 700;
        font-size: 13px;
        flex-shrink: 0;
        overflow: hidden;
    }
    .logo img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .brand-text {
        display: flex;
        flex-direction: column;
        line-height: 1.25;
        min-width: 0;
    }
    .name {
        font-size: 12px;
        font-weight: 700;
        color: var(--text);
    }
    .sub {
        font-size: 9px;
        color: var(--text-muted);
    }
    .gear {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        margin-left: auto;
        background: transparent;
        border: none;
        padding: 4px;
        border-radius: var(--radius-small);
        color: var(--text-muted);
        cursor: pointer;
        transition: transform 400ms var(--ease), color 200ms var(--ease);
    }
    .brand:hover .gear {
        transform: rotate(180deg);
        color: var(--text);
    }
    .gear:hover {
        transform: rotate(-180deg);
    }
    .cat-label {
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.6px;
        text-transform: uppercase;
        color: var(--text-dim);
        padding: 0 4px;
    }
    .nav {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 4px;
        flex: 1;
        min-height: 0;
        overflow-y: auto;
    }
    /* Sliding highlight — animates to the active item's position so
       switching categories feels like the selection "drops" into place. */
    .indicator {
        position: absolute;
        left: 0;
        right: 0;
        background: var(--bg-sidebar);
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        box-shadow: 0 0 10px rgba(255, 255, 255, 0.05);
        pointer-events: none;
        z-index: 0;
        transition:
            top 260ms var(--ease),
            height 260ms var(--ease);
    }
    .item {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: center;
        gap: 10px;
        background: transparent;
        border: 1.5px solid transparent;
        border-radius: var(--radius);
        color: var(--text-muted);
        font: inherit;
        font-size: 12px;
        font-weight: 500;
        padding: 12px 12px;
        cursor: pointer;
        text-align: left;
        white-space: nowrap;
        transition: color 140ms var(--ease);
    }
    .item:hover {
        color: var(--text);
    }
    .item.active {
        color: var(--text);
    }
    .item.alert {
        color: #ff8a8a;
    }
    .content {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: center;
        gap: 10px;
        flex: 1;
        min-width: 0;
    }
    .label {
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .dot {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: #ff5b5b;
        color: #fff;
        font-size: 9px;
        font-weight: 700;
        font-style: italic;
        line-height: 1;
        flex-shrink: 0;
    }
    /* Stack of tiny pixel squares filling the right half of the active
       button — each one a different gradient colour, fading in/out between
       0.1 and 0.4 opacity with a per-pixel delay so it ripples like a wave. */
    .pixels {
        position: absolute;
        inset: 0;
        left: 50%;
        display: grid;
        grid-template-columns: repeat(25, 1fr);
        grid-template-rows: repeat(12, 1fr);
        gap: 1px;
        z-index: 0;
        pointer-events: none;
        overflow: hidden;
        border-top-right-radius: var(--radius);
        border-bottom-right-radius: var(--radius);
    }
    .px {
        background: var(--c);
        border-radius: 0.5px;
        opacity: 0.12;
        transform: skewX(-20deg);
        animation: px-glitch var(--dur) ease-in-out var(--d) infinite;
    }
    @keyframes px-glitch {
        0%, 100% { opacity: 0.12; }
        50% { opacity: 0.65; }
    }
    /* Darken-to-transparent gradient over the left edge of the pixel block,
       so it blends into the button instead of cutting off sharply. */
    .fade {
        position: absolute;
        inset: 0;
        background: linear-gradient(to right, var(--bg-sidebar) 0%, transparent 55%);
        pointer-events: none;
    }
    .footer {
        display: flex;
        flex-direction: column;
        gap: 8px;
        padding: 8px 4px 2px 4px;
        border-top: 1px solid var(--border-soft);
    }
    .status {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 10px 12px;
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        border-radius: var(--radius);
        transition: transform 200ms var(--ease);
    }
    .status:hover {
        transform: scale(1.04);
    }
    .status-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        color: #6fdc8c;
        flex-shrink: 0;
    }
    .status-text {
        font-size: 10px;
        font-weight: 600;
        color: var(--text);
    }
    .status.bad .status-icon {
        color: #ff8a8a;
    }
    .status.bad {
        border-color: rgba(255, 138, 138, 0.35);
    }
</style>
