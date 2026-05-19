# 01 — Origin test marker

## But

Premier test du pipeline complet (attach → résolution offsets → lecture caméra → math projection → emit Tauri → rendu canvas) **avant** d'itérer sur les vrais joueurs. Un seul point monde `(0, 0, 0)` est projeté chaque frame et dessiné sur le canvas overlay.

Sert de canary : si le marker "origin" reste cohérent quand on tourne la caméra, alors la chaîne complète est saine.

## Fichiers ajoutés / touchés

| Fichier | État |
|---|---|
| `src-tauri/src/esp.rs` | **ajouté** — pipeline ESP, `EspMarker`, `EspFrame`, `start_esp_thread` |
| `src-tauri/src/lib.rs` | modifié — `mod esp;` + `start_esp_thread(app.handle().clone())` dans `setup` |
| `src/routes/+page.svelte` | refait — canvas overlay + listener `esp_frame` + readout live dans le menu noir |

## Backend

- **Module `esp.rs`** dédié aux producteurs de frames ESP.
- **`resolve_camera`** : `Roblox.base → FakeDataModel (0x5F498F0) → DataModel (+0x18) → Workspace (+0x168) → Camera (+0x460)`. Retourne `None` à la première étape invalide.
- **`read_camera`** : lit `CameraPosition` (`+0x1C0`) et `CameraOrientation` 3×3 (`+0xF0`).
- **`project_marker`** : projette `world` via `world_to_ndc`. Si `None` (derrière le near plane), fallback `world_to_offscreen_dir` et `on_screen = false`.
- **`start_esp_thread`** :
  - Boucle 60 Hz (`sleep 16 ms`).
  - Tente d'attacher à `RobloxPlayer` paresseusement (le thread peut démarrer avant que Roblox soit ouvert sans crash).
  - Émet `esp_frame` à chaque tick, y compris quand Camera n'est pas encore résolue (avec `attached: true, markers: []`).
- **FOV** : pour l'instant hardcodé à `70°` (aucun offset propre n'a été dumpé pour le FOV dynamique côté Roblox actuel).
- **Aspect ratio** : calculé depuis `roblox_bounds` (publié par `overlay::tracker`) en retirant `TITLEBAR_HEIGHT = 28 px`.

## Payload `esp_frame`

```ts
type EspFrame = {
  markers: EspMarker[];    // { name, ndc_x, ndc_y, on_screen, distance, world }
  camera_pos: [number, number, number];
  fov: number;
  aspect: number;
  attached: boolean;
};
```

## Frontend

- **Canvas plein écran** en `pointer-events: none` — n'interfère pas avec le click-through.
- **`ndcToScreen`** identique à Cherry : `x = (ndc.x + 1) * 0.5 * canvasW`, `y = TITLEBAR + (1 - ndc.y) * 0.5 * renderH`.
- **`drawMarker`** : cercle vert + label + distance quand `on_screen`. Sinon pastille jaune clampée sur un ring de 42 % de la fenêtre pointant dans la direction de l'objet (radar).
- **Menu noir** étendu en readout live : `attached`, `fov`, `aspect`, `cam.x/y/z`, `origin on-screen`, `origin ndc`, `origin dist`.

## Comment tester

1. `npm install && npm run tauri dev`
2. Lance Roblox, rejoins n'importe quel game.
3. Le menu noir doit afficher `attached: yes`, le `fov` et l'`aspect` cohérents, et `cam.x/y/z` qui changent quand tu bouges la caméra.
4. Quand tu regardes vers le point monde `(0, 0, 0)` (souvent au centre de la map de spawn), un **cercle vert "origin"** apparaît à l'écran.
5. Quand tu tournes le dos à l'origine, une **pastille jaune "origin"** glisse sur le ring pour pointer dans sa direction.

Si la pastille verte reste collée au point monde quand tu strafes / tournes / avances, la projection est correcte.
