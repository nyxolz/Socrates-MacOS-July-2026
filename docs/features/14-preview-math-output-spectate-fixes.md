# 14 — Preview projection, Output simplified, spectate stability, layout fixes

## But

Cinq petits ajustements coordonnés :

1. **Preview** : skeleton + bounding box projetés par la **vraie** math (view + projection) au lieu de positions hardcodées. Le slot `0..10` est tuné pour matcher la silhouette du dummy. Skeleton dessiné quand `visuals.skeleton` est ON.
2. **Output** : remplace la liste de steps par une ligne unique `✓ Offsets updated` / `✗ Workspace not found`, vraie côté Rust par `diag_check` qui retourne désormais juste un `bool` (chaîne complète + name "Workspace" check intégrés).
3. **Options** : section Theme placée dans une grille 2-colonnes comme Aiming/Visuals → la box ne prend plus toute la largeur du menu.
4. **PlayerList** : ordre figé. Map keyée par `char_addr` au lieu d'écraser le tableau ; les nouveaux joueurs s'ajoutent en bas, ceux qui partent disparaissent, ceux qui restent gardent leur slot. Plus de réordonnancement à chaque tick.
5. **Spectate** : la camera tournait n'importe comment parce que `read_look_dir` lisait la LookVector de la tête (qui pivote indépendamment du corps). Ajout de `CharCache.hrp_primitive` dans le cache rebuild ; spectate utilise désormais cette HRP pour le look. Plus le composant Y du look est forcé à 0 (camera reste à hauteur stable même si le target regarde haut/bas).
6. **Servers** : même fix que Options — wrap en grid 2-col, la box place name reste petite.

## Fichiers touchés

- `src-tauri/src/players.rs` — `hrp_primitive: u64` ajouté au `CharCache`, populé dans `build_char_cache` via `find_child_fast("HumanoidRootPart") → BasePart::Primitive`.
- `src-tauri/src/spectate.rs` — `look_prim = cache.hrp_primitive || head_prim` ; flatten Y du look pour caméra stable en hauteur.
- `src-tauri/src/commands.rs` — `diag_check` retourne `bool` au lieu de `Vec<DiagStep>` ; `DiagStep` retiré.
- `src/lib/menu/tabs/Output.svelte` — single-line status + grid 2-col.
- `src/lib/menu/tabs/Options.svelte` — wrappé dans `.grid` (3-col presets pour rester compact dans la box).
- `src/lib/menu/tabs/Servers.svelte` — wrappé dans `.grid`.
- `src/lib/menu/tabs/PlayerList.svelte` — `Map<char_addr, PlayerEntry>` au lieu d'array. `{#each ... (p.char_addr)}` avec keyed each pour éviter les re-renders.
- `src/lib/menu/Preview.svelte` — port complet de `build_view_matrix / build_projection_matrix / world_to_ndc` en TS, camera fake à `(0, 3, 7)` regardant -Z, identity rotation, 11-part skeleton R15 projeté et tracé en `<line>` SVG.

## Preview math

```ts
const CAM_POS = [0, 3, 7];                    // 7 studs devant le dummy
const ROT = [[1,0,0],[0,1,0],[0,0,1]];        // identity → camera regarde -Z
const FOV = 50;
const PROJ = buildProj(FOV, W/H, 0.5, 100);
const VIEW = buildView(CAM_POS, ROT);

projected = SKELETON_PARTS.map(p => worldToNdc(p, VIEW, PROJ));
skeletonPx = SEGMENTS.map(([a,b]) => ({ a: ndcToPx(projected[a]), b: ndcToPx(projected[b]) }));
BOX = bbox(projected) + padding 18% horiz / 5% vert;
headPx = ndcToPx(projected[0]);
```

Le projection-once-at-script-init est suffisant : la camera et le squelette ne bougent jamais dans le preview. Si on touche à la valeur `headPx`, tous les `style:left/top` des labels (réactifs sur `$visuals.*_offset_*`) se mettent à jour immédiatement.

## diag_check

```rust
#[tauri::command]
pub fn diag_check(state: State<AppState>) -> bool {
    let sdk = ...;
    let fdm = sdk.read::<u64>(sdk.base + Entry::FakeDataModel as u64);
    if !sdk.is_valid_ptr(fdm) { return false; }
    let dm = sdk.read::<u64>(fdm + FakeDataModel::DataModel as u64);
    if !sdk.is_valid_ptr(dm) { return false; }
    let ws = sdk.read::<u64>(dm + DataModel::WorkSpace as u64);
    if !sdk.is_valid_ptr(ws) { return false; }
    sdk::name_of(&sdk, ws) == "Workspace"
}
```

`true` quand on peut chaîner jusqu'à Workspace et que son `Name` lit "Workspace". `false` à la première étape qui casse.

## Spectate — flat look

```rust
let look_prim = if cache.hrp_primitive != 0 { cache.hrp_primitive } else { head_prim };
let look = read_look_dir(&sdk, look_prim);
let mut flat = [look[0], 0.0, look[2]];           // flatten Y
let l = (flat[0].powi(2) + flat[2].powi(2)).sqrt();
if l > 1e-3 { flat[0] /= l; flat[2] /= l; }
else { flat = [0.0, 0.0, 1.0]; }
let cam = [
    head.x - flat[0] * BACK,
    head.y + UP,                                  // fixed height, no Y tilt
    head.z - flat[2] * BACK,
];
```

HRP donne la direction du corps, jamais celle de la tête seule. Le flatten Y évite que la caméra plonge dans le sol quand le target regarde en bas.

## PlayerList stable

```ts
let players = new Map<string, PlayerEntry>();

async function refresh() {
    const list = await invoke<PlayerEntry[]>("list_players");
    const seen = new Set(list.map(p => p.char_addr));
    for (const addr of Array.from(players.keys())) if (!seen.has(addr)) players.delete(addr);
    for (const p of list) players.set(p.char_addr, p);  // update preserves position
    players = players;
}
$: list = Array.from(players.values());
```

Map.set sur une clé existante UPDATE in-place (l'ordre d'insertion JS est préservé). Nouvelle clé → append à la fin. Suppression → retire le slot. Pas de re-tri. `{#each ... (p.char_addr)}` keyed each évite que Svelte démonte/remonte les `<div>` lors des updates.

## Comment tester

1. `npm run tauri dev`.
2. **Preview** : Visuals → coche Skeleton + Box 2D. Le squelette se dessine sur la silhouette du dummy, calé sur la projection (head au sommet, legs en bas). Drag les labels Name/Distance/Health → ils glissent dans le preview ET dans le rendu in-game.
3. **Output** : sans Roblox → `✗ Workspace not found` (rouge). Lance Roblox + rejoins un game → `✓ Offsets updated` (vert) après ~1 s.
4. **Options** : la box Theme prend ~50% de la largeur du menu, presets en grille 3×2.
5. **Servers** : la box Server prend ~50% de la largeur du menu.
6. **PlayerList** : lance la liste, un joueur s'éloigne ou rejoint — son slot reste en place, pas de re-shuffle. Quand un joueur quitte le game, sa ligne disparaît ; quand un nouveau spawn, il apparaît en bas.
7. **Spectate** : clic Spectate sur un joueur. La caméra se positionne derrière son corps (HRP look, pas tête), à hauteur stable peu importe où il regarde.
