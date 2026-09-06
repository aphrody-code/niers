# `mainmenu01` — analyse visuelle de la cible n°1 du pixel-perfect

Écran de référence : le menu principal d'IEVR (logo central, tuiles AVATAR / VOTRE ÉQUIPE,
rangée de 8 tuiles en parallélogramme, barre « Victory Road », 3 tuiles du bas, version en haut
à droite, Inazuma Post, bannières DLC, « Deluxe Edition »).

Capture de travail : `/tmp/pixel/cible.png`, **2048 × 1159**, PNG converti depuis un WebP
lossless. C'est un **screenshot**, donc le rang 4 de la hiérarchie des sources : redimensionné
par rapport au rendu natif, et sa version affichée est `ver.7.1.2 0.90 301`. Les couleurs
ci-dessous en héritent — toute couleur destinée à du code doit être reprise sur la **texture du
VFS**, pas ici.

## 1. Mesures — capture entière

`pixel mesurer /tmp/pixel/cible.png --k 6 --alpha 0`

| Grandeur | Valeur |
|---|---|
| Palette, 69,0 % | `#F9FDF9` — le fond, un blanc très légèrement verdi (oklch 0,990 0,007 145°) |
| 10,4 % | `#93D3F0` — le bleu ciel des bandeaux (oklch 0,834 0,077 228°) |
| 7,7 % | `#2C497C` — le bleu nuit des tuiles (oklch 0,409 0,093 261°) |
| 7,1 % | `#4B8DD5` — le bleu moyen des icônes (oklch 0,633 0,128 252°) |
| 3,7 % | `#DDA580` — les carnations des personnages |
| 2,1 % | `#000101` — le texte noir |

## 2. Mesures — la rangée de 8 tuiles

`pixel mesurer … --boite 90 590 1880 730 --teinte 195 235 0.25 --k 4`

| Grandeur | Valeur |
|---|---|
| Bande occupée | x 96 → 1880, y 607 → 730 — **1785 × 124** |
| Remplissage | 66,92 % (cohérent avec 8 parallélogrammes séparés par des blancs) |
| Bleu dominant | `#2D5DA1` 38,4 %, `#578FD8` 29,7 %, `#0C2F64` 23,1 %, `#CEE1F6` 8,8 % |

**L'angle de pente EST mesurable — corrigé le 2026-09-06.** Ce paragraphe affirmait le
contraire (« R² entre 0,004 et 0,45, l'outil refuse de donner un angle »). Ce n'était pas la
forme qui résistait, c'était la méthode : ajuster les **bords d'une boîte** mêle le cadre au
contenu du sprite, et une seule ligne aberrante — celle où la fenêtre touche le bas de la forme
— fait tomber le R² de 1,00 à 0,07.

En lisant le **premier pixel non-fond de chaque ligne**, dans une fenêtre qui ne contient qu'un
seul bord, la même image rend :

| Bord | `dx/dy` | Angle | R² |
|---|---|---|---|
| 1ʳᵉ tuile de la rangée, bord gauche | −0,400 | −21,80° | 1,000 |
| 8ᵉ tuile, bord droit | −0,400 | −21,80° | 1,000 |
| 3ᵉ tuile de la rangée basse, bord droit | −0,403 | −21,95° | 1,000 |
| Panneau droit, bord gauche | −0,546 | −28,63° | 1,000 |

Trois bords indépendants s'accordent à 0,003 près : la pente des tuiles est **−0,4**, et le
`skewX` qui la reproduit décale le **haut vers la droite**. Les panneaux, eux, penchent plus et
**dans l'autre sens**. Le bord droit du panneau gauche reste refusé (R² = 0,875 < 0,95, le
sprite du personnage déborde du cadre) : on lui applique la pente du panneau droit en miroir, et
on le dit.

Script : `uv run scripts/validation/mesurer-mainmenu.py`. Valeurs figées dans
`packages/inacord-ui/src/shell/geometrie-mainmenu.ts`.

**Ce que la capture ne remplace pas** : la géométrie de vérité reste le layout runtime
(`nie-game --menu mainmenu01 --runtime --export-layout`) — mais il ne porte pas la position des
widgets de cet écran (§ 5), et c'est pour cela seulement qu'on mesure une image. Un écran
reconstruit ainsi n'est pas pixel-vérifié.

## 3. Carte des sources dans le VFS

Localisées et vérifiées par `niers vfs find` / `niers vfs stat` :

| Rôle | Chemin VFS | Taille |
|---|---|---|
| Config racine de l'écran | `data/common/gamedata/menu/cfg/main_menu_setting.cfg.bin` | 3 792 o (T2B, 7 racines) |
| Config du fond | `data/common/gamedata/menu/cfg/main_menu_bg_setting.cfg.bin` | 1 920 o |
| Config de la barre « Victory Road » | `data/common/gamedata/menu/cfg/victory_road_main_menu_setting.cfg.bin` | 3 536 o |
| Script Lua de l'écran | `data/common/script/lua/include/menu/main_menu_inc_3.00.01.00.lua.bin` | 13 092 o |
| Fond | `data/common/gamedata/menu/obj/mainmenu01_00_background.objbin` | 640 o |
| Infos joueur (haut) | `data/common/gamedata/menu/obj/mainmenu01_01_base_info.objbin` | 1 024 o |
| Tuile AVATAR | `data/common/gamedata/menu/obj/mainmenu01_02_base_chara_status.objbin` | 1 120 o |
| Rangée des 8 tuiles | `data/common/gamedata/menu/obj/mainmenu01_04_menu_list.objbin` | 896 o |
| Bouton de la rangée | `data/common/gamedata/menu/obj/mainmenu01_05_menu_list_button.objbin` | 848 o |
| Atlas d'icônes communes | `data/dx11/menu/200_icon/15_icon_common/icon_common.g4tx` | 1 328 144 o |
| Atlas d'icônes communes 2 | `data/dx11/menu/200_icon/15_icon_common2/icon_common2.g4tx` | 294 864 o |
| Atlas d'icônes de catégorie | `data/dx11/menu/200_icon/17_icon_category/icon_category.g4tx` | 631 056 o |
| Atlas d'icônes de catégorie 2 | `data/dx11/menu/200_icon/17_icon_category2/icon_category2.g4tx` | 191 584 o |
| Bannière DLC Deluxe | `data/dx11/menu/220_img/banner_img/banner_img_deluxe.g4tx` | 165 024 o |
| Logo « Deluxe Edition » | `data/dx11/menu/220_img/logo_dlc/logo_dlc_deluxe_edition.g4tx` | 27 712 o |
| Bannières DLC par langue | `data/dx11/menu/220_img/logo_dlc/<lang>/logo_dlc_{200..700}.g4tx` | non mesurées |
| Textes | `data/common/text/<lang>/menu_text.cfg.bin` (9 langues) | non mesurées |

Écrans voisins du même hub : `mainmenu04` (tuiles avatar/équipe/formation), `mainmenu90`
(listes génériques, en-têtes, bannières).

### Ce qui n'est PAS localisé — à ne pas deviner

- **Le logo central** : `mainmenu01_00_background.objbin` n'a aucun `.g4tx` dans son dossier de
  layout. Le seul logo trouvé (`logo_title_switch2_edition.g4tx`, 641 168 o) est catalogué sous
  l'écran **titre**. Le fond du menu principal est vraisemblablement une scène 3D, pas une image.
- **Les 8 icônes de la rangée** (éclair, bus, tour, chaussures+ballon, « BB », coupe, chariot,
  livre) : aucune n'est nommée individuellement. Elles sont probablement des sprites des atlas
  `icon_common` / `icon_category`, dont les UV vivent ailleurs — aucun atlas n'a été décodé pour
  le confirmer.
- **Les 3 icônes du bas** (livre « ! », engrenage, « i ») : mêmes atlas candidats, non confirmés.
- **Inazuma Post** : la seule occurrence indexée est `title00_10_inazuma_post*`, sous l'écran
  titre. Réutilisation en instance partagée ou objbin au nom différent — non tranché.
- Les objbin `mainmenu01_01/02/03/03_2/04/05/16` n'ont aucun `.g4mg`/`.g4tx` : ce sont
  vraisemblablement des locators ou de la logique, pas des meshes.

## 4. Ce que le dépôt sait déjà faire sur cet écran

| Brique | État |
|---|---|
| Parsing T2B `*_menu_setting.cfg.bin` | **porté et validé sur corpus complet** (485 fichiers, > 1000 layers, invariant `layer_id == CRC32(name)` à 0 écart) — `crates/engine/nie-data/src/menu_setting.rs` |
| Résolution du texte statique (`menu_text`) | **porté**, gate `menu_render_gate.rs:264` |
| Driver Lua réel des 9 onglets `main_menu` | **porté bout en bout** sur le vrai `main_menu_1.lua.bin` — `crates/engine/nie-lua/src/menu_host.rs`, gate `menu_render_gate.rs:296` |
| Résolution « CRC32 de région → rect d'atlas » | **portée**, prouvée sur `icon_rarity.g4tx` — gate `menu_render_gate.rs:344` |
| Composition statique `--menu mainmenu01 --capture` | **partielle** : 22 des 31 objbin `mainmenu01_*` rendus ; les 8 du groupe A (AVATAR, panneau perso, tuiles) n'ont aucun asset statique |
| **Placement réel des widgets** | **non fait** — c'est le blocage |
| SSIM `mainmenu01` contre la référence | **≈ 0,004**, plancher de non-régression 0,003 (`menu_render_gate.rs:588`) |

## 5. Le blocage, nommé

Les motions `g4pkm` **ne portent aucune keyframe de position** : elles n'animent que matériau et
UV. Le placement des widgets vient de la machine d'état C++ (`G4RA`) et des callbacks Lua
`Setup*`, qui n'ont jamais été reversés. Sans eux, le repli en bind-pose groupe les widgets au
centre ou les envoie hors écran — d'où une SSIM de 0,004 malgré des parseurs corrects.

Côté RE, les classes candidates existent dans la KB : `VictoryRoadMainMenu`,
`VictoryRoadTopMenu` + `VictoryRoadTopMenuStateMachine` et ses **17 états**
`VictoryRoadTopMenuState_*`, `CPostMenu`, `CMenuAttachLocator` (6 vmethods distinctes), plus les
chaînes `ChangeTabEventMainMenu`, `IsEnableChangeTabMainMenu`, `mainMenuCharaModelDrawDelayTime`
et le chemin `%s/menu/100_mainmenu/mainmenu01/mainmenu01_08/mainmenu01_08.g4tx`.

**Deux réserves à ne pas oublier en citant ces chiffres :**

1. la KB de ce VPS est ancrée sur le build **transitoire** (`4c2b91fbae6f…`, 31 468 032 o), pas
   sur `nie.exe` (`b1fa04ea3658…`) — rejouer `niers rebuild` avant toute conclusion chiffrée ;
2. `forge_unit` y est **vide** et `forge_classe` **absente** : on ne peut donc rien mesurer ici
   de ce que la forge sait déjà produire pour cet écran. C'est un manque de données, pas un 0 %.

## 6. Prochain pas, chiffré

Le chemin le plus court vers une SSIM non ridicule sur `mainmenu01` n'est ni un parseur de plus
ni un encodeur : c'est **reverser le placement**. Ordre proposé, du moins cher au plus cher :

1. désassembler `main_menu_inc_3.00.01.00.lua.bin` (le codec Lua du dépôt est byte-exact,
   `nie_lua::bytecode`) et lire les `Setup*` : le placement y est peut-être en clair ;
2. si le Lua délègue au C++, reverser `VictoryRoadMainMenu` et les 17 états
   `VictoryRoadTopMenuState_*` en partant de `CMenuAttachLocator` (dont on sait déjà que la
   position est un os d'attache dans le squelette **du locator**) ;
3. re-mesurer la SSIM à chaque pas. Un pas qui ne la bouge pas n'a rien prouvé.
