# 10 — FOV circle overlay

## But

Deux toggles ajoutés dans la section Aimbot, juste sous `FOV radius` :

- **Show FOV** — dessine un cercle de rayon `fov_radius` sur l'overlay (couleur accent crème + mini outline noir).
- **Fill FOV** — remplit ce cercle avec la même couleur crème en 50 % d'alpha.

Le centre du cercle dépend du mode aim :
- **Mouse** — suit la position du curseur OS (cohérent avec le scoring cursor-based de la feature 09).
- **Camera** — ancré au centre de la zone de rendu Roblox.

## Fichiers touchés

- `src/lib/menu/stores.ts` — `show_fov`, `fill_fov` ajoutés à `AimbotCfg`.
- `src-tauri/src/state.rs` — miroir.
- `src/lib/menu/tabs/Aiming.svelte` — deux toggles sous le slider FOV radius.
- `src-tauri/src/aimbot.rs` — `current_cursor` passe en `pub`.
- `src-tauri/src/esp.rs` — `EspFrame.cursor: Option<(f64, f64)>` (px window-local), calculé via `current_cursor() - bounds_origin`.
- `src/routes/+page.svelte` — subscribe au store `aimbot`, fonction `drawFovCircle`, intégration dans `renderLoop`.

## Rendu

```ts
function drawFovCircle(ctx, cx, cy, r, fill) {
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    if (fill) {
        ctx.fillStyle = "rgba(255, 222, 157, 0.5)";
        ctx.fill();
    }
    // black halo first, cream stroke on top
    ctx.lineWidth = 3;
    ctx.strokeStyle = "rgba(0, 0, 0, 0.7)";
    ctx.stroke();
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = "rgba(255, 222, 157, 1)";
    ctx.stroke();
}
```

Le double-stroke (3 px noir puis 1.5 px crème) laisse ~0.75 px d'outline noir de chaque côté du trait crème — lisible sur n'importe quel fond de jeu.

## Position du centre

```ts
if (aimType === "Mouse" && lastFrame.cursor) {
    cx = lastFrame.cursor[0];
    cy = lastFrame.cursor[1];
} else {
    const renderH = window.innerHeight - TITLEBAR_HEIGHT;
    cx = window.innerWidth / 2;
    cy = TITLEBAR_HEIGHT + renderH / 2;
}
```

Le curseur arrive du backend en coords overlay-local (`current_cursor() - bounds_origin`). L'overlay window étant collée à la fenêtre Roblox, ces coords matchent directement le canvas.

En mode Camera ou si le cursor n'est pas dispo, fallback sur le centre de la zone de rendu (skip de la titlebar macOS).

## Comment tester

1. `npm run tauri dev`.
2. Aimbot → coche **Show FOV**. Un cercle crème apparaît centré sur ton curseur (Mouse mode par défaut).
3. Bouge la souris → le cercle suit en temps réel.
4. Coche **Fill FOV** → l'intérieur se remplit en crème transparent 50 %.
5. Drag le slider FOV radius → le cercle grandit/rétrécit instantanément.
6. Passe Aim type en **Camera** → le cercle se fige au centre de la zone de rendu.
7. Vérifie l'outline noir : visible aussi bien sur un fond clair (skybox) que sombre (souterrain).
