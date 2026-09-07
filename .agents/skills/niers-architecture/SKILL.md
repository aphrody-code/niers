---
name: niers-architecture
description: Carte complète du projet niers et de l'écosystème IEVR — les 34 crates Rust une par une, les 5 paquets Bun, le toolkit C++ (src/), l'implémentation d'origine C# IECODE (csharp/), et comment ces quatre arbres se répondent. À charger pour savoir quelle crate ou quel paquet fait quoi, où chercher une fonctionnalité, quelle implémentation fait référence, ou avant de créer un nouveau composant.
---

# Architecture du projet niers

Quatre implémentations coexistent, dans un ordre historique qui explique tout : **C# IECODE**
(l'origine) → **C++ `cpp/`** → **Rust `crates/`** (la cible) → **TypeScript** (la surface).

Quand une logique manque en Rust, elle existe presque toujours en amont. Les en-têtes de modules
Rust citent leur source : `//! Port Rust de IECODE.Core/Formats/Level5/G4mdParser.cs`.

> Avant d'écrire un parseur ou de porter une famille, lancer l'agent **port-scout** : la
> quasi-totalité est déjà faite quelque part.

## Rust — 34 crates (32 compilées)

Membres : `crates/forge/*`, `crates/engine/*`, `crates/tools/*`.
`crates/archive/*` (`nie-engine`, `nie-rs`) est **exclu** du build : référence RE en lecture seule.

### `crates/forge/` — produire le binaire (8)

| Crate | Rôle |
|---|---|
| `nie-pe` | Lecture/écriture byte-exacte du PE64 `nie.exe`, découpage du fichier |
| `nie-asm` | Encodeur x86-64 dialecte MSVC — régénère les corps de fonction |
| `nie-forge` | La forge : découpe, régénère, mesure la part produite |
| `nie-re` | Moteur RE : RTTI MSVC, indexation goblin/iced-x86, propagation |
| `nie-index` | Base de connaissance SQLite de la boucle RE |
| `nie-seed` | Import du savoir fusionné (Ghidra, RTTI) |
| `nie-queue` | Frontière BFS dédupliquée (Redis) de la boucle RE |
| `nie-trace` | RE en direct : lecture de la mémoire d'un `nie.exe` vivant |

### `crates/engine/` — le moteur (16)

| Crate | Rôle |
|---|---|
| `nie-core` | Logique de jeu reversée : ballon, IA tactique, états |
| `nie-formats` | Parsers Level-5/Criware (CPK, cfg.bin, G4*, CriLayla…) |
| `nie-data` | Modèles de données portés (skills, auras, personnages…) |
| `nie-geom` | Types géométriques POD (Vec2/Vec3) + math scalaire |
| `nie-app` | Machine à états du jeu + rendu abstrait |
| `nie-runtime` | Boucle intégrée monde + physique + rendu top-down |
| `nie-play` | Front-end headless/golden de la machine à états |
| `nie-game` | Hôte GUI natif wgpu — rend les vrais assets |
| `nie-headless` | Runner CLI : détection de format, simulation de match |
| `nie-render3d` | Renderer 3D : charge un GLB réel issu des CPK |
| `nie-camera` | Caméra IEVR : modèle et contrôleurs portés |
| `nie-lua` | VM Lua 5.2 réelle (mlua, PUC-Rio vendored) |
| `nie-save` | Déchiffrement, lecture et édition des saves |
| `nie-explore` | Aperçu/description des entrées VFS par format |
| `nie-ffi` | Frontière C-ABI : expose la logique à Bun et aux autres langages |
| `nie-wasm` | Bindings WebAssembly du savoir vérifié |

### `crates/tools/` — outillage (8)

| Crate | Rôle |
|---|---|
| `nie-cli` | Binaire `niers` : pilote la boucle RE et explore le VFS |
| `nie-wiki` | Exploration game-data depuis le miroir SQLite |
| `nie-zukan` | Ingesteur de l'encyclopédie officielle Level-5 Inagle |
| `nie-steam` | Acquisition Steam native (download/dump des dépôts) |
| `nie-model-serve` | Serveur HTTP d'assemblage GLB (corps + face + uniforme) |
| `nie-tasks` | Jobs asynchrones annulables avec progression |
| `nie-editor` | Éditeur de scène 3D (Fyrox embarqué) |
| `nie-bench` | Banc d'essai inter-langages des hot paths |

## Bun — 5 paquets

| Paquet | Nom npm | Rôle |
|---|---|---|
| `packages/nie` | `nie` | Bindings FFI de `libnie_ffi` — la porte d'entrée TS vers Rust |
| `packages/nie-bridge` | `@niers/bridge` | Protocole de contrôle MCP ↔ explorateur |
| `packages/nie-plugin` | `nie-plugin` | Plugin Bun d'import des formats (préchargé) |
| `apps/inacord` | `nie-explorer` | Explorateur/éditeur Tauri (React + Rust) |
| `apps/nie-mcp` | `@niers/nie-mcp` | Serveur MCP `niers-game` |

Détail des conventions, des catalogues de versions et des pièges : skill `niers-monorepo`.

## C++ — `src/`

Portage antérieur au Rust, toujours vivant. 343 fichiers dans `src/`, 253 en-têtes.

Sous-systèmes : `archive`, `compression`, `converters`, `crypto`, `db`, `formats`, `gamedata`,
`io`, `modding`, `render`, `services`, `vfs`, `viola` — plus `engine/` et `game/`, qui ont leur
propre target (`iecode_engine`, `iecode_game`).

`src/decomp/` est **la voie B de la forge** : `functions/*.c` annotés `/* @nie 0x… */`, compilés
par MSVC 14.44 en `/O2 /GS- /Gy /Zl`, dont les octets doivent coïncider avec ceux du jeu.

`crates/archive/nie-rs/` contient du Rust généré par transpilation — beaucoup
de fichiers y sont des **emplacements réservés** déclarés `mod` dans `lib.rs`. Leur présence ne
prouve aucun portage, et le crate n'est compilé par personne.

## C# — `csharp/` (IECODE)

**L'implémentation d'origine**, et la référence que citent les ports.

| Projet | Taille |
|---|---|
| `csharp/IECODE.Core/` | 169 fichiers, 35 154 lignes |
| `csharp/IECODE.CLI/` | 39 fichiers, 7 901 lignes |
| `csharp/IECODE.Core.Tests/` | 22 fichiers, 3 867 lignes |

Chemins utiles : `IECODE.Core/Formats/Level5/` (G4*, NXTCH, MEVBIN),
`IECODE.Core/Formats/Menu/` (OBJBIN, G4PKM). Solution `IECODE.sln` à la racine.

Quand un portage Rust est incomplet ou douteux, **c'est ici qu'on tranche** : le C# a servi de
source, ses offsets sont annotés, et ses tests couvrent des cas réels.

## Où chercher, par question

| Question | Aller voir |
|---|---|
| Comment lire un format ? | `nie-formats`, puis `src/formats/`, puis `IECODE.Core/Formats/` |
| Comment est modélisée une donnée de jeu ? | `nie-data` (chercher par **marqueur**, pas par nom) |
| Que fait telle fonction du binaire ? | `var/niers.sqlite` via l'agent **re-lookup** |
| Pourquoi la forge stagne ? | agent **forge-analyst** |
| Où est tel asset ? | agent **vfs-scout** |
| Est-ce déjà porté ? | agent **port-scout** |
| Est-ce que tout compile ? | agent **build-doctor** |
