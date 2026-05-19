# 13 — Tabs rework: Output / Options / PlayerList / Servers, spectate, theme

## But

Refonte des onglets après Aiming/Visuals :

- **Retire** Misc, Script View, Addons.
- **Output** : log live de la résolution `Roblox.base → FakeDataModel → DataModel → Workspace` + check `Workspace.Name == "Workspace"`. Poll 1 Hz.
- **Options** : sélecteur de thème accent (6 presets pastel + color picker custom), persisté en `localStorage`. Override les variables `--accent*` du menu en inline-style.
- **Player List** : liste des joueurs visibles (depuis `char_caches`), distance + HP, bouton **Spectate** par joueur. Re-clic → stop.
- **Servers** : nom de la place lu via `Instance::Name` sur le DataModel.

## Fichiers touchés

- `src/lib/menu/stores.ts` — TABS réduit à 6 entrées, `theme` writable persistée, `THEME_PRESETS` (Cream/Pink/Red/Blue/Green/Purple).
- `src/lib/menu/Menu.svelte` — applique les variables CSS du thème en `style="--accent: ...; --accent-soft: ...; ..."` inline (gagne contre `tokens.css`). Route les nouveaux onglets.
- `src/lib/menu/tabs/Output.svelte` — log monospace, vert/rouge par étape.
- `src/lib/menu/tabs/Options.svelte` — grille 6 swatches + ColorPicker custom + hex courant.
- `src/lib/menu/tabs/PlayerList.svelte` — liste réactive 1 Hz, bouton spectate toggle.
- `src/lib/menu/tabs/Servers.svelte` — affiche `get_place_name`, poll 2 s.
- `src-tauri/src/state.rs` — `spectate_target: Mutex<u64>`.
- `src-tauri/src/commands.rs` — `diag_check`, `list_players`, `set_spectate_target`, `get_spectate_target`, `get_place_name`.
- `src-tauri/src/spectate.rs` — **nouveau** thread 60 Hz qui écrit `CameraPosition` derrière la tête du target et appelle `aim_camera_pub` pour locker l'orientation.
- `src-tauri/src/aimbot.rs` — `aim_camera_pub` wrapper public pour permettre la réutilisation.
- `src-tauri/src/lib.rs` — `mod spectate`, registration des 5 nouvelles commandes, démarrage du thread spectate.

## Theme

Le store `theme` est lu au mount depuis `localStorage.socrate.theme` (fallback `#ffde9d`), et resave à chaque mutation. `Menu.svelte` calcule :

```ts
$: themeStyle =
    `--accent: ${$theme};` +
    `--accent-on: ${$theme};` +
    `--accent-soft: ${hexToRgba($theme, 0.55)};` +
    `--accent-glow: ${hexToRgba($theme, 0.45)};`;
```

Et l'applique en inline sur `.socrate-menu` — la spécificité inline bat les déclarations scopées de `tokens.css`. Toutes les surfaces (tabs, sections, sliders, FOV menu UI) virent à la couleur choisie. Les couleurs ESP elles-mêmes restent indépendantes (`visuals.*_color` et `aimbot.fov_*_color`).

## Spectate

`spectate.rs` à 60 Hz :

```
target = state.spectate_target.lock()           // u64 char_addr (0 = off)
cache  = char_caches.find(c.char_addr == target)
head   = read_pos_fast(cache.parts[0])
look   = -[prim+0xC8, +0xD4, +0xE0] (normalised)
cam    = head - look*10  +  (0, 3, 0)            // 10 derrière, 3 au-dessus
write camera_pos = cam
aim_camera_pub(sdk, camera, cam, head, smoothing=0, current_matrix)
```

`smoothing = 0` côté camera (snap instantané) — utile pour le spectate, où on ne veut pas de lerp. Le thread tourne en permanence ; quand `target == 0` il dort 16 ms et ne touche rien.

Pour activer/désactiver depuis la UI : `invoke("set_spectate_target", { target: char_addr_hex })` ou `{ target: null }` pour stop. `PlayerList.svelte` poll `get_spectate_target` pour savoir lequel est actif et toggler le bouton.

## diag_check payload

```rust
struct DiagStep { name: String, ok: bool, detail: String }
```

Renvoie les étapes successives (SDK / FakeDataModel / DataModel / Workspace / Workspace.Name / read counters). Court-circuite à la première étape qui échoue. Le front colore en vert/rouge selon `ok`.

## get_place_name

`sdk::name_of(&sdk, datamodel_ptr)` — Roblox stocke le nom de la place sur le DataModel via l'offset `Instance::Name` (0x0B0). Renvoie une string vide si non chargé encore (la UI affiche `(no place)` dans ce cas).

## Comment tester

1. `npm run tauri dev`.
2. **Output** : ouvre l'onglet ; sans Roblox la première ligne dit "not attached". Lance Roblox et rejoins un game → toutes les lignes passent en vert avec leurs addresses hex. La dernière ligne montre les compteurs read OK/fail (fail devrait rester à 0 en régime normal).
3. **Options** : clic sur un swatch (Pink, Blue, etc.) → le thème du menu change instantanément (tabs actifs, sections actives, sliders fill, FOV menu). Recharge l'app → le thème est persistant.
4. **Player List** : la liste affiche les joueurs visibles ordonnés par distance, avec leur HP. Click **Spectate** sur un joueur → la caméra in-game saute derrière lui et le suit. Re-click → **Stop** (le bouton revient à `Spectate`).
5. **Servers** : affiche le nom de la place Roblox (vu sur l'écran de chargement / page web du game).

## Limites

- Le spectate écrit `CameraPosition` chaque frame mais Roblox peut tenter de la rapatrier via ses scripts de caméra. À 60 Hz on gagne la course visuellement, mais l'idéal serait de set `CameraType = Scriptable` (pas encore wired).
- `list_players` n'identifie pas le local player explicitement (filtre via `< 0.5 stud` seulement). En third-person tu peux voir ton propre nom dans la liste.
