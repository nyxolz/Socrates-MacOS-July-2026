<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import Menu from "$lib/menu/Menu.svelte";
    import { visuals, aimbot, type VisualsCfg, type AimbotCfg } from "$lib/menu/stores";

    type Marker = {
        name: string;
        ndc_x: number; ndc_y: number;
        on_screen: boolean;
        distance: number;
        world: [number, number, number];
    };
    type EspPlayer = {
        name: string;
        distance: number;
        health: number;
        head: [number, number] | null;
        box2d: [number, number, number, number] | null;
        box3d: [number, number, number, number][];
        view_dir: [number, number, number, number] | null;
        skeleton: [number, number, number, number][];
    };
    type EspFrame = {
        markers: Marker[];
        players: EspPlayer[];
        player_count: number;
        camera_pos: [number, number, number];
        fov: number;
        aspect: number;
        attached: boolean;
        cursor: [number, number] | null;
    };

    const TITLEBAR_HEIGHT = 28;

    let canvas: HTMLCanvasElement;
    let unlistenFrame: UnlistenFn | undefined;
    let lastFrame: EspFrame | null = null;
    let rafHandle = 0;

    let vcfg: VisualsCfg | null = null;
    let acfg: AimbotCfg | null = null;
    visuals.subscribe((v) => { vcfg = v; });
    aimbot.subscribe((a) => { acfg = a; });

    function hexToRgba(hex: string, alpha: number): string {
        const h = hex.replace("#", "");
        const r = parseInt(h.slice(0, 2), 16) || 0;
        const g = parseInt(h.slice(2, 4), 16) || 0;
        const b = parseInt(h.slice(4, 6), 16) || 0;
        return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }

    function resizeCanvas() {
        const dpr = window.devicePixelRatio || 1;
        canvas.width = window.innerWidth * dpr;
        canvas.height = window.innerHeight * dpr;
        canvas.style.width = `${window.innerWidth}px`;
        canvas.style.height = `${window.innerHeight}px`;
        const ctx = canvas.getContext("2d");
        ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function ndcToScreen(nx: number, ny: number) {
        const w = window.innerWidth;
        const h = window.innerHeight;
        const renderH = h - TITLEBAR_HEIGHT;
        return {
            x: (nx + 1.0) * 0.5 * w,
            y: TITLEBAR_HEIGHT + (1.0 - ny) * 0.5 * renderH,
        };
    }

    // ─── Drawing primitives with optional black outline ─────────────────
    function strokeSegments(ctx: CanvasRenderingContext2D, segs: [number, number, number, number][],
                            color: string, thickness: number, outline: boolean) {
        if (!segs.length) return;
        ctx.beginPath();
        for (const s of segs) {
            const a = ndcToScreen(s[0], s[1]);
            const b = ndcToScreen(s[2], s[3]);
            ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y);
        }
        if (outline) {
            ctx.lineWidth = thickness + 1.5;
            ctx.strokeStyle = "rgba(0, 0, 0, 0.75)";
            ctx.stroke();
        }
        ctx.lineWidth = thickness;
        ctx.strokeStyle = color;
        ctx.stroke();
    }

    function strokeBox2D(ctx: CanvasRenderingContext2D, box: [number, number, number, number],
                         color: string, thickness: number, outline: boolean,
                         fill: boolean, fillColor: string) {
        const a = ndcToScreen(box[0], box[3]);
        const b = ndcToScreen(box[2], box[1]);
        const x = a.x, y = a.y, w = b.x - a.x, h = b.y - a.y;
        if (fill) {
            ctx.fillStyle = hexToRgba(fillColor, 0.25);
            ctx.fillRect(x, y, w, h);
        }
        if (outline) {
            ctx.lineWidth = thickness + 1.5;
            ctx.strokeStyle = "rgba(0, 0, 0, 0.75)";
            ctx.strokeRect(x, y, w, h);
        }
        ctx.lineWidth = thickness;
        ctx.strokeStyle = color;
        ctx.strokeRect(x, y, w, h);
    }

    function drawText(ctx: CanvasRenderingContext2D, text: string, x: number, y: number,
                      color: string, outline: boolean, size = 11) {
        ctx.font = `${size}px -apple-system, "Segoe UI", sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "alphabetic";
        if (outline) {
            ctx.lineWidth = 3;
            ctx.strokeStyle = "rgba(0, 0, 0, 0.85)";
            ctx.strokeText(text, x, y);
        }
        ctx.fillStyle = color;
        ctx.fillText(text, x, y);
    }

    function drawHealthBar(ctx: CanvasRenderingContext2D, box: [number, number, number, number],
                           health: number, color: string, outline: boolean) {
        if (health < 0) return;
        const a = ndcToScreen(box[0], box[3]);
        const b = ndcToScreen(box[2], box[1]);
        const barX = a.x - 5;
        const barY = a.y;
        const barW = 3;
        const barH = b.y - a.y;
        const fillH = barH * (health / 100);
        if (outline) {
            ctx.fillStyle = "rgba(0, 0, 0, 0.7)";
            ctx.fillRect(barX - 1, barY - 1, barW + 2, barH + 2);
        }
        ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
        ctx.fillRect(barX, barY, barW, barH);
        ctx.fillStyle = color;
        ctx.fillRect(barX, barY + (barH - fillH), barW, fillH);
    }

    function drawTracer(ctx: CanvasRenderingContext2D, head: [number, number],
                        origin: "Top" | "Center" | "Bottom",
                        color: string, thickness: number, outline: boolean) {
        const h = window.innerHeight;
        const renderH = h - TITLEBAR_HEIGHT;
        const w = window.innerWidth;
        let ox = w / 2, oy = TITLEBAR_HEIGHT + renderH / 2;
        if (origin === "Top") oy = TITLEBAR_HEIGHT;
        else if (origin === "Bottom") oy = h;
        const head_px = ndcToScreen(head[0], head[1]);
        strokeSegments(ctx,
            [[(2 * ox / w) - 1, 1 - (2 * (oy - TITLEBAR_HEIGHT) / renderH), head[0], head[1]]],
            color, thickness, outline);
        // The above mixes NDC and pixels — simpler: stroke directly.
        ctx.beginPath();
        ctx.moveTo(ox, oy);
        ctx.lineTo(head_px.x, head_px.y);
        if (outline) {
            ctx.lineWidth = thickness + 1.5;
            ctx.strokeStyle = "rgba(0, 0, 0, 0.75)";
            ctx.stroke();
        }
        ctx.lineWidth = thickness;
        ctx.strokeStyle = color;
        ctx.stroke();
    }

    function drawFovCircle(ctx: CanvasRenderingContext2D,
                           cx: number, cy: number, r: number,
                           fill: boolean, borderHex: string, fillHex: string) {
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        if (fill) {
            ctx.fillStyle = hexToRgba(fillHex, 0.5);
            ctx.fill();
        }
        ctx.lineWidth = 3;
        ctx.strokeStyle = "rgba(0, 0, 0, 0.7)";
        ctx.stroke();
        ctx.lineWidth = 1.5;
        ctx.strokeStyle = borderHex;
        ctx.stroke();
    }

    function renderLoop() {
        const ctx = canvas?.getContext("2d");
        if (ctx && lastFrame && vcfg) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            const cfg = vcfg;
            const outline = cfg.outline;

            for (const p of lastFrame.players) {
                if (cfg.tracers && p.head) {
                    drawTracer(ctx, p.head, cfg.tracers_origin,
                        cfg.tracers_color, cfg.tracers_thickness, outline);
                }
                if (cfg.box3d && p.box3d.length) {
                    strokeSegments(ctx, p.box3d, cfg.box3d_color, cfg.box3d_thickness, outline);
                    // Note: true 3-D fill would need per-face polygons —
                    // skipped for now, only the wireframe draws.
                }
                if (cfg.skeleton && p.skeleton.length) {
                    strokeSegments(ctx, p.skeleton, cfg.skeleton_color, cfg.skeleton_thickness, outline);
                }
                if (cfg.view_dir && p.view_dir) {
                    strokeSegments(ctx, [p.view_dir], cfg.view_dir_color, 1.5, outline);
                }
                if (cfg.box2d && p.box2d) {
                    strokeBox2D(ctx, p.box2d, cfg.box2d_color, cfg.box2d_thickness, outline,
                        cfg.box2d_fill, cfg.box2d_fill_color);
                }
                if (cfg.health_bar && p.box2d) {
                    drawHealthBar(ctx, p.box2d, p.health, cfg.health_bar_color, outline);
                }
                if (p.head) {
                    const hp = ndcToScreen(p.head[0], p.head[1]);
                    if (cfg.names) {
                        drawText(ctx, p.name || "?",
                            hp.x + cfg.names_offset_x, hp.y + cfg.names_offset_y,
                            cfg.names_color, outline);
                    }
                    if (cfg.distance) {
                        drawText(ctx, `${p.distance.toFixed(0)}m`,
                            hp.x + cfg.distance_offset_x, hp.y + cfg.distance_offset_y,
                            cfg.distance_color, outline, 10);
                    }
                    if (cfg.health && p.health >= 0) {
                        drawText(ctx, `${p.health.toFixed(0)} hp`,
                            hp.x + cfg.health_offset_x, hp.y + cfg.health_offset_y,
                            cfg.health_color, outline, 10);
                    }
                }
            }

            // FOV circle on top of player overlays
            if (acfg?.show_fov) {
                let cx: number, cy: number;
                if (acfg.aim_type === "Mouse" && lastFrame.cursor) {
                    cx = lastFrame.cursor[0]; cy = lastFrame.cursor[1];
                } else {
                    const renderH = window.innerHeight - TITLEBAR_HEIGHT;
                    cx = window.innerWidth / 2;
                    cy = TITLEBAR_HEIGHT + renderH / 2;
                }
                drawFovCircle(ctx, cx, cy, acfg.fov_radius, acfg.fill_fov, acfg.fov_color, acfg.fov_fill_color);
            }
        }
        rafHandle = requestAnimationFrame(renderLoop);
    }

    onMount(async () => {
        resizeCanvas();
        window.addEventListener("resize", resizeCanvas);
        unlistenFrame = await listen<EspFrame>("esp_frame", (e) => { lastFrame = e.payload; });
        rafHandle = requestAnimationFrame(renderLoop);
    });

    onDestroy(() => {
        cancelAnimationFrame(rafHandle);
        unlistenFrame?.();
        window.removeEventListener("resize", resizeCanvas);
    });
</script>

<canvas bind:this={canvas} class="overlay"></canvas>

<Menu />

<style>
    .overlay {
        position: fixed;
        top: 0;
        left: 0;
        pointer-events: none;
    }
</style>
