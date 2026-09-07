---
name: pixel-perfect
description: Analyser une capture du jeu (ou une texture du VFS) au pixel près et la reproduire ailleurs — écran de menu de site ou d'app, icône SVG propre, sprite ou pose supplémentaire d'un joueur/asset. Utiliser dès qu'une image du jeu doit devenir de l'interface, une icône vectorielle, ou un nouvel asset dérivé.
---

# Pixel-perfect — de la capture du jeu à l'asset reproduit

Une image du jeu se **mesure** avant de se redessiner. Toute valeur écrite dans du code
(couleur, rayon, marge, épaisseur, cadence) doit pouvoir être rattachée à une mesure ; ce qui
ne l'est pas est du souvenir, et le souvenir est faux.

> « Pixel-perfect » est un **objectif mesuré**, jamais un adjectif que l'on s'accorde. Ne
> l'affirmer qu'avec un chiffre de comparaison à l'appui (SSIM, % de pixels dans la tolérance),
> et dire lequel. Cf. la mémoire `pixel-perfect-non-prouve`.

## 0. Choisir la source — jamais un screenshot quand l'original existe

Ordre de préférence, du plus fidèle au moins fidèle :

| Rang | Source | Comment l'obtenir |
|---|---|---|
| 1 | la **texture du VFS** (G4TX, lossless, non redimensionnée) | `niers vfs find <motif>` puis `niers vfs extract <chemin> -o <fichier>` |
| 2 | le **rendu du dépôt** (composition déterministe, reproductible) | `nie-game --menu <ECRAN> --capture /tmp/<x>.png` |
| 3 | le **layout runtime** (positions réelles, issues des scripts Lua) | `nie-game --menu <ECRAN> --runtime --export-layout /tmp/<x>.json` |
| 4 | un **screenshot** (compressé, mis à l'échelle, filtré par le GPU) | dernier recours |

Un screenshot a subi un redimensionnement et souvent une compression : ses couleurs ne sont
pas celles du jeu. S'il est la seule source, le dire dans le document d'analyse.

Rappels : le chemin VFS cité de mémoire est presque toujours faux (les fichiers portent un
numéro de version) — viser le **dossier** et vérifier par `niers vfs find`. Le service
`nie-model-serve` sert les mêmes pixels en HTTP (`/tex/<chemin-sans-.g4tx>.png`).

## 1. Mesurer

L'outillage vit dans la crate `nie-aphrody` (`crates/engine/nie-aphrody`) et s'appelle par le
binaire `pixel` :

```bash
cargo run -p nie-aphrody --bin pixel -- mesurer /tmp/x.png --json
cargo run -p nie-aphrody --bin pixel -- mesurer /tmp/x.png --boite X0 Y0 X1 Y1 --k 6
```

Sous-commandes : `mesurer`, `comparer`, `vectoriser`, `planche`, `rasteriser`.

Ce qu'il rend, et ce que chaque grandeur sert à décider :

| Grandeur | Décide |
|---|---|
| boîte englobante du sujet (par alpha ou par seuil) | le `viewBox`, le recadrage de l'icône |
| ratio largeur/hauteur | si la forme est un cercle, un carré, une bande |
| taux de remplissage de la boîte | si la silhouette est pleine (≈ 100 %), circulaire (≈ 78,5 %), ajourée |
| palette k-means (part, HEX, HSL, OKLCH) | les couleurs du dégradé, les aplats |
| épaisseur du trait en % de la largeur | `stroke-width`, qui se pose en pourcentage, jamais en px absolus |
| profil de silhouette (colonnes/lignes pleines) | les creux et les bosses à poser avant de bomber |
| pente des bords + **R²** | l'angle d'une DA en parallélogrammes, qui se traduit tel quel en `skewX`. **L'outil se tait si R² < 0,95** — un bord coupé par la boîte, ou qui suit le contenu du sprite au lieu du cadre, donnerait un angle inventé |

**Contrôle de vraisemblance, obligatoire.** Une palette contient toujours des couleurs qui
n'appartiennent pas au sujet (le fond happé par le masque, un élément voisin). Une épaisseur de
trait vaut 0,5 % à 1,5 % de la largeur de la forme : au-delà, c'est la segmentation qui a
attrapé autre chose, pas le sujet qui est épais. Une mesure qui contredit l'ordre de grandeur
connu accuse la mesure, jamais le sujet — resserrer la boîte et recommencer.

Consigner dans `docs/<sujet>-analyse-visuelle.md` : les sources retenues **et** écartées avec la
raison, puis un tableau « décision → mesure d'origine ». Sans ce tableau, le dessin vient de la
mémoire quelle que soit l'allure du code.

## 2. Les quatre sorties

### A. Reproduire un écran en interface (site ou app)

Cible : `apps/nie-web` / `packages/inacord-ui` (la DA d'Aphrody est celle du **vrai jeu**,
référence `mainmenu01`). Ne jamais poser une capture en image de fond : on rebâtit la mise en
page.

1. `nie-game --menu <ECRAN> --runtime --export-layout /tmp/<ecran>.json` — le JSON porte les
   `spriteRect`, `spriteRegionG4tx` et la priorité de dessin. **C'est la géométrie de vérité** :
   la mesurer sur le PNG est une approximation, la lire dans le layout ne l'est pas.
2. Extraire les régions citées en PNG (`nie-game --compose-layout`, ou la région seule).
3. Traduire en composants : positions relatives (%) issues du layout, couleurs issues de la
   palette mesurée, tailles de police issues des métriques de fonte
   (`nie_formats::font`, cf. la mémoire `fontes-9-cataloguees-codepoint-empaquete`).
4. Vérifier par comparaison (§ 3), pas à l'œil.

Piège connu : **les positions de widget ne sont pas dans les fichiers de données** pour tous les
écrans — certaines viennent du runtime Lua, d'autres d'un `CMenuAttachLocator` (l'os d'attache
vit dans le squelette **du locator**). Un écran reconstruit sans layout runtime n'est pas
pixel-vérifié : le dire.

### B. Extraire une icône et en produire un SVG propre

Un SVG **propre** = des `path` de géométrie, pas un PNG en base64 dans une balise `<image>`.
La fonction `assets::svg_depuis_png` de `nie-aphrody` produit aujourd'hui la seconde forme :
elle convient à une favicon dérivée de l'atlas, **pas** à une icône d'interface.

Deux voies, et le choix se justifie :

| Voie | Quand | Comment |
|---|---|---|
| **redessin** (préféré) | glyphe simple : flèche, croix, ballon, élément | mesurer, puis écrire la géométrie à la main dans une bibliothèque de `path` — une source unique par sujet, consommée à la fois par le composant et par le générateur de fichiers |
| **vectorisation** | logo complexe, emblème d'équipe, forme qu'on ne saurait pas redessiner | `pixel vectoriser` (voir `references/bibliotheques.md`) — puis **relire le résultat** : un tracé automatique rend souvent 400 chemins là où 12 suffisent |

`potrace` sur une planche est du **décalque** : à éviter pour un dessin conçu comme vectoriel.
La vectorisation se justifie pour un asset dont on possède les droits et qu'on veut simplement
rendre indépendant de la résolution.

Règle qui tient tout : **une seule source de géométrie par sujet**. Sinon l'icône et
l'illustration divergent au premier ajustement.

Déclinaisons (favicon, apple-touch, maskable, manifeste) : ne pas les redessiner —
`nie_aphrody::assets` les produit déjà depuis une image carrée, par moyenne de zone en alpha
prémultiplié (le filtre correct pour une réduction franche ; le plus proche voisin hache le
bord).

### C. Extraire un joueur / un asset et générer d'autres poses

Le personnage n'est pas une image : c'est un modèle skinné. Une « autre pose » se produit en
**rendant le modèle**, pas en déformant des pixels.

```bash
cargo run -p nie-render3d --example anim_char      # perso texturé skinné animé
```

- géométrie `G4MG` (skin 8×u16 vtype5 / 8×u8 vtype6, stride 68), squelette `G4SK`
  (`parse_poses`), animation `G4MT` (`parse_animation`), textures BC7 ;
- mapping canal → os : `rot[k] → os(4+k)`, les 4 premiers os non-squelettiques sont sautés —
  ne pas y toucher, c'est ce qui corrige le ballooning ;
- une pose = un temps d'animation ; une planche de poses = N rendus à N temps, même caméra.

Pour une **feuille de sprites** : rendre N poses à taille fixe, rogner sur l'alpha commun
(jamais pose par pose, sinon le personnage saute d'une case à l'autre), assembler en grille et
écrire les rectangles dans un JSON à côté. Le contrat d'atlas de référence du dépôt est celui du
pet Aphrody (`Pet`, `Frame`, `Rect` dans `nie-aphrody`) — le réutiliser plutôt que d'en inventer
un autre.

### C bis. Planche de sprites et CSS — passer par `nie_formats::sprite_sheet`

**Ne pas réécrire de générateur CSS.** `nie_formats::sprite_sheet` produit déjà, depuis les
rectangles d'un atlas : la feuille **CSS** (mode image ou mode masque `currentColor`), le **SVG**
autonome à `<symbol>`, et le **JSON** des régions. C'est le rendu employé pour les atlas du jeu.

`pixel planche` lui **apporte** une planche assemblée au lieu d'un `.g4tx` — un sprite venu de
poses rendues et un sprite venu d'un atlas du jeu s'emploient donc exactement pareil :

```bash
pixel planche pose_*.png -o /tmp/poses --colonnes 4 --nom poses_c01000010
# → /tmp/poses.png + .css + .svg + .json
```

Deux règles portées par le code :

- toutes les cases font la taille de la **plus grande** image, et chaque image est posée en haut
  à gauche **sans rééchantillonnage** — recentrer pose par pose fait sauter le sujet d'une case
  à l'autre, et cela ne se voit qu'une fois l'animation en marche ;
- le nom du sprite vient du **fichier**, jamais de son rang : un rang se décale au premier ajout
  et tous les sélecteurs CSS déjà écrits pointent alors ailleurs.

Les **couleurs** se posent en jetons, pas en HEX recopiés à la main :

```bash
pixel mesurer <IMG> --boite X0 Y0 X1 Y1 --css menu-tuile
# :root { --menu-tuile-0: oklch(0.4806 0.1216 257.52);  /* #2D5DA1 — 38.39 % des pixels */ }
```

`oklch()` parce que c'est la forme dans laquelle une couleur se **décline** (éclaircir = monter
`L` sans toucher `C` ni `h`). Le HEX mesuré reste en commentaire : sans lui, plus rien ne
rattache le jeton à la mesure dont il sort.

### D. Extraire une texture / un asset plat

`niers vfs extract`, puis `nie_formats::image_out` pour l'encodage (WebP lossless, GIF, JPEG,
BMP, TGA, TIFF, QOI — le PNG est hors de la feature `images`). Nommer le fichier par la
**sous-entité** exportée, jamais par le fichier source : sinon tous les téléchargements se
recouvrent (cf. la mémoire `export-nom-fichier-sous-entite`).

## 3. Vérifier — sans chiffre, rien n'est prouvé

```bash
compare -metric SSIM /tmp/reference.png /tmp/reproduction.png /tmp/diff.png    # ImageMagick
cargo run -p nie-aphrody --bin pixel -- comparer /tmp/reference.png /tmp/reproduction.png
```

- **SSIM** pour une reproduction d'interface. Le rendu de menu du dépôt plafonne aujourd'hui
  vers ~0,62 : c'est le point de comparaison honnête, pas 1,0.
- **% de pixels dans la tolérance** pour un rendu qui doit être identique (c'est le critère de
  `nie-game --verify`, qui échoue sous 99 %). Attention : `--verify` compare **CPU contre GPU**,
  donc l'auto-cohérence du dépôt — pas la conformité à `nie.exe`.
- **sha256 identique** pour ce qui doit l'être au bit près (une texture ré-extraite).
- Regarder les planches produites sur fond **clair et sombre**, à 512/128/64/32/16 px : ce qui
  se ferme en dessous de 64 px doit avoir une variante simplifiée. C'est le contrôle qui produit
  la variante, pas une intuition.

## 4. Livrer

1. le document d'analyse (`docs/<sujet>-analyse-visuelle.md`) avec le tableau des décisions ;
2. la source de géométrie (une seule) et son générateur ;
3. les fichiers produits, versionnés ; les masques, mesures et planches de contrôle restent
   dans `/tmp` ;
4. `cargo clippy -p <crate> --lib --tests` à 0 warning, `bun run typecheck` côté interface ;
5. **jamais** d'asset du jeu committé hors `data/oc/` : `data/` est gitignored, les assets sont
   © LEVEL-5.

## Cible n°1 du projet — `mainmenu01`

Le premier objectif pixel-perfect est **le menu principal du jeu**. Sa carte mesurée (palette,
géométrie de la rangée de tuiles, chemins VFS des configs, du Lua, des atlas d'icônes et des
bannières, classes RTTI candidates, et le blocage nommé) vit dans
[`docs/mainmenu01-analyse-visuelle.md`](../../../../docs/mainmenu01-analyse-visuelle.md).

Trois faits à connaître avant d'y toucher, tous mesurés :

- l'écran s'appelle `mainmenu01` ; ses configs sont `main_menu_setting.cfg.bin`,
  `main_menu_bg_setting.cfg.bin`, `victory_road_main_menu_setting.cfg.bin`, son script
  `main_menu_inc_3.00.01.00.lua.bin` ;
- la composition statique rend **22 des 31** objbin de l'écran, et la SSIM contre la référence
  vaut **≈ 0,004** — l'infrastructure de données est solide, le rendu ne l'est pas ;
- le blocage n'est **pas** un format : les motions `g4pkm` ne portent aucune keyframe de
  position. Le placement vient de la machine d'état C++ `G4RA` et des callbacks Lua `Setup*`,
  jamais reversés. Tant qu'ils ne le sont pas, aucun travail de texture ne fera bouger la SSIM.

## Références

- `references/bibliotheques.md` — les bibliothèques réellement utilisables (Rust et Bun),
  mesurées, avec leur licence et ce qu'on a rejeté.
- `crates/engine/nie-aphrody/docs/pipeline-svg.md` — la chaîne SVG rapatriée de shenron, dont
  la doctrine « une seule source de géométrie » vient.
- `docs/RE.md`, `AGENTS.md` — cadre du dépôt.
