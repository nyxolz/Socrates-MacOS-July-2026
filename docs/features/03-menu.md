# 03 — Full menu (tabs + controls)

## But

Remplacer le menu noir vide par un menu cheat complet : header avec branding, barre de 9 onglets (Aiming, Visuals, Misc, Output, Options, Script View, Player List, Addons, Servers), corps deux colonnes avec des sections empilées, et un design system réutilisable (Toggle / Slider / Dropdown / TextInput / Button / Section).

Pour ce premier jet, **Aiming** et **Visuals** sont remplis avec leurs contrôles complets, les sept autres onglets affichent un placeholder "not implemented yet". L'état des contrôles est centralisé dans des stores Svelte qui poussent automatiquement vers le backend Rust via les commandes `set_aimbot_config` et `set_visuals_config`.

Référence visuelle : `MenuSocrate.png` (palette sombre, accent orange, police petite, sections panel-based avec dot d'état coloré).

## Fichiers ajoutés / touchés

| Fichier | État |
|---|---|
| `src/lib/menu/tokens.css` | **ajouté** — variables CSS (palette, radius, font) appliquées par la classe `.socrate-menu` |
| `src/lib/menu/stores.ts` | **ajouté** — `activeTab`, `AimbotCfg`, `VisualsCfg`, TABS const |
| `src/lib/menu/bridge.ts` | **ajouté** — `startConfigBridge()` qui subscribe les stores aux commandes Tauri |
| `src/lib/menu/Menu.svelte` | **ajouté** — shell window (header + TabBar + body), push de la zone interactive |
| `src/lib/menu/TabBar.svelte` | **ajouté** — barre d'onglets cliquables |
| `src/lib/menu/controls/Section.svelte` | **ajouté** — panel avec header + dot d'état (state `active`) |
| `src/lib/menu/controls/Toggle.svelte` | **ajouté** — checkbox-style avec hint optionnel à droite |
| `src/lib/menu/controls/Slider.svelte` | **ajouté** — track + fill + range invisible, formatter customisable |
| `src/lib/menu/controls/Dropdown.svelte` | **ajouté** — select stylé, generic `<T extends string>` |
| `src/lib/menu/controls/TextInput.svelte` | **ajouté** |
| `src/lib/menu/controls/Button.svelte` | **ajouté** — variant `danger` (accent orange) |
| `src/lib/menu/tabs/Aiming.svelte` | **ajouté** — sections Aimbot + Legit |
| `src/lib/menu/tabs/Visuals.svelte` | **ajouté** — sections ESP + Text + Filters |
| `src/lib/menu/tabs/Empty.svelte` | **ajouté** — placeholder réutilisé par les 7 onglets vides |
| `src/routes/+page.svelte` | refait — host le canvas + `<Menu />`, gate skeleton sur `visuals.skeleton` côté front |
| `src-tauri/src/state.rs` | modifié — types `AimbotCfg` + `VisualsCfg` + champs dans `AppState` |
| `src-tauri/src/commands.rs` | modifié — `set_aimbot_config`, `set_visuals_config` |
| `src-tauri/src/lib.rs` | modifié — enregistrement des deux nouvelles commandes |
| `src-tauri/src/esp.rs` | modifié — la boucle ESP skip le travail de projection skeleton quand `visuals_cfg.skeleton == false` |

## Backend

### `state::AimbotCfg`

```rust
pub struct AimbotCfg {
    pub enabled: bool,
    pub keybind: String,        // ex. "MouseRight", "MouseLeft", "ControlLeft"
    pub smoothing: f32,         // 0.01..1.0
    pub fov_radius: f32,        // px
    pub part: String,           // "Head" | "Torso" | "HumanoidRootPart"
    pub legit_enabled: bool,
    pub legit_miss: f32,        // 0.0..0.9
}
```

### `state::VisualsCfg`

```rust
pub struct VisualsCfg {
    pub skeleton: bool,
    pub box2d: bool,
    pub distance: bool,
    pub names: bool,
    pub health_bar: bool,
    pub tracers: bool,
    pub team_check: bool,
    pub show_self: bool,
}
```

Les deux sont `Default` (mêmes valeurs que côté front).

### Commandes ajoutées

```rust
#[tauri::command]
pub fn set_aimbot_config(state: State<AppState>, cfg: AimbotCfg);

#[tauri::command]
pub fn set_visuals_config(state: State<AppState>, cfg: VisualsCfg);
```

Elles écrivent l'objet complet en une fois. Côté front le store push à chaque mutation (subscribe), donc l'état Rust reste toujours en sync.

### Court-circuit ESP

`esp.rs` lit `visuals_cfg.skeleton` au début de chaque frame. Quand c'est `false`, la double boucle de projection n'est jamais exécutée (ni le `read_pos_fast` ni le `segment_to_ndc`), donc toggler la case "Skeleton" off réduit immédiatement le coût CPU/IPC à zéro pour cette feature.

## Frontend

### Architecture

```
src/lib/menu/
  tokens.css            palette + font + radius
  stores.ts             activeTab / aimbot / visuals (writable)
  bridge.ts             startConfigBridge() — subscribe → invoke
  Menu.svelte           window shell, push interactive zone
  TabBar.svelte         barre d'onglets
  controls/
    Section.svelte      panel avec dot d'état
    Toggle.svelte       checkbox
    Slider.svelte
    Dropdown.svelte
    TextInput.svelte
    Button.svelte
  tabs/
    Aiming.svelte       rempli
    Visuals.svelte      rempli
    Empty.svelte        placeholder pour les 7 autres onglets
```

### Onglet Aiming

- **Section "Aimbot"** (dot ON quand `enabled`) :
  - Toggle Enabled (hint = keybind courant)
  - TextInput Keybind
  - Dropdown Target part (Head / Torso / HumanoidRootPart)
  - Slider FOV radius (10..600 px)
  - Slider Smoothing (0.01..1.0)
- **Section "Legit"** (dot ON quand `legit_enabled`) :
  - Toggle Legit mode
  - Slider Miss rate (0..90 %, formatté en `%`)

### Onglet Visuals

- **Section "ESP"** (dot ON si `skeleton || box2d`) : Toggle Skeleton, Box 2D, Tracers
- **Section "Text"** (dot ON si `names || distance || health_bar`) : Toggle Names, Distance, Health bar
- **Section "Filters"** : Toggle Team check, Show local player

Seul **Skeleton** affecte réellement le rendu pour l'instant (les autres sont en stub, l'implémentation suivra dans une feature dédiée).

### Tokens

Variables CSS clés (toutes scopées à `.socrate-menu`) :

```
--bg-window: #11121e         /* corps menu */
--bg-window-edge: #0a0b14    /* header */
--bg-panel: #08090f          /* sections */
--bg-input: #15161f
--bg-tab-active: #1c1d2a
--text: #e8e8f0
--text-muted: #6a6c82
--accent: #ff5e3a            /* dots de section + slider fill + danger button */
--accent-on: #ff5e3a         /* toggle ON */
font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Inter", sans-serif
font-size: 10px              /* base, headers à 11 */
```

### Bridge stores → backend

`startConfigBridge()` (appelée au `onMount` de Menu) subscribe `aimbot` et `visuals` et appelle `invoke("set_aimbot_config", { cfg })` / `invoke("set_visuals_config", { cfg })` à chaque changement. La subscription initiale firing une fois → l'état Rust se sync dès le mount.

### Click-through

Le `<div bind:this={menuEl}>` du `Menu.svelte` publie sa bounding rect au `set_interactive_zones` via un `ResizeObserver` + listener `window.resize`. Tant que la souris est dans cette rect, le NSWindow est interactif ; en dehors, click-through total.

## Comment tester

1. `npm run tauri dev`
2. Le menu doit apparaître à `top: 60, left: 60` de la fenêtre Roblox, format ~380×560 max.
3. **Cliquer un onglet** doit le surligner (fond `--bg-tab-active`, texte blanc) et changer le contenu du body.
4. **Onglet Aiming** : tu peux toggler Enabled (la dot orange du header de la section "Aimbot" s'allume), changer le keybind, faire glisser les sliders, choisir le target part.
5. **Onglet Visuals** : toggler "Skeleton" off doit immédiatement faire disparaître les squelettes orange (avant : verts) du canvas — vérification que le bridge front → back fonctionne et que la boucle ESP respecte le flag.
6. **Onglets vides** (Misc, Output, Options, Script View, Player List, Addons, Servers) : doivent afficher leur nom centré + "not implemented yet".
7. **Click-through** : déplace la souris en dehors du menu, tu dois pouvoir cliquer sur Roblox normalement. Reviens sur le menu, tu peux interagir avec les contrôles.
8. **Persistence pendant la session** : changer un toggle, switcher d'onglet, revenir — la valeur doit être conservée (store en mémoire).

### Pas encore implémenté (intentionnel)

- Drag-to-move du menu.
- Persistence sur disque (les configs reviennent par défaut au prochain lancement).
- Effet réel des toggles Visuals autres que Skeleton (Box2D, Tracers, Names, etc.).
- Logique d'aimbot (toute la config est stockée côté Rust mais aucun thread n'agit dessus).
- Contenu des onglets Misc, PlayerList, Servers, etc.
