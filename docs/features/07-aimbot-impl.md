# 07 — Aimbot implementation + UI cleanup

## But

Implémenter pour de vrai tout ce qui est exposé dans l'onglet Aiming (aimbot, prediction, trigger bot, targeting, legit) côté Rust, et nettoyer la UI : keybind en ligne avec le toggle, retrait de FOV (selection mode), Visibility only et la section Recoil.

## Fichiers touchés

- `src/lib/menu/controls/KeybindInput.svelte` — prop `inline` ajoutée (mode compact, sans label, aligné à droite via `margin-left: auto`).
- `src/lib/menu/tabs/Aiming.svelte` — keybind en `<div class="row">` à côté du toggle Enabled (Aimbot + Trigger). Section Recoil supprimée. Selection mode = Distance / Health / Crosshair. Toggle Visibility only supprimé.
- `src/lib/menu/stores.ts` — `visibility_check`, `recoil_comp`, `recoil_strength` retirés ; `SelectionMode` réduit ; default `selection_mode` = "Distance".
- `src-tauri/src/state.rs` — miroir des suppressions.
- `src-tauri/src/aimbot.rs` — **nouveau** thread 250 Hz.
- `src-tauri/src/lib.rs` — `mod aimbot;` + `start_aimbot_thread(...)` dans `setup`.

## Thread aimbot

Boucle 250 Hz (`sleep 4 ms`). Skip si ni aimbot ni trigger n'est `enabled`. Lit `is_held(keybind)` pour aimbot et trigger séparément via :
- `CGEventSourceButtonState(HID, button)` pour `MouseLeft/Right/Middle/4/5`
- `CGEventSourceKeyState(HID, keycode)` pour les touches clavier mappées (`KeyA..Z`, modifiers, `Space`, `Tab`, `Enter`, `Escape`)

À chaque tick si au moins un est tenu :
1. Résout Camera, lit `CameraPosition` + `CameraOrientation` 3×3.
2. Construit view + proj matrices (FOV 70°, aspect = bounds Roblox).
3. Itère `char_caches`, pour chaque char prend la part selon `cfg.part` (Head/Torso/HRP → slot 0/1/1).
4. Filtre `min_distance`/`max_distance`.
5. **Prediction** : si activée et `velocity_prediction`, garde un `HashMap<char_addr, (pos, Instant)>`, calcule la vélocité par dérivée numérique, ajoute `vel * prediction_strength * (dist / 300)` au target_pos (300 studs/s = vitesse de projectile par défaut). Si `bullet_drop_comp`, ajoute `0.5 * gravity_comp * travel²` sur Y.
6. Projette en NDC → écran (pixels absolus).
7. Filtre via `fov_radius` (cercle autour du centre écran).
8. Score selon `selection_mode` : `Crosshair` = distance écran, sinon distance monde.
9. Garde le meilleur.

Puis :
- **Aim** : lerp `current_cursor → target` avec `cfg.smoothing` ∈ [0..1]. `CGWarpMouseCursorPosition` + `CGAssociateMouseAndMouseCursorPosition(1)` (sans le second appel, macOS supprime les deltas relatifs pendant ~250 ms après chaque warp et le smooth devient saccadé). `legit_miss` skip le tick selon roll xorshift32. `humanize` ajoute un jitter ±0.5 px.
- **Trigger** : si `screen_distance < 6 px` et `now - last_shot ≥ trigger_delay`, `CGEventCreateMouseEvent(LeftMouseDown/Up)` posté sur `CGEventTapLocation::HID`. `burst_mode` arme 2 shots additionnels à 30 ms d'intervalle.

## UI inline keybind

`KeybindInput` accepte maintenant `inline: boolean`. En mode inline :
- pas de label
- bouton compact (padding 3×7, font 10, min-width 64, centré)
- `margin-left: auto` → pousse le keybind à droite du toggle dans un flex row

Dans `Aiming.svelte` :
```svelte
<div class="row">
    <Toggle bind:checked={$aimbot.enabled} label="Enabled" />
    <KeybindInput inline bind:value={$aimbot.keybind} />
</div>
```

Le `:global(.toggle) { flex: 1 }` dans le style scoppé permet au toggle de prendre tout l'espace restant avant le keybind à droite.

## Comment tester

1. `npm run tauri dev`, attache Roblox, rejoins un game avec d'autres joueurs.
2. Aiming → Aimbot section : Enabled coché, le bouton keybind est à droite ("MouseRight" par défaut). Click-droit en jeu → curseur s'aimante sur la Head la plus proche en monde (selection mode "Distance"). Lâche → curseur reste libre.
3. Switch `Smoothing` à `1.0` → snap instantané. Baisse à `0.05` → suit en plusieurs frames.
4. Section Prediction : active Enabled + Velocity-based, mets Strength à `1.5x`. Sur un joueur qui court, le viseur vise *devant* sa trajectoire.
5. Section Trigger : Auto trigger ON, MouseLeft par défaut, Delay 50 ms. Maintiens le bouton gauche, dès qu'un joueur passe à <6 px du viseur → clic auto. Burst mode → 3 clics par déclenchement.
6. Section Targeting : passe `Selection mode` à `Crosshair` → l'aimbot vise le joueur le plus proche **du viseur** plutôt que le plus proche en monde.
7. Section Legit : Miss rate 50 %, l'aimbot saute en gros 1 tick sur 2.

## Limites connues

- Health-based selection retombe sur Distance (l'offset Humanoid::Health n'est pas encore lu dans le cache).
- Wall-check et Team-check ne filtrent pas encore (toggles inertes).
- Mapping clavier limité : si tu bind une touche hors de la table (`F1-F12`, flèches, etc.), `is_held` retourne `false`.
- Projectile speed hardcodé à 300 studs/s pour la prédiction.
