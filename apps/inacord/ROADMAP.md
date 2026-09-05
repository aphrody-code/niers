# nie-explorer — état et suite

Application Tauri (React + Rust) : explorateur du VFS, éditeur de données, atelier de modding et
boîte à outils de reverse pour *Inazuma Eleven: Victory Road*.

Ce document décrit **ce que l'application fait aujourd'hui** et **ce qui reste ouvert**. Il ne
raconte pas comment on y est arrivé : l'historique est dans `git log`. Le plan du moteur et de la
forge est dans [`docs/PLAN.md`](../../docs/PLAN.md) ; la portée ici est l'application seule.

Règle de lecture : une capacité n'est décrite comme acquise que si elle a été vérifiée sur le vrai
jeu ou par un test qui s'exécute. Ce qui n'est vérifié que par la compilation est dit tel quel.

---

## Explorateur

Navigation en **onglets multiples**, chacun portant son contexte complet (préfixe, sélection,
recherche, filtre d'extension, tri, mode liste/grille, taille des vignettes) et sa propre pile
arrière/avant. Le modèle est un réducteur pur (`src/lib/explorerTabs.ts`) exposé par
`useSyncExternalStore`, persisté sous `nie-explorer:tabs` avec restauration défensive : stockage
absent, JSON corrompu ou tableau vide retombent sur un onglet unique.

Ctrl+T ouvre, Ctrl+W ferme, Ctrl+Tab cycle, le clic milieu ouvre un dossier ou un emplacement de
la barre latérale dans un nouvel onglet. Ctrl+1…9 restent la sélection de **vue**, Ctrl+D
l'épinglage, Ctrl+K la palette de commandes.

Le panneau est déclaré `keepMounted` : le panneau d'onglets de `@base-ui/react` démonte son
contenu par défaut, et sans cela l'état d'un onglet serait perdu à chaque aller-retour vers une
autre vue. Une seule instance d'`ExplorerView` est « active » à la fois — l'enregistrement dans
`editBus` (singleton) et l'écouteur Ctrl+D (global) sont derrière cette garde.

**Export au format voulu.** « Extraire » écrit les octets du jeu tels quels ; l'export les
convertit. La table des formats possibles pour un fichier donné vit dans `nie_explore::export`
(partagée, testée sur le vrai VFS) : texture → png/webp/jpg/bmp/tga/tiff/qoi/gif, modèle → glb,
audio CRI → wav, `.usm` → mp4, conteneurs T2B/RDBN → json, et le brut toujours proposé en tête.
L'interface ne propose que ce qui marche pour ce fichier — la liste est dérivée du nom côté Rust,
pas devinée côté client — et annonce les formats avec perte (JPEG, GIF) avant l'écriture, pas
après. Trois chemins : le sélecteur de format du panneau de détail, le sous-menu « Exporter au
format » du clic droit, et l'export en **lot** de la sélection (formats communs à tous les
fichiers, bilan qui nomme les échecs au lieu de les taire).

Les vignettes de texture sont **réduites côté Rust** (`vfs_texture_thumb_png_b64`, plus grand côté
128 px), avec cache borné et file de décodage limitée (`src/lib/thumbs.ts`, source unique des deux
grilles). Servir la pleine résolution — ce que faisaient les deux grilles — décode 2048×2048 RGBA
par entrée : une seule page de vignettes faisait passer le processus de rendu WebView2 de 453 à
704 Mio, et parcourir `data/dx11/menu/200_icon/10_icon_chr/uniform` (12 560 `.g4tx`) tuait la
fenêtre. La grille du navigateur de contenu monte en outre par tranches de 300 entrées.

Le reste : vues liste et grille avec vignettes réelles pour les textures (chargées à la
visibilité), multi-sélection Ctrl/Maj sur fichiers **et** dossiers, barre
flottante de sélection (compteur, taille cumulée, copier les chemins, exporter, ajouter à un mod),
emplacements épinglés et récents à la `zoxide` avec menu contextuel par entrée, menus contextuels
Win32 natifs, presse-papiers fichiers natif (CF_HDROP), suppression vers la **corbeille** Windows
pour les fichiers de mod, ouverture avec l'application par défaut, et fusion VFS + CPK bruts dans
la même arborescence.

## Éditeurs de contenu

**`.cfg.bin`** — trois vues interchangeables sur le même document : JSON (Monaco, hors ligne, sans
CDN), **table** et **arbre**, toutes servies par `CfgbinViewer` et utilisées aux deux points
d'édition (`DetailPane` et l'onglet Données de `PropertyEditor`). Le modèle
(`src/lib/cfgbinModel.ts`) conserve le **lexème brut des nombres** : passer par
`JSON.parse`/`stringify` réécrirait le `1.0` d'un flottant RDBN en `1`, et la simple bascule de vue
modifierait des octets que personne n'a édités. Un garde-fou vérifie que réécrire ce qu'on vient de
lire redonne le même texte ; sinon les vues structurées passent en lecture seule pour ce fichier
plutôt que de le corrompre. Les écritures respectent ce que le pont JSON accepte en retour : en
T2B la valeur est toujours une chaîne, en RDBN la forme d'origine est conservée et les cellules
`Blob`/`Invalid` restent inéditables. Le ré-encodage RDBN étant un patch de valeurs, l'interface
n'offre ni ajout ni suppression de ligne, de liste ou de champ.

**Textures** — remplacement intégral d'un `.g4tx` mono-texture par un PNG, vérifié par
round-trip pixel-exact sur 998/998 fichiers réels. Pas d'édition pixel ni d'atlas multi-région.

**Éditeur de propriétés** — une entité du jeu n'existe pas dans un fichier mais éclatée entre
modèle, textures, sons, lignes de `.cfg.bin` et code machine. Le panneau prend un code interne
(`c01000010`, `whs00340`) et rassemble ses fichiers, ses données éditables et les fonctions et
classes RTTI de `nie.exe` qui le mentionnent, avec leur adresse statique.

**Encodeurs** — RDBN et T2B (100 % des `.cfg.bin` du jeu sont éditables), CPK non chiffré non
compressé pour l'export de mod, G4TX mono-texture.

## Mode Éditeur

Disposition d'éditeur de moteur : viewport 3D temps réel (three, GLB assemblé côté Rust et rendu
dans le navigateur — aucun CDN), hiérarchie, détails, navigateur de contenu.

**Viewport unique.** Le détail VFS, l'exploration d'un CPK brut et le mode Éditeur livrent tous
un GLB auto-suffisant au même composant WebGL. Les anciens aperçus PNG et turntable MP4 ont été
supprimés : une seule caméra interactive, un seul cycle de vie GPU et une seule voie d'assemblage
par origine (VFS ou CPK), sans rasteriseur Rust ou vidéo intermédiaire.

**Gizmos** de déplacement, rotation et échelle, avec les trois écueils traités : la caméra orbitale
est coupée pendant la manipulation, le raycast de sélection renonce quand le gizmo est saisi, et le
cadre de sélection suit l'objet. **Scène multi-assets** : les modèles s'ajoutent au Ctrl+clic,
sont indexés par chemin VFS, libérés individuellement, et la caméra recadre sur l'ensemble.

Le navigateur de contenu ne présente comme ouvrable que ce que le backend sait réellement
assembler (un `.g4md` dont le `.g4mg` de même radical est dans le même dossier).

**Animations : listées, pas jouées.** Un onglet dédié montre les clips déclarés par les archives
`.g4pk` d'un asset (nom, CRC32, bornes, fps, bit additif, nombre de cibles). Il n'y a ni bouton de
lecture ni barre de transport, et l'interface le dit : le GLB produit par `nie_formats::assemble`
ne porte ni `skins`, ni `animations`, ni `JOINTS_0`, donc rien n'est rejouable.

**La transformation n'est écrite nulle part.** Aucun encodeur géométrique n'existe côté Rust
(`g4md`, `g4mg`, `g4sk` n'ont aucune fonction d'écriture) : proposer un bouton de sauvegarde serait
une promesse vide. L'éditeur natif Fyrox reste accessible par un bouton.

**Une panne du viewport reste dans le viewport.** React démonte l'arbre entier quand une exception
traverse un rendu ou un effet : sans barrière, un `WebGLRenderer` qui ne peut pas obtenir de
contexte, un GLB refusé par `GLTFLoader.parse` (qui lève de façon *synchrone*, avant tout rappel
d'erreur) ou une frame qui échoue vidait `#root` — une fenêtre blanche, indiscernable d'un crash du
processus. `components/ErrorBoundary.tsx` en pose une à la racine et une autour du viewport (qui se
réarme au changement d'asset) ; le viewport garde en plus l'absence de WebGL, la perte de contexte
GPU (`webglcontextlost`) et l'échec de parse, chacun rendu comme un message nommé.

Côté Rust, les décodages lourds passent par `isoler()` : thread dédié à pile de 16 Mio et panique
convertie en erreur. Un débordement de pile natif tue le processus entier sans être rattrapable
(constaté sur `cridecoder`), et une panique dans une commande laisse la promesse pendante côté
client — l'interface reste alors sur « chargement… » sans jamais rien dire.

## Viola — modding

Les quatre opérations de l'outil amont, **en process**, servies par le crate `nie-viola` : aucun
binaire externe, contrairement à Viola elle-même et aux gestionnaires de mods qui la pilotent.

| Opération | Ce qu'elle fait |
|---|---|
| **Dump** | extrait les archives CPK en arborescence claire |
| **Pack** | réécrit `cpk_list.cfg.bin` pour que le jeu charge les fichiers du mod depuis le disque |
| **Merge** | combine plusieurs mods, avec fusion **au champ** des `.cfg.bin` |
| **Criware** | chiffre et déchiffre les conteneurs audio |

Ce qui distingue cette implémentation des trois autres (Viola en C#, son port C++ du dépôt,
`ievr_toolbox` en Rust) :

- **Dump** — les paquets sont ordonnancés du plus gros au plus petit (borne de Graham sur le temps
  total, contre aucune borne pour une file arbitraire), **mappés en mémoire** au lieu d'être lus
  en entier ou déchiffrés vers un dossier temporaire, et leur sommaire est indexé une fois par
  paquet au lieu du balayage linéaire par fichier que fait `Vfs::read`. S'y ajoutent la **reprise**
  d'un dump interrompu au paquet près et le **saut des fichiers déjà à la bonne taille**, qu'aucune
  des trois n'offre.
- **Pack** — le `cpk_list.cfg.bin` d'IEVR est chiffré en **AES-256-CBC** (clé et IV reversés de
  `nie.exe`), que le port C++ ne connaît pas : on relit et réécrit dans l'enveloppe réellement
  trouvée. Écriture atomique, et signalement d'un `cpk_list` qui a déjà servi à empaqueter.
- **Merge** — fusion à trois points **au champ**, possible uniquement parce que les formats sont
  compris : deux mods qui touchent des valeurs différentes du même `.cfg.bin` sont compatibles, là
  où une fusion au fichier en perd un. Le désaccord réel est tranché par la priorité **et
  compté**, jamais masqué ; sans base vanilla, le repli au fichier est explicite.
- **Criware** — traitement par tranches : un `.awb` de plusieurs Gio ne passe pas par la RAM.

Validé sur le jeu réel (`cargo run -p nie-viola --example valider_reel`) : `cpk_list` de 255 308
entrées relu à l'identique, 9 788 fichiers dumpés octet pour octet contre la lecture VFS, reprise
sans réécriture, entrée basculée hors paquet avec la bonne taille, deux modifications concurrentes
survivant à une fusion, et aller-retour de chiffrement exact sur un `.cpk`.

## Données du jeu

**Deux sources, une interface.** L'application ouvre indifféremment une installation du jeu
(`data/cpk_list.cfg.bin` + `data/packs/*.cpk`) ou un **dump extrait** (`data/common/`,
`data/dx11/`) : le VFS sert les mêmes chemins logiques dans les deux cas, donc navigation,
aperçus, export et éditeurs fonctionnent sans rien savoir du montage. Une machine qui n'a que le
dump ouvre l'explorateur, `check_game_dir` accepte les deux, et la détection par défaut essaie
l'installation d'abord (`NIE_GAME_DIR`, cwd, Steam) avant de retomber sur un dump.

Ce qui change visiblement : les statistiques VFS annoncent la provenance (« dump extrait » /
« packs CPK ») et masquent les compteurs de packs qui vaudraient zéro sur un dump — « 255 316
fichiers, 0 CPK » se lisait comme une anomalie. Le toast de préchargement la nomme aussi. Sur un
dump, l'extraction d'un fichier est une copie disque et non un `read` en mémoire (un `.usm`
dépasse les centaines de mégaoctets), et l'édition en place vaut pour tout le contenu — il n'y a
pas d'archive à réencoder, mais elle modifie le dump lui-même.

Mesuré le 2026-08-28 sur ce poste : le dump sert **255 308 / 255 308** chemins de l'index du jeu
(100,000 %, `cargo run -p nie-formats --example dump_couverture`), et les deux montages rendent
des octets identiques (`cargo test -p nie-formats --test dump_vs_packs`).

Dix-sept familles câblées avec DTO typé et onglet dédié : techniques, objets, Avatar/Keshin,
succès, quêtes, boutiques, stades, capacités passives, tactiques spéciales, écussons, galerie,
feintes, activités, équipes, formations, uniformes, plus le sélecteur du calculateur de stats.
Le chargement est factorisé en trois primitives (`load_t2b`, `load_rdbn`, `load_text`), la
résolution du fichier de texte passant par `nie_data::text::text_file_name` au lieu d'un prédicat
en dur.

Les jointures de texte sont soit validées (208/208 noms d'équipe), soit absentes du jeu et dites
comme telles : `formation_text.cfg.bin` n'existe pas dans cette version, donc formations et
uniformes affichent des identifiants bruts. Les entrées sans code interne ne sont pas cliquables —
pas de fausse affordance.

**Calculateur de stats** sur les tables de croissance embarquées, pour 6 101 personnages résolus.

Environ 101 des 110 modules `nie-data` restent sans câblage typé ; le patron est mécanique pour
les familles à noms autoportés, spécifique dès qu'il faut une jointure de texte.

## Outils de reverse

Recherche de fonctions labellisées, classes RTTI et xrefs sur `var/niers.sqlite`, avec renommage
écrit en base (`name_source = 'user-edit'`).

**Lecture du process vivant** — détection, plages du module, lecture d'octets, dump des plages
lisibles vers `AppData`. Strictement en lecture : l'écriture mémoire et le patch EAC ne sont pas
exposés à l'IPC, et ne le seront pas.

**Scan AOB** — motif façon Cheat Engine (`44 8B ?? 10`) sur un minidump **déjà capturé**, donc
sans aucune attache au process. Le code de lecture vit dans `crates/forge/nie-dump`, dont la seule
dépendance est `thiserror` : c'est ce qui le rend liable par l'application, là où `nie-re` traîne
`rusqlite` (conflit de lien natif avec le `sqlx-sqlite` du plugin SQL) et une dépendance hors
dépôt. Les adresses traversent l'IPC en chaînes hexadécimales — `specta` refuse les entiers 64
bits, et ce refus fait paniquer l'export des bindings au démarrage.

**Forge** — la part de `nie.exe` que le dépôt produit réellement, à l'octet, et ce qui bloque
encore, trié par octets. Les deux commandes relisent les artefacts (`var/forge/cover.json`,
`forge/registry.json`, `forge/asm/*.s`) à chaque appel : aucune valeur figée, aucun shell-out vers
la CLI. Le seau « validé sémantiquement » est affiché en retrait — il n'est jamais compté comme
produit.

`nie-forge` est liable ici parce que sa dépendance `rusqlite` a été rendue optionnelle (feature
`redb`, coupée pour l'explorateur) : `rusqlite` porte `links = "sqlite3"` et l'application en a
déjà une copie via `sqlx-sqlite`. Le reste du crate — recouvrement, registre, source assembleur,
rapport — est pur. C'est l'inverse du choix fait pour le scan AOB, où c'est le *code* qui avait
été extrait (`nie-dump`) : ici la coupure passait par une feature, sans rien déplacer.

Piège vérifié : la règle « les sections-tables ré-émises comptent comme produites » vivait dans la
commande CLI et non dans le crate. L'onglet, qui rappelle la même chaîne, sous-déclarait donc de
1 427 968 octets — 4,2 points. Elle est maintenant dans `Report::add_emitted_tables`, appelée des
deux côtés, et `nie-forge/tests/chaine_rapport.rs` échoue si les deux mesures divergent.

## Lua

Décodeur et désassembleur du bytecode PUC-Rio 5.2 (1 143/1 143 scripts du jeu décodés, 985 971
instructions), exécution instrumentée avec capture de `print` et limite d'instructions, éditeur de
valeurs forcées avant exécution, et **session persistante** (vraie REPL) avec cycle de vie
`attach`/`reload`/`broadcast` sur le modèle d'Overload. Un rapport confronte l'API réclamée par les
scripts à celle que l'hôte fournit — c'est la liste de travail du portage moteur, produite par
l'exécution elle-même.

Binder `Live` : un script lit la mémoire de `nie.exe` en cours d'exécution. Aucune écriture n'est
exposée, et un test le vérifie.

C'est un **désassembleur, pas un décompilateur** : le listing est annoté, le source n'est pas
reconstruit.

## Mods, opérations longues, sauvegardes

Espace de travail de mods dans `AppData` (jamais dans le dossier du jeu), export en `.cpk` réel
rechargeable, remplacement de texture, mise en scène par lot depuis la sélection.

**Journal durable des opérations longues** — table `jobs` (état, avancement, erreur, horodatage),
gestionnaire en pied de barre latérale, et réconciliation au démarrage : un job resté « en cours »
après une fermeture est marqué interrompu plutôt que d'afficher éternellement une opération
fantôme. Il n'y a **pas de reprise automatique** — le dump Viola excepté, qui reprend au paquet
près par son propre manifeste.

**Sauvegardes** — détection du slot Steam le plus récent sur toutes les bibliothèques et tous les
comptes, chaque candidat étant validé par déchiffrement réel plutôt que par son nom.

## Blender, recherche, ponts

L'extension `plugins/niers-blender` s'installe réellement (dossier d'addons utilisateur,
préférence de racine de données persistée), et son panneau cherche dans le VFS ou par nom localisé
(miroir SQLite + GraphQL azalee) puis importe le modèle trouvé, sans jamais geler l'interface. Le
pont inverse construit une scène `.blend` depuis un personnage et une technique.

Recherche personnage/technique sur deux sources interrogées en parallèle, une source indisponible
devenant une notice et jamais un silence. Palette de commandes Ctrl+K.

Pont de contrôle avec le serveur MCP `niers-game` (l'un pilote l'autre), interface localisée
FR/EN/JA, fenêtre sans bordure réelle avec menus Win32 natifs.

---

## Ce qui reste ouvert

**Vérifié seulement par compilation.** L'application n'a pas été lancée à l'écran depuis les
derniers ajouts : onglets multiples, vues table/arbre, gizmos, panneau d'animations, scan AOB et
onglets de données sont validés par `tsc`, `cargo check` et les tests qui s'exécutent, pas par
observation. C'est la limite la plus large de ce document.

**Un seul CPK ouvert à la fois côté backend.** `RawCpkState` est écrasé à chaque ouverture et les
commandes `raw_cpk_*` ne prennent qu'un index : deux onglets descendus dans deux `.cpk` différents
se disputent le même lecteur. Un témoin partagé réémet l'ouverture quand un onglet redevient
actif, ce qui masque le problème sans le supprimer. Le vrai correctif est un handle par onglet,
côté Rust.

**Le pont MCP ignore les onglets.** `getState`/`navigate`/`open` ne connaissent qu'un couple
préfixe/sélection et visent l'onglet actif ; étendre le protocole toucherait `packages/nie-bridge`
et `apps/nie-mcp`.

**Pas de lecture d'animation.** Elle suppose d'écrire un exportateur glTF *skinné* en Rust
(`skins`, `JOINTS_0`/`WEIGHTS_0`, échantillonnage des canaux G4MT) et un échantillonneur public de
translation et d'échelle — aujourd'hui seule la rotation l'est. S'y ajoute un trou de reverse non
fermé : les indices d'os d'un maillage sont annotés « locaux » d'un côté et supposés « globaux »
de l'autre.

**Pas de sauvegarde de scène vers le jeu**, faute d'encodeur géométrique. Pas d'annulation des
transformations non plus.

**Le scan AOB n'a jamais tourné sur un vrai minidump** — il n'y en a pas sur ce poste. Il n'est
pas non plus annulable ni suivi : un motif absent parcourt tout le dump sans progression.

**Les vues table/arbre n'ont pas été mesurées à l'échelle annoncée.** La virtualisation est
dimensionnée pour les 14 448 lignes de `chara_base`, mais aucun profil de défilement n'a été pris,
et les assertions du modèle tournent sur du JSON synthétique reproduisant la forme réelle, pas sur
un fichier décodé.

**Le front n'a pas de lanceur de tests.** Les vérifications du modèle `.cfg.bin` vivent hors du
dépôt ; les câbler suppose de toucher `package.json`.

**Pas de réordonnancement d'onglets à la souris** (aucune bibliothèque de glisser-déposer, et il
n'en a pas été ajouté).

**~101 modules `nie-data` sans câblage typé**, et le `pack` Viola n'a pas été chargé dans le jeu
— l'aller-retour de format est prouvé, la relecture par `nie.exe` ne l'est pas, et l'essayer sous
EAC actif n'est pas dans le cadre du projet.

---

## Vérifier

```bash
# Front
cd apps/nie-explorer && ./node_modules/.bin/tsc --noEmit -p tsconfig.json && ./node_modules/.bin/vite build

# Backend Tauri (les tests n'y démarrent pas sur ce poste : STATUS_ENTRYPOINT_NOT_FOUND)
cd apps/nie-explorer/src-tauri && cargo check --lib && cargo clippy --lib --tests

# Bindings TypeScript, après toute commande ajoutée — jamais édités à la main
cd apps/nie-explorer/src-tauri && cargo run --bin export-bindings --features dev-bindings

# Opérations de modding, sur le vrai jeu
cargo run -p nie-viola --example valider_reel --release

# Les deux montages servent bien la même chose (installation ET dump requis)
cargo test -p nie-formats --test dump_vs_packs -- --nocapture
NIE_DUMP_DIR=<dump> cargo run -p nie-formats --example dump_couverture
```
