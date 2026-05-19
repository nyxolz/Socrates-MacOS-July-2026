# 04 — Menu polish: portrait layout, drag, SVG icons, keybind capture

## But

Quatre changements demandés sur le menu :

1. **Layout portrait** — le menu est désormais rectangulaire vertical (`320 px × 85 vh`), avec l'onglet à gauche et le body à droite.
2. **Drag** — on peut déplacer le menu en cliquant-glissant sur la barre d'en-tête. Le drag survit au passage rapide en dehors de l'en-tête (pointer capture).
3. **Icônes SVG** — plus aucun emoji ni `•` ; chaque section et chaque onglet a une icône line minimaliste inline (pas d'asset externe, pas de réseau).
4. **KeybindInput** — un clic sur le champ keybind passe en mode "listening" : la prochaine touche clavier ou clic souris est captée et stockée comme binding (Esc pour annuler).

## Fichiers ajoutés / touchés

| Fichier | État |
|---|---|
| `src/lib/menu/controls/Icon.svelte` | **ajouté** — composant générique, registre interne `ICONS: Record<name, path>` |
| `src/lib/menu/controls/KeybindInput.svelte` | **ajouté** — capture key/mouse via window listeners en mode capture |
| `src/lib/menu/controls/Section.svelte` | refait — prop optionnelle `icon` (fallback dot si absent) |
| `src/lib/menu/TabBar.svelte` | refait — vertical à gauche, icône + label par onglet, accent à gauche sur l'onglet actif |
| `src/lib/menu/Menu.svelte` | refait — dimensions portrait, drag via pointer events + `setPointerCapture`, header avec boutons minimize/close en SVG |
| `src/lib/menu/tabs/Aiming.svelte` | modifié — KeybindInput remplace TextInput, icônes `target`/`shield` sur les sections |
| `src/lib/menu/tabs/Visuals.svelte` | modifié — icônes `eye`/`text`/`filter` sur les sections |

## Détails techniques

### Drag (Menu.svelte)

- `let pos = { x, y }` Svelte-reactive ; le style `left/top` est appliqué inline.
- `pointerdown` sur le header :
  - capture l'offset `(clientX - rect.left, clientY - rect.top)` pour que le menu suive le curseur au point exact où on a cliqué (pas de saut).
  - `headerEl.setPointerCapture(e.pointerId)` — **clé** : sans ça, dès que le curseur sort de l'en-tête le tracker click-through Rust peut basculer la fenêtre en `ignoresMouseEvents = YES` et les événements `pointermove`/`pointerup` sont perdus. La capture force tous les events suivants à rester routés vers le header jusqu'au `pointerup`.
- `pointermove` met à jour `pos`, appelle `clampToViewport()` pour éviter de perdre le menu hors-écran, puis `pushZone()` pour que le mouse tracker connaisse la nouvelle rect.
- `pointerup` / `pointercancel` libère la capture.

### KeybindInput

- État `listening: boolean`. Au clic sur le bouton :
  - `setTimeout(attach, 0)` — défère l'attache des listeners de 1 tick pour ne pas capturer le mousedown qui vient juste d'activer le mode.
  - `attach()` ajoute `keydown` et `mousedown` sur `window` en mode `capture: true` (pour passer avant les listeners des sliders/dropdowns/etc.).
- `keydown` : on stocke `e.code` (stable physical-key name comme `KeyF`, `ControlLeft`, `Space`) au lieu de `e.key` qui dépend du layout clavier. `Escape` annule sans rien changer.
- `mousedown` : table `MOUSE_BUTTONS` mappe `e.button` → `"MouseLeft" | "MouseMiddle" | "MouseRight" | "Mouse4" | "Mouse5"`.
- À la capture (ou Escape) : `detach()` + `listening = false`.
- `onDestroy(detach)` pour ne jamais laisser de listener pendant si le composant est démonté en plein binding.

### Icon

Un seul fichier, un seul `<svg>`, un `viewBox="0 0 24 24"` et un registre `ICONS` (modules-level export). Ajouter une icône = ajouter une ligne dans le registre. Tous les paths sont mono-couleur (`stroke="currentColor"`), donc la couleur vient du parent (utile pour l'état `active` qui colore en accent).

Icônes embarquées actuellement : `target, shield, eye, text, filter, zap, list, server, code, package, terminal, sliders, users, x, minus`.

### TabBar vertical

- `flex-direction: column`, largeur fixe 110 px, `border-right` qui sépare des body.
- Onglet actif : `border-left: 2px solid var(--accent)` + color accent.

### Header avec contrôles fenêtre

- `Brand` + `version` à gauche, `iconbtn` minimize/close à droite.
- `on:click|stopPropagation` sur les iconbtn pour qu'ils ne déclenchent pas un drag.
- Le bouton close appelle `panic_exit` (commande existante).

## Comment tester

1. `npm run tauri dev`
2. **Portrait** : le menu doit apparaître haut et fin, plein de hauteur (~85% de la fenêtre Roblox), avec la barre d'onglets verticale à gauche.
3. **Drag** : clique-tire la barre d'en-tête (au-dessus des onglets) — le menu suit le curseur sans saut, même si tu bouges vite. Le menu ne sort jamais de la fenêtre Roblox (clamp).
4. **Icônes** : chaque onglet a une icône à gauche du label ; chaque section a une icône à gauche du titre. Aucun emoji, aucun unicode décoratif. Quand une section est `active` (toggle on), icône + titre passent en orange accent.
5. **Keybind** : onglet Aiming → bouton "Keybind". Clique dessus, il passe en `press a key...` orange. Appuie sur une touche clavier (ex. `F`) → le bouton affiche `KeyF`. Recommence et clique cette fois avec le bouton droit de la souris → `MouseRight`. Appuie `Esc` en mode listening → annulé, ancienne valeur conservée.
6. **Capture pendant drag** : déplace très vite l'en-tête en sortant du menu — le drag ne casse pas, le menu rattrape le curseur au `pointerup`.
7. **Close button** : clic sur la croix en haut à droite → l'app quitte (panic_exit). Minimize est un stub UI pour l'instant.
