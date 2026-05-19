# 11 — FOV colour pickers + smoothing inversion

## But

Deux ajustements complémentaires :

1. **Color pickers** pour la bordure et le fill du cercle FOV (alignés à droite des toggles `Show FOV` / `Fill FOV`). Defaults conservés à `#ffde9d`.
2. **Inversion de la sémantique du slider Smoothing** :
   - `smoothing = 0` → snap instantané (max puissance)
   - `smoothing = 1` → lerp très lent (max smoothness)
   - Default monté de `0.15` à `0.85` pour conserver le feel "smooth doux" qu'on avait avant l'inversion.

## Fichiers touchés

- `src/lib/menu/stores.ts` — `fov_color`, `fov_fill_color` ajoutés (default `#ffde9d`), `smoothing` default 0.15 → 0.85.
- `src-tauri/src/state.rs` — miroir.
- `src/lib/menu/controls/ColorPicker.svelte` — **nouveau** composant, swatch carré qui ouvre le picker natif `input[type=color]`.
- `src/lib/menu/tabs/Aiming.svelte` — `Show FOV` et `Fill FOV` placés dans des `.row` flex avec un `<ColorPicker>` aligné droite.
- `src-tauri/src/aimbot.rs` — `aim_camera` et `aim_mouse` inversent : `lerp_t = (1 - smoothing).max(0.01)`.
- `src/routes/+page.svelte` — store subscribe étendu à `fov_color`/`fov_fill_color`, `drawFovCircle` prend deux hex en paramètre, helper `hexToRgba` pour le fill.

## ColorPicker

Composant 5-lignes : un `<label>` stylé en swatch coloré (24×18, border-radius 4), avec un `<input type="color">` invisible par-dessus. Le `bind:value` remonte le hex sélectionné. Hover → bord crème.

```svelte
<label class="swatch" style="background: {value}" {title}>
    <input type="color" bind:value />
</label>
```

## Smoothing inversion

Côté Rust, dans les deux branches de aim :

```rust
// Mouse
let smooth = (1.0 - cfg.smoothing as f64).max(0.01);
nx = lerp(cur.x, sx, smooth);

// Camera
let s = (1.0 - smoothing.clamp(0.0, 0.99)).max(0.01);
new_m[i][j] = current[i][j] + (target_m[i][j] - current[i][j]) * s;
```

Le slider UI garde la même plage [0.01..1] et le même format `0.85/1.00`, seule l'interprétation change : haut = plus de lissage, bas = plus de puissance.

## Rendu front

```ts
function hexToRgba(hex: string, alpha: number): string {
    const h = hex.replace("#", "");
    return `rgba(${parseInt(h.slice(0,2),16)}, ${parseInt(h.slice(2,4),16)}, ${parseInt(h.slice(4,6),16)}, ${alpha})`;
}

function drawFovCircle(ctx, cx, cy, r, fill, borderHex, fillHex) {
    ctx.arc(cx, cy, r, 0, Math.PI*2);
    if (fill) { ctx.fillStyle = hexToRgba(fillHex, 0.5); ctx.fill(); }
    ctx.lineWidth = 3; ctx.strokeStyle = "rgba(0,0,0,0.7)"; ctx.stroke();
    ctx.lineWidth = 1.5; ctx.strokeStyle = borderHex; ctx.stroke();
}
```

Le hex de bordure est utilisé en opaque, le hex de fill passe par `hexToRgba(..., 0.5)`. Le halo noir reste hardcodé pour la lisibilité.

## Comment tester

1. `npm run tauri dev`.
2. Aimbot → coche **Show FOV**. Le cercle apparaît en crème. Clique sur le swatch à droite → picker système ; choisis par exemple `#5cffb8` (vert). Le cercle vire en vert instantanément.
3. Coche **Fill FOV** → l'intérieur se remplit à 50 % d'opacité avec la couleur fill courante. Change le swatch fill → la teinte intérieure suit.
4. **Smoothing** : mets le slider à 0.0, lock un joueur → snap instantané. Monte à 0.95 → la convergence devient très lente (camera/cursor glisse sur ~1 seconde). À 0.85 (default), feel équivalent à l'ancien default 0.15.
