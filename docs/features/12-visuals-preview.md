# 12 — Visuals preview with draggable text offsets

## But

Bloc "Preview" en bas à droite de l'onglet Visuals. Contient une silhouette de joueur Roblox sur laquelle on superpose les ESP actives (skeleton/box2d/tracer/health-bar/name/distance/health). Les labels texte sont **draggables** : repositionner Name / Distance / Health dans le preview met à jour des offsets dans le store, et le rendu réel sur les joueurs en jeu utilise les mêmes offsets.

## Fichiers touchés

- `static/dummy.png` — copie de `Dummy_Cinque-Photoroom.png` à la racine static pour servir via SvelteKit.
- `src/lib/menu/stores.ts` — 6 nouveaux champs : `names_offset_x/y`, `distance_offset_x/y`, `health_offset_x/y` (defaults reproduisent les positions hardcodées d'avant : name à -14 px, distance à +22, health à +34).
- `src-tauri/src/state.rs` — miroir des 6 offsets.
- `src/lib/menu/Preview.svelte` — **nouveau** composant 200×280 px : image dummy + overlay SVG (tracer) + box2d en `div` borduré + health-bar verticale + 3 labels absolus.
- `src/lib/menu/tabs/Visuals.svelte` — section "Preview" ajoutée au bas de la colonne droite, avec un hint expliquant le drag.
- `src/routes/+page.svelte` — le rendu canvas des 3 textes (`drawText`) utilise désormais `hp.x + offset_x`, `hp.y + offset_y` au lieu des positions hardcodées.

## Preview internals

```ts
const HEAD = { x: W/2, y: H * 0.22 };
$: nameStyle = `left: ${HEAD.x + $visuals.names_offset_x}px;
                top:  ${HEAD.y + $visuals.names_offset_y}px;
                color: ${$visuals.names_color};`;
```

Chaque label est un `<span>` absolu avec `transform: translate(-50%, -50%)`, sur `pointerdown` :
1. `setPointerCapture(e.pointerId)` — survit aux passages hors du label.
2. Stocke `(dragStart.x, dragStart.y, ox, oy)`.
3. `pointermove` calcule `dx, dy` et met à jour `visuals.update(v => { v.names_offset_* = ox + dx; ... })`.

Le store push automatiquement vers le Rust via le bridge existant — donc les offsets persistent côté backend aussi.

L'outline noir des labels (toggle `visuals.outline`) est un `text-shadow` 4-direction qui simule un stroke 1 px.

### Éléments affichés

| Élément | Rendu |
|---|---|
| Dummy | `<img src="/dummy.png">` plein cadre, `object-fit: contain` |
| Tracer | `<svg><line>` du bas du preview vers la tête (avec halo noir si outline) |
| Box 2D | `<div>` borduré, fill `+40` alpha hex si `box2d_fill` |
| Health bar | `<div>` 3 px à gauche du box2d, fill vertical (mock 87 %) |
| Name / Distance / Health | `<span>` draggable, position = HEAD + offset depuis le store |

Les colors / thickness / fill viennent toutes du store `visuals` en réactif — toggler un picker met à jour le preview immédiatement.

## Rendu réel synchronisé

Dans `+page.svelte` :

```ts
if (cfg.names) {
    drawText(ctx, p.name,
        hp.x + cfg.names_offset_x,
        hp.y + cfg.names_offset_y,
        cfg.names_color, outline);
}
```

(idem distance et health) — donc déplacer un label dans le preview déplace **immédiatement** le texte sur tous les joueurs en jeu.

## Comment tester

1. `npm run tauri dev`.
2. Visuals → coche **Name** (et Distance et Health). Le preview en bas à droite montre la silhouette avec les labels par défaut (name au-dessus de la tête, dist + hp en dessous).
3. Drag le label "Dummy" 30 px à gauche → il glisse dans le preview, et dans la fenêtre Roblox les noms des joueurs réels glissent du même offset.
4. Active **Box 2D** + **Black outline** → le rect apparaît bordé sur le preview ET sur les joueurs.
5. Coche **Health bar** + **Tracers** + **Skeleton** : tout se cumule sur le preview pour vérifier le rendu sans avoir à rejoindre un game.
6. Pickers couleur : changer la couleur dans la UI met à jour le preview et l'in-game en temps réel (le label texte change de teinte tout en gardant son outline noir).
