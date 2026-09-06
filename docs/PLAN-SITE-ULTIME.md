# Le site ultime — exigence de couverture sur toute la surface du dépôt

> Consigne de l'utilisateur, 2026-09-06. Ce plan **remplace** l'horizon de `/PLAN.md` (qui
> reste valable pour la bascule Azalée → Vercel et ses gates). Il ne décrit pas une semaine :
> il décrit **l'état d'arrivée** — un seul site, qui expose tout ce que ce dépôt sait faire.

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
| `pub fn` de `nie-lua` | **99** | `rg -c '^pub fn' crates/engine/nie-lua/src/*.rs` |
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

## 4. La matrice de couverture — l'instrument de mesure du plan

Le plan se pilote par **une seule table**, versionnée, régénérée par une commande, jamais tenue
à la main : `var/couverture-site.json` + une page `/couverture` sur le site.

Chaque capacité du dépôt y a une ligne, et **trois états seulement** :

| État | Sens |
|---|---|
| `servi` | une route ou un composant l'expose, et un test le compte |
| `interne` | délibérément non exposé, **avec sa raison** (privilège, écriture disque, forge, mémoire du jeu) |
| `manquant` | rien ne l'expose — c'est du travail restant, il est compté |

**Gate maîtresse du plan :** `manquant = 0`. Tout le reste en découle. Une capacité classée
`interne` sans raison écrite compte comme `manquant`.

Sources de la matrice, toutes déjà présentes : `niers --help` (41), l'`invoke_handler` de
`src-tauri` (155), les modules de `nie-data` (117) et `nie-formats` (47), les `pub fn` de
`nie-lua` (99), les pages d'Azalée (81 + 24), les sous-commandes d'`iecode` (39).

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

### Lot 5 — `nie-aphrody` sert les icônes, assets, pets et personnages

- `assets` : favicon, apple-touch, maskable, manifeste — déjà produits par la crate, à brancher.
- `pets` : le pet Aphrody (contrat `Pet`/`Frame`/`Rect`) servi et animé.
- `codex`, `gisement` : les personnages d'Aphrody et leur source.
- `pixel` : la mesure, exposée en interne (`/couverture`).

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

- **La base légale** de la diffusion des assets LEVEL-5 sur un site `aphrody-dev` : l'accord
  N° RG-L5-VR-2026-001 est signé par Rose Griffon. Aucun agent ne tranche cela.
- **Le glossaire de traduction** (2,9 Mo, hors index git) : base, dépôt, ou absence bruyante —
  aucune voie n'est neutre.
- **Le domaine `inagle_cross_*`** (153 tables, jeu mobile) : aucun décodeur Rust n'existe ;
  reste au paquet Bun tant que personne ne le tranche.

## 8. Ce que « fini » veut dire

`manquant = 0` dans la matrice, publiée sur `/couverture` et régénérée par une commande. Chaque
capacité du dépôt est soit servie et comptée par un test, soit classée `interne` avec sa
raison. L'UI a, pour chaque écran couvert, ses trois nombres (blocs, écart max, SSIM). Le sas
`legacy/` est vide. Et le site tourne — vérifié en le lançant, pas en relisant le diff.
