# ADR — deux sites, trois noms, une interface partagée, une plateforme Rust

- **Date :** 2026-09-05
- **Décision :** tranchée et **gelée**
- **Arbitre :** Claude, orchestrateur du dépôt `niers`, sur demande explicite de
  l'utilisateur ; débat A2A avec Codex (`env-fa1cdc42`, `env-b002ca32`) ; consignes
  utilisateur du même jour sur les noms, le domaine et les directions artistiques
- **Périmètre :** documentation dans `niers` ; le code suit dans [`/PLAN.md`](../../PLAN.md)

## Contexte

`azalee.rosegriffon.fr` est une seule application Next.js qui mêle deux métiers sans public
ni profil de charge communs : un **wiki éditorial** (fiches, articles, lu par des visiteurs et
des moteurs) et un **atelier d'outils** (250 800 fichiers du jeu, 53 126 textures, 6 236
modèles, sons, cinématiques, éditeur d'avatar) adossé au décodeur Rust `nie-model-serve` et
aux 111 Go du VPS. **MESURÉ** en production le 2026-09-05 : une page d'atelier répond en
392 ms (`/textures`) là où le wiki répond en 30 ms (`/`).

Trois exigences de l'utilisateur tranchent l'architecture : le wiki tourne sur **Vercel en
full serverless** ; le site d'outils **partage tout le code de l'explorateur** ; les deux sites
ont **deux directions artistiques** — celle de Rose Griffon pour le wiki, celle du vrai jeu
pour le site d'outils. La deuxième interdit toute réécriture d'interface : l'explorateur est
une SPA React/Vite de **158 fichiers TS/TSX MESURÉS**, dont **34** seulement importent
`@tauri-apps` ; sa façade vers le natif est un fichier unique, `src/lib/api.ts` (630 lignes).

## Décision

### Les noms

- **Azalée** — le wiki, `azalee.rosegriffon.fr`. Nom, domaine et design inchangés.
- **Aphrody** — le site d'outils, **`aphrody.com`** et `www.aphrody.com`. **Ni un wiki ni un
  explorateur de fichiers** : le wiki est Azalée, l'explorateur est Inacord. Son interface
  reproduit le **menu principal du jeu** (précision de l'utilisateur, 2026-09-05).
- **Inacord** — l'application de bureau et mobile, ex `nie-explorer` : `apps/inacord`,
  `productName: "Inacord"`, fenêtre titrée `Inacord`. L'identifiant Tauri
  `dev.niers.explorer`, le dossier `%APPDATA%\dev.niers.explorer\` et les deux URL de
  l'updater (`azalee.rosegriffon.fr/tools/niers/latest.json`, GitHub `aphrody-code/nie`)
  **ne changent pas** : c'est ce qui permet aux 0.5.x installés de se mettre à jour.
- **nie** — le jeu : le moteur Rust, ses hôtes natif, headless et WASM, et le préfixe des
  crates. La CLI reste `niers`.
- **`packages/inacord-ui`** — l'interface partagée par Inacord et Aphrody ; **Aphrody est
  Inacord dans le navigateur**. `packages/asset-source` en est le contrat d'accès aux données.

### La propriété

Seule **Azalée** appartient à Rose Griffon. **Aphrody, Inacord et nie sont des projets
`aphrody-dev`, hors Rose Griffon.** Conséquences gelées : pas de marque ni de mention Rose
Griffon sur `aphrody.com` ou dans Inacord ; pas de paquet `@rosegriffon/*` dans
`packages/inacord-ui`, `apps/nie-web`, `apps/inacord` (MESURÉ au départ : 13 fichiers, 23
imports, 19 mentions — les types et helpers utiles passent dans `packages/asset-source` ou
`nie-catalog`) ; pas de compte ni de SSO Rose Griffon ; l'updater d'Inacord vise d'abord
`aphrody.com/downloads/inacord/latest.json` (servi par `nie-site` avec la même logique
GitHub), la route `azalee.rosegriffon.fr/tools/niers/latest.json` survivant en redirection
pour les 0.5.x déjà installés. La base légale d'exploitation des assets LEVEL-5 hors Rose
Griffon est **à confirmer par l'utilisateur** ; ce dossier ne la présume pas.

### Les deux directions artistiques

- **Azalée = Rose Griffon.** Le thème M3 existant (`apps/azalee/app/globals.css`, 109
  tokens `--md-sys-color-*`, primaire `#f2a93b` clair / `#ffc66c` sombre) est la DA ; rien
  n'y touche cette semaine au-delà du poids des pages.
- **Aphrody et Inacord = le vrai jeu.** Le thème est **extrait des données du jeu**, jamais
  inventé : la palette de texte `common/font/font_color.cfg.bin` (70 couleurs `FONT_COLOR`,
  déjà portée dans `nie-data::font_color`), les textures de menu servies par `nie-model-serve`
  (cadres, fonds, boutons), les atlas d'icônes déjà exploités par `sprites.css` et
  `data/re/menu-icon-atlases.txt`, et la fonte du jeu (`font_def.g4tx` + métriques) pour les
  titres. Il vit dans `packages/inacord-ui/src/theme/` ; les couleurs y sont **générées** par
  une commande `niers` depuis le fichier du jeu, avec leur nom d'origine. Ce que l'extraction
  ne fournit pas (corps de texte, espacements) est **ESTIMÉ** et dit tel quel. Une seule
  interface, **deux coquilles du même jeu** : le site reprend le **menu principal**,
  l'application reprend **InaCord**, l'application de messagerie du téléphone du mode histoire.
- **La référence d'Aphrody est le menu principal du jeu** (`mainmenu01`), capture ver. 7.1.2 de
  2 497 × 1 414 fournie par l'utilisateur le 2026-09-05 et conservée hors dépôt dans
  `data/design/aphrody-ui-ref-mainmenu-7.1.2.png` (© LEVEL-5, jamais commitée). C'est
  l'écran que `docs/DESIGN.md` décompose déjà (31 objbin, textures du groupe B dans le VFS).
  La coquille d'Aphrody en reprend la grammaire : bandeau haut (logo, notification,
  version), deux panneaux latéraux illustrés, une rangée de **tuiles en parallélogramme** à
  fond photo teinté cyan et icône blanche, une bande bleue de titre, une seconde rangée de
  trois tuiles, des badges en bas. Palette **MESURÉE** sur la capture (ImageMagick, 12
  couleurs) : blanc `#FDFEFE` (60 % des pixels), cyans `#D9EFED` `#A4E4F7` `#46B9F2`, bleus
  `#5BA2E3` `#2F69C7` `#295B9F`, marine `#293D60`, jaune `#F6E028`, orange `#D55025`.
  Ces valeurs cadrent le thème ; les couleurs **finales** viennent des fichiers du jeu.
- **La référence d'Inacord est InaCord** (イナコード), l'application de messagerie du
  téléphone dans le mode histoire — c'est de là que vient le nom. Référence officielle
  fournie par l'utilisateur : `inazuma.jp/victory-road/assets/img/story/story-system/img_photography_01.webp`
  (1 280 × 720, archivée hors dépôt dans `data/design/inazuma-jp-story-photography-01.webp`).
  Grammaire : cadre de téléphone, panneaux sombres, colonne de salons à gauche, fil de
  messages avec avatars ronds, accent turquoise, motif hexagonal en fond, barre d'onglets du
  menu principal au-dessus. Palette **MESURÉE** (ImageMagick, 8 couleurs) : `#323544`
  `#374D5B` `#44484F` (panneaux), `#4FAECC` (accent), `#1E67C5` `#07346E` (bleus),
  `#A8CFD2` (clair), `#7B8F6B`. Les écrans d'InaCord existent dans le VFS du jeu : leurs
  textures et leur palette `FONT_COLOR` priment sur ces valeurs de cadrage.

### Le wiki — Vercel + Supabase Cloud

- `apps/azalee` reste **Next.js 16** (**stable 16.3.4** — corrigé le 2026-09-05 : la canary 16.3.0-canary.37 du catalogue est derrière le stable), déployé sur
  **Vercel**, runtime **Node**. ISR horaire sur les fiches détail, `dynamicParams = true`,
  `POST /api/ops/revalidate/wiki` protégé par `AZALEE_REVALIDATE_SECRET`.
- **Supabase Cloud `kvnlbhatjqqmhhxaxlbi` (eu-west-3) est la seule source de données.** Le
  wiki ne lit plus jamais un fichier : ni `var/mirror.sqlite`, ni `/home/ubuntu/...`, ni
  `process.cwd()`. Le Proxy PostgREST de `lib/supabase/server.ts` disparaît du chemin métier.
- Les tables `inagle_*` sont lisibles anonymement sous RLS par la policy `lecture_publique`
  (commit `84d4a54`). Aucune écriture anonyme. `auth.users` (1 931 lignes) **n'est pas
  migrée** : les comptes se recréent, la réinscription vaut consentement.
- Ce qui lit un fichier ou un service local (`bun:sqlite` : 41 fichiers, `node:fs` : 44,
  `/home/ubuntu` : 15, `/rest/v1|/realtime/v1|/storage/v1` : 19 — **MESURÉ** sur
  `apps/azalee` + `packages/azalee`) **part chez Aphrody** ou vise l'origine Supabase
  dédiée ; rien n'est « corrigé sur place ».

### Aphrody — `nie-site` + `nie-web`

- `crates/tools/nie-site` : **Axum 0.8**, Tokio 1.53, Tower 0.5, `tower-http` 0.7,
  `askama` 0.16 (+ `askama_web`), `moka` 0.12.16, `blake3` 1.8, `rusqlite` 0.40 ; `publish = false`, écoute
  **uniquement** `127.0.0.1:8085`, nginx termine le TLS. Il **sert** le bundle `nie-web`,
  **lit** les trois gisements du VPS en lecture seule, et **proxifie** `nie-model-serve`
  (`127.0.0.1:8790`) en lui ajoutant ce qu'il n'a pas : limite de débit, budget de temps,
  budget mémoire, cache.
- **`aphrody.com` aujourd'hui** (MESURÉ) : DNS déjà sur ce VPS, certificat Let's Encrypt
  déjà émis, `aphrody-site` (:8083, dépôt `aphrody`) y rend une page de 265 octets au corps
  vide et un `/healthz`. La bascule est une modification du vhost nginx : `aphrody.com` et
  `www.aphrody.com` vers `:8085`, **les autres hôtes du bloc** (`api.`, `downloads.`, `cdn.`,
  `bot.`, `admin.`, `mcp.`, `bxc.`, `n2b.`) **restent sur `:8083`**. `nie.aphrody.com`
  redirige en 308 vers `aphrody.com`. L'en-tête `Content-Security-Policy: default-src 'none'`
  qu'nginx ajoute aujourd'hui **doit être retiré de ce vhost** : les CSP s'additionnent et la
  plus stricte gagne — `nie-site` pose la sienne.
- `apps/nie-web` : hôte Vite de `packages/inacord-ui` avec `web-source.ts`.
  `apps/inacord` : hôte Tauri de la même UI avec `desktop-source.ts` (`api.ts` renommé).
  Les conventions d'URL viennent de `packages/nie-catalog/src/jeu.ts` (757 lignes, déjà
  testé contre `main.rs`), pas d'une réécriture.

### Le moteur et les clients natifs — inchangés cette semaine

- `wgpu 29.0.3` + `winit 0.30.13` gelés ; le bump `wgpu 30.0.1` est un lot ultérieur,
  compilé et validé par goldens D3D12/Vulkan/Metal/WASM.
- Tauri 2 reste l'enveloppe d'Inacord, desktop aujourd'hui, mobile plus tard.
- Mobile natif du jeu et adaptateur Steam : **hors semaine**, spécifications gelées dans
  [game-platforms.md](game-platforms.md).

## Alternatives rejetées

| Alternative | Raison du rejet |
|---|---|
| **Wiki self-host VPS** (décision de Codex dans `rg/docs/decision-archi-donnees-azalee.md`) | vise l'inverse de la cible ; couple le rendu web à un fichier SQLite local — cause directe du faux vert du 2026-09-05 |
| **`nie.rosegriffon.fr`** pour le site d'outils | deux marques, deux DA : Rose Griffon est la communauté et son wiki, Aphrody est l'univers du jeu ; le SSO par cookie parent est sans objet, Aphrody ne porte pas de comptes cette semaine |
| **`nie.aphrody.com`** | un sous-domaine pour le produit principal du domaine ; le placeholder d'`aphrody-site` sur `aphrody.com` ne contient rien |
| **Socle `aphrody-web`** du dépôt `aphrody` (tokens et squelette communs aux vitrines) | la DA d'Aphrody est celle du jeu, pas une charte commune ; `SITES-PLATFORM.md` du dépôt `aphrody` est à amender par son propriétaire |
| **Leptos 0.8** pour `nie-site` | seconde pile d'UI à côté de React : 0 ligne partagée avec Inacord ; mainteneur unique et maintenance « légère » (issue #4707) ; 37 975 lignes TS/TSX à porter pour l'égaler |
| **Dioxus 0.7** | même défaut de partage ; plan B seulement si le produit devient Rust-first partout |
| **SQLx + PostgreSQL** dans `nie-site` | un saut réseau pour des données que `var/mirror.sqlite` sert localement ; Inacord lit déjà ces fichiers : même source ⇒ mêmes réponses |
| **Drizzle dual-runtime `bun-sqlite`/`node-sqlite`** (Codex) | fige le SQLite local comme dépendance de production ; la partie utile — remplacer 494 lignes d'émulation PostgREST — est reprise côté Postgres |
| **Actix** | débit brut supérieur sur benchmark synthétique, mais hors continuité Tokio/Tower et hors `best-stack-2026` |
| **Absorber `nie-model-serve` dans `nie-site`** | 7 956 lignes écrites à la main (ni Axum ni tokio) ; le réécrire n'apporte rien que le proxy durci n'apporte déjà |
| **Changer l'identifiant Tauri** avec le nom Inacord | nouveau dossier de données, updater NSIS/MSI qui installe à côté au lieu de mettre à jour |
| **Une DA « Aphrody » ou « Inacord » inventée, hors du jeu** | la consigne est le vrai jeu : le site reprend le menu principal, l'application reprend InaCord ; une interface, deux coquilles, aucune couleur dessinée de mémoire |
| **Migrer `auth.users`** | données personnelles ; aucune base légale documentée |
| **Bevy / ECS, Tauri pour le jeu, SQLite distant** | inchangé : incompatibles avec le byte-exact, le rendu natif, ou le serverless |

## Ce que le débat a établi (et qui a survécu)

- **Le faux vert.** Un build vert, 70/70 pages, `/chara` 200 en 87 ms et 136 921 octets —
  et **0 lien** dedans. Deux causes en une journée : RLS sans policy (PostgREST rend 200 et
  un tableau vide) puis `SUPABASE_INTERNAL_URL` testé avant `NEXT_PUBLIC_SUPABASE_URL` par
  `pickUrl()`. Leçon gravée dans [verification.md](verification.md) : compter, pas croire.
- **Le N+1** de `chara/[id]/page.tsx` (599 requêtes `inagle_skills`) est corrigé par
  `cf11153` : 245 techniques → 2 requêtes, 10 tests, 954 assertions, deux backends.
- **La bascule a réussi sans miroir** : gate du 2026-09-05, comptes dans
  [README.md](README.md#état-mesuré-au-gel-2026-09-05-vps).
- **La sécurité** est indépendante de la bascule et la précède ; ordre dans
  [security.md](security.md).

## Historique Vercel vérifié

`abcfb69f` (prerender `/_global-error` en échec sous Bun), `3c01c323` (Node 24 introuvable),
`6fe2a626` (website Vercel, Azalée VPS), `2cf27f1c` (Vercel retiré), `9594ba0d` (failover).
Ces échecs venaient d'un runtime Bun et d'une base en `127.0.0.1` ; aucun ne tient avec le
runtime Node et Supabase Cloud. Ils justifiaient la séparation wiki/outils, pas le renoncement.

## Risques et déclencheurs de révision

1. **Auto-update d'Inacord** : une redirection qui attraperait `/tools/*` couperait la mise à
   jour de toutes les installations. `app/tools/niers/latest.json/route.ts` reste au wiki, et
   les 308 sont posés par préfixe explicite, jamais par regex. Le renommage `niers → Inacord`
   du `productName` doit être **vérifié sur une installation 0.5.9 réelle** (Windows) avant
   publication : l'installeur doit mettre à jour, pas installer à côté.
2. **`supabase-compat.inc`** : realtime et storage servis sous le domaine du wiki cassent sur
   Vercel **sans erreur de build**. Origine Supabase dédiée, CORS explicite, 19 consommateurs
   à tester un par un.
3. **Vercel ↔ eu-west-3** : aucune latence mesurée avant le premier déploiement preview ; si
   la fiche perso dépasse 800 ms au p95, la bascule DNS attend.
4. **Le vhost `aphrody.com`** porte dix hôtes dans un seul bloc `server` : la découpe doit
   laisser `api.`, `downloads.`, `cdn.`, `bot.`, `admin.`, `mcp.`, `bxc.`, `n2b.` sur `:8083`,
   et retirer la CSP nginx du seul bloc Aphrody. Une faute ici coupe les services du dépôt
   `aphrody`. Test : `nginx -t`, puis un `curl` par hôte avant et après.
5. **Exposer `nie-model-serve` nu** : jamais ; `nie-site` est obligatoire devant.
6. **Deux agents, deux dépôts** : Codex dans `rg`, Claude dans `niers`, plus un démon qui
   commit des checkpoints. Un lot peut être capté à mi-course ; relire `git log`.
7. **La DA du jeu** est une extraction, pas un dessin : ce que les fichiers ne donnent pas
   (corps de texte, espacements, comportement responsive) reste **ESTIMÉ** et se corrige sur
   capture réelle, jamais de mémoire (règle « ne rien halluciner du jeu »).

## Amendements

> **Gel v2 du 2026-09-05.** Les trois amendements du jour ont été relus ensemble et
> consolidés : A1 est **remplacé par A2** sur la question des tables, et A3 reçoit la
> distinction ressource/vue qui lui manquait. Un amendement ultérieur repart de A4.

### A1 — Aphrody, Inacord et nie fonctionnent sans le paquet `inagle` *(révisé par A2)*

**Décision.** Aucun des trois produits `aphrody-dev` ne dépend du paquet
`@rosegriffon/inagle`, propriété Rose Griffon. `inagle` reste la chaîne de publication
d'**Azalée** et rien d'autre.

**Ce qu'A1 disait de faux, et qu'A2 corrige.** A1 étendait l'indépendance aux **tables**
`inagle_*`. C'était une erreur de lecture : `inagle_` est un **préfixe de table**, pas un
lien au paquet. Les tables sont un schéma de données de jeu, légitime, et elles restent —
13 crates, le wiki, le miroir et l'installeur d'Inacord s'y adossent ; les renommer casserait
tout pour rien. Ce qui change, c'est **qui les produit** : voir A2.

**Coût mesuré le 2026-09-05, côté code — déjà acquis.** Inacord déclare
`@rosegriffon/inagle` dans son `package.json` mais ne l'importe **0 fois** ; son `src-tauri`
ne dépend que de crates `nie-*` ; les 37 crates du moteur n'y font aucune référence de code.
Retirer la déclaration suffit (J4, avec les 20 imports `@rosegriffon/azalee` et les 3
`@rosegriffon/ui`).

**Coût côté données — cinq requêtes**, qui ne disparaissent pas mais **changent de source**
(A2). `nie-model-serve`, que `nie-site` proxifie, lit le miroir pour assembler les modèles :

| Table lue | Lignes | Requêtes | Module `nie-data` équivalent |
|---|---:|---:|---|
| `inagle_characters` | 6 168 | 1 | `chara_base.rs` |
| `inagle_teams` | 208 | 1 | `team.rs` |
| `inagle_uniforms` | 627 | 1 | `uniform.rs` |
| `inagle_event_subtitles` | 2 093 | 2 | `event_subtitle.rs` |

`nie-play` lit la même table de sous-titres. `nie-formats`, `nie-data`, `nie-save` et
`nie-explore` ne la lisent **pas** : leurs occurrences d'`inagle_*` sont des commentaires.
Les quatre familles étant déjà décodées par `nie-data`, il n'y a **aucun parseur à écrire**.

**Ce qui reste chez Azalée :** les 153 tables `inagle_cross_*` (*Inazuma Eleven Cross*, jeu
mobile distinct, sans décodeur Rust) et les 2 575 lignes de publication du paquet Bun.

**Gate.** `rg '@rosegriffon/' apps/inacord/package.json packages/inacord-ui apps/nie-web`
→ **0**. Contrainte immédiate : `nie-site` ne crée **aucune nouvelle** lecture d'`inagle_*`
tant qu'A2 n'a pas livré.

### A2 — 2026-09-05 : `nie` gère nativement SQL et possède le workflow des tables `inagle_*`

**Correction d'A1.** A1 traitait les tables `inagle_*` comme une dépendance à Rose Griffon.
C'est faux : `inagle_` est un **préfixe de table**, pas un lien au paquet. Les tables sont
un schéma de données de jeu, légitime et à conserver sous ce nom (13 crates, le wiki, le
miroir et l'installeur d'Inacord s'y adossent ; renommer casserait tout pour rien).

**Décision.** `nie` acquiert une **couche SQL native** — SQLite et PostgreSQL — et reprend
**tout le workflow** des tables `inagle_*` que le paquet Bun assurait : lire les données de
jeu, normaliser, publier, vérifier. `inagle` cesse d'être le producteur ; il devient
l'ancêtre dont on garde le schéma et les leçons. Aphrody, Inacord et nie fonctionnent alors
sans le paquet, tout en lisant et écrivant les mêmes tables.

**Ce qui est porté, mesuré le 2026-09-05.**

| Élément | Aujourd'hui (TypeScript) | Cible (Rust) |
|---|---|---|
| Abstraction de base | `DataAdapter`, 2 impls : `SupabaseAdapter`, `PostgresAdapter` | un trait à 2 impls : SQLite (`rusqlite` 0.40, le lock est en 0.37) et PostgreSQL (`sqlx` 0.9, `postgres` + `runtime-tokio` + `tls-rustls-ring-native-roots` + `macros`) |
| Transport vers le Cloud | `@supabase/supabase-js`, donc **PostgREST en HTTP** | SQL direct via `sqlx` — une couche réseau **supprimée**, pas reproduite |
| Workflow | 18 fonctions `import*` / `export*`, 2 575 l. (`cli-push.ts` + `push-categories.ts`) : `importCharacters` 164 l., `importSkills` 129, `importItems` 106, `importAuras` 100, `importGrowthTables` 66, `importDrops` 51… | une commande `niers push`, un module par famille, alimenté par `nie-data` (déjà byte-exact, 130 goldens) |
| Idempotence | `ON CONFLICT` par `id`, `delete + reinsert` pour les tables curatées | identique, en transactions explicites |
| Migrations | `supabase/migrations/*.sql` | inchangées : le SQL reste la source de vérité du schéma |

**Où ça vit.** Une crate `crates/tools/nie-db` (le trait, les deux back-ends, les migrations
rejouables), exposée par **une seule commande utilisateur, `niers push`** — la doctrine « `niers`
est la seule CLI » interdit un binaire de plus. `nie-data` n'y touche pas : elle reste le
lecteur typé, sans `tokio` ni client SQL.

**Ce que ça n'est pas.** Ce n'est pas une contradiction de l'ADR, qui rejette `sqlx` pour
`nie-site` : ce dernier **lit** des fichiers locaux, où `rusqlite` est plus direct ; `nie-db`
**écrit** vers un Postgres distant, où `sqlx` est le bon outil. Deux métiers, deux clients,
la même règle — le client suit la distance à la donnée.

**Ce qui n'est pas repris tout de suite.** Les 153 tables `inagle_cross_*` (*Inazuma Eleven
Cross*, jeu mobile) n'ont aucun décodeur Rust : leur alimentation reste au paquet Bun jusqu'à
ce que quelqu'un décide de porter ce domaine. Le scraping zukan (navigateur headless `bxc`)
et l'étage RAG restent également TypeScript ; ils ne bloquent ni Aphrody, ni Inacord, ni nie.

**Gate.** `niers push --dry-run` annonce, table par table, le nombre de lignes qu'il écrirait ;
un `niers push` réel suivi d'un comptage rend **le même total qu'aujourd'hui**, table par
table, écart **0** — la migration se prouve par égalité avec l'existant, jamais par « ça
tourne ». Puis `rg -n 'inagle_' crates/tools/nie-model-serve/src crates/engine/nie-play/src`
hors commentaires → les requêtes visent le gisement produit par `niers push`.

**Ordonnancement.** Lot **hors semaine J1–J7**. Contrainte immédiate maintenue : `nie-site`
ne crée aucune nouvelle lecture d'`inagle_*` en attendant, et le miroir nocturne reste la
source jusqu'à ce que `niers push` ait prouvé l'égalité.

### A3 — 2026-09-05 : Aphrody et Inacord sont calqués sur le VFS, comme `nie.exe`

**Décision.** Slugs, URL et arborescence de base d'Aphrody et d'Inacord suivent le **VFS du
jeu**, chemin pour chemin, code pour code. **Aucun nom traduit dans une adresse.** Les noms
français ou anglais restent des **libellés d'affichage** ; ils ne désignent jamais une
ressource. Azalée garde ses slugs traduits — c'est un wiki lu par des humains et des
moteurs ; Aphrody est un atelier sur les fichiers du jeu, et parle donc la langue du jeu.

**Pourquoi, mesuré le 2026-09-05.** Le slug traduit n'identifie pas. Sur les 6 168 lignes
d'`inagle_characters` : **5 199 `base_slug` distincts, soit 969 collisions**, contre 5 737
`chara_id`. `unknown` sert 65 fois, `kr-k9` 20 fois, `shawn-froste` 17. `mark-evans` recouvre
six lignes, toutes sur le même code `c01000010`. Une adresse bâtie dessus est ambiguë par
construction — et c'est déjà le défaut documenté « l'identifiant n'est jamais un titre ».
Le VFS, lui, est unique, stable entre versions du jeu, et **vérifiable** : `niers vfs find`
dit si le chemin existe, ce qu'aucun slug ne permet.

**Les règles.**

1. **L'adresse est le chemin VFS, verbatim**, y compris `data/`, `common/`, `dx11/` :
   `aphrody.com/f/data/common/chr/_face/01_IE1/c01000010/c01000010.g4md`. Pas de
   réécriture, pas de raccourci, pas de casse normalisée — le VFS est sensible à la casse.
2. **Le slug d'une entité est son code de jeu** : `c01000010` pour un personnage,
   l'identifiant natif pour une équipe, un item, une technique. Jamais `mark-evans`. Quand
   plusieurs codes coexistent (`id` hash, `chara_id`, `internal_code`), le canonique est
   **celui qui nomme les fichiers du VFS**.
3. **L'arborescence de la base est celle du VFS** : chaque ligne porte son chemin canonique
   et son code, indexés ; la navigation par dossier est une requête de préfixe, pas une
   taxonomie inventée.
4. **Le chemin passe en segment d'URL, jamais en query.** Cela corrige la verrue mesurée de
   `nie-model-serve`, où `/vfs/*` prend `?path=` quand toutes les autres routes prennent un
   segment — source connue de 404 attribués à tort au décodage.
5. **L'extension du jeu est conservée dans l'adresse** ; la conversion est un suffixe ou un
   paramètre explicite (`.png`, `?format=`), pas une amputation. La règle actuelle
   « `/tex/<chemin sans .g4tx>.png`, garder l'extension donne un 400 » est un piège à
   supprimer, pas à propager.
6. **Les noms traduits restent affichables et cherchables**, dans le corps de la page et
   dans l'index de recherche — jamais dans l'adresse, jamais comme clé.

**Conséquences.** Aphrody et Inacord partagent alors la **même arborescence** que
l'utilisateur voit dans l'application de bureau : un chemin copié depuis Inacord s'ouvre
dans Aphrody, et inversement. `packages/asset-source` n'a plus qu'un espace de noms à
porter, celui du VFS, ce qui simplifie `url-conventions.ts`. Les 255 308 entrées du VFS
deviennent adressables sans table de correspondance.

**Ce qui n'est pas concerné.** Azalée : ses URL indexées ne bougent pas, ses slugs traduits
non plus. Les 153 tables `inagle_cross_*` (jeu mobile, pas de VFS) gardent leurs clés.

**Ressource et vue — la distinction qui manquait.** A3 gouverne l'**identité** d'une
ressource, pas le nom d'un écran. Aphrody a donc deux espaces d'adresses, et deux seulement :

| Espace | Forme | Rôle |
|---|---|---|
| **Ressource** | `/f/<chemin VFS verbatim>` | un fichier du jeu, exactement un, identité stable |
| **Parcours** | `/b/<préfixe VFS>` | un dossier du VFS, requête de préfixe |

Les noms d'écrans (`/textures`, `/modeles`, `/sons`, `/videos`) restent **autorisés** : ce
sont des **filtres enregistrés** sur le VFS, pas une taxonomie inventée ni une identité —
`/textures` équivaut à `/b/data/dx11` filtré sur `.g4tx` (les 54 203 textures y vivent ; les
modèles vivent sous `data/common/chr/`). Ils préservent les dix redirections 308 venant
d'Azalée, donc les URL indexées. Ce qu'ils ne font jamais : désigner un fichier. Un fichier
n'a qu'une adresse, `/f/<chemin>`.

**Gate.** Un échantillon de 200 chemins tirés de `niers vfs find` répond 200 sur
`/f/<chemin>` sous la forme exacte du VFS ; le même chemin ouvert dans Inacord désigne la
même ressource ; `rg` sur les routes d'Aphrody ne trouve **aucun** slug traduit dans un
identifiant ; et une entité dont le nom est `unknown` reste adressable — c'est le cas qui
prouve la règle.

### A4 — 2026-09-05 : Azalée devient le wiki de référence, pas un catalogue

**Décision.** Azalée vise la place d'`inazuma-eleven.fandom.com/fr` : un slug **lisible et
unique** par joueur, équipe et technique ; les noms **français, anglais, japonais** affichés
en écriture latine **et** japonaise ; les **corrections et contributions** de la communauté ;
et le référencement d'un vrai wiki. Programme détaillé, mesures et gates :
[wiki-azalee.md](wiki-azalee.md).

**Ce que la mesure a tranché.** Le slug unique existe déjà (`mark-evans-0x3055CF22`, 6 168
sur 6 168) et il ne vaut rien : un hash dans une URL n'est ni lisible, ni partageable, ni
référençable. Il fallait remonter d'un cran. Sur les 17 lignes portant `mark-evans` : **onze**
sont le code `c01000010` (le personnage, plus ses variantes `hero_type` *black*, *pink*, et
une `_5000`), et **six** sont des personnages différents (`c05024610`, `c05029460`,
`c07110020`, `c11500500`, `c11901150`) qui partagent un nom traduit. Le dépôt contient donc
**5 723 concepts** (`internal_code`) pour 6 168 lignes, et `is_primary = 1` désigne déjà une
ligne canonique sur 5 260 d'entre elles.

Trois conséquences :

1. **L'unité éditoriale est le concept, pas la ligne.** Une page par `internal_code` ; les
   variantes sont des sections. Créer 17 pages « Mark Evans » ferait perdre le wiki.
2. **Le slug se désambiguïse par le sens**, jamais par un hash : `/chara/mark-evans`, puis
   `/chara/mark-evans-victory-road`, le code en dernier recours ; table de slugs versionnée,
   `301` permanent à chaque renommage, page de désambiguïsation pour les homonymes. Les 969
   collisions deviennent une fonctionnalité.
3. **Azalée et Aphrody convergent sur la même identité.** `internal_code` est exactement le
   code qui nomme les fichiers du VFS (amendement A3). Aphrody l'affiche en chemin, Azalée en
   slug lisible : deux sites, deux publics, un seul objet désigné. Ce n'est pas un hasard,
   c'est ce qui rend les liens croisés fiables.

**Ce qui change dans les décisions déjà gelées.** Rien n'est retiré ; deux points s'étendent :

- **L'écriture arrive sur Vercel.** `anon` reste en lecture seule ; `authenticated` écrit
  **uniquement** dans les tables de propositions et de révisions, **jamais** dans `inagle_*`.
  Une correction humaine est une **surcouche** appliquée par-dessus la valeur extraite, qui
  reste visible — c'est ce qui permet de rejouer un import sans détruire la communauté, le
  mode d'échec qui tue les wikis adossés à des données. Gate : une correction acceptée
  **survit à un réimport complet**.
- **`auth.users` reste non migré.** Les comptes repartent de zéro sur le Cloud ; la
  réinscription vaut consentement. Décision inchangée, et la contribution ne la remet pas en
  cause.

**Ce que je ne promets pas.** « Premier sur tous les mots-clés » est un cap, pas une gate :
Fandom a quinze ans d'antériorité et une autorité de domaine qu'aucune optimisation technique
ne renverse en un mois. Ce qui se pilote et se mesure : 100 % de pages à métadonnées uniques
(départ **31 sur 91**), JSON-LD et `hreflang` partout, `/chara` sous 250 Ko (départ
2 355 397 o), sitemaps segmentés, URL stables. Le seul avantage structurel réel sur Fandom
est que **nos données viennent du jeu, décodées et vérifiables** — le référencement doit
exposer cette exactitude plutôt qu'imiter un wiki communautaire. Le suivi de position est
mensuel, sur un panier de mots-clés fixé à l'avance, publié avec sa date.

**Ordonnancement.** **Hors semaine J1–J7**, et sans la retarder : le programme s'appuie sur le
socle serverless qu'elle livre. Ses étapes 1 à 4 (slugs, noms, désambiguïsation, SEO) ne
demandent aucun compte et peuvent suivre immédiatement. Les étapes 5 et 6 (contributions,
modération) ne démarrent pas avant que **quelqu'un accepte de modérer** — c'est une charge
humaine, pas une fonctionnalité.

### A5 — 2026-09-05 : la chaîne pixel-perfect vit dans `nie-aphrody`, portable et partagée

**Décision.** La chaîne « image du jeu → asset reproduit » — mesure, comparaison chiffrée,
vectorisation, planche de sprites, jetons de couleur — vit dans **une seule crate,
`nie-aphrody`**, qui acquiert au passage le contrat « pet » de Codex. Elle sert Aphrody
(`nie-web`), Inacord et le web depuis la même source, sans réimplémentation par hôte.

**Portabilité — MESURÉE, pas supposée.** `cargo check -p nie-aphrody --target
wasm32-unknown-unknown --no-default-features --lib` → **0 erreur, 0 warning** (VPS,
2026-09-05). Deux décisions la rendent réelle plutôt que nominale :

- le module `pets` **ne lit jamais l'horloge** : le temps écoulé est un argument en
  millisecondes. Un `Instant::now()` interne aurait imposé une horloge au navigateur et rendu
  les tests dépendants du temps réel ;
- `Image::depuis_octets` est le point d'entrée portable ; `Image::charger` (disque) passe
  derrière la feature **`fs`**, active par défaut. Un hôte sans système de fichiers ne peut
  donc plus appeler une fonction qui échouerait toujours à l'exécution.

**Ce qui n'est PAS réécrit.** `nie_formats::sprite_sheet` produisait déjà, depuis les
rectangles d'un atlas du jeu, la feuille **CSS** (mode image et mode masque `currentColor`),
le **SVG** autonome à `<symbol>` et le **JSON** des régions. `pixel planche` lui *apporte* une
planche assemblée au lieu d'un `.g4tx` : un sprite issu de poses rendues et un sprite issu
d'un atlas du jeu s'emploient donc identiquement dans `inacord-ui`. Aucun générateur CSS
concurrent n'entre dans le dépôt.

**Bibliothèques — versions et licences relevées le 2026-09-05** (crates.io + `gh api`, pas de
mémoire). Ce tableau vaut pour ce dossier ; il ne modifie pas `dependencies.md`, conformément
à la règle de gel.

| Rôle | Crate | Version | Licence | Rejeté, et pourquoi |
|---|---|---|---|---|
| Rastérisation SVG | `resvg` / `usvg` | 0.48.1 | Apache-2.0 OR MIT | `vello` (exige wgpu, pas de SVG natif), `femtovg` (contexte OpenGL) |
| Back-end 2D CPU | `tiny-skia` | 0.12.0 | **BSD-3-Clause** | — (clause de non-endossement à porter dans les mentions) |
| Image | `image` | 0.25.10 | MIT OR Apache-2.0 | `sharp` côté Bun : binding libvips, inutile ici |
| Comparaison SSIM | `image-compare` | 0.5.0 | MIT | **`dssim` : AGPL-3.0** — contaminerait tout binaire distribué. C'est la seule licence non permissive du lot |
| Couleur Oklab/Oklch | `palette` | 0.7.7 | MIT OR Apache-2.0 | `oklab` (2 ans sans commit, ni Oklch ni gamut mapping) |
| Vectorisation | **aucune** | — | — | `potrace` : GPL-2.0 **et** sans crate ; `vtracer` : tirerait un **second `image`** (^0.23) plus `clap 2` et `pyo3`. Suivre le bord d'un masque puis simplifier tient en deux fonctions — un carré en sort en 4 sommets, là où un traceur générique en rend 32 |

**Contrat pet, et l'écart assumé.** Le module `pets` réimplémente le sous-système `pets`
d'`openai/codex` (**Apache-2.0**, attribution dans `crates/engine/nie-aphrody/NOTICE`) :
schéma `pet.json`, valeurs par défaut, bornes de validation, sémantique `loop_start` /
`fallback`, choix de frame par accumulation de durées, états à durée de vie (3 min / 1 h /
24 h / 7 j). **Un écart :** Codex fige la grille à 8 × 9 et rejette tout autre atlas ;
appliqué à la lettre, il rejetterait notre propre pet, qui est un 8 × **11**. La géométrie est
donc lue dans le **manifeste** — ce que Codex fait déjà pour ses pets personnalisés — et sa
validation stricte s'applique à la géométrie déclarée. Cellule 192 × 208 et 8 colonnes sont
identiques de part et d'autre : Aphrody v2 est une extension du même contrat. Les **assets**
de Codex ne sont pas repris : ils ne sont pas dans leur dépôt (CDN) et ne sont donc pas
couverts par sa licence.

**Branchement Inacord.** `nie-aphrody` est atteignable depuis Tauri par les trois pas
obligatoires du dépôt : dépendance de chemin, façade `src/aphrody.rs` à **DTO locaux**, 7
commandes dans `collect_commands!`, bindings régénérées. Aucun `specta` n'entre dans les
crates moteur — aucune n'en dépend, et leur en imposer un ferait payer une dépendance
d'interface à `nie-wasm`, aux goldens et à la forge. Toutes les commandes sont `async` : une
commande synchrone tourne sur le thread principal et figerait la fenêtre sur une lecture
disque.

**Correction apportée à la décision gelée.** Le README annonce que les tokens de la DA
d'Aphrody sont « extraits des données » de `mainmenu01`. La mesure du jour oblige à
distinguer :

- les **couleurs** le sont — palette k-means en Oklab sur la capture ver. 7.1.2 : rangée de
  tuiles `#2D5DA1` 38,4 %, `#578FD8` 29,7 %, `#0C2F64` 23,1 %, `#CEE1F6` 8,8 %, rendues en
  `oklch()` avec le HEX en commentaire ;
- la **géométrie ne l'est pas**. L'ajustement des bords des tuiles rend un R² entre **0,004 et
  0,45**, très en dessous du seuil de 0,95 : l'outil refuse de donner un angle, parce que le
  bord suit le contenu du sprite et non le cadre. Écrire un `skewX(-18°)` tiré de la capture
  serait **inventé**. La géométrie doit venir du layout runtime.

Or ce layout est précisément ce qui manque : composition statique à **22 des 31** objbin de
`mainmenu01`, SSIM contre la référence **≈ 0,004** (plancher de non-régression 0,003,
`menu_render_gate.rs:588`). Le blocage n'est pas un format — les motions `g4pkm` ne portent
**aucune keyframe de position** ; le placement vient de la machine d'état C++ `G4RA` et des
callbacks Lua `Setup*`, jamais reversés. Carte complète et prochain pas chiffré :
[`docs/mainmenu01-analyse-visuelle.md`](../mainmenu01-analyse-visuelle.md).

**Conséquence pour la semaine.** La DA d'Aphrody peut partir sur les couleurs mesurées et sur
les atlas d'icônes du VFS ; elle ne peut **pas** prétendre reproduire la mise en page de
`mainmenu01` tant que le placement n'est pas reversé. Le dire dans le thème plutôt que de
laisser croire à une conformité non prouvée — « pixel-perfect » reste un objectif mesuré,
jamais un adjectif que l'on s'accorde.

**Gate.** `cargo test -p nie-aphrody --lib` → **30/30** ; `cargo clippy -p nie-aphrody --lib
--bins --tests` → **0 warning** ; `cargo check --target wasm32-unknown-unknown
--no-default-features` → **0 erreur** ; l'app Tauri compile. Toute affirmation de fidélité
visuelle s'accompagne d'un SSIM ou d'un pourcentage de pixels dans la tolérance, et dit
lequel.

---

Toute modification de la stack s'écrit ici, datée, avec sa mesure et son alternative
rejetée — et ne modifie aucun autre fichier du dossier.
