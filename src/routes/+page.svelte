<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { invoke } from "@tauri-apps/api/core";
    import Menu from "$lib/menu/Menu.svelte";
    import { visuals, aimbot, menuOptions, theme, type VisualsCfg, type AimbotCfg, type MenuOptionsCfg } from "$lib/menu/stores";
    import { getAvatar } from "$lib/menu/avatar";

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
        relation: number; // 0 = enemy (full ESP), 1 = ally (green head dot)
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
    let mcfg: MenuOptionsCfg | null = null;
    let themeColor = "#ffde9d";
    visuals.subscribe((v) => { vcfg = v; });
    aimbot.subscribe((a) => { acfg = a; });
    menuOptions.subscribe((m) => { mcfg = m; });
    theme.subscribe((t) => { themeColor = t; });

    function hexToRgba(hex: string, alpha: number): string {
        const h = hex.replace("#", "");
        const r = parseInt(h.slice(0, 2), 16) || 0;
        const g = parseInt(h.slice(2, 4), 16) || 0;
        const b = parseInt(h.slice(4, 6), 16) || 0;
        return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }
    function hexToRgbTriplet(hex: string): string {
        const h = hex.replace("#", "");
        const r = parseInt(h.slice(0, 2), 16) || 0;
        const g = parseInt(h.slice(2, 4), 16) || 0;
        const b = parseInt(h.slice(4, 6), 16) || 0;
        return `${r}, ${g}, ${b}`;
    }

    // Snow particles. `z` ∈ [0,1] is depth — small/slow/dim at z=0 (far),
    // big/fast/opaque at z=1 (close). Drift = horizontal sway phase.
    type Snowflake = { x: number; y: number; z: number; sway: number };
    const SNOW_COUNT = 140;
    let snow: Snowflake[] = [];
    let lastSnowTick = 0;

    function makeSnowflake(spawnTop: boolean): Snowflake {
        return {
            x: Math.random() * window.innerWidth,
            y: spawnTop ? -8 - Math.random() * 40 : Math.random() * window.innerHeight,
            z: Math.random(),
            sway: Math.random() * Math.PI * 2,
        };
    }
    function initSnow() {
        snow = Array.from({ length: SNOW_COUNT }, () => makeSnowflake(false));
    }
    function drawSnow(ctx: CanvasRenderingContext2D, color: string, dt: number) {
        const w = window.innerWidth;
        const h = window.innerHeight;
        const rgb = hexToRgbTriplet(color);
        for (const s of snow) {
            const speed = 25 + s.z * 110;
            s.y += speed * dt;
            s.sway += (0.4 + s.z * 0.6) * dt * 2;
            s.x += Math.sin(s.sway) * (8 + s.z * 14) * dt;
            if (s.y > h + 8) Object.assign(s, makeSnowflake(true));
            if (s.x > w + 12) s.x = -12;
            if (s.x < -12) s.x = w + 12;
            const radius = 0.8 + s.z * 4.2;
            const alpha = 0.22 + s.z * 0.7;
            ctx.fillStyle = `rgba(${rgb}, ${alpha})`;
            ctx.beginPath();
            ctx.arc(s.x, s.y, radius, 0, Math.PI * 2);
            ctx.fill();
        }
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

    // Username → loaded HTMLImageElement. We fire the URL fetch + image
    // decode lazily on the first frame we see a player, and cache forever
    // after. `null` means "load in progress or failed" — we still try to
    // draw a placeholder so the position is visible.
    const avatarImages = new Map<string, HTMLImageElement | null>();
    function getAvatarImage(username: string): HTMLImageElement | null {
        if (!username) return null;
        if (avatarImages.has(username)) return avatarImages.get(username) ?? null;
        avatarImages.set(username, null);
        getAvatar(username).then(url => {
            if (!url) return;
            const img = new Image();
            img.crossOrigin = "anonymous";
            img.onload = () => avatarImages.set(username, img);
            img.src = url;
        });
        return null;
    }

    function drawAvatarHead(ctx: CanvasRenderingContext2D,
                            username: string,
                            cx: number, cy: number, diameter: number,
                            borderColor: string, borderThickness: number,
                            outline: boolean) {
        const r = diameter / 2;
        const img = getAvatarImage(username);

        ctx.save();
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.closePath();
        ctx.clip();
        if (img) {
            ctx.drawImage(img, cx - r, cy - r, diameter, diameter);
        } else {
            ctx.fillStyle = "rgba(0, 0, 0, 0.45)";
            ctx.fill();
        }
        ctx.restore();

        if (borderThickness > 0) {
            ctx.beginPath();
            ctx.arc(cx, cy, r, 0, Math.PI * 2);
            if (outline) {
                ctx.lineWidth = borderThickness + 1.5;
                ctx.strokeStyle = "rgba(0, 0, 0, 0.75)";
                ctx.stroke();
            }
            ctx.lineWidth = borderThickness;
            ctx.strokeStyle = borderColor;
            ctx.stroke();
        }
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
        const now = performance.now();
        const dt = lastSnowTick === 0 ? 0 : Math.min(0.05, (now - lastSnowTick) / 1000);
        lastSnowTick = now;
        if (ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            // Menu-visible decoration: dim the game (rounded rect, 30%
            // more transparent than before so the game stays readable
            // behind the menu) and rain themed snow underneath the ESP/UI.
            if (mcfg?.menu_visible) {
                const w = window.innerWidth;
                const h = window.innerHeight;
                ctx.fillStyle = "rgba(0, 0, 0, 0.4)";
                ctx.beginPath();
                if (typeof (ctx as any).roundRect === "function") {
                    (ctx as any).roundRect(0, 0, w, h, 14);
                } else {
                    ctx.rect(0, 0, w, h);
                }
                ctx.fill();
                if (mcfg.snow_enabled) drawSnow(ctx, themeColor, dt);
            }
        }
        if (ctx && lastFrame && vcfg) {
            const cfg = vcfg;
            const outline = cfg.outline;

            for (const p of lastFrame.players) {
                // Allies: a green "[Ally]" label above the head, no other ESP.
                // Backend already strips the geometry, but we still skip the
                // enemy draw block so name/distance/health labels don't show
                // for teammates.
                if (p.relation === 1) {
                    if (p.head) {
                        const hp = ndcToScreen(p.head[0], p.head[1]);
                        drawText(ctx, "[Ally]", hp.x, hp.y - 8, "#22e06a", outline, 10);
                    }
                    continue;
                }
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
                    // Anchor labels to the projected 2-D box so a "below
                    // the box" position in the preview stays "below the
                    // box" for every player regardless of distance. Falls
                    // back to head pixel if no box was projected.
                    let bL: number, bR: number, bT: number, bB: number;
                    if (p.box2d) {
                        const a = ndcToScreen(p.box2d[0], p.box2d[3]);
                        const b = ndcToScreen(p.box2d[2], p.box2d[1]);
                        bL = a.x; bT = a.y; bR = b.x; bB = b.y;
                    } else {
                        const hp = ndcToScreen(p.head[0], p.head[1]);
                        bL = bR = hp.x; bT = bB = hp.y;
                    }
                    const ax = (a: string) => a === "L" ? bL : a === "R" ? bR : (bL + bR) / 2;
                    const ay = (a: string) => a === "T" ? bT : a === "B" ? bB : (bT + bB) / 2;
                    if (cfg.avatar_head) {
                        const diameter = cfg.avatar_head_size;
                        const cx = ax(cfg.avatar_head_anchor_x) + cfg.avatar_head_offset_x;
                        const cy = ay(cfg.avatar_head_anchor_y) + cfg.avatar_head_offset_y;
                        drawAvatarHead(ctx, p.name, cx, cy, diameter,
                            cfg.avatar_head_border_color, cfg.avatar_head_border_thickness, outline);
                    }
                    if (cfg.names) {
                        drawText(ctx, p.name || "?",
                            ax(cfg.names_anchor_x) + cfg.names_offset_x,
                            ay(cfg.names_anchor_y) + cfg.names_offset_y,
                            cfg.names_color, outline);
                    }
                    if (cfg.distance) {
                        drawText(ctx, `${p.distance.toFixed(0)}m`,
                            ax(cfg.distance_anchor_x) + cfg.distance_offset_x,
                            ay(cfg.distance_anchor_y) + cfg.distance_offset_y,
                            cfg.distance_color, outline, 10);
                    }
                    if (cfg.health && p.health >= 0) {
                        drawText(ctx, `${p.health.toFixed(0)} hp`,
                            ax(cfg.health_anchor_x) + cfg.health_offset_x,
                            ay(cfg.health_anchor_y) + cfg.health_offset_y,
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

    let unlistenToggle: UnlistenFn | undefined;

    function onToggleKey(e: KeyboardEvent) {
        // Fallback for when the overlay actually has keyboard focus (menu
        // open, input focused); the Rust thread covers the click-through
        // case where Roblox owns the focus.
        const bind = mcfg?.toggle_bind;
        if (!bind) return;
        if (e.code === bind || e.key === bind) {
            e.preventDefault();
            menuOptions.update(m => ({ ...m, menu_visible: !m.menu_visible }));
        }
    }

    onMount(async () => {
        // Roblox-running gate happens in Rust (lib.rs::gate_on_roblox)
        // before the overlay window is even created. By the time we mount
        // here, Roblox is guaranteed to be running.
        resizeCanvas();
        initSnow();
        window.addEventListener("resize", resizeCanvas);
        window.addEventListener("keydown", onToggleKey, true);
        unlistenFrame = await listen<EspFrame>("esp_frame", (e) => { lastFrame = e.payload; });
        unlistenToggle = await listen("menu_toggle", () => {
            menuOptions.update(m => ({ ...m, menu_visible: !m.menu_visible }));
        });
        rafHandle = requestAnimationFrame(renderLoop);
    });

    onDestroy(() => {
        cancelAnimationFrame(rafHandle);
        unlistenFrame?.();
        unlistenToggle?.();
        window.removeEventListener("resize", resizeCanvas);
        window.removeEventListener("keydown", onToggleKey, true);
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
