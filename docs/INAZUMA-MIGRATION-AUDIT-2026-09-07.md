# Audit de migration Inazuma — 2026-09-07

## Résultat

La couche Inazuma est désormais regroupée dans `niers` sans déplacer de dump,
cache SQLite, cookie, secret, profil navigateur ou installation IEVR.

## Correspondances vérifiées

| Surface source | Canonique dans `niers` |
|---|---|
| IETV scraper, cache, parsing officiel/wiki/YouTube, vidéo | `packages/ietv/` |
| Client REST IETV | `packages/ietv-client/` |
| Encyclopédie Zukan côté JS | `packages/zukan/` |
| Encyclopédie Zukan côté Rust et ingestion | `crates/tools/nie-zukan/`, `packages/inagle/src/zukan/` |
| Wonderbot Discord | `packages/wonderbot/`, `apps/bxc/src/wonderbot.ts`, `apps/bxc/src/workflow.ts` |
| Formats/VFS/CPK/CFG IEVR | `crates/engine/nie-formats/`, `crates/engine/nie-data/`, `crates/engine/nie-viola/` |
| RE PE/ELF et analyse binaire | `crates/forge/nie-pe/`, `crates/forge/nie-re/`, `crates/forge/aphrody-re/` |
| Inventaire IEVR, sonde CRI et inspection PE standalone | `crates/tools/ievr-tools/` |

Les fichiers `ievr-tools` ont été intégrés comme outil indépendant : ils ne
remplacent pas `nie-formats::cpk`, qui reste l’implémentation de production du
VFS. Les données consommées par l’outil restent explicitement externes au
dépôt.

Les anciennes entrées `src/cli/{ietv,wonderbot}.ts` et
`src/api/ietv-server.ts` de BXC ont des équivalents adaptés dans `apps/bxc` et
dans les surfaces de service de `niers`; elles ne sont pas copiées avec leurs
imports BXC historiques. Les exemples et fixtures BXC restent des artefacts
de démonstration/test du dépôt source, pas des composants runtime de `niers`.

## Vérification

- `cargo check -p ievr-tools` : OK.
- `cargo test -p ievr-tools` : 11 réussis, 1 test d’intégration IEVR ignoré
  faute de binaire de jeu local.
- Typecheck `packages/ietv`, `packages/wonderbot`, `packages/zukan` : OK.
- Tests IETV/Zukan ciblés : 169 réussis, 0 échec.

Le dépôt cible était déjà en cours de modification avant cette migration ;
aucun fichier étranger n’a été restauré ou écrasé.
