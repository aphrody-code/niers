# Versions, licences et maintenance

Pins **gelés le 2026-09-05**. `Cargo.lock` et les manifestes sont l'autorité dès que le code
existe ; ce tableau dit ce qu'on y écrit, et ce qu'on refuse d'y écrire.

## `nie-site` — ce qui entre dans le workspace

| Crate | Version | Licence | Déjà dans `Cargo.lock` ? | Décision |
|---|---|---|---|---|
| `axum` | 0.8.9 | MIT | **non — seul vrai ajout** | serveur HTTP ; `actix-web` rejeté (hors continuité Tokio/Tower) |
| `tokio` | 1.53.1 | MIT | oui | runtime, `rt-multi-thread`, `macros`, `signal` |
| `tower` | 0.5.3 | MIT | oui | `limit`, `timeout`, `buffer` |
| `tower-http` | **0.7.1** | MIT | oui (en 0.6) | `compression-br`, `compression-zstd`, `fs`, `trace`, `set-header`, `limit`. Corrigé le 2026-09-05 : 0.7.1 est sortie le 31/08/2026 et partage le socle d'axum 0.8 (`http` 1.x, `tower` 0.5) — c'est la cible du neuf. Ruptures 0.7.0 à connaître : `Accept-Encoding: *` traité RFC 9110 (**406** possible là où on servait de l'identity), `ServeDir` rend **404 sur un slash final**, `SizeAbove` `u16`→`u64` |
| `hyper` | 1.11.0 | MIT | oui | transitif via Axum |
| `askama` | **0.16.1** | MIT OR Apache-2.0 | non | templates compilés ; `tera`/`minijinja` rejetés (parsing runtime), `maud` rejeté (DSL macro). Corrigé le 2026-09-05 : 0.14 avait **deux majeures** de retard. `askama_axum` est **mort** (`0.5.0+deprecated`) et il n'existe **pas** de feature `with-axum` → `askama_web = { version = "0.16", features = ["axum-0.8"] }` + `#[derive(Template, WebTemplate)]`, ou `IntoResponse` à la main. Ruptures : `#[filter_fn]` obligatoire (0.15), `let`/`set` sans valeur → `decl`/`declare` et bloc dupliqué = erreur (0.16), MSRV 1.88 |
| `moka` | **0.12.16** (plancher) | MIT OR Apache-2.0 | non | cache concurrent TTL/poids ; `lru` rejeté (mono-thread, sans TTL). `default = []` → feature `sync` obligatoire. Plancher 0.12.16 : en deçà, `EvictionPolicy::lru()` pouvait **figer l'éviction définitivement** (croissance non bornée, régression présente depuis 0.12.0) |
| `blake3` | **1.8.7** | CC0-1.0 OR Apache-2.0 | non | ETag ; `sha2` possible mais 3× plus lent sur gros corps. `to_hex()` rend un `ArrayString` (0 alloc) ; `update_mmap_rayon` seulement sous `spawn_blocking` (pool Rayon global) |
| `rusqlite` | **0.40.2**, feature `bundled` (SQLite 3.53.2) | MIT | oui (en 0.37) | lecture seule des gisements ; `sqlx` rejeté (un saut réseau de trop, divergence avec l'explorateur). Rupture 0.38 : `ToSql`/`FromSql` sur `u64`/`usize` **désactivés par défaut**. Les URI `file:…?mode=ro` ne sont honorées **que** si `OpenFlags::SQLITE_OPEN_URI` est passé |
| `reqwest` | 0.13.4, `rustls-tls` | MIT OR Apache-2.0 | oui | client vers `nie-model-serve` ; `native-tls` interdit |
| `zstd` | 0.13.3 | MIT | oui | pré-compression |
| `tracing` / `tracing-subscriber` | 0.1 / **0.3.22** | MIT | oui | 0.3.23+ exclu (bug de packaging documenté) |
| `clap` | 4.6.6, `derive` | MIT OR Apache-2.0 | oui | options du binaire |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 | oui | erreurs typées ; `anyhow` seulement dans `main` |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 | oui | verrous non-async (handle rusqlite) |
| `criterion` | 0.5 | MIT OR Apache-2.0 | non (dev) | benches `benches/routing.rs` |
| `insta` | 1.40 | MIT OR Apache-2.0 | non (dev) | snapshots des réponses |

## `nie-db` — la couche SQL native (amendement A2, hors semaine)

| Crate | Version | Licence | Dans `Cargo.lock` ? | Décision |
|---|---|---|---|---|
| `rusqlite` | **0.40.2**, `bundled` | MIT | oui (en 0.37) | back-end SQLite du trait `DataAdapter` ; 12 crates l'utilisent déjà — le bump est un lot de workspace, pas une décision `nie-db` |
| `sqlx` | **0.9.0**, `postgres,runtime-tokio,tls-rustls-ring-native-roots,macros,migrate` | MIT OR Apache-2.0 | **non** | back-end PostgreSQL ; `query!` vérifie le SQL à la compilation. Rejetés : `tokio-postgres` seul (pas de vérification), `diesel` (modèle bloquant), `@supabase/supabase-js` (PostgREST en HTTP — une couche réseau à supprimer, pas à reproduire) |

Ce n'est pas une contradiction du refus de `sqlx` pour `nie-site` : celui-ci **lit** des
fichiers locaux (`rusqlite` plus direct), `nie-db` **écrit** vers un Postgres distant. Le
client suit la distance à la donnée. `nie-data` ne gagne aucune de ces dépendances : elle
reste le lecteur typé, sans `tokio` ni client SQL.

**sqlx 0.9 plutôt que 0.8 (corrigé le 2026-09-05)** — 0.9.0 est stable depuis le 21/05/2026, la
branche 0.8 est gelée. Ce qui change pour nous, et qui se voit à la première compilation :

- `runtime-tokio-rustls` **n'existe plus** : runtime et TLS sont découplés
  (`runtime-tokio` + `tls-rustls-ring-native-roots`). Ne jamais mélanger deux providers rustls.
- la feature `offline` **n'existe plus** (le support passe par `macros`) ; `cargo sqlx prepare`
  écrit **un fichier par `query!()`** dans `.sqlx/` et `--merged` est supprimé.
- **`AssertSqlSafe`** : tout SQL construit dynamiquement doit être enveloppé. Mécanique — le
  compilateur désigne chaque site — mais c'est la rupture la plus visible.
- `Executor` n'est plus implémenté sur `Transaction`/`PoolConnection` (depuis 0.8) : **`&mut *tx`**.
- MSRV **1.94**, cohérent avec le Cargo ≥ 1.94.1 déjà exigé plus bas.

Pour l'ETL : `UNNEST($1::int8[], …)` en un aller-retour typé, `QueryBuilder::push_values`
plafonné à **65 535 paramètres**, et `copy_in_raw` pour le volume — dont le `finish()` est
obligatoire, sinon la connexion reste bloquée en état COPY.

Toutes compatibles avec la licence du workspace ; aucune GPL/AGPL. `[workspace.lints]`
s'applique : `todo!`, `unimplemented!`, `dbg!` interdits — **aucun squelette non implémenté**,
une route existe quand elle répond et qu'un test compte sa réponse.

## Le wiki — ce qui est gelé côté Bun/Next

| Brique | Version (catalogue racine) | Décision |
|---|---|---|
| Next.js | **16.3.4 (stable)** | corrigé le 2026-09-05 : la canary 16.3.0-canary.37 du catalogue est **derrière** le stable. Runtime **Node** sur Vercel |
| React | 19 (catalogue) | conservé, `reactCompiler: true` |
| Bun | 1.4.0 | outil de build local et de scripts ; **jamais le runtime servi** |
| `@supabase/supabase-js` | catalogue | client anon ; Drizzle SQLite de Codex **écarté** du rendu web |
| `better-auth` | catalogue | auth ; tables dans le Postgres Cloud |

## Le moteur et les clients — gelés, hors semaine

| Brique | Version | Décision |
|---|---|---|
| `wgpu` | **29.0.3** (réel) | 30.0.1 = lot ultérieur, compilé et golden-testé sur D3D12/Vulkan/Metal/WASM |
| `winit` | 0.30.13 | inchangé |
| Tauri | 2.x du dépôt | enveloppe d'**Inacord** (ex `nie-explorer`) ; `productName` → `Inacord`, identifiant `dev.niers.explorer` conservé ; mobile plus tard |
| `steamworks` | 0.13.1 | **absent** du lock, et le reste : feature/crate PC isolée, revue de licence Valve d'abord |
| Leptos, Dioxus, SQLx | — | **n'entrent pas** ; voir l'ADR |

## Maintenance et sécurité Rust

- Edition 2024, toolchain `nightly-2026-05-17` épinglée par `niers` : inchangée par la semaine.
- Cargo ≥ 1.94.1 (CVE-2026-33056) ; `tar` ≥ 0.4.45 pour tout ce qui touche des archives.
- `cargo deny` refuse les licences incompatibles et les advisories ouvertes sur la crate.
- Tokio : les lectures `rusqlite` et les décodages passent par `spawn_blocking` ; rien de
  bloquant sur le chemin HTTP ; toute tâche est annulable par timeout.
- WASM : l'audit des symboles indéfinis Rust 1.96 précède toute promotion de `nie-wasm`.

## Sources primaires

- [Axum](https://github.com/tokio-rs/axum) · [tower-http](https://github.com/tower-rs/tower-http)
- [askama](https://github.com/askama-rs/askama) · [moka](https://github.com/moka-rs/moka) ·
  [blake3](https://github.com/BLAKE3-team/BLAKE3) · [rusqlite](https://github.com/rusqlite/rusqlite)
- [Next.js — Vercel runtime Node](https://nextjs.org/docs) · [Supabase RLS](https://supabase.com/docs/guides/database/postgres/row-level-security)
- [Tauri 2](https://github.com/tauri-apps/tauri) · [wgpu](https://github.com/gfx-rs/wgpu)
- [Steamworks](https://partner.steamgames.com/doc/api) · [steamworks-rs](https://github.com/Noxime/steamworks-rs)

---

## Le front d'Aphrody — état MESURÉ le 2026-09-06

> Ce document ne couvrait que le Rust de `nie-site`. Le front n'y figurait pas, alors qu'il a
> changé de nature ce jour-là : `apps/nie-web` a reçu **Tailwind v4** et monte désormais les
> primitives partagées de `packages/inacord-ui`.

### Ce qui a été ajouté, et pourquoi

| Paquet | Version | Rôle | Alternative écartée |
|---|---|---|---|
| `tailwindcss` | 4.3.x | **débloque les 37 primitives** de `inacord-ui`, écrites en classes Tailwind. Sans lui elles se rendent **sans un seul style**, en silence | écrire un second jeu de composants en CSS inline — c'est ce que faisait chaque écran, et ils divergeaient |
| `@tailwindcss/vite` | 4.3.x | le plugin v4 ; pas de PostCSS, pas de `tailwind.config.js` | `@tailwindcss/postcss` (utilisé par Azalée, qui est en Next) |

Deux pièges, tous deux silencieux, tous deux payés :

1. **`@source` est obligatoire.** Tailwind v4 ne scanne que le paquet courant : sans
   `@source "../../../packages/inacord-ui/src"` dans `base.css`, les classes des primitives ne
   sont **jamais générées**. Le composant se rend, nu.
2. **Trois systèmes de noms de couleurs coexistent** — shadcn (`background`, `muted-foreground`)
   pour les primitives d'Inacord, Material 3 (`surface-container-high`, `on-surface-variant`)
   pour les composants venus du wiki, et les jetons `--jeu-*` du menu du jeu, qui sont la seule
   palette réelle d'Aphrody. Le bloc `@theme inline` de `base.css` fait le pont. Une classe dont
   la couleur n'est pas mappée s'affiche **transparente sur transparent** : visible dans le DOM,
   invisible à l'écran. Vu sur `SearchBar`, qui s'affichait sans cadre à côté de champs qui en
   avaient un.

### Les retards, mesurés par `bun outdated --filter '*'`

| Paquet | Ici | Dernier | Verdict |
|---|---|---|---|
| `typescript` | 5.9.3 | **7.0.2** | **à ne pas monter à l'aveugle** : TS 7 supprime `baseUrl`, et plusieurs `tsconfig` du dépôt s'en servent — l'erreur `TS5101` est déjà constatée sur `bunx tsc` global |
| `vite` | 6.4.3 | **8.2.2** | deux majeures ; `nie-web` et `inacord` ensemble, à mesurer par un build réel |
| `@vitejs/plugin-react` | 4.7.0 | **6.1.1** | suit Vite, même lot |
| `vitest` | 4.1.11 | 5.0.0 | une majeure, périmètre Azalée |
| `jsdom` | 29.1.1 | 30.0.1 | idem |
| `@testing-library/jest-dom` | 6.10.0 | 7.0.1 | idem |
| `next` | 16.3.0-**canary**.37 | 16.3.4 | une canary épinglée en production ; la stable existe |
| `@types/node` | 25.9.5 | 26.4.1 | mineur |
| `@types/bun` | 1.3.14 | 1.4.1 | mineur, 28 paquets concernés |
| `@types/react-dom` | 19.2.4 | 19.2.7 | correctif |
| `@playwright/test` | 1.62.1 | 1.63.0 | mineur |
| `hls.js` | 1.7.1 | 1.7.2 | correctif |
| `libsodium-wrappers` | 0.7.16 | 0.8.4 | une majeure, périmètre bot |

**Ce que ce tableau dit et ne dit pas.** Il dit qu'aucune de ces libs n'est abandonnée ni
vulnérable au sens de `cargo deny`/`bun audit` — ce sont des retards, pas des dettes. Il ne dit
pas qu'il faut toutes les monter : `typescript` 7 casserait `baseUrl` **de façon mesurée**, et
une montée de Vite se juge sur un build réel, pas sur un numéro.
