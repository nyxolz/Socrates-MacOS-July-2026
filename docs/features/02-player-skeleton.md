# 02 — Player skeleton

## But

Affiche le squelette ESP (lignes vertes reliant les parts) pour **chaque joueur** présent dans le Workspace. Trois rigs supportés : R15, R6, et un fallback HRP↔Head pour les rigs custom.

Le coût mémoire/CPU reste tenable même à 60 Hz parce que la découverte de personnages (walk Workspace + résolution des Primitive ptrs) se fait dans un **thread de cache** séparé qui tourne à ~1.4 Hz. La boucle ESP ne fait que `read::<f32>` sur des pointeurs déjà résolus.

## Fichiers ajoutés / touchés

| Fichier | État |
|---|---|
| `src-tauri/src/players.rs` | **ajouté** — `CharCache`, tables de rigs, `find_child_fast`, `read_pos_fast`, `build_char_cache`, `discover_characters`, `start_char_cache_thread` |
| `src-tauri/src/state.rs` | modifié — nouveau champ `char_caches: Mutex<Arc<Vec<CharCache>>>` |
| `src-tauri/src/esp.rs` | modifié — clone l'Arc des caches, lit les positions, projette les segments via `segment_to_ndc`, ajoute `bones[]` + `player_count` au payload |
| `src-tauri/src/lib.rs` | modifié — `mod players;` + `start_char_cache_thread(...)` dans `setup` |
| `src/routes/+page.svelte` | modifié — typings `Bone` + `EspFrame.bones`, fonction `drawBones`, readout `players` / `bones` dans le menu |

## Backend

### `players::CharCache`

```rust
pub struct CharCache {
    pub char_addr: u64,
    pub name: String,
    pub parts: [(u64, bool); 11],   // (primitive_ptr, valid)
    pub rig_type: u8,               // 0 = R15, 1 = R6, 2 = HRP
    pub humanoid_addr: u64,
    pub last_validated: Instant,
}
```

### Tables de rigs

- **R15** — 11 parts, 10 arêtes (head → upper_torso → lower_torso, puis 4 membres en 2 segments chacun).
- **R6** — 6 parts, 5 arêtes (head → torso, puis 4 membres directs).
- **HRP fallback** — 2 parts (Head + HumanoidRootPart), 1 arête. Utilisé pour les rigs qui ne matchent ni R15 ni R6 (jeux custom).

### `build_char_cache`

Essaie les **trois templates indépendamment** et garde celui avec le plus de matches. Évite le bug "R6 mis-tagged R15" — sans ça, un rig R6 matcherait l'entrée 0 (`Head`) de la table R15 et gagnerait, mais aucun segment ne se dessinerait parce que `UpperTorso`/`LowerTorso` n'existent pas.

Seuils minimums :
- R15 : ≥ 3 parts matched (sinon trop ambigu).
- R6 : ≥ 2 parts matched.
- HRP : ≥ 1 part matched.

### `start_char_cache_thread`

Boucle infinie, sleep 700 ms entre les rebuilds. Résout `Workspace`, walk ses enfants directs (cappé à 200), garde ceux qui ont un enfant `Humanoid`, et écrit le `Vec<CharCache>` dans `state.char_caches` via `Arc::swap` (lecteurs côté ESP ne bloquent jamais le rebuild).

### Boucle ESP étendue

Pour chaque cache, chaque frame :
1. Lit la position de chaque part via `read_pos_fast` (Vec3 à `primitive + 0x134`).
2. Filtre `(0, 0, 0)` (joueur mort en respawn) et les `NaN` (pointeur stale entre deux rebuilds).
3. Pour chaque `(a, b)` dans `cache.segments()`, appelle `segment_to_ndc(pa, pb, view, proj)` (clip near-plane intégré, donc une part derrière la caméra ne fait pas exploser la ligne à l'infini).
4. Push un `Bone { ax, ay, bx, by, name }`.

Bones et markers partent dans le même `esp_frame`.

## Payload `esp_frame` (mis à jour)

```ts
type Bone = { ax: number; ay: number; bx: number; by: number; name: string };

type EspFrame = {
  markers: EspMarker[];
  bones: Bone[];                  // ← nouveau
  player_count: number;           // ← nouveau
  camera_pos: [number, number, number];
  fov: number;
  aspect: number;
  attached: boolean;
};
```

## Frontend

- **`drawBones`** : un seul `ctx.beginPath() … ctx.stroke()` pour tous les segments — beaucoup plus rapide qu'un stroke par segment quand il y a beaucoup de joueurs.
- Couleur : `rgba(0, 255, 120, 0.85)`, `lineWidth = 1.5`.
- Le menu affiche `players` (nb de caches actifs) et `bones` (nb de segments visibles cette frame).

## Comment tester

1. `npm run tauri dev`
2. Rejoins un game Roblox avec d'autres joueurs (idéalement un mode social où ils ne bougent pas trop pour le premier test).
3. Les squelettes verts doivent s'afficher sur **chaque autre joueur** visible.
4. Le menu doit afficher `players: N` (N ≥ 1 si tu es seul, car ton propre character compte) et `bones` ~ `N × 10` pour du R15, `× 5` pour du R6.
5. Strafe / tourne la caméra : les squelettes doivent suivre les joueurs sans dérive.
6. Marche derrière un mur : le squelette reste affiché (l'ESP voit à travers) — c'est le but.
7. Si un joueur passe **derrière la caméra**, ses segments sont clippés au near plane (pas de ligne qui shoot à l'infini).

### Cas dégénérés à vérifier

- Joueur en respawn (`pos == (0, 0, 0)`) → pas de squelette dessiné.
- Joueur trop loin (> 400 studs, far plane) → segments invisibles, c'est attendu.
- Rig custom (jeu type Phantom Forces) → fallback HRP devrait dessiner au moins une ligne tête↔root.
