# 08 — Aimbot type: Mouse vs Camera

## But

Ajouter un mode camera-write au thread aimbot. Le précédent ne savait que warper la souris OS (`CGWarpMouseCursorPosition`). Le nouveau dropdown `Aim type` dans la section Aimbot bascule entre :

- **Mouse** — ancien comportement, déplace le curseur système avec smoothing lerp.
- **Camera** — écrit directement la matrice 3×3 d'orientation à `camera + 0xF0` (`CameraOrientation`). Le viseur in-game s'aligne sur la target indépendamment de l'input utilisateur.

## Fichiers touchés

- `src/lib/menu/stores.ts` — type `AimType = "Mouse" | "Camera"`, champ `aim_type` (default `"Mouse"`)
- `src-tauri/src/state.rs` — `aim_type: String` dans `AimbotCfg`, default `"Mouse"`
- `src/lib/menu/tabs/Aiming.svelte` — Dropdown `Aim type` ajouté entre la row Enabled/keybind et `Target part`
- `src-tauri/src/aimbot.rs` — fonctions `sub3 / cross3 / norm3 / aim_camera`, dispatch dans la branche aim selon `cfg.aim_type`

## `aim_camera`

Standard look-at construction :

```
forward = normalize(target - cam_pos)
right   = normalize(cross(forward, world_up))   // world_up = (0, 1, 0)
up      = normalize(cross(right,   forward))
```

Roblox CFrame stocke `Right / Up / Back` comme les **trois colonnes** du 3×3, avec `Back = -Forward`. La matrice cible est donc :

```
M[i][0] = right[i]
M[i][1] = up[i]
M[i][2] = -forward[i]
```

On lerp avec la matrice actuelle (lue dans la même boucle pour la projection), puis on écrit les 9 floats via `sdk.write::<f32>` aux offsets `i*12 + j*4` à partir de `camera + 0xF0`. Même layout que le read dans `esp.rs`.

### Cas dégénérés

- `forward` parallèle à `world_up` (caméra qui regarde tout droit en haut/bas) → `cross` collapse à 0. Fallback `right = (1, 0, 0)`.
- Target sur la caméra → `norm3` retourne `None`, on skip le tick.
- Smoothing clamp `[0.01, 1.0]` — 1.0 = snap instantané, 0.01 = lerp très lent.

## Smoothing — mode-agnostique

Le `cfg.smoothing` a la même sémantique dans les deux modes :

- **Mouse** : `cursor = lerp(cur, target_pixel, smoothing)`
- **Camera** : `M_new = lerp(M_current, M_target, smoothing)` élément par élément

À 250 Hz et smoothing 0.15, la convergence est visuellement comparable.

## Trigger bot

Inchangé — il continue à poster un `LeftMouseDown/Up` quand la distance écran < 6 px. En mode Camera, dès que la caméra est lockée sur la cible, le screen distance tombe à ~0 et le trigger fire immédiatement (sujet au `trigger_delay`).

## Comment tester

1. `npm run tauri dev`, attache Roblox, rejoins un game avec d'autres joueurs.
2. Aiming → Aimbot, choisis **Aim type: Camera**, garde Smoothing à 0.15, MouseRight comme keybind.
3. Maintiens clic-droit. La caméra in-game pivote toute seule vers la Head la plus proche, **sans bouger le curseur OS**.
4. Vérifie : si tu bouges la souris pendant que tu maintiens, ça n'a aucun effet — la caméra reste lockée (différence clé vs mode Mouse où ton input s'ajoute au lerp).
5. Smoothing à 1.0 → snap instantané. À 0.05 → la caméra glisse lentement vers la cible sur ~20 frames.
6. Repasse en **Mouse** → comportement antérieur (curseur OS qui se déplace).

### Limites

- Si Roblox passe la caméra en `Scriptable` ou si un script client réécrit l'orientation après nous, notre write peut être écrasé. Pour l'instant on n'a aucune protection contre ça — feature future possible (lock CameraType).
- Pas de re-orthonormalisation après le lerp : sur des milliers de ticks, la matrice peut dériver légèrement (skew). Le `target_m` recalculé chaque tick remet le système à plat.
