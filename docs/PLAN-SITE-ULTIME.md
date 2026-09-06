# Le site ultime — exigence de couverture sur toute la surface du dépôt

> Consigne de l'utilisateur, 2026-09-06. Ce plan **remplace** l'horizon de `/PLAN.md` (qui
> reste valable pour la bascule Azalée → Vercel et ses gates). Il ne décrit pas une semaine :
> il décrit **l'état d'arrivée** — un seul site, qui expose tout ce que ce dépôt sait faire.

---

> **Amendement du 2026-09-06 (3) — la façade est passée au crible, et `nie-aphrody` est branchée.**
> Cette session n'a pas ajouté de capacité : elle a **retiré** ce que la façade montrait à tort et
> **servi** ce que le dépôt portait déjà. Trois mesures la résument :
>
> - l'accueil affichait la même information jusqu'à **trois fois** et exposait sept liens
>   d'infrastructure (§ 3, lignes UI) ;
> - le site avait **deux chartes** — l'accueil dans la DA du jeu, les écrans secondaires dans une
>   seconde interface sombre avec son propre modèle de tuiles ;
> - `nie-aphrody` n'était servie **par aucune route** alors qu'elle embarque au build le package
>   du personnage et tout son dossier. Elle l'est maintenant par **sept** (lot 5, § 5).
>
> Le lot 5 passe donc de « à brancher » à **fait, mesuré** ; le lot 4 gagne son premier écran
> réellement épuré ; le lot 6 gagne son SEO (18 URL au plan de site au lieu de 15). Ce qui reste
> ouvert est dit tel quel au § 5 bis.

---

> **Amendement du 2026-09-06 (4) — la couche 3D est branchée, l'écran d'attente est celui du
> jeu, et les filtres sont enfin comptés.**
> Quatre chantiers menés en parallèle sur des périmètres disjoints. Ce qui les relie est une
> même découverte : **le site ne manquait pas de capacités, il manquait de routes et de
> mesures.** Quatre chiffres la résument :
>
> - la 3D est servie en **12 routes** mesurées (`/api/v1/3d/*` décrit, `/model/*` sert) sur un
>   corpus de **6 191 modèles assemblables** — et le catalogue ne proposait jusque-là que des
>   *pièces* (`.g4mg` seuls, 143 000 fichiers dont aucun n'était affichable) ;
> - les filtres d'Aphrody sont inventoriés contre leurs équivalents : **48 recensés,
>   42 manquants** (`docs/FILTRES.md`) ;
> - les 4 vues du catalogue ne couvrent que **143 246 des 255 308** entrées — **112 062**
>   fichiers (`.bin` 72 308, `.p3lip` 21 047, `.objbin` 12 190) ne sont atteignables que par le
>   parcours, sans le moindre filtre ;
> - `nie-lua` expose **34** `pub fn`, pas 99 — le § 4 citait un compte qui ne se rejoue pas.
>
> Le lot 8 (les filtres) entre au plan. Le § 7 gagne une question que je ne tranche pas seul :
> couvrir tout le VFS en slugs contredit une décision documentée de ce même plan.

---

> **Amendement du 2026-09-06 (5) — la gate maîtresse est atteinte côté VFS : `manquant = 0`,
> `partiel = 0`.**
> Le lot 9.1 est fait, et il n'a **rien exigé de neuf**. Les neuf parseurs qui manquaient au
> site vivaient derrière la feature `std` de `nie-formats` — une feature **par défaut** : ils
> étaient déjà liés dans le binaire, aucune route ne les appelait. Le diagnostic du § 9 tenait
> donc à la lettre : « 82 % du reste à faire est du câblage, pas de la recherche ».
>
> | État | Avant | Après | Ce qui l'a déplacé |
> |---|---:|---:|---|
> | `servi` | 162 219 | **246 003** (96,35 %) | 9 familles décodées en process, plus 31 fichiers rares |
> | `manquant` | 67 878 | **0** | `/api/v1/formats/decode/{chemin}` |
> | `partiel` | 15 875 | **0** | les `.g4mg`, description comprise |
> | `interne` | 5 512 | 5 512 | inchangé, la raison est écrite |
> | `bloqué` | 3 776 | 3 784 | descend par le RE, pas par le câblage |
> | `inconnu` | 48 | **9** | § 9.2 — 37 des 46 identifiés, les 9 `.g4tg` assumés |
>
> Trois faits mesurés qui n'étaient dans aucun document :
>
> - **Aucun `.g4mg` n'est orphelin.** 8 955 ont leur `.g4md` frère, **6 920 l'ont empaqueté
>   dans leur `.g4pkm` voisin**, 0 n'ont ni l'un ni l'autre. Le second cas n'est pas une
>   exception : c'est 44 % du corpus, et `g4pkm::extract_g4md` savait déjà l'ouvrir.
> - **Quatorze fichiers portent le magic `G4PK` sous un suffixe de révision** (`.g4pk.r41152`,
>   `.r47929`, `.r51528`…). Les classer `inconnu` sur leur extension aurait été croire le nom
>   plutôt que le contenu — la route reconnaît désormais au **magic** quand le suffixe se tait.
> - **Le VFS contient des noms de fichier avec un espace** (`…/u021801/u021802 .g4md`).
>   Découper l'inventaire par espaces en fait deux faux « fichiers sans extension » : deux des
>   48 `inconnu` n'en étaient pas.
>
> Preuve : `scripts/validation/mesurer-geometrie.sh`, échantillon à pas régulier, 25 par
> famille — **225/225 décodages conformes (100 %)**. « Conforme » exige le jeton de famille
> dans le corps : un 200 qui rendrait un résumé vide compte comme un échec.
>
> Le point 2 des ouvertures (§ 5 bis) est réglé au passage : `app::ROUTES` figeait 19 routes
> pour un routeur qui en monte **37**. Une macro les déclare une seule fois et en tire le
> montage **et** la liste ; `tests/routes.rs` interroge les 37 par couverture de motifs, pas par
> égalité de longueurs.
>
> **Ce que cet amendement ne dit pas :** la matrice de couverture du § 4
> (`var/couverture-site.json` + `/couverture`) n'existe toujours pas. La gate est atteinte sur
> le **VFS**, qui n'est qu'une des sources de la matrice — les 41 commandes de `niers`, les 155
> d'Inacord et les 81 pages d'Azalée restent non classées. Dire « gate maîtresse atteinte » sans
> cette phrase serait exactement le genre de raccourci que le § 3 recense.

> **Amendement du 2026-09-06 (6) — la gate maîtresse est TENUE : `manquant = 0`, `partiel = 0`.**
> Vingt-deux routes plus tard, l'instrument construit le matin même rend `tenue: true`. Le
> compte, par `nie-site --regenerer-couverture var/couverture-site.json` :
>
> | État | Le matin | Ce soir |
> |---|---:|---:|
> | `servi` | 255 | **278** |
> | `manquant` | **26** | **0** |
> | `partiel` | 0 | 0 |
> | `bloqué` | 10 | 10 |
> | `interne` | 292 | 294 |
> | routes montées | 56 | **77** |
> | incohérences | 0 | 0 |
>
> **Aucune des vingt-deux n'a demandé une feature de plus.** C'est, à la lettre, ce que le
> § 9 avait déjà mesuré : le code était compilé dans le binaire, il manquait une adresse. Les
> six `nie-formats` servis en process sont sous `std` — la feature par défaut — et
> `images`/`textures` restent éteintes, comme le `Cargo.toml` de la crate l'écrit et comme
> `routes::formats` continue de le publier.
>
> **Ce que la matrice a fait, et qu'aucune relecture n'aurait fait :** elle a désigné 26
> capacités, et la moitié n'était pas ce que sa raison annonçait.
>
> - **Trois raisons écrites étaient fausses.** `nie_explore::icons` et
>   `nie_explore::mode_index` n'existent pas — les deux modules vivent dans `nie-cli`, qui n'a
>   **pas de cible `[lib]`** et n'est donc importable par personne ; et
>   `parse_player_passives` prend **trois** tables de texte, pas deux. Une raison qui cite un
>   chemin inexistant envoie le lot suivant chercher au mauvais endroit : elle coûte plus qu'une
>   ligne vide.
> - **Deux capacités étaient déjà servies** sans que personne l'ait vu : `niers avatar` et les
>   deux commandes d'avatar d'Inacord le sont par `/api/v1/donnees/famille/chara_edit`
>   (16 listes), depuis que `chara_edit` est entré dans `decode_by_key` le matin même. Deux des
>   six pages `/tools/*` d'Azalée l'étaient aussi (`/tools/stats` par `/api/v1/regles/stats`,
>   `/tools/compare` par `/api/v1/regles/comparaison`) — la règle de préfixe les classait
>   `manquant` **en bloc**, ce qui est exactement le défaut que l'état `partiel` avait été créé
>   pour empêcher, transposé d'un corpus à une source.
> - **Un doublon a été fusionné plutôt que servi.** `nie-data::team` et
>   `nie-data::enjoy_mode_team` étaient deux ports du même fichier, arrivés dans le **même
>   commit** : mêmes 7 variables, même parseur d'inagle en référence. Servir le doublon aurait
>   fait un `servi` de plus et un défaut de fond de moins visible. `nie-data` passe de 116 à
>   115 modules.
>
> **Les comptes, relevés sur le binaire lancé avec le VFS monté (255 308 entrées) — jamais
> relus dans le diff :**
>
> | Route | Ce qu'elle rend | Compte mesuré |
> |---|---|---|
> | `/api/v1/passives` | joueur, équipe, lots, **5 fichiers joints** | 1 716 / 21 / 653, 128 effets ; `?q=tir` → 1 716 **→ 221** |
> | `/api/v1/playstyles` | style de jeu et sa distribution | 6 166 personnages, **{1055, 1034, 1059, 1003, 989, 1026}** |
> | `/api/v1/conditions/{blob}` | cadrage **et** sémantique d'un blob | blob réel → v0 valide, `story`, seuil 20010, épisode 1 |
> | `/api/v1/inspect/font/{path}` | métriques de fonte | `font_def` : **7 469 glyphes**, atlas 4096×2048, ascent 46 / cell 71 |
> | `/api/v1/inspect/menu/{path}` | géométrie d'un `.objbin` | `mainmenu01_00_background` : priorité 650, et le `.g4pkm` **absent est nommé** |
> | `/api/v1/icons` | index nom → atlas + rectangle | **3 770** icônes, 212 atlas ; `?q=abl` → **3** ; PNG amont 200 / 71 200 o |
> | `/api/v1/modes/{slug}` | écrans, calques, scripts d'un mode | `victory-road` : 28 écrans, 919 composants, 32 scripts, funcLua 3 659 |
> | `POST /api/v1/team/synergy` | notation d'équipe | score 67, 2 synergies, 1 recommandation |
> | `/api/v1/text/translate` | le même terme d'une langue à l'autre | « Tornade » fr → en/ja : **69** correspondances |
> | `POST /api/v1/save/roster` | résolution d'effectif en lot | 5 identifiants → 2 nommés, 1 doublon, 1 rejeté, 1 `name: null` |
> | `/assets/export/…?format=` | les 8 formats d'`ImageOut::TOUS` | 8/8 en 200, chacun avec son `Content-Type` |
>
> **La distribution des styles de jeu mérite d'être notée** : elle retombe **exactement** sur
> celle documentée dans `nie-data/src/playstyle.rs`, obtenue par un chemin entièrement
> indépendant (route HTTP contre dump de développement). C'est le genre de recoupement que ce
> plan réclame et qu'il obtient rarement.
>
> **Deux leçons, au-delà du lot.**
>
> 1. **Un témoin de `manquant` choisi parmi le travail restant se périme à chaque lot.** Les
>    deux précédents (`nie-data::shop`, puis `/tools/compare`) ont fait rougir
>    `couverture::tests` le jour où ils ont été servis. Le plan visant `manquant = 0`, il n'en
>    resterait aucun à la fin — et le test serait devenu immaintenable au moment précis où il
>    compte. Le témoin est désormais le **filet** (`data-familles`, `Motif::Tout`) : un
>    invariant, pas un état d'avancement.
> 2. **La matrice n'était pas versionnée**, alors que le § 4 l'exige en toutes lettres. Elle
>    vivait sous `/var`, exclu en bloc pour une raison mesurée (15,5 Go). La ré-inclusion en
>    trois temps (`!/var/`, `/var/*`, `!/var/couverture-site.json`) ne fait descendre git dans
>    aucun sous-arbre : **`git status --short` = 0,03 s avant, 0,02 s après**. Le coût invoqué
>    n'existait pas ; il n'avait jamais été mesuré.
>
> **Ce que cet amendement ne dit pas.** `bloqué = 10` (3 600 fichiers : shaders, particules,
> tissu, navigation) ne descend pas par du câblage — il descend par du reverse, et rien ici n'y
> touche. La gate maîtresse porte sur `manquant` et `partiel` ; les quatre autres conditions du
> § 8 — les 475 écrans, la SSIM par écran, le sas `legacy/` à 87 fichiers — restent ouvertes.

---

---

## 1. Ce que « ultime » veut dire ici

Un seul site — Aphrody, servi par `nie-site`, monté par `apps/nie-web` et par Inacord — où :

- l'interface est **l'UI du jeu**, mesurée contre des captures, pas dessinée de mémoire ;
- **tous les composants d'Azalée** sont disponibles, dans la DA du site ;
- la **« space UI »** de l'ancien `nie-explorer` est servie **page par page**, là où elle a un
  sens (parcours, inspection, aperçu), et pas ailleurs ;
- **tout `nie-formats`, tout `nie-data`, tout `nie-game`** est atteignable ;
- **`nie-lua` sert les menus et les scripts** — la disposition et le comportement viennent du
  runtime, pas d'un gabarit écrit à la main ;
- **`nie-aphrody` sert les icônes, les assets, les pets et les personnages** d'Aphrody ;
- **tout ce que `niers` sait faire est servi par l'API de `nie-site`** ;
- **tout ce que `nie-explorer` savait faire est servi par `nie-web` et Inacord**.

**Ce qui n'est pas montré à l'utilisateur final n'est pas exclu du périmètre.** Une capacité
peut n'exister qu'en API, sans page : c'est du backend, il doit quand même être servi,
documenté et testé. La couverture se mesure sur la **surface exposée**, pas sur le nombre
d'écrans.

## 2. Le capital existant, mesuré le 2026-09-06

Rien ici n'est cité de mémoire ; chaque ligne a une commande.

| Surface | Compte | Commande |
|---|---|---|
| Crates du workspace | **37** | `cargo metadata --no-deps \| jq '.packages \| length'` |
| Sous-commandes de `niers` | **41** | `niers --help` |
| Commandes Tauri d'Inacord | **155** uniques | `rg -A2 '#\[tauri::command\]' apps/inacord/src-tauri/src` |
| Routes servies par `nie-site` | **19** déclarations, **~14** chemins distincts | `rg -o '\.route\("[^"]+"' crates/tools/nie-site/src/app.rs` |
| Modules de `nie-data` | **117** | `ls crates/engine/nie-data/src/*.rs` |
| Modules de `nie-formats` | **47** | idem |
| `pub fn` de `nie-lua` | **34** | `rg 'pub fn' crates/engine/nie-lua/src/` — mesuré le 2026-09-06. Le **99** cité jusqu'ici venait d'un `rg -c '^pub fn'` sur `src/*.rs` seul (les sous-modules manquaient dans un sens, les lignes non publiques comptaient dans l'autre) : il ne se rejoue pas. |
| Modules de `nie-aphrody` | 5 (`assets`, `codex`, `gisement`, `pets`, `pixel`) | `ls` |
| Pages d'Azalée | **81** pages, **24** routes API | `fd 'page.tsx' apps/azalee/app` |
| Fichiers d'`inacord-ui` | **51** | `fd -e tsx -e ts . packages/inacord-ui/src` |
| Fichiers en **sas** `nie-web/src/legacy/` | **87** | `fd . apps/nie-web/src/legacy` |
| Pages réelles de `nie-web` hors sas | **5** | `fd -e tsx . apps/nie-web/src --exclude legacy` |
| Sous-commandes du toolkit C++ `iecode` | **39** | `ls src/cli/commands/*.cpp` |
| Entrées du VFS | **255 308** | `niers info` |

**L'écart qui définit ce plan :** 41 commandes CLI et 155 commandes desktop, pour **14 chemins
d'API**. Le dépôt sait faire environ dix fois ce qu'il expose. Ce plan ne demande pas d'écrire
des capacités nouvelles : il demande de **servir celles qui existent**.

## 2 bis. Ce que la session RE/Lua a établi — le capital le plus sous-exploité

Mesuré par Codex les 2026-09-05 et 06, rejouable par `niers lua audit` :

| Mesure | Valeur | Ce qu'elle change |
|---|---|---|
| Scripts Lua du jeu exécutés par notre runtime | **1 197 / 1 197**, `ok = 1 197`, **0 erreur** | le runtime Lua n'est plus une preuve de concept : il exécute la totalité des scripts |
| Scripts de menu | **552 / 552**, 0 erreur | la couche menu est entièrement franchie |
| Includes non résolus | **0** | la résolution VFS des `include` est complète |
| Constantes non définies | **47 symboles, 225 occurrences** | c'est le SEUL écart restant, et il est chiffré |
| KB `var/niers.sqlite` | 153 073 fonctions, 1 748 classes RTTI ; `pdata` 94 785, `ghidra` 60 183, `vtable-struct` 13 653 | la carte est là, elle n'est pas exploitée par le site |
| Vtables vérifiées dans l'image | **1 748 / 1 748** lisibles, 1 745 en `.rdata`, 1 745 pointant du code à +8 | la carte RTTI est structurellement cohérente |
| Couverture brute `niers rebuild --rounds 4` | 100 664 / 108 650 = **92,65 %**, nommées 13 653 = 12,57 % | l'écart nommé/classé reste le vrai chantier RE |

Les 47 constantes en tête d'occurrences : `CHARA_EDIT_RECIPE_TYPE_FASHION` (49),
`EVEN_BONE_L21..L24` et `R21..R24` (13 chacune), `VICTORY_TOP_INC` (11),
`SOCCER_RESULT_MENU` (9), `CHARA_FILTER_MENU` (6). Ce sont des valeurs à retrouver dans le
binaire ou les includes, pas du code à écrire.

**Conséquence pour le lot 3 :** servir les menus par le runtime Lua n'est plus un pari. Le
travail restant est une route et 47 constantes, pas un moteur.

## 3. Ce que les dernières sessions ont raté, et la règle que chaque échec impose

Ce plan est fondé sur ces échecs. Chaque ligne est un défaut réellement payé ici.

| Échec mesuré | Ce qu'il a coûté | Règle qui en découle |
|---|---|---|
| `/chara` rendait **200 en 87 ms, 136 921 o, 0 lien** — `SUPABASE_INTERNAL_URL` gagnait dans `pickUrl()` | une journée, et une gate annoncée verte | **Compter le contenu, jamais le statut.** Une gate qui ne rend pas un nombre n'est pas une gate. |
| Preuves `uemu` : **0 ✓ sur 47** (28 ✗, 19 délais) | l'oracle byte-exact est hors service et personne ne l'a vu | **Une suite qui ne tourne plus est un échec, pas un silence.** À rejouer avant toute affirmation d'exactitude. |
| `bun run typecheck` rouge sur **2 paquets** (`@rosegriffon/mcp` 5× TS2307, `@rosegriffon/cron` 3× TS2305) | le portail TS ne distingue plus une régression d'un rouge de fond | **Un portail rouge de fond doit être réparé avant d'ajouter quoi que ce soit.** |
| `toSorted()` dans `packages/inacord-ui`, monté par un hôte **ES2022** | 2 typechecks sur 3 étaient verts | **Une bibliothèque partagée tient au dénominateur commun de ses hôtes.** |
| `nie-site` en ligne sert un **binaire périmé** : `/api/v1/episodes` rend 500 | le correctif existait dans les sources depuis des heures | **Corriger la source ne corrige pas la production.** Le lot n'est fini qu'une fois le binaire rebâti. |
| Export de layout `mainmenu01` : **8 objets muets sur 34**, **24 jamais positionnés** | une reconstruction prise pour une mesure | **Dire ce que la donnée ne contient pas.** Un objet sans position est un manque de l'export, pas un détail de rendu. |
| SSIM `mainmenu01` ≈ **0,004** (plancher de non-régression 0,003) | « pixel-perfect » annoncé sans chiffre | **« Pixel-perfect » est un nombre ou n'est rien.** |
| L'angle des parallélogrammes déclaré « non mesurable » (R² < 0,45) | une DA posée à l'œil pendant des semaines | **Un R² bas accuse la méthode avant la forme.** Mesuré ligne à ligne : R² = 1,000. |
| Règle `*.txt` : les 4 templates askama de `nie-site` hors du dépôt | la crate ne compilait pas sur un clone frais | **`git check-ignore -v` sur tout fichier non-code nouveau.** |
| `188e409` a capté 3 fichiers d'une autre session à mi-course | un lot attribué au mauvais auteur | **`claim:` avant d'écrire, un commit par lot.** |
| L'accueil affichait le total des ressources **3 fois**, le compte par catalogue **2 fois**, le bouton « Calque » **3 fois**, l'explorateur et le flux Atom **2 fois chacun** | un écran illisible, et trois endroits à corriger pour changer un chiffre | **Une information, un seul endroit.** Un second affichage du même fait est un bug, pas une redondance utile. |
| Le calque exporté du jeu, rendu **sous** l'interface à 18 % d'opacité, laissait son texte dans le document : « Photos commémoratives disponibles après », « Exclure plusieurs joueurs », « Fusion rapide » | des libellés d'un AUTRE écran du jeu lus par les lecteurs d'écran et les moteurs, au milieu de l'accueil | **Une opacité n'efface rien.** Un calque de validation ne se met pas en façade ; ce qui est décoratif ne doit pas être lisible. |
| Deux guides de touches — « F » et « V » — alors que `rg 'keydown|onKeyDown'` rend **0** dans `nie-web` et `inacord-ui` | une interface qui promet un raccourci inexistant | **Une affordance se vérifie avant d'être dessinée.** |
| Sept liens d'infrastructure en façade : `nie-site 0.5.9`, `255 308` entrées indexées, `/api/v1/health`, `sitemap.xml`, `llms.txt`, GitHub, et le domaine du site écrit sur le site | l'utilisateur lisait l'exploitation, pas le produit | **Aucune donnée ni lien technique en façade.** Le SEO et l'API restent servis — pour les robots et les agents, pas dans le menu. |
| `/explorateur` sortait avec `<title>explorateur — Aphrody</title>`, identique dans les trois langues, et manquait au plan de site comme à `robots.txt` | une entrée du menu invisible pour les moteurs | **Une entrée que le serveur ne connaît pas est une page sans titre.** Le front et le back partagent la même liste d'entrées. |
| Une capture Chrome headless à budget court montrait la page complète et **la place du sprite vide**, alors que le DOM portait le bon élément et la bonne position de fond | une heure passée à chercher un bug de composant | **Une capture ne prouve pas une absence.** Vérifier `--dump-dom` avant d'accuser le rendu ; un atlas de 1,5 Mo n'est pas décodé dans un budget de 3 s. |
| Le personnage était agrandi **×1,52** — 1,35 posés par le composant, ×1,125 par la mise à l'échelle du canevas 1280 → 1440 | un sprite crénelé sans qu'aucune valeur soit fausse | **Dans un canevas mis à l'échelle, les facteurs se multiplient.** L'échelle d'un élément se raisonne en pixels rendus, pas en pixels du canevas. |
| `format!("{:?}")` sur une `Option` publiait `"Some(V2)"` dans un JSON destiné à être lu | le nom Rust d'une variante, entouré de son conteneur, servi comme donnée | **Un JSON public ne se sérialise pas par `Debug`.** |
| Un test de gamut bâti sur `FromColor::from_color`, qui **écrête lui-même** (`palette-0.7.7 … from_color_unclamped(t).clamp()`) | un test qui s'exécute, passe, et ne vérifie rien — pire qu'une suite absente, parce qu'il rassure | **Un test doit pouvoir échouer.** Le prouver par falsification : casser volontairement la valeur et voir rougir. |
| La palette mesurée d'Aphrody est **crème 25 %, blond 21 %, bleu 2 %** | peindre au prorata donnerait un site où rien ne se détache | **Une palette de personnage n'est pas une palette d'interface.** On en dérive des rôles à clarté posée, en conservant la teinte mesurée. |
| Le personnage — tenue **blanche** — posé sur un ciel passé au **crème** de la palette : DOM juste, URL juste, PNG juste, **rien à l'écran** | une heure de suspicion sur le composant, alors que le défaut était né de la refonte des couleurs faite le même jour | **Un changement de palette est un changement de contraste.** Deux corrections justes prises séparément peuvent s'annuler ; ce qui se vérifie est l'écran, pas le fichier. |
| Le canevas portait `FOND_MENU = "#f9fdf9"` (hex en dur, verdâtre) quand le `<body>` portait `--jeu-ciel-clair` (`#f9f6f4`, crème) | une bande d'une autre teinte en bas d'écran, qu'aucune valeur fausse n'expliquait | **Une source unique de couleur ne tolère aucune exception**, pas même une constante de géométrie mesurée sur une capture. |
| Le viewport 3D écrit en **WebGL 2** là où la consigne était **WebGPU** | une couche entière à retraduire (GLSL → WGSL, NDC z ∈ [-1,1] → [0,1]) | **La technologie d'une couche est une décision de l'utilisateur, pas un détail d'implémentation.** Se la faire confirmer coûte une phrase ; la deviner coûte un lot. |
| `/b` **accepte** `q` et ne l'applique jamais (déclaré dans le type de query, absent du handler) | un client qui filtre croit filtrer, et la liste complète passe pour un résultat | **Un paramètre accepté est un paramètre honoré.** L'ignorer en silence est pire que le refuser. |
| `cpk_filename` **jeté** à la construction de l'index alors que `nie-formats` le fournit | le filtre « quel CPK » déclaré impossible pour une donnée déjà lue | **Ce qu'on a lu, on le garde** — ou on écrit pourquoi on le jette. |
| `App.tsx` ne sondait `/api/v1/health` **qu'une fois** (`useEffect` à dépendances vides) alors que le VFS se monte en fond | l'écran d'attente n'aurait jamais basculé vers le menu : le site aurait attendu pour toujours un état qu'il ne redemandait plus | **Une sonde unique ne mesure pas un état qui change.** Un montage asynchrone se reboucle jusqu'à un état tranché. |
| `title00` pris pour l'écran de démarrage : **67 objets, 21 à la position par défaut**, sprites = atlas entiers jusqu'à 5828×6840 | une composition cassée présentée comme le rendu du jeu | **Le nom d'un écran ne dit pas s'il est exportable.** Le vrai écran d'attente est `loading01` : **1** objet, une bande 784×136, entièrement décrit par son export. |
| Le § 4 citait « les `pub fn` de `nie-lua` (99) » ; la mesure du jour en rend **34** | une source de la matrice de couverture fausse d'un facteur 3 | **Un compte cité dans un plan porte sa date et sa commande.** Sinon il devient une légende. |
| `nie-menu` n'existe pas — la couche menu est `nie-lua::menu_host` | un lot planifié sur une crate imaginaire | **Vérifier qu'une crate existe avant de lui écrire un lot.** `cargo metadata --no-deps`, pas la mémoire. |

### Ce qui a été corrigé le 2026-09-06, et ce que ça enseigne

| Défaut | Correction | Preuve |
|---|---|---|
| Portail TS rouge sur 2 paquets | `mcp` redirigé vers `@niers/azalee-tools/server/index` ; `cron` déclare `@aphrody/bxc` et reçoit une passerelle de types | `bun run typecheck` = **0 sur les 5 workspaces** |
| Binaire `nie-site` périmé en ligne | rebâti et redémarré | `/healthz`, `/api/v1/{health,episodes,textures}`, `/feed.atom` = **200**, TTFB 0,66–6,4 ms |
| Pagination `/chara` commitée mais jamais déployée | déploiement bleu/vert sans coupure | **60 fiches uniques** servies, bascule en 887 ms puis 596 ms, `/` 200 tout du long |

Et une leçon de mesure, qui rejoint les autres : **`/chara` « pèse 2 355 397 o »… en brut.**
En `br` — l'unité de la gate — il pèse **49 413 o**, très loin des 250 Ko exigés. Un HTML long
et répétitif se compresse d'un facteur 48. Mesurer dans la mauvaise unité fait ouvrir un
chantier qui n'existe pas ; **la gate dit son unité, on la mesure dans cette unité-là.**

Trois causes empilées sur `@aphrody/bxc` méritent d'être retenues, parce que chacune, prise
seule, mène à une fausse conclusion :

1. `node_modules/@aphrody` est absent **à la racine** — mais le linker est `isolated` : le
   paquet vit dans le `node_modules` de chaque paquet qui le déclare. Conclure « non installé »
   là-dessus est une erreur de méthode.
2. `packages/cron` compile les sources d'un paquet du workspace (`@aphrody/ietv`) sans déclarer
   les dépendances de celui-ci : **ce qu'on compile, on le déclare.**
3. `@aphrody/bxc` publie ses **sources** `.ts` avec `"types": "./src/api/browser.ts"` : en
   traversée, `tsc` ne lit pas le sous-chemin `./privacy` et retombe sur la racine — d'où un
   message qui cite le mauvais module et envoie chercher au mauvais endroit.

## 4. La matrice de couverture — l'instrument de mesure du plan

Le plan se pilote par **une seule table**, versionnée, régénérée par une commande, jamais tenue
à la main : `var/couverture-site.json` + une page `/couverture` sur le site.

Chaque capacité du dépôt y a une ligne. Le plan a longtemps prévu **trois** états ; la
cartographie du VFS du 2026-09-06 (`docs/VFS.md`) en a imposé **cinq**, parce que trois
écrasaient deux distinctions qui décident du travail à faire :

| État | Sens |
|---|---|
| `servi` | une route ou un composant l'expose, et un test le **compte** (code HTTP mesuré, pas supposé) |
| `partiel` | une route existe et ne couvre **pas tout le corpus** — elle ne peut pas être comptée `servi` |
| `manquant` | **le décodeur existe dans ce dépôt**, aucune route ne l'appelle. C'est du **câblage** |
| `bloqué` | aucune route **et aucun décodeur** : il faut du **reverse** d'abord |
| `interne` | délibérément non exposé, **avec sa raison** (privilège, écriture disque, forge, mémoire du jeu, exécution de code) |

Les deux ajouts, et ce qu'ils ont évité :

- **`partiel`** — compter les 15 875 `.g4mg` comme `servi` aurait annoncé 15 875 fichiers
  atteignables là où **7 466 codes** le sont. Un état binaire transforme une couverture
  incomplète en couverture annoncée.
- **`bloqué`** — `manquant` confondait « écrire une route sur un parseur qui existe » et « faire
  du reverse sur un format inconnu ». Deux ordres de grandeur d'effort sous le même mot : le
  plan devenait inchiffrable.

**Gate maîtresse du plan :** `manquant = 0` **et** `partiel = 0`. `bloqué` est compté à part et
descend par le RE, pas par le câblage. Une capacité classée `interne` sans raison écrite compte
comme `manquant` ; une extension classée `inconnu` compte comme `bloqué` — on ne route pas ce
qu'on n'a pas identifié.

Sources de la matrice, toutes déjà présentes : **le VFS lui-même (255 308 entrées, la source la
plus large — `docs/VFS.md`)**, `niers --help`, l'`invoke_handler` de `src-tauri`, les modules de
`nie-data` et `nie-formats`, les `pub fn` de `nie-lua`, les pages d'Azalée, les sous-commandes
d'`iecode`.

### L'instrument existe — construit et exécuté le 2026-09-06

`crates/tools/nie-site/src/couverture/`, servi par `/couverture` (page) et
`/api/v1/couverture` (JSON), régénéré par **une commande** :

```
nie-site --regenerer-couverture var/couverture-site.json --racine-depot /home/ubuntu/niers
```

Il tient en trois pièces séparées, et cette séparation est ce qui l'empêche de mentir :

1. **la mesure** (`couverture/mesure.rs`) énumère et ne décide de rien ;
2. **le classement** (`couverture/regles.rs`) est un jeu de **règles** — un motif, un état, une
   raison écrite. Chaque capacité classée cite la règle qui l'a classée : on remonte d'une ligne
   de la matrice à la décision qui l'a produite ;
3. **la jointure** (`couverture/mod.rs`) applique les règles au mesuré.

Trois gardes le rendent **falsifiable**, ce qui manquait à toute matrice tenue à la main :

- une capacité qu'aucune règle ne reconnaît sort en `manquant` « non classée » — une commande
  ajoutée à `niers` demain apparaît d'elle-même, personne n'a à y penser ;
- une capacité dont la règle cite une route **qui n'est montée nulle part** est *rétrogradée* en
  `manquant`, et l'incohérence est publiée. La matrice ne se croit pas sur parole : elle
  confronte chaque `servi` à `app::chemins()` ;
- une règle qui ne classe plus rien est publiée. Un test refuse en plus toute raison de moins de
  vingt caractères, et l'invariant « `interne` sans raison » est porté par le **type** — il ne
  compile pas.

**Premier résultat, 2026-09-06 — 583 capacités, 255 848 unités de poids, 39 routes montées**,
puis l'état après le premier lot qu'il a déclenché (`routes::level5`, le soir même) :

| État | Capacités (départ → 2026-09-06 au soir) | Poids |
|---|---:|---:|
| `servi` | 114 → **254** | 225 033 → **246 418** |
| `partiel` | 0 → 0 | 0 → 0 |
| `manquant` | **205 → 27** | **21 450 → 27** |
| `bloqué` | 10 → 10 | 3 600 → 3 600 |
| `interne` | 254 → 292 | 5 765 → 5 801 |
| **total** | **583** | **255 848** |

**`manquant` est passé de 205 à 27, et son poids de 21 450 à 27, dans la journée qui a suivi la
construction de l'instrument.** Plus aucun *fichier du jeu* n'est manquant : il ne reste que des
capacités unitaires. C'est ce que sert une matrice — elle a désigné, chiffré, et le câblage a
suivi. Aucun de ces lots n'était difficile ; ils étaient **invisibles**.

Ce que la journée a produit, dans l'ordre où la matrice l'a désigné :

| Lot | Ce qu'il a fermé | Mesure |
|---|---|---|
| `routes::level5` | 21 250 fichiers (5 familles, parseurs déjà écrits) | 124/124 décodés |
| `/api/v1/donnees/{chemin}` | 110 modules `nie-data` → 22 | 1 056 fichiers typés, 121 familles |
| `/api/v1/recherche` | il n'existait **aucune** recherche dans le VFS | `ext=p3lip` → 21 047 |
| `/api/v1/donnees/famille/{cle}` | 23 `game_data_*` + les catalogues d'Azalée | `skill_config` → 1 004 skills |
| 16 familles dans `typed::decode_by_key` | `nie-data` 22 → 6, et l'ajout profite aussi à `nie-model-serve` et `nie-wasm` | stadium 82, win_treasure 113 |

**Les 27 restants, nommés** : 8 modules `nie-formats` (fontes, images, feuilles de sprites — ils
produisent des *images*, et les features `images`/`textures` restent éteintes dans ce service),
6 pages `/tools/*` d'Azalée, 6 modules `nie-data` dont deux qui ne peuvent pas passer par la
façade (`passives` exige deux tables de texte, `team` fait doublon avec `enjoy_mode_team`),
4 sous-commandes `niers` (`avatar`, `convert`, `icons`, `mode`), 2 commandes d'avatar d'Inacord,
et `/api/save/resolve-roster`.

| Source | Total | `manquant` (avant → après) |
|---|---:|---:|
| `niers` — sous-commandes | 40 | 6 → 6 |
| Inacord — commandes IPC | 158 | 28 → 27 |
| Azalée — pages | 81 | 38 → 38 |
| Azalée — routes d'API | 26 | 1 → 1 |
| `nie-data` — modules | 116 | **110 → 110** |
| `nie-formats` — modules | 46 | 13 → 8 |
| `nie-lua` — fonctions publiques | 34 | 4 → 4 |
| `iecode` — sous-commandes | 39 | 0 → 0 |
| VFS — par extension | 43 | 5 (21 250 fichiers) → **0** |

**La gate maîtresse est ROMPUE : `manquant = 205`.** C'est le premier chiffre honnête que ce
plan possède, et il corrige trois comptes que le plan citait de mémoire :

- **`niers` a 40 sous-commandes, pas 41** (`help` n'en est pas une, c'est clap qui la pose) ;
  Inacord en a **158**, pas 155 ; `nie-data` **116** modules et `nie-formats` **46**, pas 117 et
  47 ; Azalée a **26** routes d'API et non 24 (trois `route.ts` vivent hors d'`app/api`).
  Seuls `nie-lua` (34), les pages d'Azalée (81) et `iecode` (39) étaient exacts.
- **110 des 116 modules de `nie-data` sont `manquant`.** Chacun est un parseur écrit, testé par
  golden, qu'aucune route n'appelle. C'est la mesure qui a motivé ce plan, enfin chiffrée : le
  dépôt ne sait pas *dix fois* ce qu'il expose, il en sait *dix-neuf fois* sur cette source-là.
- **Le VFS n'était pas à `manquant = 0`.** `docs/VFS.md` l'annonçait le matin même ; l'instrument
  l'a contredit le soir avec **21 250 fichiers** — et le câblage a suivi dans la foulée
  (`routes::level5`, 124/124 mesurés). Il y est maintenant, sous une définition qui, elle, peut
  échouer. Cf. § 9 bis.

**Ce que la matrice ne dit pas, et qu'il faut lire :** `interne = 254` est le plus gros poste en
capacités. Chacune porte sa raison, et aucune n'est « pas le temps » — ce sont l'exécution de
Lua, la mémoire d'un process, le disque de l'utilisateur, l'écriture, les secrets et
l'éditorial d'Azalée. Mais un `interne` mal motivé est indiscernable d'un `manquant` déguisé :
c'est le fichier `regles.rs` qu'il faut relire, pas le total.

## 5. Les lots, par ordre de dépendance

Chaque lot a une gate qui **compte**. Aucun lot ne commence avant que la gate du précédent soit
verte — sauf mention explicite.

### Lot 0 — réparer les portails (bloquant absolu)

1. `bun run typecheck` = 0 sur les 5 workspaces.
2. `cargo check --workspace --tests` = 0 (déjà vert le 2026-09-06, à maintenir).
3. Rejouer `just preuves` (uemu) et **publier le compte réel** : 0/47 aujourd'hui. Soit
   l'oracle repart, soit il est déclaré hors service dans `docs/RE.md` — mais il ne reste pas
   dans l'ambiguïté.
4. `cargo build --release -p nie-site`, puis installation (**go utilisateur**).

**Gate :** les trois portails rendent leur compte ; `/api/v1/episodes` répond 200 en ligne.

### Lot 1 — l'API totale : `niers` (41) → `nie-site`

Chaque sous-commande de `niers` devient une route `/api/v1/*` ou est classée `interne`.

Répartition attendue, à trancher commande par commande dans la matrice :

| Famille | Commandes | Destination probable |
|---|---|---|
| Lecture du jeu | `vfs`, `find`, `grep`, `decode`, `textures`, `img`, `render`, `video`, `icons`, `avatar`, `save`, `strings` | **API publique** |
| Données et wiki | `wiki`, `mode`, `coverage`, `uniform-map`, `refresh-typed-json` | **API publique** |
| Menus et scripts | `lua`, `menu-predecode`, `seed-ui`, `vn` | **API publique** (lot 3) |
| Reverse et forge | `disasm`, `pdata`, `rtti`, `index`, `rebuild`, `recover`, `queue`, `propagate`, `seed` | **interne** — coûteux, privilégié, sans public |
| Machine locale | `mem`, `steam`, `mod`, `convert`, `format` | **interne** — écrit sur le disque ou lit un process |
| Façades | `cpp` (39 sous-commandes), `cs`, `backends`, `viola` | **API d'administration**, non affichée |

**Gate :** `41 = servi + interne`, `manquant = 0`, et pour chaque route un test qui **compte**
son contenu (pas son statut). TTFB local < 50 ms sur les routes de lecture.

### Lot 2 — l'explorateur : 155 commandes Tauri → `nie-web` + Inacord

Le contrat existe (`packages/asset-source`), la règle aussi : **jamais de condition sur l'hôte
dans un composant**, `useAssetSource()` et `capacites()` portent l'asymétrie.

- Classer les 155 : portable (web + desktop) / desktop seul (Lua, forge, modding, Blender,
  mémoire du jeu, disque) — l'estimation de départ était ~66 / 81, **à re-mesurer**.
- Chaque commande portable a son pendant dans `web-source.ts`, adossé à une route du lot 1.
- Vider le sas `apps/nie-web/src/legacy/` : **87 fichiers**, réécrits contre `/f`, `/b`,
  `/api/v1`, ou supprimés.

**Gate :** `fd . apps/nie-web/src/legacy | wc -l` → **0** ; `capacites()` publie la matrice
réelle ; `rg -l '@tauri-apps' packages/inacord-ui` → **0**.

### Lot 3 — `nie-lua` sert les menus et les scripts

Aujourd'hui : le codec bytecode est byte-exact, `menu_host` est porté, 66 commandes runtime
sont reconnues sur `kizuna_town_mainmenu`. Ce qui manque, c'est la **route**.

Deux corrections d'assiette, mesurées le 2026-09-06 :

- **`nie-menu` n'existe pas.** La couche menu est `crates/engine/nie-lua/src/menu_host.rs`,
  aux côtés de `host.rs`, `session.rs` et `runtime.rs`. Un lot écrit contre une crate imaginaire
  n'aurait rien pu livrer.
- **`nie-game` n'est pas une bibliothèque** : `crates/engine/nie-game/src/` ne contient que
  `main.rs`, `gpu_select.rs` et deux shaders WGSL. « L'exploiter nativement » veut donc dire soit
  l'appeler en sous-processus — ce que fait déjà l'export de layout — soit en extraire une lib.
  Le plan doit dire laquelle des deux, il ne peut pas l'éluder.

**Ce que le site n'exposera pas, et pourquoi :** `execute_with_include`, `run_menu`,
`drive_menu`, `install_menu_host`, `eval`/`set_global` exécutent du Lua. Une route publique qui
les appelle est un interpréteur ouvert.

Et deux qui **ressemblent** à de l'analyse sans en être — c'est le piège de ce lot :
`discover_host_calls` et `enumerate_header_tabs` posent une métatable sur `_G` puis **appellent
la fonction principale du script**, sur un `Lua::unsafe_new`. Un nom qui contient « discover »
ou « enumerate » ne dit pas si la fonction lit ou exécute : **on lit le corps, pas le nom.**

Le refus est **structurel** : `nie-lua` est déclaré `default-features = false`, aucun
interpréteur n'est lié dans le binaire, et un `const { assert!(!VM_LIEE) }` l'impose à la
compilation. Une politique qui tient par la discipline du prochain appelant n'est pas une
politique.

- `/api/v1/menu/<ecran>` : la disposition **exportée par le runtime**, pas un gabarit.
- `/api/v1/script/<chemin>` : le Lua décodé, ses `Setup*`, ses commandes reconnues.
- Le front consomme ces routes : un écran nouveau apparaît **sans une ligne de TSX**.

**Gate :** N écrans servis par le runtime réel, avec pour chacun le compte d'objets, d'objets
positionnés et d'objets muets. L'arbre en compte **440** ; le plan exige un compte, pas une
promesse : publier `servis / 440` à chaque étape.

### Lot 4 — l'UI pixel-perfect, mesurée écran par écran

L'état réel : `mainmenu01` reconstruit, 14 blocs mesurés, écart ≤ 10 px sur 6 d'entre eux,
392 px sur la rangée (assumé, justifié par la mesure), **SSIM jamais calculée sur cette
reconstruction**, et l'ancienne SSIM du rendu moteur vaut 0,004.

- Un test versionné qui, pour chaque écran couvert, compare **boîte par boîte** le rendu au
  jeu (le mécanisme existe : `scripts/validation/mesurer-mainmenu.py`) ;
- puis **SSIM** contre la capture, avec un plancher de non-régression qui ne baisse jamais ;
- la géométrie vient de `geometrie-mainmenu.ts` (mesurée) ; aucune valeur nouvelle sans sa
  commande de mesure.

**Gate :** par écran — nombre de blocs, écart max en px, SSIM. **Aucune affirmation
« pixel-perfect » sans ces trois nombres.**

### Lot 5 — `nie-aphrody` sert les icônes, assets, pets et personnages — **fait le 2026-09-06**

La crate est importée par `nie-site` et republiée en entier, **sept routes, une par capacité
réelle** — aucune copie d'asset dans `apps/nie-web/public/`, parce que le manifeste porte un
condensé par frame et qu'une copie ne le porterait plus.

| Route | Ce qu'elle tire de la crate | Mesuré |
|---|---|---|
| `/pet/aphrody.json` | `AnimationsManifest`, réduit aux rectangles et aux durées | 3 192 o (contre 101 Ko pour le manifeste complet) |
| `/pet/atlas.webp` | `BUNDLED_ATLAS_WEBP` — VP8L sans perte, même RGBA que le PNG | 1 510 454 o, 480 Ko de moins que le PNG |
| `/pet/frame/{anim}/{n}.png` | `Pet::extract` + `assets::encoder_png` | les **74** frames, découpées à la demande |
| `/pet/aphrody.svg` | `pixel::vectoriser` | 40 376 o — un **décalque**, pas un dessin vectoriel |
| `/api/v1/aphrody` | `BUNDLED_DOSSIER_JSON` | 221 324 o : identité trilingue, 3 séries, stats, techniques, auras, variantes |
| `/api/v1/aphrody/diagnostic` | `Pet::diagnose` + `codex::conformite` | **74/74** frames conformes à leur condensé, Codex Pet **v2**, 0 écart |
| `/api/v1/aphrody/palette` | `pixel::mesurer` + `pixel::tokens_css` | 5 couleurs Oklab et leurs jetons |

Le personnage **remplace le titre** au centre de l'accueil, et il n'anime rien gratuitement :
`failed` quand le site ne joint pas ses ressources, `waiting` pendant la préparation du
catalogue, `idle` au repos, une des seize poses de `look-directions` quand la souris bouge,
`waving` au survol, `jumping` au clic. L'affichage se fait en deux temps — la pose de repos
seule (30 Ko) puis l'atlas après `img.decode()` — parce qu'un `background-image` en cours de
chargement n'affiche rien et ne le dit pas.

Reste du lot, avec sa raison : les huit favicons et le manifeste que `assets::assets_de_marque`
sait produire ne sont **pas** encore branchés ; l'icône du site est aujourd'hui un portrait du
personnage recadré à la main depuis le zukan officiel. C'est un asset LEVEL-5 et non une sortie
de la crate — à faire passer par `assets_de_marque` ou à assumer comme tel.

**Gate :** `rg -c '<svg' packages/inacord-ui/src` → les glyphes restants sont **justifiés un à
un** (un tracé géométrique du dépôt est légitime ; une icône du jeu redessinée à la main ne
l'est pas). Zéro asset de marque en dur.

### Lot 6 — tout Azalée dans le site

81 pages, 24 routes API. Chaque page : portée dans la DA du site, ou classée `interne`, ou
déclarée « reste sur Azalée » **avec sa raison** (Azalée demeure le wiki de référence,
`azalee.rosegriffon.fr`, produit Rose Griffon — la séparation de marque tient).

**Gate :** 81 = portées + restées + classées, `manquant = 0`.

### Lot 7 — les gisements et le toolkit C++

- `iecode` : 39 sous-commandes derrière `niers cpp`, atteignables par l'API d'administration.
- Les quatre gisements (`jeu`, `extrait`, `re`, `anime`) passent par `@niers/catalog` — jamais
  une base rouverte à la main.
- `nie-db` / `niers push` (amendement A2) : la couche SQL native remplace les 18 importeurs
  Bun. Gate connue : `niers push --dry-run` annonce table par table, puis un push réel rend
  **le même total qu'aujourd'hui, écart 0**.

### Lot 8 — les filtres : chaque page d'Aphrody vaut son équivalent

Inventaire du 2026-09-06 : `docs/FILTRES.md`. Il compare page à page Aphrody, Azalée (21 pages
de liste publiques, 24 clés de `searchParams` validées par zod) et Inacord (17 vues filtrantes,
dont un vrai langage de requête sur le Cinéma — `s3e12 lang:vf vu:non`).

**Le compte : 48 filtres recensés, 3 pleinement présents, 3 partiels, `manquant = 42`.**

L'écart n'est pas ergonomique, il est structurel : l'explorateur d'Aphrody n'a **aucun** filtre,
le catalogue a `q` + `page` avec un `PAR_PAGE = 60` écrit en dur, et les 4 vues ne couvrent que
**143 246 des 255 308** entrées. Les **112 062** restantes — `.bin` 72 308, `.p3lip` 21 047,
`.objbin` 12 190 — ne sont atteignables que par le parcours, sans le moindre filtre. C'est ce
qui met le filtre par extension en tête, et non un souci de confort.

Trois défauts à réparer avant d'ajouter quoi que ce soit :

1. `/b` **accepte** `q` et l'ignore. Un client qui filtre croit filtrer.
2. `cpk_filename` est **jeté** à la construction de l'index alors que `nie-formats` le porte :
   le filtre « quel CPK » est déclaré impossible pour une donnée déjà lue.
3. L'état des filtres ne vit **pas dans l'URL** — préalable à tout le reste : sans lui, aucun
   filtre n'est partageable, rechargeable, ni indexable.

Point de faisabilité, **mesuré** : aucun des 42 manques n'exige une seconde passe sur les
255 308 entrées — la boucle qui pré-calcule les 4 vues produit au même passage les listes par
extension, par CPK et l'ordre par taille. En revanche la conclusion qu'on en tirait — « le
surcoût est en mémoire, pas en temps » — était **trop large** : le montage passe de
**1,10–1,25 s à 1,33–1,37 s, soit +0,12 s (+10 %)**, à cause du tri de la permutation par
taille. C'est un coût payé une fois au démarrage, sur un index monté en fond. La leçon vaut
au-delà de ce lot : *une affirmation de complexité n'est pas une mesure* — on exige les deux
chiffres, avant et après, et ici c'est le second qui a corrigé le premier.

**Gate :** `manquant = 0` sur la matrice des 48, chaque filtre mesuré par une requête qui rend
un **total**, pas un statut. Un filtre servi mais jamais appliqué compte comme manquant — c'est
exactement le défaut 1.

### Lot 9 — 100 % du VFS servi, comme `nie.exe` le lit

C'est le lot terminal du plan : **255 308 fichiers, aucun non classé.** La carte est
`docs/VFS.md`, établie le 2026-09-06 par six agents sur un inventaire figé
(`var/vfs/inventaire.txt`), et recalculable en une commande.

#### L'état, mesuré — départ et arrivée du 2026-09-06

| État | Au départ | Aujourd'hui | Nature du travail |
|---|---:|---:|---|
| `servi` | 162 219 (63,54 %) | **246 003 (96,35 %)** | — |
| `manquant` | 67 878 (26,59 %) | **0** | c'était du **câblage** : le décodeur était déjà là |
| `partiel` | 15 875 (6,22 %) | **0** | idem — élargir une route qui existait |
| `interne` | 5 512 (2,16 %) | 5 512 | rien à faire, la raison est écrite |
| `bloqué` | 3 776 (1,48 %) | 3 784 | **reverse** préalable — le format est nommé quand son en-tête le dit |
| `inconnu` | 48 (0,02 %) | **9** | les `.g4tg` seuls, assumés (§ 9.2) |

**82 % du reste à faire était du câblage, pas de la recherche** — et le fait s'est vérifié :
les 83 753 fichiers ont basculé sans une dépendance nouvelle, parce que les neuf parseurs
étaient derrière `std`, feature par défaut. Le dépôt savait déjà décoder les deux tiers de ce
qu'il n'exposait pas, au sens le plus littéral : le code était **compilé dans le binaire**.

#### 9.1 — Le câblage (83 753 fichiers, 32,8 %)

Chaque ligne a son décodeur déjà écrit ici. Aucune n'exige de recherche.

| Corpus | Fichiers | Décodeur existant | Ce qui manque |
|---|---:|---|---|
| `.g4pk` | 45 591 | `nie-formats/src/g4pk.rs:137` | une route — aujourd'hui **400 mesuré** |
| `.g4mg` non couverts | 15 875 → 8 409 restants | pipeline 3D existant | élargir le catalogue au-delà des 6 familles |
| `.objbin` | 12 190 | `objbin.rs:66` | une route |
| `.g4pkm` | 6 992 | parseur présent | une route |
| `.g4cm` (caméras) | 1 217 | `g4cm.rs:336` | une route |
| `.col` (collision) | 1 150 | `col.rs` | une route |
| `.g4sk` (squelettes) | 339 | parseur présent | une route |
| `.mevbin` | 328 | `mevbin.rs:136` | une route |
| `.g4mt` (animation) | 71 | parseur présent | une route |
| famille `uniform` | 1 022 modèles | pipeline 3D existant | **une ligne de famille**, même filtre que `waza` |
| `common/font/font/*.g4tx` | 14 | `g4tx_decode.rs:197` | un mapping de route — **404 aujourd'hui**, le miroir `dx11` répond 200 |

**Gate 9.1 — tenue le 2026-09-06.** `manquant = 0`, `partiel = 0`, et chaque corpus est
prouvé par une requête : `scripts/validation/mesurer-geometrie.sh <base> 25` rend
**225/225 décodages conformes** sur les neuf familles, en exigeant le jeton de famille dans le
corps et pas seulement un 200. Les deux voies de description des `.g4mg` sont vérifiées en
direct — `description: "g4md"` sur `_face/01_IE1/c01000010`, `description: "g4pkm"` sur
`_animal/an000150`.

Deux corpus du tableau ci-dessus ont changé de nature en cours de route, et c'est la mesure qui
l'a imposé : les `.g4mg` ne se lisent pas seuls (leur description vit ailleurs), et la famille
`uniform` n'est **pas** « une ligne de famille, même filtre que `waza` » — l'amont borne
l'assemblage à cinq sous-domaines (`waza`, `item`, `animal`, `armd`, `keshin`), et `_uniform`
comme `_face` y répondent « sous-domaine chr non servable ». Ils sont servis ici par le
**décodage** du fichier, pas par l'assemblage d'une entité : les deux ne promettent pas la même
chose.

#### 9.2 — L'identification — **faite le 2026-09-06 : 37 / 46**

Le volume était dérisoire et la gate `100 %` le rendait bloquant, à dessein : un plan qui
s'autorise 48 exceptions s'en autorisera 4 800. Résultat, par
`scripts/validation/mesurer-extensions-rares.sh <base> 15` :

| Trouvé | Fichiers |
|---|---:|
| archives **G4PK** sous un suffixe de révision, reconnues **au magic** | 14 |
| **texte** (`.log`, `.cfg`), servi en `text/plain` | 12 |
| conteneurs **Level-5** nommés sans être interprétés (`G4VS`, `G4LA`) | 8 |
| `cfg.bin` **T2B** sous un suffixe de révision | 2 |
| table **`@UTF` CriWare** (`sound.acf`) | 1 |
| **non identifiés** — les 9 `.g4tg`, assumés dans `docs/VFS.md` | 9 |

Deux enseignements que le plan retient au-delà de ce lot :

- **48 n'était pas le bon compte.** Deux des 48 étaient un artefact de mesure : le VFS porte de
  vrais noms de fichier **avec un espace** (`…/u021801/u021802 .g4md`), et un découpage par
  espaces en fait des « fichiers sans extension ». Le corpus réel était de 46.
- **La mesure a trouvé deux faux positifs dans le code qui la servait** : `objbin::is_objb`
  teste le pied de page `t2b` commun à tous les `cfg.bin` (donc n'est pas un magic), et un
  fichier texte commençant par `BLOCK_LIST_BEG` passait pour un conteneur de magic « BLOC ».
  Une gate qui ne trouve jamais rien n'est pas une gate.

#### 9.3 — Le reverse (3 776 fichiers, 1,48 %)

Shaders (`fxbin`, `vfxo`, `pfxo`, `cfxo`, `gfxo` — 2 869, plus les 2 870 de
`dx11/shader/1.00.41/`), particules (`ptlb`), tissu (`clobin`), navigation (`g4nv`), `linb`.
**Aucun parseur n'existe.** Ces corpus ne descendent pas par du câblage et **rien ne doit
promettre une route avant que le format soit lu**. Ils restent comptés `bloqué` — visibles,
chiffrés, jamais silencieux.

#### 9.4 — Les écrans, l'autre couverture

Le VFS n'est qu'une des deux couvertures. L'autre est celle des **écrans** : `/menu-tree.json`
en compte **475** (mesuré en direct ; le code source en annonçait 440, jamais rejoué). Un seul
a été vérifié en profondeur à ce jour. Publier `écrans servis / 475` à chaque étape, avec pour
chacun le compte d'objets, d'objets positionnés et d'objets muets — un export dit aussi ce
qu'il ne contient pas.

**Piège à porter dans le code des routes :** il existe **trois** nomenclatures d'écran, pas
deux. Le nom du calque (`mainmenu01`, 34 objets, 0 script), le nom du script
(`kizuna_town_mainmenu`, 0 objet, 1 script) et le **stem du `*_setting.cfg.bin.json`** attendu
par `/menu-tree/{stem}.json` — où `mainmenu01` rend **404**. Les confondre produit un 404 qu'on
attribuera au fichier.

#### 9.5 — Ce que « servir comme `nie.exe` » exige en plus du volume

Servir 100 % des octets ne suffit pas ; le jeu ne lit pas des fichiers, il lit des **entités**.
Trois conséquences déjà mesurées :

1. **Cataloguer par l'entité, pas par le fichier.** Une banque `.awb` n'est pas une piste :
   7,688 Gio d'AWB contre 0,103 Gio d'ACB (**74×**), pour **284 115 cues** réelles. Le catalogue
   se fait par cue, et un export se nomme par sa sous-entité — sinon tous les téléchargements
   se recouvrent.
2. **Un fichier présent n'est pas une entité affichable.** Un modèle n'existe que si
   `<code>/<code>.g4mg` est là : **7 466 codes assemblables sur 7 679**. Un catalogue qui liste
   les pièces produit des 404 ; il doit lister ce qui s'assemble, et dire pourquoi le reste
   ne s'assemble pas.
3. **Le slug est le code du jeu**, jamais un nom traduit — c'est la règle d'identité déjà gelée
   pour Aphrody et Inacord.

#### 9.6 — Couverture ≠ indexation

Servir tout le VFS et **indexer** tout le VFS sont deux décisions distinctes. `/f/` ne rend que
des octets : il n'a pas à entrer dans un plan de site, quel que soit le niveau de couverture
atteint. La question de l'indexation reste ouverte au § 7, et elle appartient à l'utilisateur.

#### 9 bis — Ce que la matrice a contredit le soir même

`docs/VFS.md`, établi le matin du 2026-09-06, annonçait `manquant = 0` et `partiel = 0` sur le
VFS. La matrice, construite le soir, rend **21 250 fichiers `manquant`**. Les deux mesures ne
sont pas en désaccord sur les faits : elles ne comptaient pas la même chose.

`docs/VFS.md` comptait `servi` **tout ce qu'une route rend**, `/f/{*chemin}` compris — or `/f`
sert les octets bruts de n'importe quel fichier du jeu. Sous cette définition, la gate est vraie
**par construction** : elle ne peut pas échouer, donc elle ne mesure rien. C'est le défaut que ce
dépôt a déjà payé sur un contrôle de gamut vert quoi qu'on lui donne, et sur un `0 passed`
annoncé comme un succès.

La matrice durcit la définition : **`servi` veut dire qu'une route rend le contenu *interprété***
— décodé, converti en image, en audio, en GLB, en script. `/f` reste le filet universel ; il ne
suffit plus à classer une extension `servi`.

Ce que ce durcissement fait apparaître, fichier par fichier :

| Extension | Fichiers | Le décodeur existe | Ce que le site en rend |
|---|---:|---|---|
| `.p3lip` | **21 047** | `nie_formats::lip` (193 l., pistes de lip-sync) | les octets, rien de plus |
| `.g4nv` | 160 | `nie_formats::navm` (534 l., magic **NAVM**, pas « G4NV ») | idem |
| `.g4ma` | 35 | `nie_formats::g4ma`, validé byte sur les 35 fichiers réels | idem |
| `.g4vs` | 4 | `nie_formats::g4vs`, validé byte sur les 4 fichiers réels | idem |
| `.g4la` | 4 | `nie_formats::g4la`, validé byte sur les 4 fichiers réels | idem |

Les quatre dernières lignes corrigent une **erreur de fait** de `docs/VFS.md` : il les classait
`bloqué` — « aucun parseur » — alors que les quatre modules sont écrits, documentés et validés
sur les fichiers réels. La distinction `manquant` / `bloqué` est celle entre « écrire une route »
et « faire du reverse » : les y laisser aurait promis du reverse là où il n'y a qu'un branchement
à faire. `bloqué` retombe donc de 3 784 à **3 600**.

**Les cinq familles sont câblées le soir même** — `crates/tools/nie-site/src/routes/level5.rs`,
décodage en process, aucune dépendance ajoutée (les parseurs étaient déjà liés derrière la
feature `std`, comme les neuf géométriques du lot 9.1). Mesure :
`scripts/validation/mesurer-level5.sh` échantillonne **à pas régulier** — jamais en tête, les
premiers fichiers d'un dossier se ressemblent — et exige le **jeton de la famille dans le
corps**, jamais un code 200 : **124 / 124**. Le VFS passe à `manquant = 0`, cette fois sous une
définition qui peut échouer.

Ce que le câblage ne prétend pas : `.g4ma`, `.g4vs` et `.g4la` n'ont que leur **en-tête**
interprété, leur corps n'est pas reversé, et la ligne « produit » de chaque famille le dit dans
la réponse. Servir ce qu'on sait lire n'oblige pas à laisser croire qu'on lit tout.

**La leçon dépasse ce lot.** Une gate se conçoit en se demandant d'abord *comment elle pourrait
échouer*. Celle du VFS ne le pouvait pas, et elle a annoncé 100 % de couverture sur un corpus
dont 21 250 fichiers ne sont servis qu'en octets. Ce n'est pas une erreur de calcul — la somme
retombait à l'unité près sur 255 308 — c'est une erreur de **définition**, et aucune vérification
arithmétique ne la trouve.

## 5 bis. Ce que la session du 2026-09-06 a livré, et ce qu'elle laisse ouvert

### Livré et vérifié

| Chantier | Preuve |
|---|---|
| La façade purgée : doublons, texte étranger, liens d'infrastructure | le DOM rendu ne porte plus qu'une fois chaque information ; aucune route technique dans le menu |
| Une seule coquille (`pages/Ecran.tsx`) au lieu de deux chartes | l'accueil et les écrans secondaires partagent fond, biseaux, typographie et la même rangée de tuiles |
| `nie-aphrody` servie en sept routes ; le personnage remplace le titre | lot 5 ci-dessus |
| Le SEO recentré sur ce que le site est | `<title>` de l'accueil = « Aphrody » ; `/explorateur` titré et traduit ; **18** URL au plan de site (15 avant), `robots.txt` et `llms.txt` alignés |
| La compatibilité `?vue=` retirée | le type de `entreeDemandee` l'interdit désormais **à la compilation**, ce qu'aucun test ne garantissait |
| Portails | `cargo test -p nie-site` 96/96, `bun test` 87/87, clippy et lint sans avertissement sur les crates et paquets touchés |
| Le design system de couleur : `nie-aphrody` est la **source** des 29 couleurs du site | `game-tokens.css` est **engendré** (`cargo run -p nie-aphrody --bin design`), 48 propriétés dont 29 couleurs, **zéro hexadécimal écrit à la main** ; un golden le prouve par falsification |
| La couche 3D branchée nativement | **12 routes** mesurées : `/api/v1/3d` (capacités), `/api/v1/3d/modeles` (catalogue des **6 191** modèles assemblables — perso 5 490, techniques 273, objets 237, keshin 100, armures 89, animaux 2), la fiche, `…/analyse` (géométrie **réelle** du GLB : 7 580 triangles et 36 textures pour Mark Evans), `/model/{f}/{c}.glb` (3 180 456 o, 4 ms) et `/model/{f}/{c}.png` (rendu `nie-render3d` côté serveur, **171 ms à froid, 0,9 ms en cache**, ETag + 304). **97 %** de rendus réussis sur 102 modèles échantillonnés |
| Le catalogue 3D liste des **modèles**, plus des pièces | il proposait `.g4mg` seuls — 143 000 fichiers dont **aucun** n'était affichable. Le critère d'assemblabilité est mesurable sur l'index (`<code>/<code>.g4mg`) ; et `inagle_characters` ne contient pas que des personnages : **66** de ses 5 721 codes commencent par `an`/`n`/`e`/`s`/`i` et rendaient 404 |
| L'écran d'attente est celui du jeu | `loading01` — **1** objet, bande 784×136, texture servie par `/assets/tex/…` (200, `image/png`, 11 008 o), **jamais copiée** dans `public/`. `title00` a été mesuré puis écarté : 67 objets, 21 à la position par défaut, atlas entiers |
| La sonde d'état se reboucle | `App.tsx` ne demandait `/api/v1/health` qu'une fois : l'écran d'attente n'aurait jamais basculé. Elle se rejoue toutes les 2 s et s'arrête sur `pret` ou `absent` |
| Le personnage est **visible** | il était blanc sur un ciel crème, DOM juste et écran vide. Halo `aria-hidden` derrière lui, et `FOND_MENU` (hex en dur) remplacé par `--jeu-ciel-clair` : une seule source de couleur, sans exception |
| L'inventaire des filtres | `docs/FILTRES.md` : **48** filtres, **42** manquants, avec l'ordre de dépendance (lot 8) |
| Le viewport 3D est en **WebGPU** | `navigator.gpu`, un module **WGSL**, `createRenderPipeline` avec `depthStencil`. La profondeur est adaptée au NDC z ∈ [0,1] de WebGPU (`a = LOIN/(LOIN−PROCHE)`, `b = −LOIN·PROCHE/(LOIN−PROCHE)`) — le piège de cette traduction : avec la forme OpenGL le modèle est écrêté sans qu'aucune valeur ne paraisse fausse. Prouvé par une passe **hors écran** de 64×64 relue par `copyTextureToBuffer` : demi-côté du quad unitaire = 0,548 en NDC, soit exactement `1,7 / 3,1` — focale et distance cadrent comme le rastériseur ; le quad proche l'emporte sur le lointain dessiné avant ET après lui, donc `less` et [0,1] sont justes |
| `nie-lua` et `nie-formats` servis nativement (lot 3) | **13 routes** mesurées. `/api/v1/lua/scripts/{chemin}` rend une analyse **statique** réelle — `kizuna_town_mainmenu` : 49 prototypes, 1 933 instructions, `funcLuaCommand` ×66 ; `?forme=chunk` rend le décodage intégral de `nie_lua::bytecode::parse` (52 173 o) ; `/api/v1/lua/desassemblage/…` 101 805 o en `text/plain` + ETag fort. `/api/v1/formats` compte sur les 255 308 entrées (71 101 `.cfg.bin`, 1 197 `.lua.bin`, 54 203 `.g4tx`) ; `?forme=structure` expose la table de types, la table de champs et le CRC32 qui résout les noms — ce que `to_iecode_json` ne donne pas, et sans quoi un client ignore pourquoi un champ sort en `Unknown_0x…`. Traversée de chemin → **400**, extension étrangère → **400**, absent → **404** |
| L'exécution de Lua est refusée **structurellement**, pas déclarativement | `nie-lua` est déclaré `default-features = false` : `vm` (mlua) et `analysis` (tree-sitter) ne sont pas liés, et un `const { assert!(!VM_LIEE) }` le vérifie **à la compilation**. `/api/v1/lua` publie `vm_liee: false`. Les globaux sont lus dans les `GETTABUP`/`SETTABUP` sur l'upvalue `_ENV` — la définition d'un accès global en Lua 5.2 |

### Ouvert, et pourquoi

1. **Trois modèles sur 102 ne se rendent pas** (`keshin/k000010`, `keshin/k000100`,
   `item/d010020`), et la cause est identifiée à l'octet : le GLB assemblé par
   `nie-model-serve` porte des **indices de sommet globaux** (jusqu'à 11 493) pour des
   accesseurs `POSITION` **locaux** par primitive (818 / 858 / 2 394). Les personnages et les
   armures sont conformes (`maxidx == count − 1`). Le correctif appartient à `nie-model-serve` ;
   côté site l'erreur est classée **502** — l'amont a produit l'artefact — et le viewport écarte
   les triangles hors bornes plutôt que d'abandonner la scène.
2. ~~**`app::ROUTES` est périmé**~~ — **réglé le 2026-09-06.** Il figeait 19 routes pour un
   routeur qui en montait **37** : les 7 d'Aphrody, les 5 de la 3D et les 6 de Lua/formats n'y
   étaient jamais entrées, chaque lot ayant respecté son périmètre et la liste n'appartenant à
   aucun. La macro `declarer_routes!` supprime la classe de défaut : une route ajoutée est
   montée **et** listée, une route retirée disparaît des deux. La garde ne tient plus à une
   égalité de longueurs — deux instances peuvent viser la même route — mais à une **couverture
   de motifs** : toute route déclarée doit être atteinte par au moins une instance, et toute
   instance doit correspondre à une route.
3. **Le rendu serveur reste CPU.** `nie-render3d` a bien une feature `gpu` (wgpu) mais elle est
   éteinte, et ce VPS n'a pas de GPU. C'est le navigateur qui gagne le **WebGPU**, pas le
   serveur — et cette asymétrie doit rester écrite, sans quoi on la redécouvrira.
4. **Le viewport 3D n'a pas été vu rendre le vrai modèle, en pixels.** Dans Chrome headless
   avec SwiftShader, un canevas WebGPU n'est **jamais composité** : un témoin de vingt lignes
   sans une ligne du fichier relit lui aussi `0,0,0,0`. Ce qui est prouvé (géométrie,
   projection, profondeur, ombrage) l'est par lecture hors écran ; le trajet canevas →
   compositeur, et la concordance fine viewport ↔ vignette (SSIM), restent à mesurer sur une
   machine à GPU réel. Écart assumé et documenté : WebGPU n'a pas de `generateMipmap`, la
   texture n'a donc qu'un niveau.
5. **Un débordement horizontal** de la coquille (`packages/inacord-ui`), visible identiquement
   sur `/textures` et sur `/modeles` : il préexiste aux deux lots.
6. **Les huit favicons de `assets_de_marque`** ne sont pas branchées (cf. lot 5).
7. ~~**Le sas `apps/nie-web/src/legacy/`**~~ — **vidé le 2026-09-06 : 90 fichiers, 23 647
   lignes.** Il en portait **87** au dernier compte du plan ; la mesure du jour en trouve 90,
   et **40 d'entre eux** citaient Rose Griffon — ce que la décision du 2026-09-05 interdit
   côté Aphrody. Après : `fd . apps/nie-web/src/legacy` → **0**, et
   `rg -il '@rosegriffon/|rose ?griffon' apps/nie-web/src packages/inacord-ui apps/inacord` →
   **une seule** occurrence, l'URL de repli de l'updater, qui est l'exception écrite.
   `bun run typecheck` : 12 paquets à 0 ; `bun run --filter '*nie-web*' build` : 54 modules,
   244,14 ko de JS (78,56 ko gzip, **306,2 ko en brotli** pour l'ensemble).

   **Ce qui a rendu la suppression légitime, et qu'il fallait vérifier avant** : ce code n'était
   pas *déplaçable*, il était **inexécutable**. Ce sont des `page.tsx` de Next.js, des
   *server actions* et des `route.ts` — un hôte Vite ne peut rien en faire, quel que soit le
   travail de portage. Et le sas était l'**unique copie** : `apps/azalee/app/{avatar,cpk,…}`
   n'existe plus depuis J2 (`c4a1da8`), et `azalee.rosegriffon.fr/avatar` rend **404**. La
   régression que le README du sas voulait éviter avait donc déjà eu lieu ; garder les sources
   ne la réparait pas, il les rendait seulement invisibles. Git les conserve à `c4a1da8`.

   **Ce que le dépôt perd, et ce qui le sert déjà** — dit ici pour qu'aucune capacité ne
   disparaisse en silence :

   | Supprimé | Lignes | Ce qui sert la donnée aujourd'hui |
   |---|---:|---|
   | `app/avatar` | 11 683 | `/api/v1/donnees/famille/chara_edit` (16 listes) — **l'écran, lui, reste à écrire** |
   | `app/cpk` + `lib/cpk` | 3 086 | `/b`, `/f`, `/api/v1/formats/decode`, `/api/v1/donnees` — l'explorateur riche est le métier d'**Inacord**, pas d'Aphrody |
   | `components/wiki` + `lib/cutin` | 3 456 | `/api/v1/formats/decode` sur `.g4cm` (parseur **Rust**) ; les composants de fiche sont le métier d'**Azalée** |
   | `app/videos`, `sons`, `modeles`, `textures` | 3 108 | les **4 vues en ligne** d'Aphrody, lecture audio et vidéo comprises |
   | `app/vroid` | 907 | rien, et c'est voulu : classé `interne` (OAuth et secrets tiers) |
   | `app/mode` | 755 | `/api/v1/modes/{slug}`, câblé le jour même |
   | `app/demo` | 329 | `/api/v1/3d/*` et `/model/{famille}/{fichier}` |
   | `app/api` | 233 | `/f`, `/api/v1/episodes`, `/assets/tex` — des `route.ts` Next.js n'ont aucun sens ici |
   | `app/save` | 45 | `POST /api/v1/save/roster` |
   | `lib/wiki`, `lib/game-text` | 24 | `/api/v1/text` |

   **Le seul trou réel est un écran d'avatar.** Sa donnée est servie ; son interface n'existe
   nulle part. C'est écrit ici plutôt que laissé à découvrir.
8. **La page « Sons »** liste les `.awb` comme des sons individuels : `bgm_chronicle.awb` y
   apparaît avec un lecteur audio et **1 291,9 Mo**. Ce sont des banques, pas des pistes ; il
   faut cataloguer par ACB, comme le fait déjà Azalée.

## 6. Les invariants — ce qui vaut pour tous les lots

1. **Compter, toujours.** Un statut HTTP n'est pas un contenu ; `exit 0` n'est pas un succès ;
   « 0 passed » n'est pas vert.
2. **Le binaire, pas la source.** Un lot n'est fini que lancé : la page rendue, la route
   interrogée, l'exécutable relancé. La moitié des défauts de ce dépôt ne produit aucun message.
3. **Rien en dur qui dépende de l'état.** Les listes, les comptes, les entrées de menu viennent
   du serveur ou du VFS. Ce qui dépend du joueur ou du contenu ne s'écrit pas dans le code.
4. **Une seule source de géométrie, une seule source de données par sujet.** Sinon les deux
   divergent au premier ajustement.
5. **Ce qui n'est pas exposé est classé, pas oublié.** `interne` exige une raison écrite.
6. **La DA vient d'une mesure.** Couleurs sur la texture du VFS, positions sur le layout
   runtime quand il les porte, sur une capture sinon — et on dit laquelle.
7. **Les six gestes de production** (DNS, `nginx reload`, `systemctl`, rotation de secret,
   `vercel --prod`, suppression) restent soumis au go de l'utilisateur. Ils se **préparent** :
   commande, vérification, retour arrière.

## 7. Ce qui reste ouvert, et qui décide

- **Couvrir tout le VFS en DOM et en slugs contredit une décision écrite de ce plan.**
  L'utilisateur demande que « son DOM et tous les slugs couvrent tout le VFS ». Or `/f/` (les
  octets bruts) et `/b/` (le parcours) sont **délibérément** exclus de `robots.txt` et du plan
  de site, pour la raison consignée ici : 255 308 fichiers noient n'importe quel robot. Les deux
  positions sont défendables et je ne les arbitre pas seul. La forme praticable, si l'on veut la
  couverture : un **index de plans de site** découpé par tranches de 50 000 URL, des **pages**
  rendues côté serveur par entrée, et `/f/` — qui ne rend que des octets, jamais un document —
  maintenu hors index. C'est ce que je propose ; l'inverse (tout exposer, `/f/` compris) reste
  la décision de l'utilisateur.
- **Le préchargement du VFS par nginx** touche le vhost `aphrody.com` : c'est l'un des six
  gestes de production. La commande et son retour arrière se préparent ; l'application attend
  le go explicite. Noter que le serveur monte **déjà** le VFS en fond
  (`EtatSite::monter_vfs_en_fond`, 255 308 entrées) — l'étage nginx est une optimisation du
  premier octet, pas la condition du chargement.
- **La base légale** de la diffusion des assets LEVEL-5 sur un site `aphrody-dev` : l'accord
  N° RG-L5-VR-2026-001 est signé par Rose Griffon. Aucun agent ne tranche cela.
- **Le glossaire de traduction** (2,9 Mo, hors index git) : base, dépôt, ou absence bruyante —
  aucune voie n'est neutre.
- **Le domaine `inagle_cross_*`** (153 tables, jeu mobile) : aucun décodeur Rust n'existe ;
  reste au paquet Bun tant que personne ne le tranche.

## 8. Ce que « fini » veut dire

**Six conditions, toutes chiffrées, aucune déclarative.**

1. **`manquant = 0` et `partiel = 0`** dans la matrice, publiée sur `/couverture` et régénérée
   par une commande — jamais tenue à la main. Chaque capacité est servie et **comptée par un
   test**, ou classée `interne` **avec sa raison**.
2. **Les 255 308 fichiers du VFS sont classés**, aucun non identifié : ni `inconnu`, ni
   silencieux. `bloqué` reste un état légitime — mais il est **chiffré et visible**, avec le
   format qui manque, pas noyé dans « manquant ».
3. **Chaque corpus est prouvé par une requête qui rend un total et un code HTTP.** Un statut
   seul ne prouve rien : `/chara` a rendu 200 en 87 ms avec **0 lien** pendant une journée.
4. **`écrans servis / 475`** est publié, et chaque écran couvert porte ses trois nombres —
   objets, objets positionnés, objets muets — plus, quand elle existe, sa SSIM.
   **Publié depuis le 2026-09-06** : `GET /api/v1/screens` rend
   **475 écrans, 171 servis, 304 partiels — 36,00 %**, catalogue construit en 6,1 s ;
   `GET /api/v1/screens/{screen}` rend les trois nombres d'un écran
   (`ability_learning_report_menu` : **7 objets, 7 positionnés, 1 muet**, 140 ms ;
   `kizuna_town_mainmenu` : **13 / 13 / 4**, 203 ms). Le total **475 retombe exactement** sur
   celui de `/menu-tree.json`, par un chemin entièrement indépendant. La SSIM, elle, n'est
   **pas** mesurée là : la route le dit dans son champ `caveat` au lieu de le laisser croire.
   Un écran n'est `servi` que si **tous** ses calques résolvent vers un `.objbin` présent —
   définition choisie pour pouvoir échouer, et qui échoue sur 304 des 475.
5. Le sas `legacy/` est vide.
6. **Le site tourne** — vérifié en le lançant, pas en relisant le diff.

Et une clause de véracité, parce que ce plan s'est déjà trompé sur ses propres chiffres
(440 écrans au lieu de 475, 99 `pub fn` au lieu de 34, 24 objets non positionnés au lieu de 0) :
**tout compte cité ici porte la commande qui le produit et la date où elle a tourné.** Un nombre
sans commande n'est pas une mesure, c'est un souvenir.
