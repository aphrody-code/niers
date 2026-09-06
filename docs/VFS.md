# VFS — la carte complète, et ce qu'il faudrait pour tout servir

> Établi le 2026-09-06 par six agents, un par domaine, sur un inventaire figé
> (`var/vfs/inventaire.txt`, 255 308 entrées produites par `niers vfs find 'data/' -n 300000`).
> Chaque domaine a son document détaillé ; **celui-ci ne fait que la synthèse et la matrice**.
> Aucun compte de ce fichier n'est estimé : ils sont tous recalculés depuis l'inventaire.

L'objectif que ce document sert : **qu'Aphrody expose le VFS comme `nie.exe` le lit** — chaque
fichier, chaque dossier, chaque mode atteignable par une route, ou classé avec sa raison.

## 1. Les six domaines

| # | Domaine | Fichiers | Document |
|---|---|---:|---|
| 01 | Événements, scénario, scripts | 67084 | [`01-evenements.md`](vfs/01-evenements.md) |
| 02 | Texte, localisation, polices | 44412 | [`02-texte.md`](vfs/02-texte.md) |
| 03 | Menus et interface | 44612 | [`03-menus.md`](vfs/03-menus.md) |
| 04 | Audio et vidéo | 32369 | [`04-audio-video.md`](vfs/04-audio-video.md) |
| 05 | Personnages, modèles 3D, effets | 39146 | [`05-personnages.md`](vfs/05-personnages.md) |
| 06 | Monde, gamedata, rendu | 27685 | [`06-monde-donnees.md`](vfs/06-monde-donnees.md) |
| | **Total** | **255308** | couverture vérifiée : aucun fichier hors lot |

La découpe est disjointe et exhaustive — vérifié par `sort -u` sur la réunion des six lots.
Un piège de cette découpe, payé et corrigé : `data/dx11/effect/` (2 003 fichiers) figurait
d'abord dans **deux** lots. Une découpe ne se déclare pas complète, elle se **compte**.

## 2. La matrice, par volume de fichiers

Cinq états, parce que trois ne suffisaient pas. Le plan (`PLAN-SITE-ULTIME.md` § 4) en connaît
trois — `servi` / `interne` / `manquant` — mais la mesure a fait apparaître deux cas que ces
trois écrasaient :

- **`partiel`** : une route existe et ne couvre qu'une partie du corpus. La compter `servi`
  aurait annoncé 15 875 fichiers atteignables quand 7 466 codes le sont.
- **`bloqué`** : rien ne l'expose **et aucun parseur n'existe**. La distinction avec `manquant`
  est toute la différence entre « écrire une route » et « faire du reverse » — deux ordres de
  grandeur de travail, que le mot « manquant » confondait.

| État | Fichiers | Part | Ce que ça veut dire |
|---|---:|---:|---|
| **servi** | 162219 | 63.54 % | une route l'expose, avec un code HTTP **mesuré** |
| **manquant** | 67878 | 26.59 % | **le parseur existe dans le dépôt**, aucune route ne l'appelle — c'est du câblage, pas de la recherche |
| **partiel** | 15875 | 6.22 % | une route existe, elle ne couvre pas tout le corpus |
| **interne** | 5512 | 2.16 % | délibérément non exposé, **avec sa raison écrite** |
| **bloqué** | 3776 | 1.48 % | aucune route **et aucun parseur** : reverse préalable |
| **inconnu** | 48 | 0.02 % | extension non identifiée — ne rien router avant de savoir ce que c'est |
| | **255308** | **100 %** | |

**Le chiffre qui commande la suite : 67 878 fichiers (26,6 %) sont `manquant`, c'est-à-dire
qu'un parseur les décode déjà dans ce dépôt et qu'aucune route ne l'appelle.** Ce n'est pas un
trou de connaissance, c'est un trou de câblage — et c'est le motif de toute la session : le
dépôt sait faire bien plus qu'il n'expose.

À l'opposé, **3 776 fichiers (1,5 %) sont `bloqué`** : shaders, effets, particules, tissu,
navigation. Aucune route n'est possible avant du reverse. Les promettre serait mentir.

## 3. État par extension

| Extension | Fichiers | État | Preuve / raison |
|---|---:|---|---|
| `.bin` | 72308 | servi | `.cfg.bin` (71 101) et `.lua.bin` (1 197) — `/api/v1/formats/decode/{chemin}` et `/api/v1/lua/scripts/{chemin}`, 200 mesurés |
| `.g4tx` | 54203 | servi | `/assets/tex/<chemin>.png`, 200 mesuré — **sauf** les 14 de `common/font/font/`, qui rendent 404 |
| `.g4pk` | 45591 | manquant | parseur `nie-formats/src/g4pk.rs:137` — aucune route, **400 mesuré** |
| `.p3lip` | 21047 | servi | `/lip/<chemin>.p3lip`, visèmes datés |
| `.g4mg` | 15875 | partiel | servi pour les codes assemblables (`<code>/<code>.g4mg` présent), muet pour le reste — **7 466 codes assemblables sur 7 679** |
| `.objbin` | 12190 | manquant | parseur `objbin.rs:66` — aucune route |
| `.g4md` | 8955 | servi | `/api/v1/3d` + `/model/{famille}/{code}.glb` |
| `.g4pkm` | 6992 | manquant | parseur présent — aucune route |
| `.awb` | 5512 | interne | une banque seule n'a **aucune** métadonnée exploitable (`cueCount: 0`) : l'accès correct passe toujours par son `.acb`. Ce n'est pas un manque, c'est un usage sans forme correcte |
| `.acb` | 5512 | servi | `/audio-info/<x>.acb` puis `/audio/<x>.acb?id=<cue>` — **284 115 cues** mesurées sur les 5 512 banques |
| `.vfxo` | 1335 | bloqué | **aucun parseur** — reverse préalable |
| `.g4cm` | 1217 | manquant | parseur `g4cm.rs:336` (caméras d'événement) — aucune route |
| `.col` | 1150 | manquant | parseur `col.rs` (collision) — aucune route |
| `.pfxo` | 1113 | bloqué | **aucun parseur** — reverse préalable |
| `.ptlb` | 657 | bloqué | **aucun parseur** (particules) |
| `.fxbin` | 372 | bloqué | **aucun parseur** — reverse préalable |
| `.g4sk` | 339 | manquant | parseur présent (squelettes) — aucune route |
| `.mevbin` | 328 | manquant | parseur `mevbin.rs:136` — aucune route |
| `.usm` | 194 | servi | `/video/<x>.usm`, remux MP4 — **sauf** 2 fichiers MPEG-2 (`IE_15th`, `L5logo`), sans conteneur web |
| `.g4nv` | 160 | bloqué | **aucun parseur** (navigation) |
| `.g4mt` | 71 | manquant | parseur présent (animation) — aucune route |
| `.clobin` | 39 | bloqué | **aucun parseur** (tissu) |
| `.g4ma` | 35 | bloqué | **aucun parseur** |
| `.cfxo` | 29 | bloqué | **aucun parseur** — reverse préalable |
| `.gfxo` | 20 | bloqué | **aucun parseur** — reverse préalable |
| `.linb` | 16 | bloqué | **aucun parseur** |
Les 15 extensions restantes (moins de 15 fichiers chacune, 48 au total) sont **non
identifiées** — dont huit de la forme `.rNNNNN` (`.r41152`, `.r47929`, `.r66286`…). Aucun
parseur ne les connaît et aucun document du dépôt ne les nomme. Elles restent `inconnu` :
un nom de fichier ne dit pas ce qu'un fichier contient.

## 4. Ce que la mesure a corrigé dans nos propres documents

| Ce qu'on croyait | Ce que la mesure dit |
|---|---|
| L'arbre des menus compte **440** écrans | `/menu-tree.json`, interrogé en direct : **475**. Le 440 venait d'un commentaire du code source, jamais rejoué. |
| `mainmenu01` a « 24 objets jamais positionnés » | **0/34 sans position**. En revanche 8/34 sont sans sprite et 26/34 sans texte. Le défaut existait, il n'était pas celui-là. |
| Deux nomenclatures d'écran (calque ≠ script) | **Trois.** `/menu-tree/{stem}.json` attend le nom du `*_setting.cfg.bin.json` — `mainmenu01` y rend **404**. Confondre les trois donne un 404 qu'on attribuera au fichier. |
| Le T2B, c'est `common/property/**` | Vérifié **à l'octet** (`xxd`) : `menu_text`, `property/camera` et `font.cfg.bin` sont T2B aussi. La règle est plus large que ce qu'on avait écrit. |
| La 3D sert 6 familles, 6 191 modèles | **7 466 codes assemblables sur 7 679.** La famille **`uniform` (1 022 modèles) n'est servie par aucune route** — absente des six familles déclarées. |
| `.awb` est un fichier de son | C'est une **banque** : 7,688 Gio d'AWB contre 0,103 Gio d'ACB, soit **74×**. Le catalogue correct se fait par cue — il y en a **284 115**. |

## 5. Ce qui reste vrai après la mesure

- **Une seule information, un seul endroit.** Les six documents ne dupliquent pas les comptes :
  celui-ci les recalcule depuis l'inventaire, eux les détaillent.
- **Un compte porte sa commande.** Tout chiffre d'ici est reproductible sur
  `var/vfs/inventaire.txt` et `var/vfs/extensions.txt`.
- **Un chemin VFS ne se cite pas de mémoire** : les fichiers portent un numéro de version
  (`chara_act_cfg.1.03.91.00.cfg.bin`). Vérifier par `niers vfs find` avant d'écrire un chemin
  dans du code ou un test.
- **Une mesure porte sa date.** `nie-model-serve` a été mesuré saturé (16,3 Gio contre un
  `MemoryHigh` de 16 G, tous les rendus en 504) pendant que six agents interrogeaient le VFS en
  parallèle. Revérifié après : 5,7 Gio, `/model/perso/c01000010.png` en **200 / 1,1 s**. Le
  rapport n'était pas faux, il était daté.

