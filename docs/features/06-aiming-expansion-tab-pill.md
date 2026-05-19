# 06 — Aiming tab expansion + tab pill restyle

## But

Deux changements liés dans la même itération :

1. **Onglets en pills** sans glow. Le `border-radius` passe de 6 px à 14 px (≈ demi-hauteur, donc forme de pilule complète). Les `box-shadow` extérieurs (halo interne + glow externe) sont retirés ; il ne reste qu'une bordure colorée nette quand l'onglet est actif ou survolé.

2. **Onglet Aiming étoffé** avec quatre nouvelles sections (Prediction, Targeting, Trigger, Recoil) en plus des Aimbot/Legit existantes. La structure `AimbotCfg` est étendue à 25+ champs côté front et back, gardés en miroir.

## Fichiers touchés

| Fichier | État |
|---|---|
| `src/lib/menu/TabBar.svelte` | radius 6 → 14 px, suppression du `box-shadow` actif/hover |
| `src/lib/menu/controls/Icon.svelte` | ajout `crosshair, mouse, activity, compass` au registre |
| `src/lib/menu/stores.ts` | `AimbotCfg` étendue + valeurs par défaut |
| `src/lib/menu/tabs/Aiming.svelte` | refait — 6 sections sur 2 colonnes |
| `src-tauri/src/state.rs` | `AimbotCfg` étendue côté Rust, miroir exact du type front |

## Backend

`AimbotCfg` est désormais regroupée en cinq blocs (Core, Prediction, Targeting, Trigger, Legit, Recoil). Le `#[serde(rename_all = "snake_case")]` garde la convention `prediction_enabled` côté JSON pour matcher les champs TypeScript inchangés.

Tous les champs sont stockés mais aucun thread ne les consomme encore — c'est le contrat avant l'implémentation. La commande `set_aimbot_config` reste la même (elle prend l'objet entier).

## Sections de l'onglet Aiming

### Colonne gauche

- **Aimbot** *(icon: target)* — Enabled, Keybind, Target part, FOV radius, Smoothing
- **Prediction** *(icon: zap)* — Enabled, Strength (0..2x), Velocity-based, Bullet drop, Gravity comp (0..200)
- **Trigger** *(icon: mouse)* — Auto trigger, Trigger key (KeybindInput), Delay (0..500 ms), Burst mode

### Colonne droite

- **Targeting** *(icon: crosshair)* — Selection mode (FOV/Distance/Health/Crosshair), Team check, Wall check, Visibility only, Min/Max distance (studs)
- **Legit** *(icon: shield)* — Legit mode, Miss rate (%), Humanize aim, Smooth path
- **Recoil** *(icon: activity)* — Compensation, Strength (0..2x)

Chaque section a son indicateur `active` lié à son toggle principal (icône + titre passent en accent crème quand la feature est ON).

## Détails visuels — tabs

```css
.tab {
    border-radius: 14px;        /* pill */
    padding: 5px 13px;
    border: 1px solid var(--border);
    background: var(--bg-input);
}
.tab:hover {
    border-color: rgba(255, 222, 157, 0.30);
}
.tab.active {
    background: rgba(255, 222, 157, 0.10);
    border-color: var(--accent);
    color: var(--accent);
}
```

Pas de `box-shadow`. La transition garde background / border / color (140 ms). Le `flex-wrap: wrap` du container reste, donc les 9 pills retombent sur deux lignes naturellement quand le menu est ~490 px de large.

## Comment tester

1. `npm run tauri dev`.
2. **Tabs** : chaque onglet est maintenant une pilule plate (pas de glow). Hover → bord crème pâle. Active → fond crème transparent + bord crème net, rien de plus.
3. **Aiming** : 6 sections visibles, 3 par colonne. Toggler le principal de chaque section (Enabled / Auto trigger / Compensation / etc.) doit allumer l'icône + le titre en crème.
4. **Prediction → Strength** : slider 0..2x avec format `1.00x`. La boule blanche reste collée au pointeur (pas de smooth, c'est voulu).
5. **Targeting → Selection mode** : dropdown FOV / Distance / Health / Crosshair.
6. **Trigger → Trigger key** : KeybindInput indépendant du keybind aimbot principal. Click → press a key.
7. **Persistence en mémoire** : changer plusieurs sliders/toggles, naviguer Visuals puis revenir → l'état est conservé.

### Pas encore implémenté (intentionnel)

- Aucune des nouvelles features n'est exécutée par un thread Rust — tout est stocké dans `AppState.aimbot_cfg` en attente du thread aimbot.
- Pas de validation côté backend (min < max distance par exemple), c'est du config-passthrough.
