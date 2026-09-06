# DESIGN-UI — `nie-ui`, la source unique et typée des jetons de design du jeu

`nie-ui` (`crates/engine/nie-ui`, lib pure, zéro dépendance externe hors `nie-formats`) transpose
en Rust typé les jetons de la direction artistique du jeu — aujourd'hui écrits dans
`packages/inacord-ui/src/shell/game-tokens.css` — pour que le Rust qui compose des images
(`nie-game`, `nie-render3d`, `nie-core`) puisse les lire sans passer par du CSS.

Pour le rendu pixel-perfect des écrans START/MENU eux-mêmes (positions, sprites, police, gate
SSIM), voir **`docs/DESIGN.md`** — ce document ne le répète pas.

## État au 2026-09-06 — deux crates existaient déjà, `nie-ui` les relie

Avant d'écrire un seul jeton, deux crates couvraient déjà une partie du problème (chercher
l'existant d'abord) :

- **`crates/engine/nie-aphrody/src/design.rs`** — commité le jour même (`0374333`, quelques
  heures avant cette session) : c'est **le** générateur de `game-tokens.css`
  (`cargo run -p nie-aphrody --bin design`). Depuis ce commit, les 26 `--jeu-*` + 3 `--inacord-*`
  couleurs ne sont plus mesurées à la main sur une capture du jeu (c'était le cas avant) : elles
  sont **dérivées** par un k-means Oklab sur l'atlas du personnage Aphrody, 74 frames
  (`pixel mesurer …/spritesheet.png --k 10 --json`). `nie-ui` ne réimplémente pas cette
  dérivation — elle appartient à `nie_aphrody::design` — et **croise son propre calcul contre
  elle** par un test dev-dependency-only
  (`color::tests::les_couleurs_du_jeu_suivent_le_calcul_reel_de_nie_aphrody`, voir plus bas)
  plutôt que de dupliquer la palette mesurée. `cargo run -p nie-aphrody --bin design -- --verifier`
  confirme qu'au moment d'écrire ces lignes le fichier livré est **conforme** au calcul.
- **`crates/engine/nie-formats/src/sprite_sheet.rs`** — les atlas `.g4tx` d'interface du jeu vers
  CSS/SVG/JSON, rectangles recopiés jamais recalculés. `nie-ui::icons` **appelle** ce module (voir
  plus bas), il ne redécoupe rien.

## Ce que la crate génère

| Module | Contenu | Compte |
|---|---|---|
| `color` | 29 couleurs OKLCH (`--jeu-*` + `--inacord-*`), chacune documentée : hex, teinte source, part de l'atlas, rôle — copiés du commentaire CSS | 29 |
| `tokens` | Géométrie, rythme, mouvement, typographie (valeurs brutes, recopiées telles quelles) + 3 élévations composites (géométrie fixe écrite + couleur lue sur un jeton `color`, jamais un hex dupliqué) | 17 |
| `css` | `root_block()` — le bloc `:root { … }` complet, byte-identique à celui livré | 46 déclarations |
| `roles` | 27 rôles sémantiques shadcn (20) + Material 3 (7), mappés sur les jetons — mêmes valeurs que le bloc `@theme inline` d'`apps/nie-web/src/base.css` (lu, jamais modifié) | 27 |
| `compose` | `TileStyle`/`PanelStyle`/`PlateStyle` : la composition d'une tuile, d'un panneau, de la plaque centrale du menu — chaque champ pointe vers un jeton existant, rien de neuf | 3 types, 5 instances |
| `icons` | `icon_sheet_css()` : pont vers `nie_formats::sprite_sheet` pour les icônes `.g4tx` | 1 fonction |

Total : **46 jetons `:root` transposés** (29 couleurs + 17 non colorés) — le compte du fichier
livré, vérifié par `css::tests::root_block_commence_et_finit_comme_attendu`.

## La preuve : le CSS produit est identique à celui livré

```sh
cargo test -p nie-ui --lib css::tests::le_bloc_root_est_identique_au_css_livre
```

Ce test lit `packages/inacord-ui/src/shell/game-tokens.css`, en extrait le bloc `:root { … }`, et
le compare **octet à octet** à `nie_ui::css::root_block()`. Il annonce son saut à voix haute
(`GOLDEN SAUTE`) si le fichier est absent de l'arbre — jamais un vert silencieux.

**Preuve par falsification, rejouée puis annulée** : `ABYSS_BACKGROUND.oklch.l` changé de
`0.1963` à `0.9999` (une valeur qui ne veut rien dire physiquement — la clarté maximale sur le
fond le plus sombre du jeu) a fait ROUGIR **deux** tests :

- le golden CSS ci-dessus, avec la ligne exacte en désaccord affichée (`livre` vs `genere`) ;
- le cross-check contre `nie_aphrody::design`
  (`left: "oklch(0.9999 0.0242 280.23)" right: "oklch(0.1963 0.0242 280.23)"`).

Revert immédiat (`git diff` vide sur ce fichier après coup), suite revenue à 20/20. Commandes :

```sh
cargo test -p nie-ui --lib             # 20 passed; 0 failed (avant/après la falsification)
cargo clippy -p nie-ui --lib --tests   # 0 warning
```

Au passage, cette falsification a aussi débusqué un vrai bug d'écriture (pas de la conception) :
mes premiers comptes de tirets des en-têtes de section comptaient TOUS les tirets de la ligne
(y compris les trois de `/* --- `), pas seulement le padding — 46 déclarations généraient un
bloc `:root` qui ne correspondait pas au fichier livré dès le premier test. Le golden test l'a
attrapé immédiatement (`ligne 2 : livre … 52 tirets, genere … 55 tirets`) ; c'est exactement ce
qu'un test qui peut rougir doit faire.

## Le pont icônes

`icons::icon_sheet_css(sheet, image_url)` appelle
`nie_formats::sprite_sheet::SpriteSheet::vers_css` (les rectangles du jeu, recopiés, jamais
recalculés) puis étend sa classe de base commune (`.nie-sprite`) avec l'habillage interactif que
cette crate mesure déjà : transition `--jeu-duree-rapide`, rayon `--jeu-rayon`, anneau de focus
`--jeu-accent-azur` — les mêmes jetons que [`compose::TILE`] emploie pour une tuile du menu, posés
ici sur une icône. Testé contre un atlas réel du jeu (`data/dx11/font/gaiji_game.g4tx`, localisé
par `niers vfs find gaiji_game`), gardé par `nie_formats::vfs` (s'auto-saute à voix haute si le
jeu n'est pas monté sur la machine qui lance les tests — jamais un vert silencieux).

## Polices — recensé, pas porté

`nie_formats::font` parse déjà `font.cfg.bin` (métriques par glyphe : largeur, avance, position
dans l'atlas) et blitte sur l'atlas `font.g4tx` (DDS 4096×2048) — cinq exemples existants le
prouvent (`font_catalog`, `font_render`, `font_accents`, `render_text`, `dialogue_scene`, sous
`crates/engine/nie-formats/examples/`). Servir une police du jeu au web reste un choix **non
tranché** : soit convertir en `@font-face` (WOFF2), ce qui suppose de re-rendre les glyphes bitmap
en contours vectoriels — non fait, non commencé ; soit servir l'atlas existant en sprite-sheet
CSS/canvas comme `icons::icon_sheet_css` le fait déjà pour les icônes — plus proche de l'existant,
mais non câblé. `nie-ui` ne tranche pas ce choix : il est simplement recensé ici pour la
prochaine session.

## Ce que cette crate ne fait pas (encore)

- Elle ne mesure ni ne dérive aucune couleur — c'est le rôle de `nie_aphrody::design`, qu'elle
  croise en test plutôt que de le dupliquer.
- Elle ne rend aucun pixel des écrans du jeu (placement, texte, 3D in-menu) — voir
  `docs/DESIGN.md`.
- Elle n'écrit jamais `packages/inacord-ui/src/shell/game-tokens.css` : aucun binaire de cette
  crate n'y touche (hors périmètre de cette session, qui interdisait d'éditer ce paquet — deux
  autres agents y travaillaient en parallèle). La preuve de non-régression passe par un test qui
  **lit** ce fichier, jamais ne l'écrit.
- Pas de binaire `--verifier` autonome façon `nie-aphrody --bin design` : la preuve passe par
  `cargo test`, qui suffit au gate demandé (`cargo clippy -p nie-ui --lib --tests`, 0 warning).
- Pas de dépendance à un framework CSS-in-Rust (`stylist`, `grass`…) ni à un framework Rust
  « React-like » (Leptos/Dioxus/Yew) : cette crate ne rend rien, elle transpose des valeurs et
  assemble du texte — `std::fmt` y suffit, sans dépendance nouvelle. `CLAUDE.md` exclut déjà
  Leptos de la pile Aphrody (`nie-site` est Axum + askama). Tailwind CSS v4 et shadcn/ui restent
  la pile réelle côté hôtes web (`apps/nie-web/src/base.css`, `@import "tailwindcss"` +
  `@theme inline`) ; `roles.rs` est le pont **typé** vers cette pile, pas un remplacement.
- Pas de conversion sRGB↔OKLCH : les jetons stockent la valeur OKLCH telle qu'écrite dans le CSS
  et l'hexadécimal tel que commenté à côté — aucune bibliothèque de couleur n'était nécessaire
  pour ça, et `nie-aphrody` (qui, elle, calcule cette conversion pour dériver la palette) reste la
  seule à en dépendre.

## Captures de référence — `data/menu` → `nie-ui` → `game-screens.css` (2026-09-06)

`data/menu/` (local, © LEVEL-5, jamais poussé) porte **33 captures** 2560×1440 des menus réels
et un `manifest.json` (schema 1). Le pipeline décidé : `data/menu` → `nie-ui` (source typée) →
`packages/inacord-ui/src/shell/game-screens.css` (contrat CSS) → composants
`inacord-ui/src/components/game/**` et `apps/nie-web`. Cette crate possède le contrat CSS ; les
composants appartiennent à d'autres batches.

### Ce qui est mesuré, et avec quoi

L'instrument est celui du dépôt, étendu de deux choses dans `nie-aphrody` : `pixel::Crop` +
`pixel::palette_crop()` (une région `x,y,w,h` d'une capture opaque → classes k-means Oklab,
déterministes) et la sous-commande `pixel capture` (plus `--crop` sur `mesurer`) :

```sh
cargo run -p nie-aphrody --bin pixel -- capture data/menu/<png> --crop X,Y,W,H --k N [--json]
```

**45 couleurs `--screen-*`** (`crates/engine/nie-ui/src/surfaces.rs`), chacune avec capture,
recadrage, `--k` et part de classe dans son doc-comment ET dans le commentaire CSS. Les ancres :

| Rôle | Commande (`pixel capture data/menu/…`) | Classe retenue |
|---|---|---|
| Barre de titre | `options.png --crop 700,40,1701,81 --k 3` | #0874FF (35.61 %), rayures #0663F8 (34.21 %) |
| Onglet actif / bandeau / touche | `options.png --crop 1065,265,111,56` / `900,265,141,56` / `700,265,161,56` | #0149FF (83.17 %) / #6CA8F0 (100 %) / #4E4E4E (95.42 %) |
| Ligne focalisée | `options.png --crop 600,415,501,11` et `600,460,501,11` | #0078FF (98.93 % / 99.9 %) — **plat**, pas de dégradé vertical |
| Ligne au repos / libellé | `options.png --crop 520,515,881,51 --k 3` | #FEFFFF (88.23 %) / #79797A (8.59 %) |
| Colonne de valeur (repos / focus) | `controls.png --crop 1380,420,381,51` / `1380,1010,381,51 --k 4` | #A7DDFF (92.60 %) / #08B5FF (94.39 %) |
| Panneau FILTRES haut / bas / corps / filigrane | `filters_elements.png --crop 900,160,1151,26` / `512,1262,1537,26` / `540,705,741,321 --k 4` | #0048B9 (84.62 %) / #002496 (82.54 %) / #001E73 (68.85 %) / #163181 (29.60 %) |
| Coche / case | `filters_elements.png --crop 636,386,52,50 --k 4` | #45FFF8 (38.00 %) / #012075 (32.58 %) |
| Pastilles Vent / Feu / Forêt / Montagne | `723,476,65,63` / `723,570,65,62` / `1459,476,65,63` / `1459,570,65,62` | #8ED5FF / #FF7155 / #ABFF38 / #FFB936 (69–73 %) |
| Boutons Confirmer / Réinitialiser | `filters_elements.png --crop 1400,1170,101,71` / `850,1190,31,41` | #009DFF (42.57 %) / #3672E5 (100 %) |
| Curseur | `options.png --crop 455,405,71,76 --k 6` | #B5FF6B (29.19 %), #00CE87 (26.30 %), #CDF9FF (9.17 %) |
| Tuile du menu | `main_menu.png --crop 1062,768,245,167 --k 6` | #09316B (27.12 %), #245293, #4077C0 |
| Barre de description / touche / compteur | `options.png --crop 100,1235,401,71` / `915,1350,41,41` ; `filters_elements.png --crop 1780,1200,51,41` | #616E7D (66.94 %) / #4A4949 (47.83 %) / #545454 (79.29 %) |

**L'angle du parallélogramme** (`--game-skew`) est mesuré par l'ajustement de bord de
`pixel mesurer --sombre S` (R² exigé ≥ 0,95) :

```
pixel mesurer data/menu/options.png  --boite 1050 262 1190 326 --sombre 100  → gauche -10.70°, droit -10.64°, R² 0.992
pixel mesurer data/menu/controls.png --boite 300 1000 420 1085  --sombre 120  → gauche  -9.41°,             R² 0.992
pixel mesurer data/menu/controls.png --boite 2200 1000 2330 1085 --sombre 120 → droit   -9.34°,             R² 0.988
```

Moyenne des quatre bords −10,02° → `--game-skew: -10deg`. Les **tuiles** de `main_menu.png`
(fond photo) ne donnent pas de bord ajustable (R² 0,60 / 0,01 avec `--sombre 150`) : elles
reprennent cet angle, et le commentaire le dit. Cinq longueurs sont aussi mesurées (barre 160 px,
onglet 63 px, ligne 79 px, touche 41 px, tuile 167 px à 1440, ÷ 2 pour le canevas 1280×720).

### Le contrat de classes (`game-screens.css`, 533 lignes, 22 189 octets au 2026-09-07)

`.game-skew` (+ `--game-skew`) · `.game-header-bar{,__icon,__title}` · `.game-tab-strip{,__key,__label}`,
`.game-tab{,--active}` · `.game-panel{,__title,__body,__footer,__watermark}` ·
`.game-filter-panel{,__extra}` · `.game-check{,__box,__label,--checked,__count}` · `.game-icon-chip` ·
`.game-setting-list{,__scrollbar}`, `.game-setting-row{,--focused,__label,__value,__arrow,__more}` ·
`.game-button-primary`, `.game-button-secondary` · `.game-key-cap`, `.game-key-hint`,
`.game-hint-bar` · `.game-cursor` · `.game-tile-row`, `.game-tile{,__icon,--active}` ·
`.game-search-bar{,__input,__key}` · `.game-description-bar` · `.game-count-badge` ·
`.game-info-window{,__title}`. Toute couleur est `var(--screen-*)` (déclaré dans le `:root` du
fichier) ou `var(--jeu-*)` (déjà servi par `game-tokens.css`) — un test refuse tout hex nu hors
commentaire et toute `var(--screen-*)` non déclarée.

`.game-filter-panel`, `.game-check__count` et `.game-tab-strip__label` sont arrivées le 2026-09-07 :
les composants de `packages/inacord-ui/src/components/game/` (`GameFilterPanel.tsx`,
`GameTabStrip.tsx`) les employaient déjà, compensées par un `style={{…}}` en ligne — trois règles
manquantes au contrat, régularisées dans `surfaces.rs` puis retirées du TSX. Voir la falsification
plus bas.

### Les comptes (2026-09-06)

| Mesure | Commande | Résultat |
|---|---|---|
| Captures typées | `nie_ui::screens::CAPTURES` | 33 ; 14 écrans canoniques (le manifeste en a 14, pas 13 : `jq -r '[.entries[].canonical_screen]|unique|length'`) |
| Parité manifeste | `cargo test -p nie-ui screens::tests::le_manifeste_local_est_identique_aux_captures_typees` | 33/33 entrées identiques |
| Dimensions PNG | `cargo test -p nie-ui screens::tests::chaque_png_existe_en_2560x1440` | 33/33 IHDR = 2560×1440 (parsé à la main, 24 octets) |
| Familles documentées | `screens::tests::chaque_ecran_canonique_est_documente_par_le_manifeste` | 14/14 dans `lua_analysis.documented_roots` ∪ `runtime_matrix.results[].screen` |
| Golden CSS | `cargo run -p nie-ui --bin game_screens_css -- --verify` | exit 0, 21 595 octets conformes |
| Ancres re-mesurées | `surfaces::tests::les_ancres_suivent_la_mesure_reelle_de_nie_aphrody` | 3/3 (corps du panneau, ligne focalisée, tuile) à ΔE < 0,02 et part à ±0,5 % |
| Suite nie-ui | `cargo test -p nie-ui` | **35 passed, 0 failed** (20 avant ce batch) |
| Suite nie-aphrody | `cargo test -p nie-aphrody --lib` | **40 passed, 0 failed** (le rouge `design::tests::le_css_livre_est_celui_qu_on_produit` du 2026-09-06 est corrigé le 07, voir plus bas) ; +2 tests `pixel::tests` ajoutés |
| Gate clippy | `cargo clippy -p nie-ui --lib --tests --bins` et `-p nie-aphrody --lib --tests --bins` | 0 warning |

### Falsification (rejouée, transcrite)

```
$ cp crates/engine/nie-ui/src/surfaces.rs /tmp/surfaces.rs.sauv         # sha 8a179c8116ce5c52
# PANEL_BODY : L 0.2896 → 0.9896 (le corps navy devient presque blanc)
$ cargo test -p nie-ui --lib -- css::tests::game_screens_css_est_identique surfaces::tests::les_ancres
test css::tests::game_screens_css_est_identique_au_fichier_livre ... FAILED
  livre  : --screen-panel-body: oklch(0.2896 0.1484 263.16);  /* #001E73 - filters_elements.png crop 540,705,741,321 k=4 (68.85 %) … */
  genere : --screen-panel-body: oklch(0.9896 0.1484 263.16);  /* … */
test surfaces::tests::les_ancres_suivent_la_mesure_reelle_de_nie_aphrody ... FAILED
  screen-panel-body : ΔE 0.7000 entre la mesure et la constante
test result: FAILED. 0 passed; 2 failed
$ cp /tmp/surfaces.rs.sauv crates/engine/nie-ui/src/surfaces.rs          # sha 8a179c8116ce5c52, jamais git checkout
test result: ok. 2 passed; 0 failed
```

### Ce qui n'est pas fait

- Le dégradé « haut/bas » de la ligne focalisée demandé par le cahier n'existe pas sur la
  capture (deux bandes à 45 px d'écart rendent le même #0078FF) : la classe pose un aplat, pas un
  dégradé inventé.
- L'angle des tuiles du menu principal n'est pas mesuré sur les tuiles elles-mêmes (fond photo,
  R² < 0,95) — il est repris des onglets et des lignes, et signalé comme tel.

## Corrections du 2026-09-07

Trois défauts découverts en vérifiant le batch du 06, aucun n'était une erreur de couleur ou de
géométrie :

1. **`design::tests::le_css_livre_est_celui_qu_on_produit` rougissait sur une fin de ligne, pas
   une couleur.** `core.autocrlf=true` réécrivait `game-tokens.css`/`game-screens.css` en CRLF au
   checkout (`.gitattributes` ne les déclarait pas) ; le golden de `nie-aphrody` compare les
   octets bruts et annonçait « 111 lignes livrées, 111 attendues » sans aucune ligne différente.
   Les deux CSS de `packages/inacord-ui/src/shell/` sont maintenant `text eol=lf` dans
   `.gitattributes`. `nie-aphrody --lib` : 39 passed/1 failed → **40 passed/0 failed**. Falsifié
   (`cp` du fichier, sha `4b6d30b7c21e2f60`, une valeur `0.1963` → `0.9999`) → rouge exact ligne
   33 → restauré par `cp`, jamais `git checkout`.
2. **`scripts/e2e-site.sh` ne tournait pas du tout sous Windows** — `python3` du `PATH` est le
   raccourci Microsoft Store (n'exécute rien), et le `jq` natif Windows écrit en CRLF, que `curl`
   refuse ensuite comme URL malformée sur les 201 vérifications VFS. Port choisi en bash pur
   (`/dev/tcp`), chemins passés par `tr -d '\r'` après `jq`. Avant : exit 3, 0 compte final. Après :
   exit 0, **65 vérifications, 0 échec**, VFS 255 308 entrées.
3. **`CLAUDE.md`/`.gitignore` contredisaient le dépôt réel.** La règle disait « `data/` gitignoré —
   ne jamais commiter, `start.png`/`menu.png` compris », alors que `data/menu/` est suivi par git
   depuis `a0d464d6` et poussé sur un dépôt public (couvert par l'accord RG-L5-VR-2026-001). Les
   deux fichiers disent maintenant la même chose que le dépôt : le gros de `data/` reste ignoré
   pour le poids, `data/menu/` est explicitement rouvert (`!data/menu/**`) parce que c'est la
   source mesurée de cette crate.

## Pipeline jumeau — réglages typés (Codex, 2026-09-07)

En parallèle de cette crate (couleurs/géométrie → CSS → composants), Codex a construit une seconde
lecture de `data/menu` sur le même corpus `*_setting.cfg.bin`, sans toucher un seul fichier de
`nie-ui`/`inacord-ui`/`nie-web` :

| Commit | Portée |
|---|---|
| `be1eaa11` | `crates/tools/nie-cli` — `niers mode coverage --strict` : 475 écrans Steam vus, 109 classés dans 12 modes, 366 non classés, 0 doublon |
| `5679b238` | `crates/engine/nie-game` — audit de tous les réglages de menu en une passe VFS |
| `78eac842`/`08c6db76` | `crates/engine/nie-ffi` — `decodeMenuSetting()`/`vfs.menuSetting()` exposés à Bun, 13 tests |
| `c720d9b0` | binding WASM — `cfgbin_menu_setting_json()` |
| `d05f10e7` | `packages/nie-catalog/src/jeu.ts` — constructeur d'URL canonique `/typed/<...>_menu_setting.cfg.bin.json` |

Les deux pipelines partagent la source (`data/menu`) et divergent volontairement en aval : celui-ci
produit du **design** (jetons, CSS, composants visuels), celui de Codex produit des **données
typées** (FFI/WASM/catalogue) consommables côté client sans repasser par Rust. Voir
`data/menu/manifest.json.validation.codex_typed_settings_pipeline` pour le détail chiffré.
