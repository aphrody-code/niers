# Stack 2026 — décision tranchée et gelée

**Statut : GELÉE le 2026-09-05.** Décision arbitrée par Claude (orchestrateur `niers`) sur
demande explicite de l'utilisateur, après le débat A2A avec Codex (`env-fa1cdc42`,
`env-b002ca32`), et complétée le même jour par trois consignes de l'utilisateur : le site
d'outils s'appelle **Aphrody** et vit sur **`aphrody.com`**, le jeu s'appelle **nie**,
l'application de bureau et mobile s'appelle **Inacord** ; `aphrody.com` porte la **direction
artistique du vrai jeu**, Azalée garde celle de **Rose Griffon**. Ce dossier ne change plus :
une brique qui doit bouger passe par un **amendement daté** dans
[decision-record.md](decision-record.md), section *Amendements*, et nulle part ailleurs. Le
plan d'exécution qui en découle est [`/PLAN.md`](../../PLAN.md), une semaine de bout en bout.

## Décision en une phrase

Le wiki **Azalée** (`azalee.rosegriffon.fr`, DA Rose Griffon) part sur **Vercel en full
serverless**, adossé à **Supabase Cloud** comme seule source de données ; les outils et
assets deviennent un second site, **Aphrody** (`aphrody.com`, DA du vrai jeu), servi par une
crate **`nie-site` (Axum 0.8, 100 % Rust)** sur le VPS, qui héberge **`nie-web`** : la même
interface que l'application **Inacord** (ex `nie-explorer`), partagée par extraction dans
`packages/inacord-ui` et non réécrite. Le jeu, **nie**, reste `wgpu 29.0.3` + `winit` cette
semaine ; mobile et Steam sont **hors semaine**, gelés tels que documentés.

## Les noms

| Produit | Nom public | Ce que c'est | Où dans le dépôt | Direction artistique |
|---|---|---|---|---|
| Le wiki | **Azalée** | fiches, articles, actualités — `azalee.rosegriffon.fr` | `apps/azalee`, `packages/azalee` | **Rose Griffon** : les 109 tokens M3 de `app/globals.css` (primaire `#f2a93b` / `#ffc66c`), inchangés |
| Le site d'outils et d'assets | **Aphrody** | 250 800 fichiers, 53 126 textures, 6 236 modèles, sons, vidéos, avatar — `aphrody.com` | `crates/tools/nie-site` (serveur) + `apps/nie-web` (bundle) | **le vrai jeu** : la référence est le **menu principal** du jeu (`mainmenu01`, capture ver. 7.1.2 fournie par l'utilisateur le 2026-09-05, `data/design/`, hors dépôt) — tuiles en parallélogramme, blanc et cyan, icônes blanches ; tokens extraits des données (palette `FONT_COLOR`, textures de menu, atlas d'icônes, fonte du jeu) |
| L'application de bureau et mobile | **Inacord** | l'explorateur/éditeur Tauri, aujourd'hui `productName: "niers"` v0.5.9 | `apps/inacord` (ex `apps/inacord`) | **le vrai jeu** : **InaCord** (イナコード), l'application de messagerie du téléphone du mode histoire, d'où vient le nom — panneaux sombres, accent turquoise, motif hexagonal (référence officielle `inazuma.jp`, archivée dans `data/design/`, hors dépôt) |
| Le jeu | **nie** | le moteur Rust et ses hôtes (natif, headless, WASM) | `crates/engine/*`, `nie-*` | le jeu lui-même |
| L'interface partagée | — | écrans, composants, hooks communs à Inacord et Aphrody | `packages/inacord-ui` + contrat `packages/asset-source` | un jeu de composants, **deux coquilles du jeu** : `shell/main-menu/` (Aphrody), `shell/inacord/` (Inacord) |

**Propriété (consigne utilisateur, 2026-09-05).** Seule **Azalée** est un produit Rose
Griffon. **Aphrody, Inacord et nie sont des projets `aphrody-dev`, hors Rose Griffon** :
aucune marque, aucun compte, aucun paquet `@rosegriffon/*` et aucune URL `rosegriffon.fr`
dans `nie-site`, `nie-web`, `inacord-ui` ou `apps/inacord`. Départ MESURÉ dans
l'explorateur : 13 fichiers importent `@rosegriffon/azalee` (20) et `@rosegriffon/ui` (3),
19 fichiers mentionnent Rose Griffon — tout sort à J4. Seule exception, temporaire :
l'updater des 0.5.x installés lit `azalee.rosegriffon.fr/tools/niers/latest.json` ; cette route
reste vivante (redirection vers `aphrody.com/downloads/inacord/latest.json`) tant que ces
installations existent, et les nouvelles releases pointent d'abord `aphrody.com`. **À
CONFIRMER par l'utilisateur** : la base légale d'exploitation des assets sur `aphrody.com`,
l'Accord N° RG-L5-VR-2026-001 étant signé par Rose Griffon.

Corollaire acté (amendements **A1** et **A2**) : **les trois produits fonctionnent sans le
paquet `inagle`**. Côté code c'est déjà vrai (Inacord le déclare mais ne l'importe jamais).
Les tables `inagle_*`, elles, ne sont **qu'un préfixe** et restent : `nie` acquiert une
couche SQL native (SQLite par `rusqlite`, PostgreSQL par `sqlx`) et reprend tout leur
workflow — les 2 575 lignes et 18 importeurs du paquet Bun deviennent `niers push`, alimenté
par `nie-data`. Restent au TypeScript, sans bloquer personne : les 153 tables
`inagle_cross_*` du jeu mobile, le scraping zukan et le RAG.

**Identité (amendements A3 et A4).** Aphrody et Inacord calquent slugs, URL et arborescence
de base sur le **VFS**, comme `nie.exe` : l'adresse est le chemin du jeu verbatim, le slug est
le code du jeu (`c01000010`), **aucun nom traduit dans une adresse**. Azalée vise la place de
Fandom et garde des slugs **lisibles**, mais adresse le **concept** et non la ligne : les deux
sites convergent donc sur le même identifiant, `internal_code`, affiché en chemin d'un côté et
en slug de l'autre. Mesures qui tranchent : 6 168 lignes pour **5 723 concepts** et seulement
5 199 `base_slug` distincts — 969 collisions, `unknown` 65 fois, et 17 lignes « mark-evans »
qui recouvrent six personnages différents. Programme du wiki : [wiki-azalee.md](wiki-azalee.md).

Ce qui ne change **pas** de nom : les crates `nie-*` (le préfixe est celui du jeu), la CLI
`niers`, le plugin Blender `niers-blender`, l'identifiant Tauri `dev.niers.explorer` (dossier
de données et continuité de l'updater), les URL de l'updater.

## Ce qui est tranché

| Sujet | Décision | Rejeté, et pourquoi |
|---|---|---|
| Hébergement du wiki | **Vercel**, runtime Node, ISR + revalidation on-demand | VPS self-host : couple le wiki à une machine et à un miroir SQLite local — la cause du faux vert du 2026-09-05 |
| Données du wiki | **Supabase Cloud** `kvnlbhatjqqmhhxaxlbi` (eu-west-3), lecture anonyme sous RLS `lecture_publique` | PostgREST self-host : `127.0.0.1` n'existe pas depuis Vercel ; miroir SQLite : un fichier, donc pas serverless |
| Comptes utilisateurs | **Pas de migration** des 1 931 lignes `auth.users` ; réinscription = consentement | Copie silencieuse de données personnelles |
| Domaine du site d'outils | **`aphrody.com`** (+ `www`), site nommé **Aphrody** ; `nie-site` remplace `aphrody-site` (:8083) sur ces deux hôtes seulement | `nie.rosegriffon.fr` : deux marques, deux DA — Rose Griffon est la communauté, Aphrody est l'univers du jeu ; `nie.aphrody.com` : un sous-domaine pour le produit principal |
| Serveur du site | **`nie-site`**, Axum 0.8 sur `127.0.0.1:8085` derrière nginx, TLS Let's Encrypt déjà émis pour `aphrody.com` | socle `aphrody-web` du dépôt `aphrody` (tokens communs) : la DA d'Aphrody est celle du jeu, pas une charte commune aux vitrines |
| Interface du site | **`packages/inacord-ui`** (React/Vite, extrait d'Inacord) montée par `apps/nie-web` et par Inacord | **Leptos** : une seconde pile d'UI, 0 ligne partagée avec l'app, mainteneur unique (issue #4707) ; **Dioxus** : même défaut |
| Données du site | Les trois gisements du VPS (`var/mirror.sqlite`, `var/niers.sqlite`, `data/anime/episodes.db`) lus par **`rusqlite` 0.40** en lecture seule — les fichiers qu'Inacord embarque | **SQLx + PostgreSQL** pour `nie-site` : un saut réseau pour des données servies localement, et des réponses qui divergeraient d'Inacord |
| HTML rendu côté serveur | **`askama` 0.16** (+ `askama_web`, `askama_axum` est mort) pour `index.html` enrichi (`og:`), erreurs, `robots.txt`, `security.txt`, `sitemap.xml` | `tera`, `minijinja` : parsing à l'exécution ; `maud` : DSL macro |
| Cache mémoire | **`moka` 0.12.16** (plancher : en deçà l'éviction LRU peut se figer) + ETag **`blake3`** | `lru` : mono-thread, sans TTL |
| Décodage des assets | **`nie-model-serve` reste le décodeur**, proxifié par `nie-site` avec débit, délai, mémoire | l'absorber : 7 956 lignes réécrites pour rien ; l'exposer nu : le risque n° 1 |
| Application Inacord | **Tauri 2** inchangé, hôte mince d'`inacord-ui` ; `productName` → `Inacord`, identifiant conservé | réécriture native : rien ne l'exige ; changer l'identifiant : casse le dossier de données et l'updater |
| Le moteur nie | **`wgpu 29.0.3` gelé** ; bump 30.0.1 = lot ultérieur, compilé et golden-testé | bump « au passage » |
| Mobile, Steam | **Hors semaine.** [game-platforms.md](game-platforms.md), [desktop-mobile.md](desktop-mobile.md) restent vrais et gelés | — |

## Cartographie gelée

```text
azalee.rosegriffon.fr ── Vercel (serverless) ── apps/azalee (Next 16, Node, DA Rose Griffon)
                                │
                                └── HTTPS ──> Supabase Cloud kvnlbhatjqqmhhxaxlbi

aphrody.com / www ────── VPS nginx (TLS) ── crates/tools/nie-site (Axum 0.8) :8085
                                │        ├── sert apps/nie-web (bundle Vite de packages/inacord-ui, DA du jeu)
                                │        ├── lit var/mirror.sqlite, var/niers.sqlite, episodes.db (rusqlite, ro)
                                │        └── proxifie nie-model-serve :8790 (débit, délai, mémoire, cache moka)
api. downloads. cdn. bot. admin. mcp. bxc. n2b.aphrody.com ── inchangés, aphrody-site :8083 (dépôt aphrody)

apps/inacord (Tauri 2) ── même packages/inacord-ui, packages/asset-source (desktop) ── updater inchangé
```

## Ordre obligatoire — la semaine

1. **J1** — gate serverless **avec comptes de contenu** (fait, voir *État mesuré*), gel de ce
   dossier, `PLAN.md`.
2. **J2** — sortir du wiki tout ce qui lit un fichier ; premier déploiement Vercel *preview*.
3. **J3** — poids de `/chara` et ISR ; matrice de latence avant/après.
4. **J4** — extraction `packages/asset-source` + `packages/inacord-ui` ; `apps/inacord`
   devient `apps/inacord` et redevient vert.
5. **J5** — `apps/nie-web` + `crates/tools/nie-site` + thème du jeu ; vhost `aphrody.com`
   basculé de `:8083` à `:8085` (go de l'utilisateur).
6. **J6** — bascule DNS du wiki sur Vercel (go de l'utilisateur), redirections 308 vers
   `aphrody.com`, arrêt d'`azalee-web` sur le VPS.
7. **J7** — performance, durcissement, documentation, marge.

Détail, propriétaires et gates : [`/PLAN.md`](../../PLAN.md).

## Règle de preuve

Chaque chiffre est **MESURÉ** (commande, hôte, date), **ESTIMÉ** (objectif) ou **À
VÉRIFIER**. Et depuis le 2026-09-05, une règle de plus, payée deux fois dans la journée :
**un code de sortie et un code HTTP ne mesurent pas la présence de données.** Seul un
compte d'éléments attendus dans la réponse la mesure. `scripts/ops/gate-serverless.sh`
l'applique ; aucun verdict serverless n'est recevable sans ses comptes.

## État mesuré au gel (2026-09-05, VPS)

- **Gate serverless réussi.** Build `apps/azalee` avec `SQLITE_DB_PATH=/nonexistent`,
  `SUPABASE_INTERNAL_URL` **et** `NEXT_PUBLIC_SUPABASE_URL` sur le Cloud : `EXIT_REEL=0`,
  120/120 pages, 1 114 replis `SQLITE_CANTOPEN → Postgres`. Contenu servi par `next start`
  sans miroir : `/chara` **200** liens de personnages, `/skill` 60, `/item` 48, `/equipe`
  208, `/chara/mark-evans` HTTP 200. TTFB `/` 17 ms, `/chara` 52 ms, fiche 6 ms.
- **Le même gate rendait un faux vert deux heures plus tôt** : `/chara` en 87 ms, 136 921
  octets, **0 lien** — `SUPABASE_INTERNAL_URL=http://127.0.0.1:8811` de `.env.local`
  l'emportait sur l'URL Cloud dans `pickUrl()` (`lib/supabase/server.ts:42`).
- **Supabase Cloud** : 224 tables, 1 478 colonnes, 5 vues, 155 policies, plus 64 policies
  `lecture_publique` (commit `84d4a54`) ; 65 tables / 165 277 lignes chargées, 0 écart.
  L'inventaire local 66 / 165 244 reste **À RÉCONCILIER** par manifeste.
- **`aphrody.com` aujourd'hui** : DNS → ce VPS (51.77.147.152), certificat Let's Encrypt
  émis, `aphrody-site` (:8083) rend une page de 265 octets (`<title>Aphrody`, corps vide),
  `/healthz` 200, `/downloads` et `/version` 404. Rien à préserver sur ces deux hôtes.
- **Poids** : `/chara` pèse 2 708 582 octets non compressés (2 355 397 en production) ;
  81 % sont du DOM réel — 620 liens, 404 `<img>` sans `srcset`. Cible **< 250 Ko**.
- **Sécurité** : expositions critiques de l'infrastructure self-host inchangées, listées et
  ordonnées dans [security.md](security.md) ; aucune n'est reproduite sur Vercel ni sur Aphrody.

## Documents

- [Décision, alternatives, amendements](decision-record.md)
- [Azalée — le wiki de référence : slugs, langues, contributions, SEO](wiki-azalee.md)
- [Web, API, Supabase, Vercel, nginx](web-platform.md)
- [Versions, licences et maintenance](dependencies.md)
- [Sécurité et prérequis d'exposition](security.md)
- [Vérification, gates et définition de fini](verification.md)
- [Benchmarks et mesures](benchmarks.md)
- [Moteur, mobile, WASM et Steam — gelé, hors semaine](game-platforms.md)
- [Desktop et mobile Tauri — gelé, hors semaine](desktop-mobile.md)
