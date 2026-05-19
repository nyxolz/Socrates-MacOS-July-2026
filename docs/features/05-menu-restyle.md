# 05 — Menu restyle: wider, instant sliders, tab buttons

## But

Trois ajustements visuels :

1. **Largeur** : `MENU_W` 430 → **490 px**. Toujours portrait, plus aéré.
2. **Sliders sans transition** sur la position. Le knob et le fill suivent le drag à 1:1, plus de lag de 110 ms.
3. **Fill qui enrobe le knob**. La bande crème ne s'arrête plus au centre de la boule — elle file jusqu'au bord droit du knob + 2 px, donnant l'impression que le knob blanc est posé *sur* le fill.
4. **Onglets en boutons** avec border-radius ~20 % (6 px sur ~28 px de hauteur), border visible sur toutes les arêtes, et état actif marqué par couleur accent + glow.

## Fichiers touchés

| Fichier | État |
|---|---|
| `src/lib/menu/Menu.svelte` | `MENU_W: 430 → 490` |
| `src/lib/menu/controls/Slider.svelte` | retiré la transition `width` / `left`, nouveau calc pour `.fill` |
| `src/lib/menu/TabBar.svelte` | tabs en pill-buttons avec `flex-wrap`, border + radius + glow |

## Détails

### Fill calc

```css
.fill {
    width: calc((100% - var(--knob)) * var(--pct) / 100% + var(--knob) + 2px);
    max-width: 100%;
    border-radius: 8px;            /* arrondi des deux côtés pour épouser le knob */
}
```

- À `pct = 0`, `width = knob + 2 px` — la bande est juste sous la boule, légèrement débordante à droite.
- À `pct = 100`, le calc donnerait `100% + 2 px` ; le `max-width: 100%` clampe au bord du track.
- Pas de `transition` ⇒ le mouvement suit le pointeur instantanément.

Le knob reste positionné via `left: calc((100% - var(--knob)) * var(--pct) / 100%)` (même formule qu'avant), donc son centre va de `knob_w/2` à `track_w - knob_w/2` — toujours intégralement à l'intérieur du track.

### Tab buttons

```css
.tab {
    flex: 0 1 auto;                /* largeur naturelle */
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 11px;
}
.tab.active {
    background: rgba(255, 222, 157, 0.08);
    border-color: var(--accent);
    color: var(--accent);
    box-shadow:
        0 0 0 1px rgba(255, 222, 157, 0.15),  /* halo interne */
        0 0 8px var(--accent-glow);            /* halo externe */
}
```

- Container en `flex-wrap: wrap` avec `gap: 5px` et `padding: 8px` — les 9 tabs s'arrangent naturellement sur 1 ou 2 lignes selon la largeur.
- Hover : bord teinté accent à 25 %.
- Active : fond crème transparent + bord accent plein + double glow (1 px interne, 8 px externe).
- Transitions 140 ms `cubic-bezier(.4,0,.2,1)` sur background / border / color / box-shadow (uniquement les états, pas la position).

## Comment tester

1. `npm run tauri dev`.
2. **Largeur** : la fenêtre du menu fait maintenant 490 px de large.
3. **Slider** : drague le knob d'un slider. Pas de retard, le fill et le knob suivent le curseur exactement. La bande crème **dépasse le centre de la boule** et vient s'arrêter à son bord droit — la boule a l'air posée dessus.
4. **Tabs** : chaque onglet est maintenant un bouton arrondi avec un bord. Au survol, son bord vire au crème pâle. L'onglet actif est rempli de crème transparent, bord crème plein, halo lumineux autour.
