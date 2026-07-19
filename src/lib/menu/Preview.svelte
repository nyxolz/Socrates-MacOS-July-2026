<script lang="ts">
    import { onMount } from "svelte";
    import { visuals } from "./stores";
    import { getAvatar } from "./avatar";

    // Preview = mock player projected through the real view + projection
    // matrices. The fake camera and skeleton are tuned so the projected
    // bones overlay roughly on the dummy image. Text labels remain
    // draggable; their offsets are written back to the store, which is
    // the same source the real ESP reads from.

    const W = 200;
    const H = 280;
    const FOV = 50;
    const NEAR = 0.5;
    const FAR = 100;

    // Camera placed in front of the dummy on +Z, looking back at -Z. With
    // identity rotation, Right = (1, 0, 0), Up = (0, 1, 0), Back = (0, 0, 1).
    const CAM_POS: [number, number, number] = [0, 3, 7];
    const ROT: number[][] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];

    // R15-ish skeleton — slot index matches the rig order used backend-side.
    //   0 Head, 1 UpperTorso, 2 LowerTorso,
    //   3 LU arm, 4 LL arm, 5 RU arm, 6 RL arm,
    //   7 LU leg, 8 LL leg, 9 RU leg, 10 RL leg
    const SKELETON_PARTS: [number, number, number][] = [
        [ 0.00, 5.10, 0],
        [ 0.00, 4.20, 0],
        [ 0.00, 3.30, 0],
        [ 0.55, 4.20, 0], [ 0.55, 3.30, 0],
        [-0.55, 4.20, 0], [-0.55, 3.30, 0],
        [ 0.22, 2.40, 0], [ 0.22, 1.20, 0],
        [-0.22, 2.40, 0], [-0.22, 1.20, 0],
    ];
    const SEGMENTS: [number, number][] = [
        [0, 1], [1, 2],
        [1, 3], [3, 4],
        [1, 5], [5, 6],
        [2, 7], [7, 8],
        [2, 9], [9, 10],
    ];

    function buildViewMatrix(camPos: [number, number, number], rot: number[][]): number[][] {
        const r00 = rot[0][0], r01 = rot[1][0], r02 = rot[2][0];
        const r10 = rot[0][1], r11 = rot[1][1], r12 = rot[2][1];
        const r20 = rot[0][2], r21 = rot[1][2], r22 = rot[2][2];
        const tx = -(r00 * camPos[0] + r01 * camPos[1] + r02 * camPos[2]);
        const ty = -(r10 * camPos[0] + r11 * camPos[1] + r12 * camPos[2]);
        const tz = -(r20 * camPos[0] + r21 * camPos[1] + r22 * camPos[2]);
        return [
            [r00, r01, r02, tx],
            [r10, r11, r12, ty],
            [r20, r21, r22, tz],
            [0, 0, 0, 1],
        ];
    }
    function buildProjMatrix(fovDeg: number, aspect: number, near: number, far: number): number[][] {
        const f = 1 / Math.tan((fovDeg * Math.PI / 180) / 2);
        return [
            [f / aspect, 0, 0, 0],
            [0, f, 0, 0],
            [0, 0, (far + near) / (near - far), (2 * far * near) / (near - far)],
            [0, 0, -1, 0],
        ];
    }
    function worldToNdc(p: [number, number, number], v: number[][], pm: number[][]): [number, number] | null {
        const vx = v[0][0]*p[0] + v[0][1]*p[1] + v[0][2]*p[2] + v[0][3];
        const vy = v[1][0]*p[0] + v[1][1]*p[1] + v[1][2]*p[2] + v[1][3];
        const vz = v[2][0]*p[0] + v[2][1]*p[1] + v[2][2]*p[2] + v[2][3];
        const vw = v[3][0]*p[0] + v[3][1]*p[1] + v[3][2]*p[2] + v[3][3];
        const cx = pm[0][0]*vx + pm[0][1]*vy + pm[0][2]*vz + pm[0][3]*vw;
        const cy = pm[1][0]*vx + pm[1][1]*vy + pm[1][2]*vz + pm[1][3]*vw;
        const cw = pm[3][0]*vx + pm[3][1]*vy + pm[3][2]*vz + pm[3][3]*vw;
        if (cw < 0.1) return null;
        return [cx / cw, cy / cw];
    }
    function ndcToPx(ndc: [number, number]): { x: number; y: number } {
        return { x: (ndc[0] + 1) * 0.5 * W, y: (1 - ndc[1]) * 0.5 * H };
    }

    const view = buildViewMatrix(CAM_POS, ROT);
    const proj = buildProjMatrix(FOV, W / H, NEAR, FAR);

    // Project once at script init — the camera and skeleton are static.
    const projected = SKELETON_PARTS.map(p => worldToNdc(p, view, proj));
    const skeletonPx = SEGMENTS.map(([a, b]) => {
        const pa = projected[a], pb = projected[b];
        if (!pa || !pb) return null;
        return { a: ndcToPx(pa), b: ndcToPx(pb) };
    }).filter((x): x is { a: { x: number; y: number }; b: { x: number; y: number } } => x !== null);

    // 2-D box = bounding rect of every projected joint.
    let bxmin = Infinity, bymin = Infinity, bxmax = -Infinity, bymax = -Infinity;
    for (const p of projected) {
        if (!p) continue;
        const px = ndcToPx(p);
        if (px.x < bxmin) bxmin = px.x;
        if (px.y < bymin) bymin = px.y;
        if (px.x > bxmax) bxmax = px.x;
        if (px.y > bymax) bymax = px.y;
    }
    const padX = (bxmax - bxmin) * 0.18;
    const padY = (bymax - bymin) * 0.05;
    const BOX = { x: bxmin - padX, y: bymin - padY, w: bxmax - bxmin + 2 * padX, h: bymax - bymin + 2 * padY };
    const headPx = projected[0] ? ndcToPx(projected[0]) : { x: W / 2, y: H * 0.2 };

    // Mock player data
    const NAME = "Dummy";
    const DIST = 42;
    const HP = 87;

    // Sample avatar pulled for the preview. "Roblox" is a permanent account so
    // the request is reliable; if the fetch fails we just leave a plain border
    // ring, which still demonstrates positioning.
    let previewAvatarUrl: string | null = null;
    onMount(() => { getAvatar("Roblox").then(u => { previewAvatarUrl = u; }); });

    // Anchor → box-edge pixel coordinates (preview-space).
    function boxAnchorX(a: "L" | "C" | "R"): number {
        return a === "L" ? BOX.x : a === "R" ? BOX.x + BOX.w : BOX.x + BOX.w / 2;
    }
    function boxAnchorY(a: "T" | "C" | "B"): number {
        return a === "T" ? BOX.y : a === "B" ? BOX.y + BOX.h : BOX.y + BOX.h / 2;
    }
    function pickAnchorX(px: number): "L" | "C" | "R" {
        const f = (px - BOX.x) / BOX.w;
        return f < 0.33 ? "L" : f > 0.66 ? "R" : "C";
    }
    function pickAnchorY(py: number): "T" | "C" | "B" {
        const f = (py - BOX.y) / BOX.h;
        return f < 0.33 ? "T" : f > 0.66 ? "B" : "C";
    }

    let dragging: string | null = null;
    let activePointer = -1;
    let dragStart = { cx: 0, cy: 0, stageLeft: 0, stageTop: 0 };

    function startDrag(kind: "name" | "distance" | "health" | "avatar", e: PointerEvent) {
        e.stopPropagation();
        e.preventDefault();
        dragging = kind;
        activePointer = e.pointerId;
        const target = e.currentTarget as HTMLElement;
        target.setPointerCapture(e.pointerId);
        const stage = target.parentElement as HTMLElement;
        const sr = stage.getBoundingClientRect();
        const lr = target.getBoundingClientRect();
        // Offset between label center and the cursor at grab — keeps the
        // label from snapping under the pointer when drag starts.
        dragStart = {
            cx: (lr.left + lr.width / 2) - e.clientX,
            cy: (lr.top + lr.height / 2) - e.clientY,
            stageLeft: sr.left,
            stageTop: sr.top,
        };
    }
    function onMove(e: PointerEvent) {
        if (!dragging || e.pointerId !== activePointer) return;
        const px = (e.clientX + dragStart.cx) - dragStart.stageLeft;
        const py = (e.clientY + dragStart.cy) - dragStart.stageTop;
        const ax = pickAnchorX(px);
        const ay = pickAnchorY(py);
        const ox = px - boxAnchorX(ax);
        const oy = py - boxAnchorY(ay);
        visuals.update(v => {
            if (dragging === "name") {
                v.names_anchor_x = ax; v.names_anchor_y = ay;
                v.names_offset_x = ox; v.names_offset_y = oy;
            } else if (dragging === "distance") {
                v.distance_anchor_x = ax; v.distance_anchor_y = ay;
                v.distance_offset_x = ox; v.distance_offset_y = oy;
            } else if (dragging === "health") {
                v.health_anchor_x = ax; v.health_anchor_y = ay;
                v.health_offset_x = ox; v.health_offset_y = oy;
            } else if (dragging === "avatar") {
                v.avatar_head_anchor_x = ax; v.avatar_head_anchor_y = ay;
                v.avatar_head_offset_x = ox; v.avatar_head_offset_y = oy;
            }
            return v;
        });
    }
    function endDrag(e: PointerEvent) {
        if (e.pointerId !== activePointer) return;
        try { (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId); } catch {}
        dragging = null;
        activePointer = -1;
    }

    $: nameX = boxAnchorX($visuals.names_anchor_x) + $visuals.names_offset_x;
    $: nameY = boxAnchorY($visuals.names_anchor_y) + $visuals.names_offset_y;
    $: distX = boxAnchorX($visuals.distance_anchor_x) + $visuals.distance_offset_x;
    $: distY = boxAnchorY($visuals.distance_anchor_y) + $visuals.distance_offset_y;
    $: hpX   = boxAnchorX($visuals.health_anchor_x)   + $visuals.health_offset_x;
    $: hpY   = boxAnchorY($visuals.health_anchor_y)   + $visuals.health_offset_y;
    $: nameStyle = `left: ${nameX}px; top: ${nameY}px; color: ${$visuals.names_color};`;
    $: distStyle = `left: ${distX}px; top: ${distY}px; color: ${$visuals.distance_color};`;
    $: hpStyle   = `left: ${hpX}px;   top: ${hpY}px;   color: ${$visuals.health_color};`;

    // Avatar circle — fixed pixel diameter so the on-screen size is the
    // same regardless of player distance (matches the real ESP).
    $: avatarDiameter = $visuals.avatar_head_size;
    $: avatarCx = boxAnchorX($visuals.avatar_head_anchor_x) + $visuals.avatar_head_offset_x;
    $: avatarCy = boxAnchorY($visuals.avatar_head_anchor_y) + $visuals.avatar_head_offset_y;
    $: avatarStyle =
        `left: ${avatarCx}px; top: ${avatarCy}px; ` +
        `width: ${avatarDiameter}px; height: ${avatarDiameter}px; ` +
        `border: ${$visuals.avatar_head_border_thickness}px solid ${$visuals.avatar_head_border_color};` +
        ($visuals.outline ? ` box-shadow: 0 0 0 1px rgba(0,0,0,0.85);` : ``);
</script>

<div class="stage" style="width: {W}px; height: {H}px;">
    <img src="/dummy.png" alt="" class="dummy" draggable="false" />

    <svg class="layer" viewBox="0 0 {W} {H}">
        {#if $visuals.tracers}
            {#if $visuals.outline}
                <line x1={W/2} y1={H} x2={headPx.x} y2={headPx.y}
                      stroke="rgba(0,0,0,0.75)" stroke-width={$visuals.tracers_thickness + 1.5} />
            {/if}
            <line x1={W/2} y1={H} x2={headPx.x} y2={headPx.y}
                  stroke={$visuals.tracers_color} stroke-width={$visuals.tracers_thickness} />
        {/if}

        {#if $visuals.skeleton}
            {#each skeletonPx as s}
                {#if $visuals.outline}
                    <line x1={s.a.x} y1={s.a.y} x2={s.b.x} y2={s.b.y}
                          stroke="rgba(0,0,0,0.75)" stroke-width={$visuals.skeleton_thickness + 1.5} />
                {/if}
                <line x1={s.a.x} y1={s.a.y} x2={s.b.x} y2={s.b.y}
                      stroke={$visuals.skeleton_color} stroke-width={$visuals.skeleton_thickness} />
            {/each}
        {/if}
    </svg>

    {#if $visuals.box2d}
        {#if $visuals.box2d_style === "Corner"}
            {@const cl = Math.min(BOX.w, BOX.h) * 0.25}
            {@const cornerPath =
                `M ${BOX.x} ${BOX.y + cl} L ${BOX.x} ${BOX.y} L ${BOX.x + cl} ${BOX.y} ` +
                `M ${BOX.x + BOX.w - cl} ${BOX.y} L ${BOX.x + BOX.w} ${BOX.y} L ${BOX.x + BOX.w} ${BOX.y + cl} ` +
                `M ${BOX.x} ${BOX.y + BOX.h - cl} L ${BOX.x} ${BOX.y + BOX.h} L ${BOX.x + cl} ${BOX.y + BOX.h} ` +
                `M ${BOX.x + BOX.w - cl} ${BOX.y + BOX.h} L ${BOX.x + BOX.w} ${BOX.y + BOX.h} L ${BOX.x + BOX.w} ${BOX.y + BOX.h - cl}`}
            {#if $visuals.box2d_fill}
                <div class="box2d" style="
                    left: {BOX.x}px; top: {BOX.y}px; width: {BOX.w}px; height: {BOX.h}px;
                    border-width: 0;
                    background: {$visuals.box2d_fill_color + '40'};
                "></div>
            {/if}
            <svg class="layer" viewBox="0 0 {W} {H}">
                {#if $visuals.outline}
                    <path d={cornerPath} fill="none"
                          stroke="rgba(0,0,0,0.75)" stroke-width={$visuals.box2d_thickness + 1.5} />
                {/if}
                <path d={cornerPath} fill="none"
                      stroke={$visuals.box2d_color} stroke-width={$visuals.box2d_thickness} />
            </svg>
        {:else}
            <div class="box2d" style="
                left: {BOX.x}px; top: {BOX.y}px; width: {BOX.w}px; height: {BOX.h}px;
                border-color: {$visuals.box2d_color};
                border-width: {$visuals.box2d_thickness}px;
                box-shadow: {$visuals.outline ? '0 0 0 1px rgba(0,0,0,0.85)' : 'none'};
                background: {$visuals.box2d_fill ? $visuals.box2d_fill_color + '40' : 'transparent'};
            "></div>
        {/if}
    {/if}

    {#if $visuals.avatar_head}
        <div class="avatar" style={avatarStyle}
             class:dragging={dragging === 'avatar'}
             on:pointerdown={(e) => startDrag('avatar', e)}
             on:pointermove={onMove}
             on:pointerup={endDrag}
             on:pointercancel={endDrag}>
            {#if previewAvatarUrl}
                <img src={previewAvatarUrl} alt="" draggable="false" />
            {/if}
        </div>
    {/if}

    {#if $visuals.health_bar}
        <div class="hpbar" style="
            left: {BOX.x - 6}px; top: {BOX.y}px; height: {BOX.h}px;
            outline: {$visuals.outline ? '1px solid rgba(0,0,0,0.7)' : 'none'};
        ">
            <div class="hpbar-fill" style="
                background: {$visuals.health_bar_color};
                height: {HP}%;
            "></div>
        </div>
    {/if}

    {#if $visuals.names}
        <span class="label" style={nameStyle}
              class:dragging={dragging === 'name'}
              class:outline={$visuals.outline}
              on:pointerdown={(e) => startDrag('name', e)}
              on:pointermove={onMove}
              on:pointerup={endDrag}
              on:pointercancel={endDrag}>{NAME}</span>
    {/if}
    {#if $visuals.distance}
        <span class="label small" style={distStyle}
              class:dragging={dragging === 'distance'}
              class:outline={$visuals.outline}
              on:pointerdown={(e) => startDrag('distance', e)}
              on:pointermove={onMove}
              on:pointerup={endDrag}
              on:pointercancel={endDrag}>{DIST}m</span>
    {/if}
    {#if $visuals.health}
        <span class="label small" style={hpStyle}
              class:dragging={dragging === 'health'}
              class:outline={$visuals.outline}
              on:pointerdown={(e) => startDrag('health', e)}
              on:pointermove={onMove}
              on:pointerup={endDrag}
              on:pointercancel={endDrag}>{HP} hp</span>
    {/if}
</div>

<style>
    .stage {
        position: relative;
        background: linear-gradient(180deg, #1a1d2c 0%, #0a0c14 100%);
        border: 1px solid var(--border);
        border-radius: 6px;
        overflow: hidden;
        margin: 0 auto;
        user-select: none;
    }
    .dummy {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: contain;
        pointer-events: none;
    }
    .layer {
        position: absolute;
        inset: 0;
        pointer-events: none;
    }
    .box2d {
        position: absolute;
        border-style: solid;
        pointer-events: none;
    }
    .hpbar {
        position: absolute;
        width: 3px;
        background: rgba(0, 0, 0, 0.5);
        pointer-events: none;
        display: flex;
        flex-direction: column-reverse;
    }
    .hpbar-fill { width: 100%; }
    .avatar {
        position: absolute;
        transform: translate(-50%, -50%);
        border-radius: 50%;
        background: rgba(0, 0, 0, 0.4);
        overflow: hidden;
        cursor: grab;
        box-sizing: border-box;
    }
    .avatar img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
        pointer-events: none;
    }
    .avatar.dragging { cursor: grabbing; }
    .avatar:hover {
        outline: 1px dashed rgba(255, 222, 157, 0.6);
        outline-offset: 2px;
    }
    .label {
        position: absolute;
        transform: translate(-50%, -50%);
        font-size: 11px;
        font-weight: 600;
        white-space: nowrap;
        cursor: grab;
        font-variant-numeric: tabular-nums;
    }
    .label.small { font-size: 10px; font-weight: 500; }
    .label.outline {
        text-shadow:
            -1px -1px 0 rgba(0,0,0,0.85),
             1px -1px 0 rgba(0,0,0,0.85),
            -1px  1px 0 rgba(0,0,0,0.85),
             1px  1px 0 rgba(0,0,0,0.85);
    }
    .label.dragging { cursor: grabbing; }
    .label:hover {
        outline: 1px dashed rgba(255, 222, 157, 0.6);
        outline-offset: 2px;
    }
</style>
