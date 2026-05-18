<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";

    let menuEl: HTMLDivElement;
    let resizeObs: ResizeObserver | undefined;

    async function pushInteractiveZone() {
        if (!menuEl) return;
        const r = menuEl.getBoundingClientRect();
        await invoke("set_interactive_zones", {
            zones: [{ x: r.left, y: r.top, w: r.width, h: r.height }],
        });
    }

    onMount(() => {
        pushInteractiveZone();
        resizeObs = new ResizeObserver(() => pushInteractiveZone());
        resizeObs.observe(menuEl);
        window.addEventListener("resize", pushInteractiveZone);
    });

    onDestroy(() => {
        resizeObs?.disconnect();
        window.removeEventListener("resize", pushInteractiveZone);
    });
</script>

<!-- Empty black menu, no styling yet. Sized small in the corner so the rest of
     the overlay stays click-through over Roblox. -->
<div bind:this={menuEl} class="menu"></div>

<style>
    .menu {
        position: fixed;
        top: 80px;
        left: 80px;
        width: 420px;
        height: 560px;
        background: #000;
        border-radius: 6px;
    }
</style>
