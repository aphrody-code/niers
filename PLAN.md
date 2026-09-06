# PLAN — une semaine de bout en bout, du 2026-09-05 au 2026-09-11

**But.** À la fin de la semaine : **Azalée** (`azalee.rosegriffon.fr`, DA Rose Griffon)
tourne sur Vercel en full serverless sur Supabase Cloud ; **Aphrody** (`aphrody.com`, DA du
vrai jeu) sert les outils et les assets depuis `nie-site` (Axum, 100 % Rust) ; **Inacord**
(ex `nie-explorer`) et Aphrody sont **la même interface**, `packages/inacord-ui`, montée par
deux hôtes ; le jeu s'appelle **nie** et ne bouge pas. Chaque jour a un propriétaire, une
gate qui **compte**, et un rollback.

La stack est **gelée** : [`docs/stack/`](docs/stack/README.md) (décisions, versions,
alternatives rejetées, gates). Ce fichier est le plan d'exécution de cette semaine ; le plan
maître du moteur et de la forge reste [`docs/PLAN.md`](docs/PLAN.md).

> **Amendement du 2026-09-06 (6) — la matrice de couverture existe, et la gate est rompue.**
> Le § 4 du cap prévoyait « une seule table, régénérée par une commande, jamais tenue à la
> main » ; elle n'existait pas, et c'était le trou le plus coûteux du plan — sans instrument, le
> reste se pilote au souvenir. Elle est construite :
> `nie-site --regenerer-couverture var/couverture-site.json`, servie par
> [`/couverture`](https://aphrody.com/couverture) et `/api/v1/couverture`, code dans
> `crates/tools/nie-site/src/couverture/`.
>
> **Premier résultat : 583 capacités sur 9 sources, `servi` 114, `manquant` 205, `partiel` 0,
> `bloqué` 10, `interne` 254 — gate ROMPUE.** Elle corrige quatre comptes que ce dépôt citait de
> mémoire (`niers` a **40** sous-commandes et non 41, Inacord **158** et non 155, `nie-data`
> **116** modules et `nie-formats` **46** au lieu de 117 et 47, Azalée **26** routes d'API et non
> 24), et elle en contredit un cinquième : **le VFS n'est pas à `manquant = 0`**, il porte
> **21 250 fichiers** dont le décodeur est écrit ici sans qu'aucune route ne l'appelle — 21 047
> `.p3lip` en tête. La raison n'est pas arithmétique : la première mesure comptait `servi` tout
> ce que `/f` rend, octets bruts compris, si bien que sa gate **ne pouvait pas échouer**.
>
> **Le lot a suivi le soir même** : `routes::level5` décode les cinq familles en process, sans
> une dépendance de plus, **124 / 124** à la mesure
> (`scripts/validation/mesurer-level5.sh`). `manquant` passe de **21 450 à 194 en poids** — plus
> aucun fichier du jeu n'est manquant, il ne reste que des capacités unitaires, dont **110
> modules de `nie-data`**. Détail au § 4 et au § 9 bis de
> [`docs/PLAN-SITE-ULTIME.md`](docs/PLAN-SITE-ULTIME.md).

> **Amendement du 2026-09-06 (5) — le VFS est cartographié, la cible devient 100 %.** Six agents
> ont couvert les 255 308 entrées, un document par domaine plus la synthèse
> [`docs/VFS.md`](docs/VFS.md). Résultat : **63,5 % servi, 26,6 % `manquant`, 6,2 % `partiel`,
> 1,5 % `bloqué`, 48 fichiers `inconnu`**. La matrice du cap passe de trois à **cinq** états —
> `partiel` et `bloqué` — parce que trois confondaient « élargir une route » et « faire du
> reverse ». Le fait qui commande la suite : **82 % du reste à faire est du câblage**, le
> décodeur existant déjà dans ce dépôt (`g4pk.rs`, `objbin.rs`, `g4cm.rs`, `col.rs`…). Le
> **lot 9** du cap fixe la trajectoire chiffrée vers `manquant = 0`.

> **Amendement du 2026-09-06 (4) — la 3D, l'écran d'attente et les filtres.** Quatre lots menés
> en parallèle sur des périmètres disjoints. La 3D est servie en **12 routes** mesurées sur
> **6 191** modèles assemblables, avec un rendu `nie-render3d` côté serveur (171 ms à froid,
> 0,9 ms en cache) ; l'écran d'attente est celui du jeu (`loading01`, texture servie, jamais
> copiée) ; et l'inventaire des filtres rend **48 recensés, 42 manquants**
> ([`docs/FILTRES.md`](docs/FILTRES.md)) — l'explorateur d'Aphrody n'en a aucun, et les 4 vues
> du catalogue ne couvrent que **143 246 des 255 308** entrées. Deux corrections d'assiette :
> `nie-menu` n'existe pas (la couche menu est `nie-lua::menu_host`) et `nie-lua` expose **34**
> `pub fn`, pas 99. Deux points attendent une décision de l'utilisateur, tous deux au § 7 du cap :
> le préchargement du VFS par **nginx** (geste de production) et la **couverture de tout le VFS
> en slugs**, qui contredit une décision documentée du même plan.

> **Amendement du 2026-09-06 (3) — la façade d'Aphrody est passée au crible.** Session UI :
> l'accueil montrait la même information jusqu'à trois fois et sept liens d'infrastructure ; le
> site portait deux chartes ; `nie-aphrody` n'était servie par aucune route. Corrigé et mesuré —
> sept routes `/pet/*` et `/api/v1/aphrody*`, une coquille unique, le personnage à la place du
> titre, 18 URL au plan de site, et les 29 couleurs du site désormais **engendrées** depuis la
> palette mesurée du personnage. Le détail, les échecs qui l'ont motivée et ce qui reste ouvert
> sont au § 3 et au § 5 bis de [`docs/PLAN-SITE-ULTIME.md`](docs/PLAN-SITE-ULTIME.md).

> **Amendement du 2026-09-06 (2) — l'horizon change d'echelle.** L'utilisateur demande un
> niveau d'exigence couvrant **toute la surface du depot** vers **un seul site ultime** :
> [`docs/PLAN-SITE-ULTIME.md`](docs/PLAN-SITE-ULTIME.md). Mesure de depart : 41 sous-commandes
> `niers` et 155 commandes Tauri pour **14 chemins d'API** — le depot sait faire dix fois ce
> qu'il expose. Ce plan-ci reste la reference pour la bascule Azalee vers Vercel et ses gates.
>
> **Amendement du 2026-09-06 — la semaine est compressée en une journée.** L'utilisateur confie
> **tout** ce plan à **Codex**, qui l'exécute en un jour, en raisonnement maximal et en
> exécution proactive. La frontière du 2026-09-05 (« Codex dans `rg`, Claude dans `niers` ») ne
> vaut plus : Codex écrit dans `niers` et y committe ses lots. L'ordre d'exécution, les huit
> gates chiffrées, l'état mesuré du 2026-09-06 et les six gestes qui exigent un go sont dans
> [`docs/CODEX-JOUR-UNIQUE.md`](docs/CODEX-JOUR-UNIQUE.md). Le tableau ci-dessous décrit la
> répartition d'origine ; il est conservé parce que les gates et les rollbacks de chaque
> journée restent valables tels quels, quel que soit celui qui les exécute.

## Les trois agents

| Agent | Moteur | Dépôt en écriture | Mission | Commits |
|---|---|---|---|---|
| **Fable 5.1** | Claude Code (`claude@aphrody-code/niers`) | `/home/ubuntu/niers` | orchestrateur ; tout le code : wiki serverless, `asset-source`, `inacord-ui`, `apps/inacord`, `apps/nie-web`, `crates/tools/nie-site`, DA du jeu, docs | seul auteur de commits dans `niers` |
| **GPT 6** | Codex (`codex@aphrody-code/niers`) | `/home/ubuntu/rg` et l'infrastructure du VPS | la production actuelle et son extinction : remédiation sécurité (8 actions), nginx (vhost `aphrody.com`, `supabase-compat.inc`, vhost `nie-model-serve`), unités systemd, arrêt d'`azalee-web` à J6, `deploy.ts` sans cible `azalee` | seul auteur de commits dans `rg` |
| **Astra** | Gemini (CLI `gemini` / `agy`) | **aucun** — écrit dans `var/mesures/` (hors dépôt) | vérificateur indépendant : rejoue chaque gate depuis un autre shell, matrices `curl`/`hyperfine`, captures `bxc` (rendu réel, CSP comprise), revue DA contre la référence, compte tout ; ne corrige rien | aucun ; rend des `fact:` A2A |

**Frontière (consigne utilisateur du 2026-09-05) :** Codex dans `rg`, Claude dans `niers`,
chacun son dépôt, chacun sa mission. Plus de rapatriement, plus de build concurrent croisé.
Le tick Codex `env-b002ca32` a lu la frontière **à l'envers** ; le tick de J1 la rétablit.

**Protocole :** `aphrody a2a tick --iteration <n> --side <moi> --peer <lui> --kind fact
--subject "<type>: <sujet>" --body "<mesure>"` ; types `goal:` (ordre), `claim:` (périmètre),
`fact:` (mesure), `block:` (arbitrage), `done:` (lot fini, avec ses comptes). Un `fact` porte
un chiffre, jamais une intention. Astra a reçu son identité `astra@aphrody-code/niers`
(rôle `verifier`) dans `ai.json` et sa boîte `.coord/inbox-from-astra.jsonl` — fait à J1.

**Exige le go explicite de l'utilisateur** (aucun agent ne le fait seul) : bascule DNS,
`nginx reload`, `systemctl stop/daemon-reload`, rotation d'un secret, premier
`vercel --prod`, toute suppression de données, tout `git push`.

## Départ mesuré — J1, 2026-09-05, VPS

| Mesure | Valeur | Source |
|---|---|---|
| Gate serverless (Cloud, miroir `/nonexistent`) | `/chara` **200** liens, `/skill` 60, `/item` 48, `/equipe` 208, fiche 200 ; build `EXIT_REEL=0`, 120/120 pages | `scripts/ops/gate-serverless.sh --no-build`, 07:05 |
| Le faux vert qui a précédé | `/chara` 200 en 87 ms, 136 921 o, **0 lien** (`SUPABASE_INTERNAL_URL` de `.env.local` gagnait dans `pickUrl()`) | tick A2A 20, retiré |
| Production `/chara` | TTFB 33 ms, **2 355 397 o** HTML, 620 liens, 404 `<img>` sans `srcset` | `curl -w`, prod |
| Production `/textures`, `/modeles` | TTFB **392 ms**, **229 ms** | idem |
| Supabase Cloud | 224 tables, 1 478 colonnes, 5 vues, 155 + 64 policies ; 65 tables / 165 277 lignes, 0 écart | `load-mirror-to-cloud.sh`, `84d4a54` |
| Ce que le wiki lit encore de local | `bun:sqlite` 41 fichiers, `node:fs` 44, `/home/ubuntu` 15, compat Supabase 19 ; 91 pages, 30 routes API | `rg -l`, `apps/azalee` + `packages/azalee` |
| Explorateur | 158 fichiers TS/TSX, 34 avec `@tauri-apps`, `api.ts` 630 l., `productName: "niers"` 0.5.9, identifiant `dev.niers.explorer` | `rg`, `tauri.conf.json` |
| `aphrody.com` | DNS → ce VPS, TLS émis, 10 hôtes dans un seul bloc nginx, CSP `default-src 'none'`. **Corrigé le 2026-09-05 au soir** : `aphrody-site` (:8083) n'écoutait plus du tout — les dix hôtes rendaient **502**, pas 265 o | `dig`, `curl`, `ss -ltnp`, `conf.d/aphrody.com.conf` |
| `Cargo.lock` | `axum` **absent** ; `tokio` 1.53.1, `tower` 0.5.3, `tower-http` 0.6.11, `rusqlite` 0.37.0, `reqwest` 0.13.4, `wgpu` 29.0.3 présents | `awk` sur le lock |
| Sécurité self-host | RPC anonyme destructif, `anon` écrit sur 129 tables, 2 105 lignes `discord_members` publiques, JWT lisible, SSH root par mot de passe | `docs/SECURITE-BASCULE.md`, `4f53936` |

## Où en est la semaine — mesuré le 2026-09-05

Chaque ligne porte son compte, et chaque compte a été rejoué sur cette machine. Ce qui n'a pas
été fait est dit tel quel, avec sa raison.

| Journée | Fait | Compte |
|---|---|---|
| **J2** | gate serverless, Proxy SQLite retiré, assets sortis, dix 308 écrites | gate **76 → 5** fichiers |
| **J3** | pagination `/chara`, ISR (déjà en place), unité miroir→Cloud écrite | 60 par page ; **3/3** |
| **J4** | contrat `asset-source`, socle `inacord-ui`, renommage Inacord | **0** Tauri dans le socle, 45 primitives |
| **J5** | crate `nie-site`, suite E2E, DA du jeu, Aphrody monté | 13 routes, 44 tests, **E2E 62/62** |
| **J7** | pré-compression du bundle, baseline `criterion` | JS **202 541 → 54 959 o** en brotli |
| **J5+** | **Aphrody est EN LIGNE** — `nie-site` installé, vhost appliqué | `aphrody.com` **502 → 200**, TTFB **16 ms** |
| **J5+** | i18n fr/en/ja, hreflang, JSON-LD, contenu rendu côté serveur, `llms.txt`, PWA | routes **13 → 18**, tests **44 → 72**, `/textures` **8 → 60** liens |
| **J5+** | `nie.` et `api.` branchés sur les services du dépôt | 11 hôtes avant, **11 après**, 0 perdu |
| **J5+** | `nie-model-serve` réparé et borné | ne répondait plus en 30 s → **0,37 ms** |
| **J5+** | audit de couverture du serving d'assets | octets bruts **255 308 / 255 308 = 100 %** |
| **J6−** | domaine éditorial migré sur Cloud, **build vert contre Cloud** | **117/117** pages, 0 erreur Postgres |
| **J6−** | `nie-model-serve` : cause racine de ses saturations | `RssAnon` **7,17 Gio → 861 Mio**, reclaim **4/9 → 0** |
| **J6−** | plafonds relevés pour l'après-bascule | 24 workers ; **24 requêtes simultanées, 24 × 200** |
| **J6−** | session RE/Lua de Codex intégrée | **+1 890 l.**, 80 tests verts, **66 commandes runtime** reconnues |

### Ce qui reste, et à qui

**Le blocage de J6 est LEVÉ — mesuré le 2026-09-05 à 23 h.** `bun run build`, miroir SQLite
introuvable et `DATABASE_URL` vers le pooler Supabase Cloud : **`EXIT_REEL=0`, 117/117 pages en
66 s, 0 erreur Postgres**, `server.js` produit. La bascule n'attend plus qu'un go.

Ce qui bloquait n'était pas la configuration mais un **schéma à moitié migré** : Cloud portait
224 tables, toutes `inagle_*`, et **aucune** table du domaine éditorial. Le wiki n'en interroge
que cinq ; quatre sont migrées avec **écart 0** (`tweets` 15 300, `patch_notes` 34, `articles` 3,
`article_series` 0), et `profiles` reste **vide par décision** — 1 821 profils d'utilisateurs ne
se déplacent pas sans qu'on l'ait voulu. `is_admin()` lit alors une table vide, rend `false`,
l'administration reste fermée et la lecture publique fonctionne. Rejouable :
`scripts/ops/migrer-editorial-vers-cloud.sh --compter`.

Le récit d'origine, conservé parce qu'il dit comment le défaut s'est masqué :

1. `lib/og-logo.ts` fetchait son PNG par `fetch(new URL(…, import.meta.url))` — `file:` n'est pas
   un schéma que `fetch` doit servir, et Bun le refuse (« not implemented... yet... »). Le build
   s'arrêtait **avant toute page** : 0/117. Corrigé par une constante embarquée → 87/117.
2. Il s'arrête maintenant au prérendu de `/` : `Error: Connection terminated due to connection
   timeout`. `app/page.tsx` et **16 autres fichiers** du domaine éditorial ouvrent une connexion
   Postgres **directe vers `127.0.0.1`**. Cela passe sur le VPS, jamais depuis Vercel.
   `DATABASE_URL` devra viser le pooler Supabase Cloud — c'est la décision 1 ci-dessous.

Mesuré en servant le wiki avec Postgres injoignable : `/chara` **200 liens**, `/skill` 60,
`/item` 48, `/equipe` 208 — les pages de données du jeu sont serverless-safe. Mais `/news` rend
**200 avec 0 lien** et `/api/tags/popular` rend `[]` en **5,05 s**, sur **6** « Connection
terminated » au journal et **aucune** page en erreur. Le domaine éditorial se vide en silence.

**Deux décisions de l'utilisateur**, sans lesquelles rien n'avance :

1. **Rotation du mot de passe Postgres.** `lib/auth.ts` portait les identifiants de production
   en dur comme repli. Retirés — mais le secret doit être considéré comme exposé. Il n'a jamais
   été commité (`git log -S` rend 0) : la portée est cette machine, pas le dépôt.
2. **Le glossaire de traduction** (2,9 Mo, absent de l'index git) : le porter en base engage un
   schéma, le verser dans le dépôt engage son historique, rendre son absence bruyante corrige le
   silence sans satisfaire la gate. Aucune voie n'est neutre.

**Trois gestes qui touchent la production**, tous prêts, tous en attente d'un go :

- `deploy/systemd/nie-miroir-cloud.{service,timer}` — écrites, `daemon-reload` non fait ;
- les dix redirections vers Aphrody — écrites, inactives tant que `NEXT_PUBLIC_TOOLS_ORIGIN`
  n'est pas posée ;
- la suppression des pages `/tools/*` — `docs/MIGRATION-EXPLORATEUR.md` §4 la qualifie
  lui-même de « décision de mise en ligne ». Les rediriger est réversible, les supprimer non.

**Un risque levé en cours de route.** Sortir `app/api/ietv` du wiki aurait figé les Inacord déjà
installés : leur repli lit un 503 comme « ce serveur ne moissonne pas la série », donc sans
erreur visible. `nie-site` sert désormais `/api/v1/episodes` — vérifié contre les 1 141 lignes
réelles, delta compris.

**Neuf fichiers étaient hors du dépôt.** `git check-ignore -v` le dit : la règle `*.txt`
(ligne 208) faisait sortir les **quatre templates askama de `nie-site`** — dont `robots.txt` et
`security.txt`, écrits à J5 et jamais versionnés. askama résout ses templates à la
**compilation** : sur un clone frais, la crate ne compilait pas. Et côté Azalée,
`public/ads.txt` : absent, la régie publicitaire s'arrête sans message. Réinclusions explicites,
vérifiées fichier par fichier.

**Ce que la session a appris.** Les défauts les plus coûteux ne se signalent pas : un bundle
jamais chargé sous 42 tests verts, une capacité de compression écrite et inutilisée, trois URL
d'assets fausses par déduction plausible, un identifiant de production actif dès que la
configuration manquait, une navigation qui se serait vidée au deuxième niveau. Aucun n'aurait
produit un message d'erreur. Tous ont été trouvés en lançant le binaire réel plutôt qu'en
relisant le code.

## Jour par jour

### J1 — samedi 2026-09-05 — trancher, geler, prouver

| Qui | Quoi | Gate |
|---|---|---|
| Fable | gate serverless **avec comptes** ✔ · `docs/stack` tranché, gelé, puis **relu et consolidé en v2** (A1 révisé par A2, A3 complété) ✔ · ce `PLAN.md` ✔ · `AGENTS.md`/`CLAUDE.md`/`README.md` alignés ✔ · Astra inscrit dans `ai.json` ✔ · tick Codex : frontière rétablie + verdict + plan ✔ · commits | les comptes sont dans les commits |
| Codex | prendre acte de la frontière (`rg` + infra) · écrire les **8 actions de sécurité sous forme de commandes prêtes, non exécutées**, une par une avec sa vérification · inventorier tout ce que `azalee-web` et `supabase-compat.inc` servent encore (17–19 consommateurs) | un `fact:` listant les 8 commandes et les consommateurs |
| Astra | recevoir son identité A2A · rejouer `gate-serverless.sh --no-build` depuis un autre shell et publier **ses** comptes · matrice de départ : 5 URL × 20 runs en production (`/`, `/chara`, `/chara/mark-evans`, `/textures`, `/modeles`), p50/p95/p99, poids avec et sans `Accept-Encoding: br` | ses comptes égalent ceux de Fable ; matrice dans `var/mesures/j1-prod.json` |

**Rollback :** aucun changement de production ce jour.

### J2 — dimanche 2026-09-06 — le wiki ne lit plus rien

| Qui | Quoi | Gate |
|---|---|---|
| Fable | déplacer hors d'`apps/azalee` ce qui lit un fichier : `/cpk`, `/textures`, `/modeles`, `/mode`, `/sons`, `/videos`, `/avatar`, `/demo`, `/save`, `/vroid`, `api/cpk`, `api/mode-tex`, `lib/cpk/index.ts`, le wasm — vers `apps/nie-web` (en attente J5) · retirer le Proxy SQLite du chemin métier, **une seule** URL Supabase (plus de `pickUrl` en cascade) · les 19 consommateurs compat → `*.supabase.co` · `vercel link` + variables (`~/.config/niers/vercel.env`, jamais affichées) · **preview #1** · les 308 vers `aphrody.com` écrites derrière `NEXT_PUBLIC_TOOLS_ORIGIN`, inactives | `rg -l 'bun:sqlite\|node:fs\|/home/ubuntu\|SQLITE_DB_PATH\|SUPABASE_INTERNAL_URL\|DATABASE_URL' apps/azalee packages/azalee` → **0** ; Gate 1 contre la preview : `/chara` ≥ 50, fiche 200 |
| Codex | sécurité **1–2** (révoquer l'exécution `anon` des RPC d'écriture et les grants d'écriture ; retirer `discord_members`/`settings` de l'accès anonyme), **avec go** · `nie-model-serve` : `limit_req`/`limit_conn` prêts | `POST /rest/v1/rpc/rg_liberer_profil_discord` → 401/403 ; `GET /rest/v1/discord_members?limit=1` → 401/403 ; comptes de grants avant/après |
| Astra | Gate 1 contre la preview depuis **l'extérieur** du VPS · latence preview → eu-west-3 : fiche perso n = 20, p50/p95/p99 · `bxc` : rendu réel de `/`, `/chara`, une fiche (CSP, chunks) | p95 fiche **< 800 ms**, sinon `block:` |

**Rollback :** la preview n'est pas la production ; `azalee-web` inchangé.

### J3 — lundi 2026-09-07 — poids et ISR

| Qui | Quoi | Gate |
|---|---|---|
| Fable | `/chara` : pagination (620 → 60 liens), `srcset` par `cdn-variants ?w=&format=webp` sur les 404 vignettes, markup aplati · ISR `revalidate = 3600` + `dynamicParams` sur les 6 fiches, `POST /api/ops/revalidate/wiki` · lot 2 de `docs/MIGRATION-EXPLORATEUR.md` §4 : pages `/tools/*` mortes et leurs 7 références entrantes — **sauf** `app/tools/niers/latest.json/route.ts` · unité `deploy/nie-miroir-cloud.service` (miroir → Cloud → revalidation) écrite, pas installée · preview #2 | `/chara` **< 250 Ko** en `br`, `<img>` sans `srcset` = **0**, Gate 1 sur preview #2 |
| Codex | sécurité **3–4** : plan de rotation atomique de `SUPABASE_JWT_SECRET` (liste des services qui le valident, ordre, retour arrière) ; SSH : vérifier une session par clé puis `PermitRootLogin no`, `PasswordAuthentication no` — **avec go** · installer `nie-miroir-cloud.timer` (`daemon-reload` **avec go**) | `sshd -T` montre les deux valeurs ; le timer est `active` ; le plan de rotation est un `fact:` |
| Astra | matrice **avant/après** J3 sur la preview (mêmes 5 URL × 20) · Lighthouse mobile sur `/`, `/chara`, une fiche · vérifie le compte de `<img srcset>` par lui-même | deltas publiés ; aucun régressif sur `/` |

**Rollback :** revert du commit ; la production n'a pas bougé.

### J4 — mardi 2026-09-08 — Inacord et l'interface partagée

| Qui | Quoi | Gate |
|---|---|---|
| Fable | `packages/asset-source` : `contract.ts` (`AssetSource` : vfs, texture, model, avatar, audio, video, wiki + `capabilities`), `types.ts`, `desktop-source.ts` (= `api.ts` renommé), `web-source.ts` (fetch vers `/api/v1` et `/assets`), `url-conventions.ts` (ré-export de `nie-catalog/src/jeu.ts`) · `packages/inacord-ui` : les 124 fichiers sans Tauri + modules purs (`galerie`, `traduction`, `filtrage`, `equipe`, `recherche`, `thumbs`, `cinema`, `sources`), les 22 composants passent par `useAssetSource()` · `git mv apps/nie-explorer apps/inacord`, `productName: "Inacord"`, titre de fenêtre `Inacord`, identifiant **conservé**, tous les chemins (`release-desktop.sh`, `packager-bases-explorer.sh`, `justfile`, hooks, `CLAUDE.md`) · **tout `@rosegriffon/*` et toute mention Rose Griffon sortent** d'`inacord-ui`/`apps/inacord` (départ : 13 fichiers, 23 imports, 19 mentions) — Aphrody, Inacord et nie sont `aphrody-dev`, hors Rose Griffon | `rg -l '@tauri-apps' packages/inacord-ui` → **0** ; `rg -il '@rosegriffon/|rose ?griffon' packages/inacord-ui apps/inacord apps/nie-web` → **0** ; `rg -l 'apps/nie-explorer' --glob '!docs/**' --glob '!*.md'` → **0** ; `bun run typecheck` vert ; `cargo check` dans `apps/inacord/src-tauri` vert ; `bun run --filter inacord build` vert |
| Codex | sécurité **5–6** : `limit_req`/`limit_conn` et `MemoryMax` sur `nie-model-serve` (**go**), jeton fine-grained pour l'updater GitHub · **brouillon** de la découpe du vhost `aphrody.com` (deux blocs : `aphrody.com www` → `:8085` sans CSP nginx ; les 8 autres hôtes → `:8083` inchangés ; `nie.aphrody.com` → 308), `nginx -t` sur une copie, **pas de reload** | `nginx -t` vert sur le brouillon ; `curl -sI` des 10 hôtes archivé comme référence « avant » |
| Astra | relit le diff d'extraction : cherche un import Tauri résiduel, un chemin `apps/nie-explorer` oublié, un composant hors `useAssetSource` · rejoue `typecheck` seul | ses comptes = ceux de Fable ; sinon `block:` |

**Rollback :** revert ; rien en production. **À VÉRIFIER hors VPS :** l'installeur « Inacord »
met à jour une 0.5.9 réelle (Windows) au lieu d'installer à côté — sinon la release attend.

### J5 — mercredi 2026-09-09 — Aphrody : `nie-web`, `nie-site`, la DA du jeu

> **Précision de l'utilisateur, 2026-09-05 :** Aphrody n'est **ni un wiki ni un explorateur de
> fichiers** — Azalée est le wiki, Inacord l'explorateur. L'interface d'Aphrody **reproduit le
> menu principal du jeu**. Les listes de catalogues livrées ce jour-là relèvent du métier
> d'Inacord et sont à reprendre : la disposition réelle s'exporte
> (`nie-game --runtime --menu mainmenu01 --export-layout`), elle ne se dessine pas.

| Qui | Quoi | Gate |
|---|---|---|
| Fable | `crates/tools/nie-site` : `main/app/config/error`, routes `health`, `well_known`, `static_files` (pré-compressé `br`/`zstd`, immuable par empreinte), **`/f/<chemin VFS verbatim>`** (une ressource, extension du jeu conservée) et **`/b/<préfixe VFS>`** (parcours d'un dossier) — amendement A3, chemin en **segment**, jamais en query ; les vues nommées (`/textures`, `/modeles`, `/sons`, `/videos`) sont des **filtres enregistrés** sur ces deux espaces, elles ne désignent jamais un fichier · `api/v1` (`rusqlite` ro, pagination, DTO), `assets` (proxy `nie-model-serve :8790` : `limit`, `timeout` 10 s, taille bornée, cache `moka`, ETag `blake3`), `index.html` via `askama` (titre, `og:` par route), erreurs, `robots.txt`, `security.txt`, `sitemap.xml`, CSP posée par la crate ; tests qui **comptent** ; `benches/routing.rs` · `apps/nie-web` : hôte Vite d'`inacord-ui` + `web-source.ts`, les routes sorties du wiki à J2 · **DA du jeu** : `niers design tokens` → `game-tokens.css` (70 variables), coquille **menu principal** pour Aphrody (`shell/main-menu/` : `SkewTile`, `TileRow`, `HeaderBanner`, `SidePanel`, `TitleBand`, `VersionChip`, `Callout`, `Badge`) sur les textures du jeu servies par `/assets` · coquille **InaCord** pour Inacord (`shell/inacord/` : `PhoneFrame`, `RoomList`, `MessageThread`, `HexBackdrop`, `TabBar` ; panneaux `#323544`/`#374D5B`, accent `#4FAECC`), références archivées dans `data/design/` · `deploy/nie-site.service` (`Restart=always`, `MemoryMax`) · `cargo build --release -p nie-site` | `cargo clippy -p nie-site --all-targets -- -D warnings` = 0 ; `cargo test -p nie-site` compte ; bundle initial **< 300 Ko gz** ; 70 tokens ; TTFB local `/api/v1/textures?page=1` **< 50 ms** |
| Codex | installer `nie-site.service` (**go**) · appliquer la découpe du vhost et retirer la CSP nginx du bloc Aphrody, `nginx -t`, **reload avec go** · `nie.aphrody.com` → 308 · vérifier les 10 hôtes après | `aphrody.com/healthz` répond `nie-site` ; les 8 autres hôtes répondent **comme avant** (diff des `curl -sI`) ; la CSP vue est celle de `nie-site` |
| Astra | Gate 5 : `hyperfine --warmup 3` sur `/`, `/api/v1/textures?page=1`, `/f/<une texture>` ; **200 chemins tirés de `niers vfs find` répondent 200 sur `/f/`** sous leur forme VFS exacte, dont une entité nommée `unknown` (gate A3) ; poids du bundle ; **capture `bxc` de `aphrody.com`** posée à côté de `data/design/aphrody-ui-ref-mainmenu-7.1.2.png` pour revue ; les 10 hôtes avant/après | TTFB `/textures` **< 50 ms** (départ 392), `/modeles` **< 50 ms** (départ 229) ; 200/200 chemins VFS ; aucune régression sur `api.`, `mcp.`, `downloads.` |

**Rollback :** restaurer le vhost précédent, ce qui restaure les **502** — car rien n'écoute sur `:8083` et `aphrody-site` n'existe plus comme service. Le rollback écrit ici supposait un repli qui n'existait pas ; le vrai filet est que ces hôtes étaient déjà hors service, donc la bascule ne pouvait rien casser.

### J6 — jeudi 2026-09-10 — la bascule

| Qui | Quoi | Gate |
|---|---|---|
| Fable | `vercel --prod` (**go**) · `NEXT_PUBLIC_TOOLS_ORIGIN=https://aphrody.com` : les dix 308 s'activent · Gate 1 contre la production dès le DNS basculé · `docs/EXPLOITATION.md`, `docs/AZALEE.md`, `docs/MIGRATION-EXPLORATEUR.md` mis à jour · release Inacord **si** la mise à jour depuis 0.5.9 est vérifiée, sinon J7 ou après | `dig +short azalee.rosegriffon.fr` = Vercel ; `/chara` ≥ 50 liens en prod ; `/tools/niers/latest.json` **200** ; les dix préfixes **308** vers `aphrody.com` ; `/tools` **pas** redirigé |
| Utilisateur | bascule DNS `azalee.rosegriffon.fr` → Vercel (registrar) | — |
| Codex | `systemctl stop azalee-web` (**go**, unité conservée 7 jours) · `supabase-compat.inc` et le vhost `azalee` retirés de nginx, vhost public de `nie-model-serve` retiré, **reload avec go** · sécurité **7–8** : `NEXT_PUBLIC_SUPABASE_URL` n'est plus servie par le VPS, ports publics et vhosts revus · `deploy.ts` de `rg` sans cible `azalee` (déjà `410ed795`), `rg-releases/azalee` gardé 7 jours | `ss -ltnp` : rien de nouveau en écoute publique ; `nginx -t` ; les 19 consommateurs compat répondent depuis `*.supabase.co` |
| Astra | Gate 6 complet : Gate 1 en production, updater 200, les dix 308, Realtime **101**, une URL signée Storage télécharge, `aphrody.com` intact, `azalee-web` arrêté | tout publié en un `fact:` avec chaque compte |

**Rollback :** DNS → VPS + `systemctl start azalee-web` (l'unité et le slot sont conservés) ;
Vercel garde la version précédente ; rien n'est supprimé avant J13.

### J7 — vendredi 2026-09-11 — performance, durcissement, docs, marge

| Qui | Quoi | Gate |
|---|---|---|
| Fable | `nie-site` : réglage `moka` (TTL, poids), pré-compression complète, baseline `criterion` commitée · `/chara` : matrice finale · docs : `CLAUDE.md`, `AGENTS.md`, `docs/README.md`, `docs/stack` (amendement daté si une brique a bougé) · mémoire · ce `PLAN.md` : chaque ligne marquée ✔ / ✗ avec son compte | toutes les gates vertes **avec leurs comptes dans les commits** |
| Codex | rotation `SUPABASE_JWT_SECRET` (**go**, si le plan de J3 est validé) · SSH fermé si pas encore fait · calendrier de suppression J13 (`azalee-web`, `rg-releases/azalee`, `supabase-compat.inc.bak`) | `sshd -T` ; les services qui valident le JWT répondent après rotation |
| Astra | régression complète : les 6 gates, la matrice J1 rejouée à l'identique, rapport final | un `fact:` par gate ; aucun compte inférieur au seuil |

La marge de J7 absorbe un glissement de J4 ou J5. Si J6 glisse, la bascule attend la semaine
suivante : **on ne bascule pas un vendredi soir.**

## Hors semaine — décidé, pas encore planifié

- **Azalée devient le wiki de référence** (amendement A4, programme
  [`docs/stack/wiki-azalee.md`](docs/stack/wiki-azalee.md)) : une page par **concept**
  (5 723 `internal_code`, pas 6 168 lignes), slugs lisibles désambiguïsés par le sens et
  versionnés avec `301`, noms FR/EN/JP + romaji, `hreflang`, JSON-LD et métadonnées sur les
  91 pages (départ : 31), puis corrections et contributions de la communauté sur surcouche
  révisable. Étapes 1 à 4 enchaînables juste après la semaine ; étapes 5 et 6 seulement quand
  quelqu'un accepte de modérer.

- **`nie-db` et `niers push`** (amendement A2) : couche SQL native — `rusqlite` pour SQLite,
  `sqlx` 0.8 pour PostgreSQL — et reprise du workflow des tables `inagle_*` (18 importeurs,
  2 575 l. du paquet Bun), alimentée par `nie-data`. Gate : `niers push --dry-run` annonce
  les lignes table par table, puis un push réel rend **le même total qu'aujourd'hui, écart
  0**. Jusque-là, `nie-site` ne crée **aucune nouvelle** lecture d'`inagle_*` et le miroir
  nocturne reste la source. C'est le premier lot après la semaine.
- **Rebranchement des cinq requêtes** de `nie-model-serve` et `nie-play` sur le gisement
  produit par `niers push` — suit `nie-db`, ne le précède pas.
- Mobile Tauri d'Inacord, jeu mobile natif, adaptateur Steam : spécifiés dans
  `docs/stack/game-platforms.md` et `desktop-mobile.md`, **non commencés**.
- Bump `wgpu 29.0.3 → 30.0.1` : lot compilé et golden-testé, pas cette semaine.
- Portage du domaine `inagle_cross_*` (153 tables, jeu mobile) : **non décidé**, aucun
  décodeur Rust n'existe ; reste au paquet Bun tant que personne ne le tranche.
- Leptos, Dioxus, Drizzle SQLite, Actix, `sqlx` **dans `nie-site`** : **rejetés**, voir l'ADR.
- `auth.users` : **jamais** migré.
- Réconciliation du manifeste 66 / 165 244 contre 65 / 165 277 : à faire, sans bloquer.

## La seule question ouverte, et elle est pour l'utilisateur

L'Accord Commercial N° RG-L5-VR-2026-001, qui autorise la diffusion des assets LEVEL-5, est
signé **par Rose Griffon**. Aphrody, Inacord et nie en sortent (décision `aphrody-dev`). La
base légale de leur exploitation des assets sur `aphrody.com` n'est donc **pas acquise** :
elle est couverte par l'accord existant, elle demande un avenant, ou Aphrody diffuse sous
couvert de Rose Griffon malgré la séparation de marque. Aucun agent ne tranche cela.

Rien d'autre n'attend de réponse. Le plan avance sans elle jusqu'à J5 ; c'est l'ouverture
publique d'`aphrody.com` avec des assets du jeu qui la rend nécessaire.

## Risques et rollback

| Risque | Propriétaire | Parade | Rollback |
|---|---|---|---|
| Une redirection attrape `/tools/*` et coupe l'updater d'Inacord | Fable | dix préfixes explicites, jamais de regex ; `/tools/niers/latest.json` testé à J6 | retirer la ligne, redéployer |
| Latence Vercel → eu-west-3 > 800 ms au p95 | Astra mesure, Fable corrige | ISR sur les fiches ; sinon `block:` et la bascule attend | pas de bascule |
| `supabase-compat.inc` : realtime/storage morts sans erreur de build | Fable + Codex | 19 consommateurs → `*.supabase.co` à J2, handshake 101 vérifié à J6 | `azalee-web` redémarré |
| Découpe du vhost `aphrody.com` coupe `api.`/`mcp.`/`downloads.` | Codex | brouillon J4, `nginx -t`, `curl -sI` des 10 hôtes avant/après | vhost → `:8083` |
| CSP nginx + CSP `nie-site` s'additionnent et cassent le site | Codex | pas d'`add_header CSP` sur le bloc Aphrody | idem |
| Renommage `niers → Inacord` installe à côté au lieu de mettre à jour | Fable | identifiant conservé ; test sur une 0.5.9 réelle avant release | ne pas publier |
| Un agent écrit dans le dépôt de l'autre, ou le démon capte un lot à mi-course | tous | `claim:` avant d'écrire ; relire `git log` ; un commit par lot | `git revert` |
| Faux vert (200, exit 0, N pages) sur un site vide | Astra | **compter**, toujours ; deux agents publient les mêmes comptes | — |

## Ce que « fini » veut dire

Un jour est fini quand sa gate a **tourné**, a **compté**, et que le compte est dans le commit
avec la commande, l'hôte et la date. La semaine est finie quand `azalee.rosegriffon.fr` sert
ses données depuis Vercel sans un seul fichier local, quand `aphrody.com` sert Aphrody dans la
DA du jeu en moins de 50 ms sur ses catalogues, quand Inacord et Aphrody sont un seul code, et
quand `azalee-web` est arrêté sans que personne ne l'ait remarqué.
