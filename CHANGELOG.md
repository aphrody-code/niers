# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions en
[SemVer](https://semver.org/lang/fr/). La version qui fait foi est celle de
`[workspace.package]` dans `Cargo.toml` ; chaque ligne ci-dessous correspond à un tag `vX.Y.Z`
poussé par `scripts/release-desktop.sh`.

Ce fichier résume ce qu'une version a apporté. Il ne remplace pas `git log`, qui reste la
source : chaque section porte le nombre réel de commits de l'intervalle, et rien n'y est
listé qui ne s'y retrouve.

## [Non publié]

## [0.5.11] — 2026-09-07

- Configure l'exécution du CLI BXC natif 0.9.7 depuis niers sans charger le
  preload Bun du monorepo.
- Documente l'installation Windows et la séparation entre le paquet npm BXC
  publié et le binaire standalone.

## [0.5.10] — 2026-09-07

- Regroupe la couche Inazuma/IEVR, IETV, Zukan et Wonderbot dans `niers`.
- Ajoute l’outil standalone `ievr-tools` pour l’inventaire et l’analyse binaire.

### Modifié

- Réorganisation du dépôt sur le modèle de [`openai/codex`](https://github.com/openai/codex) :
  racine ramenée aux fichiers standards, un `README.md` par arbre, fichiers de projet
  attendus par GitHub (`CHANGELOG.md`, `NOTICE`, `SECURITY.md`, `.gitattributes`,
  `.github/`). Voir [`docs/ORGANISATION.md`](docs/ORGANISATION.md).

## [0.5.9] — 2026-09-05

30 commits. Accès partagé au dépôt restauré pour les mods, atelier avatar et pipeline
sauvegardés, orientation des fiches personnage corrigée, `AGENTS.md` promu contexte commun
à tous les agents, lockfile du workspace rafraîchi.

## [0.5.6] — 2026-09-03

11 commits. Explorateur (3 ajouts, 2 correctifs), mode cinéma, catalogue d'épisodes `ietv`,
outillage d'exploitation.

## [0.5.4] — 2026-09-03

22 commits. Forge (5 ajouts) et encodeur `nie-asm` (2), explorateur (4), mode cinéma (4),
FFI, documentation de la forge et des agents.

## [0.5.3] — 2026-09-03

66 commits. Fusion des dépôts (`docs/FUSION.md`), outillage Python, `wonderbot`,
performances de l'explorateur, correctifs image et avatar.

## [0.5.2] — 2026-08-30

215 commits — la version la plus dense. Éditeur d'avatar (`chara_edit`) porté de bout en
bout et documenté (49 documents, 21 ajouts, 10 correctifs), arbre des menus, forge et
reverse-engineering.

## [0.5.1] — 2026-08-14

14 commits. Correctifs de publication, textures, audio, export, pages médias d'azalée.

## [0.5.0] — 2026-08-12

95 commits. CLI `niers`, explorateur Tauri, forge, `nie-lua`, plugin.

## [0.4.0] — 2026-08-08

8 commits. Explorateur, `nie-steam`, correctifs CLI.

## [0.3.0] — 2026-08-08

5 commits. Absorption d'IECODE, licence officielle, `.gitignore` restauré.

## [0.2.0] — 2026-08-08

3 commits. Première version de l'explorateur `nie-explorer`.

## [0.1.0] — 2026-08-07

365 commits — l'amorçage. Familles de données `nie-data` (52), cœur `nie-core` (34),
formats Level-5 `nie-formats` (15), serveur de modèles, premières vagues de reverse.

[Non publié]: https://github.com/aphrody-code/nie/compare/v0.5.10...HEAD
[0.5.10]: https://github.com/aphrody-code/nie/compare/v0.5.9...v0.5.10
[0.5.9]: https://github.com/aphrody-code/nie/compare/v0.5.6...v0.5.9
[0.5.6]: https://github.com/aphrody-code/nie/compare/v0.5.4...v0.5.6
[0.5.4]: https://github.com/aphrody-code/nie/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/aphrody-code/nie/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/aphrody-code/nie/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/aphrody-code/nie/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/aphrody-code/nie/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/aphrody-code/nie/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/aphrody-code/nie/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/aphrody-code/nie/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/aphrody-code/nie/releases/tag/v0.1.0
