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
