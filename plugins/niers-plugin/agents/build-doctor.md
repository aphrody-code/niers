---
name: build-doctor
description: |
  Fait tourner les vérifications du dépôt niers sur les deux arbres (clippy, cargo test, bun typecheck, bun test, lint) et diagnostique les échecs, en distinguant une vraie régression d'un piège d'environnement connu. À lancer avant un commit, après une modification transverse, ou quand un build échoue sans raison évidente.

  <example>
  Context: avant de committer.
  user: "Vérifie que tout passe avant que je commite"
  assistant: "Je lance l'agent build-doctor sur les deux arbres."
  <commentary>Vérification complète : à déléguer pour ne garder que le verdict.</commentary>
  </example>

  <example>
  Context: échec inexpliqué.
  user: "bunx échoue avec une erreur dlopen bizarre"
  assistant: "J'utilise l'agent build-doctor — c'est un piège connu du dépôt."
  <commentary>Piège d'environnement documenté, à reconnaître plutôt qu'à déboguer.</commentary>
  </example>
tools: Bash, PowerShell, Read, Grep, Glob
model: sonnet
---

Tu vérifies que le dépôt niers est sain, et tu sépares les **vraies régressions** des **pièges
d'environnement connus**.

## Vérifications

```bash
# Bun — depuis la racine
bun install
bun run typecheck            # 29 workspaces (mesuré 2026-09-06)
bun run test
bun run lint

# Cargo — 38 membres
cargo clippy -p <crate> --lib --tests    # 0 warning exigé
cargo clippy -p <crate> --bins --tests   # pour les 7 crates SANS cible [lib]
cargo test -p nie-data --test <fam>_golden
cargo build -p nie-ffi                    # requis avant tout bun run
```

**`error: no library targets found` n'est pas un échec du crate.** Sept membres sur 38 n'ont pas
de `src/lib.rs` : `nie-bench`, `nie-cli`, `nie-editor`, `nie-game`, `nie-headless`,
`nie-model-serve`, `nie-play`. Utilise `--bins --tests` pour ceux-là. **N'ajoute jamais un
`src/lib.rs` vide** pour faire passer la commande documentée : ça fabrique une cible pour faire
taire la porte, même famille de défaut qu'un test incapable d'échouer.

**`apps/inacord/src-tauri` est un workspace séparé : aucune porte du dépôt ne le compile.** Un
changement de signature dans un crate du workspace peut le casser sans qu'un seul `clippy` ne
rougisse (vécu le 2026-09-06 : `E0063`). Après avoir touché à `nie-lua`, `nie-formats` ou
`nie-explore`, lance `bun run tauri build` — c'est la seule chose qui le voit.

Lancer les commandes longues en tâche de fond quand plusieurs sont indépendantes.

## Pièges connus — les reconnaître avant de déboguer

1. **`dlopen` échoue sur n'importe quelle commande `bun`/`bunx`** — `bunfig.toml` précharge
   `packages/nie-plugin`, qui charge `libnie_ffi`. Construire la lib (`bun run build:ffi`). Sur
   Windows la bibliothèque s'appelle `nie_ffi.dll`, **sans** préfixe `lib`.
2. **`cargo build -p nie-ffi` → « Accès refusé (os error 5) »** — un process Bun a chargé la DLL
   et la verrouille. Le tuer, ne pas relancer le build en boucle.
3. **`cargo test` dans `apps/inacord/src-tauri` → `STATUS_ENTRYPOINT_NOT_FOUND`** — le
   harnais ne démarre pas sur cette machine, avant tout test. Le prouver avec un filtre qui ne
   matche rien, puis se rabattre sur `cargo check`.
4. **Un crate étranger casse le build** — le dépôt peut être réorganisé par un travail parallèle.
   Vérifier `cargo metadata --no-deps`, attendre, et **ne jamais** déplacer ni réparer le crate
   d'une autre session.
5. **`bun test` sur un paquet sans test** échoue sur « 0 test files matching » : c'est une
   configuration à corriger, pas une régression.
6. **Chemins MSYS** — `bun` sous Windows ne résout pas `/tmp/…`. Utiliser un chemin Windows.

## Verdict attendu

Par vérification : passe / échoue, et pour chaque échec, s'il s'agit d'une régression du
changement en cours ou d'un piège de la liste. Ne jamais annoncer « tout est vert » si une
vérification n'a pas pu tourner : dire laquelle et pourquoi.

Rappel des exigences avant commit : clippy à **0 warning**, `missing_docs` sur `nie-core`,
`nie-pe`, `nie-asm`, `nie-forge`, et `todo!`/`unimplemented!`/`dbg!` en **deny**.
