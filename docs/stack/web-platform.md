# Web, API, Supabase, Vercel et nginx

## Architecture gelée

```text
Navigateur ── azalee.rosegriffon.fr ── Vercel ── apps/azalee (Next 16, Node, ISR, DA Rose Griffon)
                                                    │ supabase-js (clé anon, RLS lecture)
                                                    ▼
                                    Supabase Cloud kvnlbhatjqqmhhxaxlbi
                                    Postgres · PostgREST · Auth · Storage · Realtime

Navigateur ── aphrody.com / www ── nginx (TLS) ── nie-site :8085 (Axum 0.8) — « Aphrody »
Inacord (Tauri) ─────────────────────────────────┐   ├── /            bundle nie-web (inacord-ui, DA du jeu)
                                                 │   ├── /api/v1/*    DTO serde, rusqlite ro sur les 3 gisements
                                                 │   └── /assets/*    proxy durci → nie-model-serve :8790
                                                 └── desktop-source.ts : mêmes écrans, invoke() au lieu de fetch()

api. downloads. cdn. bot. admin. mcp. bxc. n2b.aphrody.com ── aphrody-site :8083, dépôt aphrody, inchangés
```

Le wiki ne reçoit jamais autre chose que la clé **anon** ; `nie-site` ne reçoit jamais de
requête venant d'Internet sans nginx devant ; aucun client ne voit une URL de base Postgres,
une clé `service_role` ou un chemin machine.

## Versions retenues

| Couche | Version | État dans `Cargo.lock` au 2026-09-05 | Rôle |
|---|---|---|---|
| Axum | 0.8.9 | **absent — seul vrai ajout** | routes, extracteurs, état |
| Tokio | 1.53.1 | présent | runtime ; `rt-multi-thread` pour le serveur |
| Tower | 0.5.3 | présent | middlewares |
| `tower-http` | 0.6.11 | présent (0.7.1 existe ; pris quand le workspace bougera, pas en double) | compression brotli/zstd, `ServeDir`, trace, limites |
| hyper | 1.11.0 | présent | transport, via Axum |
| `askama` | 0.14 | absent | templates compilés : `index.html` enrichi, erreurs, `robots.txt`, `security.txt`, `sitemap.xml` |
| `moka` | 0.12 (`sync`) | absent | cache LRU concurrent avec TTL et poids |
| `blake3` | 1.5 | absent | ETag fort, empreinte des variantes |
| `rusqlite` | 0.37.0 (`bundled`) | présent | lecture seule des gisements, `?mode=ro`, réouverture au swap d'inode du miroir |
| `reqwest` | 0.13.4 (`rustls-tls`) | présent | appels vers `nie-model-serve` |
| `zstd` | 0.13.3 | présent | pré-compression des assets statiques |
| `tracing-subscriber` | **0.3.22 épinglé** | présent | journaux structurés |
| `clap` | 4.6.6 (`derive`) | présent | `--listen`, `--bundle-dir`, `--upstream` |
| `thiserror` | 2.0.20 | présent | erreurs de la crate |
| `criterion`, `insta` | 0.5, 1.40 | absents (dev) | benches de routage, snapshots de réponses |

Pas de `rustls` dans la crate : nginx termine le TLS, `nie-site` parle HTTP en clair sur
`127.0.0.1`. Pas de Leptos, pas de SQLx, pas de `native-tls`, pas d'`actix` : voir l'ADR.

## Le wiki sur Vercel

- **Runtime Node**, pas Bun : les échecs historiques (`abcfb69f`, `3c01c323`) venaient du
  runtime. `scripts/next-build.sh` reste un outil du VPS ; Vercel construit avec `next build`.
- **ISR** : `revalidate = 3600` sur `/chara/[id]`, `/skill/[id]`, `/item/[id]`, `/equipe/[id]`,
  `/stade/[id]`, `/tactic/[id]`, `dynamicParams = true`. Invalidation on-demand par
  `POST /api/ops/revalidate/wiki` (secret dans les variables Vercel, jamais dans le dépôt).
- **Variables** : `NEXT_PUBLIC_SUPABASE_URL` et `NEXT_PUBLIC_SUPABASE_ANON_KEY` seulement
  côté public ; `SUPABASE_INTERNAL_URL` **n'existe plus** (la cascade `pickUrl()` disparaît
  avec le Proxy). `SQLITE_DB_PATH`, `DATABASE_URL`, `/home/ubuntu` : **aucune occurrence**
  dans `apps/azalee` à la fin de J2 — c'est un gate, pas un vœu.
- **Ce qui quitte le wiki** (vers Aphrody) : `/cpk`, `/textures`, `/modeles`, `/mode`,
  `/sons`, `/videos`, `/avatar`, `/demo`, `/save`, `/vroid`, `api/cpk`, `api/mode-tex`, le
  wasm de 2,1 Mo, `lib/cpk/index.ts`. **Ce qui reste** : `/gallery` (sélection éditoriale de
  360 items, pas le catalogue) et **`app/tools/niers/latest.json/route.ts`** — l'updater
  d'Inacord le lit en dur ; il n'a besoin que de l'API GitHub, donc il est serverless.
- **Redirections** : le wiki répond `308` vers `https://aphrody.com<chemin>` pour les dix
  préfixes ci-dessus, déclarés un par un dans `next.config.ts`, jamais par motif. `/tools`
  n'est **pas** dans la liste.
- **Images** : les vignettes viennent de `cdn-variants` (VPS, `:8805`) avec `?w=&format=webp`
  et un `srcset` ; c'est le levier perf n° 1 (404 `<img>` pleine résolution aujourd'hui) et
  il n'exige aucun code nouveau côté CDN.
- **Design** : la DA Rose Griffon (`app/globals.css`, 109 tokens M3) ne change pas.

## Supabase Cloud

- Projet `kvnlbhatjqqmhhxaxlbi`, région eu-west-3. **MESURÉ** au gel : 224 tables, 1 478
  colonnes, 5 vues, 155 policies + 64 `lecture_publique` ; 65 tables / 165 277 lignes
  chargées par `scripts/ops/load-mirror-to-cloud.sh` (`11ee9e0`), 0 écart. L'inventaire
  local 66 / 165 244 est **À RÉCONCILIER** par manifeste : quelle table manque, pourquoi.
- **RLS reste activée partout.** L'ouverture se fait par policy explicite `for select to
  anon, authenticated using (true)` sur `inagle_*` seulement ; `profiles`, `account`,
  `two_factor`, `audit_logs` et le schéma `auth` restent fermés.
- **Auth** : `better-auth` avec ses tables dans le même Postgres ; JWT validés côté serveur ;
  aucun `service_role` côté client. `DISABLE_SUPABASE_JWT_BRIDGE` reste non défini tant que
  le secret n'est pas resynchronisé.
- **Storage / Realtime** : sur l'origine `*.supabase.co`, jamais sous le domaine du wiki.
  `supabase-compat.inc` (nginx du VPS) meurt avec `azalee-web` à J6.
- **Rafraîchissement des données de jeu** : le miroir nocturne du VPS (`nie-miroir.service`)
  reste la source ; `load-mirror-to-cloud.sh` le pousse (TRUNCATE + COPY, idempotent) puis
  appelle la revalidation. La chaîne est un cron du VPS, pas une fonction Vercel.

## Aphrody : `nie-site` sur `aphrody.com`

**Ce qu'Aphrody n'est pas** (précision de l'utilisateur, 2026-09-05) : ni le wiki — c'est
Azalée — ni l'explorateur de fichiers — c'est Inacord. Son interface reproduit le **menu
principal du jeu**. La disposition n'est pas dessinée : `nie-game --runtime --menu <écran>
--export-layout` l'exporte depuis le jeu, avec pour `mainmenu01` un canevas de 1280×720 et
34 objets portant `transform`, `drawPriority`, sprite et textes déjà traduits.

### Ce que le serveur fait

- **Sert** `apps/nie-web` : `index.html` passe par `askama` pour recevoir titre, description
  et balises `og:` de la route demandée (une texture, un modèle partagé sur un réseau social
  a sa vignette) ; le reste du bundle est servi pré-compressé (`br`, `zstd`), immuable par
  empreinte de fichier.
- **Répond** `/api/v1/*` : DTO `serde` versionnés ; erreurs HTTP stables, sans détail SQL ni
  chemin ; pagination obligatoire (`per_page` ≤ 200) — jamais le catalogue entier (250 800
  fichiers, 53 126 textures) ; lecture des gisements par `rusqlite` en `mode=ro`, requêtes
  préparées, `LIMIT` partout ; le miroir est un lien symbolique daté : réouvrir le handle quand
  l'inode change.
- **Adresse par le VFS** (amendement A3) : l'URL est le chemin du jeu verbatim, en **segment**
  et jamais en query (`/f/data/common/chr/_face/01_IE1/c01000010/c01000010.g4md`), extension du
  jeu conservée, conversion en suffixe ou paramètre explicite. Le slug d'une entité est son code
  de jeu (`c01000010`), **jamais un nom traduit** : 6 168 personnages ne portent que 5 199
  `base_slug` distincts, `unknown` y sert 65 fois. Les noms restent affichés et cherchables,
  jamais adressés. Cela corrige deux verrues mesurées de `nie-model-serve` : `/vfs/*` en
  `?path=` quand les autres routes prennent un segment, et `/tex/` qui exige de retirer
  `.g4tx`.
- **Met en cache** avec `moka` par clé canonique, TTL court sur les listes, long sur les objets
  immuables ; `ETag` = `blake3` du corps ; `Cache-Control: public, max-age,
  stale-while-revalidate`.
- **Proxifie** `nie-model-serve` (`127.0.0.1:8790`) sous `/assets/*` : `tower::limit`
  (débit et concurrence), `timeout` 10 s, taille de réponse bornée, cache des décodages, une
  seule adresse amont configurable. `nie-model-serve` lui-même ne sort plus de `127.0.0.1`.
- **Rend** `/healthz` (JSON), `/robots.txt`, `/.well-known/security.txt`, `/sitemap.xml`,
  et des pages d'erreur `askama` dans la DA du jeu.
- **Sert `/downloads/inacord/latest.json`** : le manifeste de mise à jour d'Inacord, même
  logique que la route du wiki (API GitHub `aphrody-code/nie`, cache 1 h, aucun disque). Les
  nouvelles releases visent `aphrody.com` en premier endpoint ; la route
  `azalee.rosegriffon.fr/tools/niers/latest.json` **reste vivante et redirige ici**, tant
  qu'il existe des installations 0.5.x — leur `tauri.conf.json` la porte en dur.
- **Ne porte aucune marque Rose Griffon** : Aphrody, Inacord et nie sont des projets
  `aphrody-dev`. Ni logo, ni mention, ni lien, ni paquet `@rosegriffon/*`, ni compte partagé.

### La direction artistique du jeu

> **Corrigé le 2026-09-06 — trois chemins de cette section n'existaient pas.** Vérifié par
> `test -d` : `packages/inacord-ui/src/theme/`, `shell/main-menu/` et `shell/inacord/` sont
> **absents**. Les onze composants qu'ils étaient censés contenir, eux, existent — dans
> `shell/main-menu.tsx` et `shell/inacord.tsx`, deux **fichiers** et non deux répertoires. Les
> jetons, eux, vivent dans `shell/game-tokens.css`. Un chemin faux dans un document de stack
> coûte plus qu'une absence : on cherche là où il indique.

Le thème vit dans `packages/inacord-ui/src/shell/` et **se génère** depuis les données du jeu :

| Élément | Source dans le jeu | Comment il arrive dans le thème |
|---|---|---|
| Coquille (shell) | le **menu principal** `mainmenu01` — capture ver. 7.1.2 (`data/design/aphrody-ui-ref-mainmenu-7.1.2.png`, hors dépôt) et sa décomposition dans `docs/DESIGN.md` | composants `SkewTile`, `TileRow`, `HeaderBanner`, `SidePanel`, `TitleBand`, `VersionChip`, `Callout`, `Badge` dans `packages/inacord-ui/src/shell/main-menu.tsx` (Aphrody) ; `PhoneFrame`, `RoomList`, `MessageThread`, `HexBackdrop`, `TabBar` dans `shell/inacord.tsx` (Inacord) |
| Palette de cadrage | capture, quantifiée en 12 couleurs (MESURÉ, ImageMagick) : `#FDFEFE` `#D9EFED` `#A4E4F7` `#46B9F2` `#5BA2E3` `#2F69C7` `#295B9F` `#293D60` `#F6E028` `#D55025` | variables `--shell-*` ; remplacées une à une par la valeur du fichier du jeu quand elle est identifiée |
| Couleurs de texte | `common/font/font_color.cfg.bin` — 70 entrées `FONT_COLOR`, déjà portées (`nie-data::font_color`) | `niers design tokens` écrit `game-tokens.css` : une variable par couleur, sous son nom d'origine |
| Cadres, fonds, boutons | textures de menu (`menu/…`, 40 469 PNG indexés dans `inagle_game_assets`) | servies par `/assets/tex/...` de `nie-site`, référencées en CSS ; jamais copiées dans le dépôt |
| Icônes | atlas déjà exploités par `sprites.css` et `data/re/menu-icon-atlases.txt` | **le générateur existe et n'est pas branché** : `nie_formats::sprite_sheet` transpose un `.g4tx` d'interface en CSS (`background-position` par région) ou en SVG (`<symbol>`), rectangles du jeu **recopiés** jamais recalculés — `depuis_g4tx`, `vers_css`, `vers_svg`, `vers_json`. Côté web, `SpriteIcon.tsx` + `config/sprites.ts` tiennent la même table **à la main**, en parallèle. `gaiji_game.g4tx` = 117 régions |
| Titres | fonte du jeu (`font_def.g4tx` + métriques) | **À VÉRIFIER** : sprites de glyphes CSS depuis l'atlas, ou fonte web la plus proche mesurée sur capture. Ce qui existe déjà côté Rust : `parse_metrics` / `glyph_blitter` de `nie-formats`, et six exemples (`font_catalog`, `font_render`, `font_accents`, `render_text`, `dialogue_scene`) — ils rendent un glyphe en **image**, rien ne va vers `@font-face` |
| Corps de texte, espacements, responsive | pas dans les fichiers | **ESTIMÉ**, corrigé sur capture réelle, jamais de mémoire |

Aucun token, composant ou paquet `@rosegriffon/*` n'entre dans ce thème : Aphrody et Inacord
sont `aphrody-dev`, Azalée garde le sien de son côté.

Inacord charge les mêmes composants avec **sa** coquille : celle d'**InaCord** (イナコード),
l'application de messagerie du téléphone dans le mode histoire du jeu — cadre de téléphone,
panneaux sombres, colonne de salons, fil de messages, accent turquoise, motif hexagonal.
Référence officielle (utilisateur, 2026-09-05) :
`inazuma.jp/victory-road/assets/img/story/story-system/img_photography_01.webp`, 1 280 × 720,
archivée hors dépôt dans `data/design/inazuma-jp-story-photography-01.webp` ; palette
**MESURÉE** (8 couleurs) : `#323544` `#374D5B` `#44484F` `#4FAECC` `#1E67C5` `#07346E`
`#A8CFD2` `#7B8F6B`. Deux coquilles (`shell/main-menu/` pour Aphrody, `shell/inacord/` pour
Inacord), un seul jeu de composants et un seul contrat de données. `@rosegriffon/ui` (tokens
Rose Griffon) sort de `packages/inacord-ui` et reste à Azalée.

### nginx — la bascule du vhost

Aujourd'hui (MESURÉ) : `conf.d/aphrody.com.conf` a **un** bloc `server` pour dix hôtes
(`aphrody.com www api downloads cdn bot admin bxc nie n2b`), tout vers `:8083`, avec
`add_header Content-Security-Policy "default-src 'none'; …"`. La bascule :

1. un bloc `server_name aphrody.com www.aphrody.com` → `proxy_pass http://127.0.0.1:8085`,
   **sans** `add_header Content-Security-Policy` (les CSP s'additionnent, la plus stricte
   gagne, `nie-site` pose la sienne), avec `X-Forwarded-*`, `client_max_body_size` bas,
   `limit_req` et `limit_conn` appliqués ;
2. un bloc pour les huit autres hôtes, **inchangé**, vers `:8083` ;
3. `nie.aphrody.com` → `return 308 https://aphrody.com$request_uri` ;
4. `nginx -t`, puis un `curl -I` par hôte avant et après le `reload` ; le `reload` est un
   acte de production : **go de l'utilisateur**.

Le certificat `letsencrypt/live/aphrody.com` couvre déjà les hôtes ; rien à émettre.
`SITES-PLATFORM.md` du dépôt `aphrody`, qui prévoyait « Niers » sur `nie.aphrody.com`, est à
amender par son propriétaire.

## Déploiement

| Composant | Où | Comment | Rollback |
|---|---|---|---|
| Azalée | Vercel | `vercel deploy` depuis `niers`, preview puis production ; DNS `azalee.rosegriffon.fr` → Vercel à J6 | repointer le DNS sur le VPS, `azalee-web` gardé arrêté-mais-installé 7 jours |
| `nie-site` | VPS, `systemd` | `cargo build --release -p nie-site`, unité `nie-site.service`, `Restart=always`, `MemoryMax` ; vhost `aphrody.com` → `:8085` | vhost → `:8083`, `aphrody-site` jamais arrêté |
| `nie-web` | VPS, fichiers | `bun run build` dans `apps/nie-web`, dossier daté + lien `current` | rebasculer le lien |
| `nie-model-serve` | VPS | inchangé ; vhost public retiré à J6 | — |
| Inacord | GitHub Releases | `scripts/release-desktop.sh <X.Y.Z>` inchangé ; `productName` Inacord | l'updater lit `latest.json`, la release précédente reste publiée |

Aucun composant ne suppose que `127.0.0.1` est le VPS, sauf `nie-site` qui y tourne.
