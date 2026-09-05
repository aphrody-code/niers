---
name: niers-monorepo
description: Navigation et conventions du dépôt niers — où vit quoi entre les 34 crates Cargo (forge, engine, tools, archive) et les workspaces Bun (packages/*, apps/*), quelle commande de build ou de test lancer, comment ajouter un crate ou un paquet, et quelles règles s'appliquent avant de committer. À charger avant de créer un fichier, choisir un emplacement, lancer un build ou un test, ou committer dans ce dépôt.
---

# Monorepo niers — où vit quoi

Deux arbres cohabitent, avec des règles distinctes.

## Cargo — 34 crates rangées par rôle

| Dossier | Rôle | Exemples |
|---|---|---|
| `crates/forge/*` | Produire le binaire + échafaudage RE | `nie-pe`, `nie-asm`, `nie-forge`, `nie-re`, `nie-index`, `nie-seed`, `nie-queue`, `nie-trace` |
| `crates/engine/*` | Le moteur | `nie-core`, `nie-formats`, `nie-data`, `nie-ffi`, `nie-render3d`, `nie-runtime` |
| `crates/tools/*` | Outillage | `nie-cli`, `nie-wiki`, `nie-steam`, `nie-model-serve`, `nie-tasks` |
| `crates/archive/*` | **Hors build**, référence seule | `nie-engine` |

`apps/inacord/src-tauri` est un package Cargo **volontairement hors du workspace** (table
`[workspace]` vide) : il réutilise 14 crates niers par chemin relatif. Les exclusions y sont
documentées et vérifiées — notamment tout ce qui dépend de `rusqlite` (conflit de lien natif
`links = "sqlite3"` avec le `sqlx-sqlite` de `tauri-plugin-sql`). Ne pas « réparer » ces
exclusions sans relire le commentaire.

## Bun — workspaces `packages/*` et `apps/*`

| Paquet | Rôle |
|---|---|
| `packages/nie` | Bindings Bun FFI de `libnie_ffi` — la porte d'entrée TS vers les crates Rust |
| `packages/nie-bridge` | Protocole de contrôle partagé entre le serveur MCP et l'explorateur |
| `packages/nie-plugin` | Plugin Bun d'import des formats de jeu (préchargé par `bunfig.toml`) |
| `apps/inacord` | Explorateur/éditeur Tauri |
| `apps/nie-mcp` | Serveur MCP `niers-game` |

Règle d'emplacement : **une bibliothèque va dans `packages/`, une application avec un `bin` va
dans `apps/`**. `tools/` héberge l'outillage hors workspace Bun (addon Blender Python, ce
plugin). `var/` est gitignoré — vendoring et données, jamais un workspace.

Versions partagées : catalogue `catalog` (typescript, `@types/bun`) et catalogue nommé
`catalogs.mcp` (SDK MCP, zod) dans le `package.json` racine. Référencer par `catalog:` ou
`catalog:mcp`, **jamais** une version en dur — c'est ce qui avait fait cohabiter trois
TypeScript et deux zod.

## Commandes

```bash
# Bun — depuis la racine
bun install                       # un seul lockfile, à la racine
bun run typecheck                 # tous les workspaces
bun run test
bun run lint
bun run build:ffi                 # cargo build -p nie-ffi — requis avant tout bun run

# Cargo
cargo clippy -p <crate> --lib --tests    # 0 warning exigé avant commit
cargo test -p nie-data --test <fam>_golden
just forge                        # split → lift → cc → build → verify → report

# Bindings Tauri, sans ouvrir de fenêtre
cd apps/inacord/src-tauri && cargo run --bin export-bindings --features dev-bindings
```

Python : **toujours** `uv run`. Jamais `python` ni `python3`.

## Avant de committer

- `cargo clippy -p <crate> --lib --tests` → **0 warning**.
- `nie-core`, `nie-pe`, `nie-asm`, `nie-forge` : `#![warn(missing_docs)]` — documenter chaque
  item `pub`.
- Lints workspace : `todo!`, `unimplemented!`, `dbg_macro` sont **deny**.
- Jamais de branche ni de PR : `add` + `commit` + `push` directement sur `main`.
- Jamais de trailer `Co-Authored-By: Claude`, jamais de footer « Generated with Claude Code ».
- Ne jamais committer `data/` ni `var/` (assets © LEVEL-5).

## Pièges du dépôt

- **`bunfig.toml` précharge `packages/nie-plugin`**, qui `dlopen` `libnie_ffi`. Si la
  bibliothèque manque, **toute** commande `bun`/`bunx` lancée depuis le dépôt échoue, même sans
  rapport avec le jeu. Construire la lib avant de chercher ailleurs.
- Un process Bun ayant chargé la DLL la **verrouille** : `cargo build -p nie-ffi` échoue alors
  sur « Accès refusé (os error 5) ». Tuer le process, pas relancer le build.
- `cargo test` dans `apps/inacord/src-tauri` **ne démarre pas** sur la machine de
  développement (`STATUS_ENTRYPOINT_NOT_FOUND`, avant tout test). Le vérifier avec un filtre qui
  ne matche rien avant d'accuser son propre code ; `cargo check` reste fiable.
- Le dépôt peut être réorganisé **pendant** une session par un travail parallèle : si un build
  échoue sur un crate étranger, vérifier `cargo metadata --no-deps`, attendre, et ne jamais
  déplacer ni réparer le crate d'une autre session.
- Ne jamais écrire un fichier Rust via un heredoc Python (un `\0` finit dans la source).
- Ne pas nommer un script du scratchpad comme un module stdlib (`dis.py` casse capstone).
