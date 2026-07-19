<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import Section from "../controls/Section.svelte";
    import Toggle from "../controls/Toggle.svelte";
    import { debugEspFrame, type DebugEspFrame } from "../stores";

    let children: string[] = [];
    let error = "";
    let timer = 0;

    // ─── Advanced diagnostics — flicker root-cause hunting ─────────────
    // Backend counters are monotonic; we diff consecutive 1s snapshots to
    // get rates. A spike in `culled/s` is the direct signature of "a
    // player's ESP blinked off this frame"; `jumps/s` tests the camera-tear
    // theory; `full walks/s` tests whether the char-cache thread's cheap
    // path is actually being reused.
    type DiagStats = {
        esp_ticks: number;
        esp_players_culled: number;
        esp_camera_jumps: number;
        cache_ticks: number;
        cache_full_walks: number;
        cache_known_containers: number;
        char_caches_len: number;
        read_ok: number;
        read_fail: number;
    };
    let diagPrev: DiagStats | null = null;
    let diagCur: DiagStats | null = null;
    let diagRates = {
        espHz: 0, culledPerSec: 0, jumpsPerSec: 0,
        cacheHz: 0, fullWalksPerSec: 0, readsPerSec: 0, failPerSec: 0,
    };
    let diagTimer = 0;
    async function refreshDiag() {
        try {
            const snap = await invoke<DiagStats>("get_diag_stats");
            if (diagCur) {
                diagPrev = diagCur;
                const d = (a: number, b: number) => Math.max(0, a - b);
                diagRates = {
                    espHz: d(snap.esp_ticks, diagPrev.esp_ticks),
                    culledPerSec: d(snap.esp_players_culled, diagPrev.esp_players_culled),
                    jumpsPerSec: d(snap.esp_camera_jumps, diagPrev.esp_camera_jumps),
                    cacheHz: d(snap.cache_ticks, diagPrev.cache_ticks),
                    fullWalksPerSec: d(snap.cache_full_walks, diagPrev.cache_full_walks),
                    readsPerSec: d(snap.read_ok, diagPrev.read_ok),
                    failPerSec: d(snap.read_fail, diagPrev.read_fail),
                };
            }
            diagCur = snap;
        } catch { /* not attached yet */ }
    }

    let objectEsp = false;
    $: invoke("set_object_esp", { enabled: objectEsp }).catch(() => {});

    let dbg: DebugEspFrame | null = null;
    debugEspFrame.subscribe((v) => { dbg = v; });
    function fmt(n: number) { return n.toFixed(3); }

    async function refresh() {
        try {
            children = await invoke<string[]>("dev_players_children");
            error = "";
        } catch (e) {
            children = [];
            error = String(e);
        }
    }

    onMount(() => {
        refresh();
        timer = window.setInterval(refresh, 1000);
        refreshDiag();
        diagTimer = window.setInterval(refreshDiag, 1000);
    });
    onDestroy(() => {
        clearInterval(timer);
        clearInterval(diagTimer);
        // Don't leave the heavy object walk running once the tab is closed.
        invoke("set_object_esp", { enabled: false }).catch(() => {});
    });
</script>

<div class="wrap">
    <Section title="Camera Debug — view matrix" icon="activity" active={dbg !== null}>
        {#if !dbg}
            <div class="empty">pas de frame reçue…</div>
        {:else}
            <div class="hint">attached={String(dbg.attached)} fov={fmt(dbg.fov)} aspect={fmt(dbg.aspect)}</div>
            <div class="hint">CameraPosition (0x1C0)</div>
            <div class="matrow mono">x={fmt(dbg.camera_pos[0])}  y={fmt(dbg.camera_pos[1])}  z={fmt(dbg.camera_pos[2])}</div>
            <div class="hint">Forward (dérivé de la matrice)</div>
            <div class="matrow mono">x={fmt(dbg.forward[0])}  y={fmt(dbg.forward[1])}  z={fmt(dbg.forward[2])}</div>
            <div class="hint">CameraOrientation (0xF0) — matrice 3x3 brute</div>
            {#each dbg.view_matrix_raw as row}
                <div class="matrow mono">[{row.map(fmt).join(", ")}]</div>
            {/each}
        {/if}
    </Section>

    <Section title="Players Debug — health / distance / world" icon="users" active={(dbg?.players?.length ?? 0) > 0}>
        {#if !dbg || !dbg.players.length}
            <div class="empty">aucun joueur détecté…</div>
        {:else}
            <div class="count">{dbg.player_count} joueur{dbg.player_count === 1 ? "" : "s"}</div>
            {#each dbg.players as p}
                <div class="objrow">
                    <div class="objhead">
                        <span class="name">{p.name || "?"} {p.relation === 1 ? "(ally)" : ""}</span>
                        <span class="dist mono">{p.distance.toFixed(1)} studs</span>
                    </div>
                    <div class="objpath mono">
                        hp={p.health < 0 ? "?" : p.health.toFixed(1)}  world=[{p.world.map(fmt).join(", ")}]
                    </div>
                </div>
            {/each}
        {/if}
    </Section>

    <Section title="Players — children" icon="users" active={children.length > 0}>
        {#if error}
            <div class="err">{error}</div>
        {:else if !children.length}
            <div class="empty">no children (not attached?)</div>
        {:else}
            <div class="count">{children.length} child{children.length === 1 ? "" : "ren"}</div>
            {#each children as name, i (i)}
                <div class="row">
                    <span class="idx mono">{i}</span>
                    <span class="name">{name}</span>
                </div>
            {/each}
        {/if}
    </Section>

    <Section title="ESP all game objects" icon="package" active={objectEsp}>
        <Toggle bind:checked={objectEsp} label="ESP all game objects" />
        <div class="hint">3D box + name on every Workspace part (capped, heavy).</div>
    </Section>
</div>

<style>
    .wrap { padding: 6px; }
    .count {
        font-size: 9px;
        color: var(--text-muted);
        margin-bottom: 4px;
    }
    .row {
        display: flex;
        align-items: baseline;
        gap: 8px;
        padding: 3px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        margin-bottom: 3px;
    }
    .idx {
        font-size: 9px;
        color: var(--text-muted);
        min-width: 18px;
        text-align: right;
    }
    .name {
        font-size: 11px;
        font-weight: 600;
        color: var(--text);
        word-break: break-all;
    }
    .mono { font-variant-numeric: tabular-nums; }
    .matrow {
        font-size: 10px;
        padding: 2px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        margin-bottom: 2px;
        color: var(--text);
    }
    .objrow {
        padding: 3px 6px;
        background: var(--bg-input);
        border: 1px solid var(--border-soft);
        border-radius: var(--radius-small);
        margin-bottom: 3px;
    }
    .objhead {
        display: flex;
        justify-content: space-between;
        align-items: baseline;
        gap: 8px;
    }
    .dist {
        font-size: 9px;
        color: var(--text-muted);
        flex-shrink: 0;
    }
    .objpath {
        font-size: 9px;
        color: var(--text-muted);
        opacity: 0.85;
        word-break: break-all;
        margin-top: 1px;
    }
    .err {
        font-size: 10px;
        color: #e88;
    }
    .empty {
        font-size: 11px;
        color: var(--text-muted);
        text-align: center;
        padding: 8px 0;
    }
    .hint {
        font-size: 9px;
        color: var(--text-muted);
        margin-top: 4px;
    }
</style>
