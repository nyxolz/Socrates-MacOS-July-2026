# 09 — Cursor-based targeting in Mouse mode + self filter

## But

Deux ajustements ciblés du thread aimbot :

1. **En mode Mouse**, la référence pour le FOV et le scoring "closest" devient la **position du curseur OS** au lieu du centre géométrique de l'écran. Le mode Camera continue d'utiliser le centre (parce que là il n'y a pas de "curseur" distinct).
2. **Filtre self/overlap** : tout char à moins de **0.5 stud** de la position caméra est ignoré. Couvre le cas first-person (sa propre tête sur la caméra) et les reads dégénérés où une part overlap la caméra.

## Fichiers touchés

- `src-tauri/src/aimbot.rs` — `(ref_x, ref_y)` calculés selon `aim_type`, utilisés pour le filtre FOV et le score. Hardcoded `if dist < 0.5 { continue; }` avant le check `min/max_distance`.

## Détails

### Référence de scoring

```rust
let (ref_x, ref_y) = if cfg.aim_type == "Camera" {
    (cx_center, cy_center)
} else {
    current_cursor()
        .map(|c| (c.x, c.y))
        .unwrap_or((cx_center, cy_center))
};
```

- Le `current_cursor()` lit la position OS via `CGEvent::new(source).location()` à chaque tick (~250 Hz, négligeable).
- Fallback sur le centre si le read échoue (rare).

### FOV check + score

Le `screen_distance_squared` est désormais calculé depuis `(ref_x, ref_y)`. Conséquences :

- **FOV radius** : le cercle de tolérance suit le curseur. Si tu mets ton curseur sur la moitié droite de l'écran, l'aimbot ne lockera que sur les joueurs dans un cercle autour du curseur.
- **Scoring** : en Mouse mode, le score est **toujours** la distance écran depuis le curseur, indépendamment du dropdown `selection_mode`. Ça override pour garantir le comportement "le plus proche de là où je vise". En Camera mode, le dropdown garde son sens (Distance / Crosshair / Health).

### Self filter

```rust
if dist < 0.5 { continue; }   // before min_distance check
```

- `dist` = distance monde entre la part visée et `cam_pos`.
- 0.5 stud est bien sous le rayon d'une part Humanoid (Head ≈ 1 stud), donc on n'élimine jamais un vrai ennemi proche par accident.
- Le check est avant `min_distance` pour qu'il ne puisse pas être désactivé en mettant `min_distance` à 0 dans la UI.

## Comment tester

1. `npm run tauri dev`, attache Roblox, rejoins un game.
2. Aim type = **Mouse**. Mets ton curseur loin à gauche de l'écran et tiens MouseRight.
3. L'aimbot doit cibler le joueur le plus proche du curseur (à gauche), pas le plus proche du centre. Si aucun joueur n'est dans le `fov_radius` autour du curseur → rien.
4. Passe en **Camera** → la cible repasse au scoring défini par le dropdown (Distance par défaut = monde 3D).
5. First-person : tu ne dois jamais voir le viseur se locker sur ta propre tête au démarrage du frame (0.5 stud filter).
