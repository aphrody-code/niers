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

> **Recalculée le 2026-09-06 au soir par la matrice de couverture**
> (`nie-site --regenerer-couverture`, § 4 du cap). Les comptes ci-dessous **remplacent** ceux de
> la première passe, et l'écart n'est pas arithmétique : il tient à la définition de `servi`.
> La première passe comptait `servi` tout ce que `/f/{*chemin}` rend — c'est-à-dire les octets
> bruts de **n'importe quel** fichier du jeu. Sous cette définition la gate ne peut pas échouer,
> donc elle ne mesure rien. `servi` veut désormais dire qu'une route rend le contenu
> **interprété** : décodé, image, audio, GLB, script.

| État | Fichiers | Part | Ce que ça veut dire |
|---|---:|---:|---|
| **servi** | 246196 | 96.43 % | une route rend le contenu **interprété**, avec un code HTTP mesuré |
| **manquant** | **0** | 0 % | les 21 250 fichiers qui l'étaient ont été câblés le 2026-09-06 au soir (`routes::level5`), 124/124 décodés à la mesure |
| **partiel** | **0** | 0 % | aucun : les 15 875 `.g4mg` sont décodés en process, y compris ceux que l'amont ne sait pas assembler (§ 2 bis) |
| **interne** | 5512 | 2.16 % | délibérément non exposé, **avec sa raison écrite** |
| **bloqué** | 3600 | 1.41 % | ni route ni parseur : du reverse d'abord. Y compris les 9 `.g4tg` non identifiés et 10 `.bin` hors `.cfg.bin`/`.lua.bin` |
| | **255308** | **100 %** | |

**Les 21 250 sont câblés le soir même.** `crates/tools/nie-site/src/routes/level5.rs` décode les
cinq familles en process, sans une dépendance de plus — les parseurs étaient là.
`scripts/validation/mesurer-level5.sh` échantillonne à pas régulier et exige le **jeton de la
famille dans le corps**, jamais un code 200 : **124 / 124**. Ce qui suit reste écrit parce que
c'est le diagnostic qui a rendu le câblage possible.

**Quatre extensions changeaient de camp, et c'était une erreur de fait.** `.g4nv`, `.g4ma`, `.g4vs`
et `.g4la` étaient classées `bloqué` — « aucun parseur ». Les quatre modules existent
(`crates/engine/nie-formats/src/{navm,g4ma,g4vs,g4la}.rs`), sont documentés et **validés byte sur
les fichiers réels du VFS**. Elles sont `manquant` : il y a une route à écrire, pas du reverse à
faire. Confondre les deux, c'est promettre des semaines là où il y a des heures.

**Et `.p3lip` — 21 047 fichiers, le plus gros corpus non servi — n'était pas visible du tout**,
parce que `/f` en rend les octets. `nie_formats::lip` les décode depuis longtemps.

**`partiel` est retombé à zéro le 2026-09-06 ; `manquant` ne l'a jamais été.** Les 67 878 fichiers (26,6 %) qu'un parseur
du dépôt décodait déjà sans qu'aucune route ne l'appelle sont servis par
`/api/v1/formats/decode/{chemin}` — cf. § 2 bis. Le diagnostic tenait : ce n'était pas un trou
de connaissance mais un trou de câblage, et il s'est refermé sans une dépendance de plus.

À l'opposé, **3 600 fichiers (1,41 %) sont `bloqué`** : shaders, effets, particules, tissu.
Aucune route n'est possible avant du reverse — les promettre serait mentir. La navigation
(`.g4nv`) en est SORTIE : son parseur existe, elle est `manquant`.

## 2 bis. Le câblage du 2026-09-06 — `manquant` : 67 878 → 0, `partiel` : 15 875 → 0

Les neuf familles géométriques sont servies par `/api/v1/formats/decode/{chemin}`
(`crates/tools/nie-site/src/routes/geometrie.rs`), `?forme=resume` par défaut,
`?forme=complet` pour la structure entière.

**`.g4mg` a rejoint le lot en cours de route, et c'est la mesure qui l'y a mis.** Il était
classé `partiel` parce que le catalogue 3D ne sert que les codes *assemblables* (7 466 sur
7 679) : les maillages de décor, d'effet et de menu n'ont pas de recette d'assemblage et
restaient hors d'atteinte. Or un `.g4mg` ne se lit pas seul — sa description vit dans le
`.g4md`. Comptés sur l'inventaire : **8 955 ont leur `.g4md` frère, 6 920 l'ont empaqueté dans
leur `.g4pkm` voisin, et 0 n'ont ni l'un ni l'autre.** La couverture est donc totale, et sans
recherche : `g4pkm::extract_g4md` existait déjà, c'est ce que fait l'amont pour les cut-in
`_waza`.

Décoder un fichier et assembler un modèle restent deux services distincts : `/api/v1/3d` dit
ce qui s'affiche comme entité, `/decode` dit ce que le fichier contient. Confondre les deux
aurait annoncé 15 875 modèles affichables — le défaut exact que le `partiel` avait été inventé
pour éviter.

**Aucune dépendance nouvelle.** Les neuf parseurs sont derrière `#[cfg(feature = "std")]`, une
feature **par défaut** : ils étaient déjà liés dans le binaire du site, personne ne les
appelait. Seule la feature `serde` de `nie-formats` a été ajoutée, pour que la forme complète
rende la structure décodée au lieu d'un `Debug`.

Mesure, `scripts/validation/mesurer-geometrie.sh <base> 25`, le 2026-09-06 :

| Famille | Fichiers | Échantillon | Conformes |
|---|---:|---:|---:|
| `.g4pk` | 45 591 | 25 | 25 |
| `.g4mg` | 15 875 | 25 | 25 |
| `.objbin` | 12 190 | 25 | 25 |
| `.g4pkm` | 6 992 | 25 | 25 |
| `.g4cm` | 1 217 | 25 | 25 |
| `.col` | 1 150 | 25 | 25 |
| `.g4sk` | 339 | 25 | 25 |
| `.mevbin` | 328 | 25 | 25 |
| `.g4mt` | 71 | 25 | 25 |
| **total** | **83 753** | **225** | **225 (100 %)** |

L'échantillon est pris à **pas régulier** dans l'inventaire, pas en tête de fichier : les
premiers chemins d'une extension viennent tous du même dossier, donc du même producteur
d'assets — un échantillon en tête aurait mesuré un seul cas.

« Conforme » ne veut pas dire « 200 » : le script exige que le corps porte le **jeton de
famille attendu**. Un 200 qui rendrait un résumé vide ou d'une autre famille compte comme un
échec, parce que c'est exactement le défaut que ce document traque depuis `/chara` (200 en
87 ms, 0 lien).

Deux familles ne rendent qu'un en-tête, et le résumé le **dit** au lieu de le laisser croire :
`.col` (`interieur_interprete: false` — l'intérieur est du PhysX *cooked*) et `.g4mt`
(`animation_decodee: false` quand le conteneur n'est pas suivi par `Motion::parse`). Un compte
à zéro parce que rien n'a été décodé n'est pas un compte à zéro.

## 3. État par extension

| Extension | Fichiers | État | Preuve / raison |
|---|---:|---|---|
| `.bin` | 72308 | servi | `.cfg.bin` (71 101) et `.lua.bin` (1 197) — `/api/v1/formats/decode/{chemin}` et `/api/v1/lua/scripts/{chemin}`, 200 mesurés |
| `.g4tx` | 54203 | servi | `/assets/tex/<chemin>.png`, 200 mesuré — **sauf** les 14 de `common/font/font/`, qui rendent 404 |
| `.g4pk` | 45591 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (table des sous-fichiers) |
| `.p3lip` | 21047 | servi | `/lip/<chemin>.p3lip`, visèmes datés |
| `.g4mg` | 15875 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes. La description vient du `.g4md` frère (8 955) ou du `.g4pkm` qui l'empaquette (6 920) ; **0 orphelin**. Le GLB *assemblé* reste `/api/v1/3d` — **7 466 codes assemblables sur 7 679**, ce qui n'est pas la même couverture ni la même promesse |
| `.objbin` | 12190 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (objet de menu et ses composants) |
| `.g4md` | 8955 | servi | `/api/v1/3d` + `/model/{famille}/{code}.glb` |
| `.g4pkm` | 6992 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (squelette 2D et poses de liaison) |
| `.awb` | 5512 | interne | une banque seule n'a **aucune** métadonnée exploitable (`cueCount: 0`) : l'accès correct passe toujours par son `.acb`. Ce n'est pas un manque, c'est un usage sans forme correcte |
| `.acb` | 5512 | servi | `/audio-info/<x>.acb` puis `/audio/<x>.acb?id=<cue>` — **284 115 cues** mesurées sur les 5 512 banques |
| `.vfxo` | 1335 | bloqué | **aucun parseur** — reverse préalable |
| `.g4cm` | 1217 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (clips, objets et canaux) |
| `.col` | 1150 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 conformes ; l'en-tête `PXCL` seul, l'intérieur PhysX *cooked* reste non interprété et le résumé le dit |
| `.pfxo` | 1113 | bloqué | **aucun parseur** — reverse préalable |
| `.ptlb` | 657 | bloqué | **aucun parseur** (particules) |
| `.fxbin` | 372 | bloqué | **aucun parseur** — reverse préalable |
| `.g4sk` | 339 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (hiérarchie d'os) |
| `.mevbin` | 328 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 de l'échantillon conformes (motions et événements datés) |
| `.usm` | 194 | servi | `/video/<x>.usm`, remux MP4 — **sauf** 2 fichiers MPEG-2 (`IE_15th`, `L5logo`), sans conteneur web |
| `.g4nv` | 160 | bloqué | **aucun parseur** (navigation) |
| `.g4mt` | 71 | servi | `/api/v1/formats/decode/{chemin}` — **200 mesuré**, 25/25 conformes ; `animation_decodee` distingue un conteneur non suivi d'une animation vide |
| `.clobin` | 39 | bloqué | **aucun parseur** (tissu) |
| `.g4ma` | 35 | bloqué | **aucun parseur** |
| `.cfxo` | 29 | bloqué | **aucun parseur** — reverse préalable |
| `.gfxo` | 20 | bloqué | **aucun parseur** — reverse préalable |
| `.linb` | 16 | bloqué | **aucun parseur** |
Les 15 extensions restantes (moins de 15 fichiers chacune) ont été **identifiées une par une
le 2026-09-06**, par requête et non par supposition : `scripts/validation/mesurer-extensions-rares.sh <base> 15`
rend **37 / 46 fichiers identifiés**.

| Ce que la mesure a trouvé | Fichiers | Comment |
|---|---:|---|
| archives **G4PK** sous un suffixe de révision (`.r41152`, `.r47929`, `.r51528`…) | 14 | **au magic** — leur extension est unique au fichier près, leur contenu ne l'est pas |
| **texte** (`.log` journaux `fbx2g4`, `.cfg` listes de blocs) | 12 | servis par `/f` en `text/plain`, type de contenu mesuré |
| conteneurs **Level-5** nommés mais non interprétés (`.g4vs` → `G4VS`, `.g4la` → `G4LA`) | 8 | en-tête commun de 16 octets |
| `cfg.bin` **T2B** sous un suffixe de révision (`.r65902`, `.r66286`) | 2 | le T2B n'a pas de magic : il ne se reconnaît qu'en le lisant |
| table **`@UTF` CriWare** (`sound.acf`, la configuration du moteur audio) | 1 | magic `@UTF`, décodage délégué à l'amont |
| **non identifiés** : `.g4tg` | **9** | assumé, cf. ci-dessous |

**Le compte est passé de 48 à 46 sans qu'un fichier disparaisse** : deux des 48 étaient un
artefact de mesure. Le VFS contient de vrais noms de fichier **avec un espace**
(`…/u021801/u021802 .g4md`), et découper l'inventaire par espaces en faisait deux « fichiers
sans extension ». Ce sont un `.g4md` et un `.g4mg`, tous deux déjà servis.

Les **9 `.g4tg` restent non identifiés, et c'est dit plutôt que comblé.** Ce qu'on en sait,
mesuré : tous vivent sous `dx11/` (8 sous `effect/`, 1 sous `menu/`), aucun ne porte de magic —
le fichier commence directement par ses données, sur le motif répété `7f 7f ff ff` — et leurs
tailles vont de 65 536 à 4 587 520 octets, toutes multiples de 1 024. Aucun parseur du dépôt ne
les connaît et aucun document ne les nomme. Une hypothèse de format n'est pas une
identification : un nom de fichier ne dit pas ce qu'un fichier contient, et une conjecture non
plus.

Deux faux positifs ont été trouvés **par cette mesure**, et corrigés :

1. `objbin::is_objb` cherche le pied de page `01 74 32 62` (« t2b »), commun à **tous** les
   `cfg.bin` — ce n'est pas un magic d'objbin. L'appeler dans la reconnaissance au magic
   envoyait les deux `…placement.cfg.bin.rNNNNN` au parseur objbin, qui répondait « OBJ_BGN
   attendu » : une erreur qui accuse le fichier quand c'est l'aiguillage qui s'est trompé.
2. `w10i000_placement.cfg`, un fichier **texte** commençant par `BLOCK_LIST_BEG`, passait pour
   un conteneur Level-5 de magic « BLOC ». Son `header_size` lu à `0x04` valait `K_` = 24 395,
   valeur qu'aucun format Level-5 ne déclare : la taille d'en-tête doit être un multiple de 16,
   au plus `0x100`.

Une mesure qui ne trouve rien n'a pas prouvé grand-chose ; celle-ci a trouvé deux défauts dans
le code qui la servait.

## 4. Ce que la mesure a corrigé dans nos propres documents

| Ce qu'on croyait | Ce que la mesure dit |
|---|---|
| L'arbre des menus compte **440** écrans | **`nie-model-serve` rend 475**, et `nie-site` les relaie désormais par `/api/v1/menu/screens`. Le 440 venait d'un commentaire du code source, jamais rejoué. |
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

### 2026-09-06 — local/VPS parity and Lua extraction

The explicit Steam-root mount is now the reference for menu work: local and
`ovh-vps-ubuntu-direct` both report 255 308 VFS entries and 936 CPKs. The local loose-file count
is 11 versus 5 on the VPS; the logical path and format histograms match. `niers vfs extract data
--ext lua.bin --out var/lua-vfs-all` produced 1 197 files and 10 694 973 bytes with 0 failures;
the extracted path set is identical to the inventory and every chunk has Lua 5.2 magic.

The exhaustive audit then decoded and executed 1 197/1 197 scripts with 0 decode/runtime errors,
1 053 252 decoded instructions, 21 661 713 live instructions, 76 include families, 0 missing
includes, and 0 missing host invocations. These are local/VPS-backed measurements, not claims
that the ignored game payload belongs in Git.

