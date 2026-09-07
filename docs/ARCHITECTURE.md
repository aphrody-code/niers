# Architecture

Quatre implémentations d'IEVR sous une racine. Ce document dit **qui fait quoi**, **par où les
arbres se parlent**, et **ce qu'il ne faut jamais fusionner**.

## Les quatre arbres

| Arbre | Racine | Volume | Build |
|---|---|---|---|
| Rust — moteur + forge | `crates/`, `forge/` | 582 f. / 174 386 l | `cargo` |
| C++ — toolkit iecode | `src/` (+ `third_party/`, `cmake/`) | 595 f. / 88 622 l | `cmake` + vcpkg |
| C# — IECODE | `csharp/` | 230 f. / 46 922 l | `dotnet` (`IECODE.sln`) |
| TypeScript/Bun | `packages/`, `apps/` | 94 f. / 16 315 l | `bun` |

`just all-build` · `just all-test` · `just all-check` pilotent les quatre.

## Doctrine — un rôle, un langage

| Langage | Rôles |
|---|---|
| **C++** | C décompilé → jeu `nie` jouable ; libs sans équivalent (assimp, Bullet) |
| **C#** | dump, pack, memory, conversion de texture |
| **Rust** | la seule CLI, GUI, core lib, wasm, RE, byte-exact |
| **Bun/TS** | MCP, serveur web, types, API, UI |

Règles qui en découlent :

- La conversion de texture C++ est la moins bonne des trois : ne pas l'étendre. Elle ne subsiste
  que pour l'export WebP, qui n'existe nulle part ailleurs.
- La lecture mémoire du process et l'outillage de dump sont **C#** (`csharp/IECODE.Core/Dump`,
  `Native`).
- `nie-formats::g4tx_decode` reste Rust : sans lui, wasm n'a pas d'images.
- Porter une capacité se justifie par la doctrine ou par une contrainte technique (byte-exact,
  wasm, dépendance native) — jamais par le goût du langage.

## La CLI unique

```bash
niers backends        # ce qui est construit, et où
niers cpp <args...>   # → build/<preset>/src/cli/iecode[.exe]
niers cs  <args...>   # → csharp/IECODE.CLI/bin/*/net10.0/iecode.dll
niers decode <src>    # fichier ou arborescence → JSON / PNG (rayon)
```

Les arguments passent tels quels (`--help` compris), le code de sortie du délégué est propagé.
Surcharges : `NIE_IECODE_EXE`, `NIE_IECODE_DLL`. Code : `crates/tools/nie-cli/src/delegate.rs`.

## Les crates Rust

**38 membres** (`cargo metadata --no-deps --format-version 1 | jq '.packages | length'`, mesuré
2026-09-07 : 10 forge + 19 engine + 9 tools), rangés par rôle ci-dessous, colonne `tests` =
`rg -c '#\[test\]' <dossier>` le même jour. `crates/archive/*` (2 crates, hors des 38) est **hors
du workspace** : `nie-engine` en est exclu explicitement (`exclude = […]` dans le `Cargo.toml`
racine — ~15 000 lignes portées des fichiers C décompilés, 434 marqueurs `// EXTERN:`, consommées
par aucune crate vivante) ; `nie-rs` n'a jamais figuré dans `members` (son propre `Cargo.lock`
autonome, origine dans l'outil externe `iecode-re`, pas un livrable niers). Les deux restent en
lecture seule, référence de portage, jamais compilées par `cargo build --workspace`.

### `crates/forge/*` — produire le binaire (10)

| Crate | Rôle | Tests |
|---|---|---:|
| `aphrody-re` | Triage PE/ELF/Mach-O pur Rust (sections, entropie, empreintes) + extraction de chaînes + désassemblage x86 | 0 |
| `nie-pe` | Lecture/écriture byte-exacte du PE64 + découpage du fichier en unités de forge | 24 |
| `nie-asm` | Encodeur x86-64 dialecte MSVC — réassemble les corps depuis `forge/asm/*.s` | 23 |
| `nie-forge` | Boucle `split`/`lift`/`cc`/`build`/`verify`/`report`, mesure la part produite | 33 |
| `nie-re` | RTTI MSVC, indexation goblin/iced-x86, propagation de labels sur le call-graph | 73 |
| `nie-index` | Base de connaissance SQLite (`var/niers.sqlite`) | 4 |
| `nie-seed` | Import du savoir fusionné (index Ghidra, RTTI, formats iecode, hash→nom inagle) | 24 |
| `nie-queue` | Frontière BFS dédupliquée (redis), workers parallèles sur fonctions non résolues | 0 |
| `nie-dump` | Lecture/scan AOB d'un minidump Windows de `nie.exe` | 6 |
| `nie-trace` | RE en direct : lecture de la mémoire d'un `nie.exe` en cours d'exécution | 93 |

### `crates/engine/*` — le moteur (19)

| Crate | Rôle | Tests |
|---|---|---:|
| `nie-formats` | Parsers Level-5 (CPK, cfg.bin, G4*, CriLayla, Criware), `no_std`-friendly | 386 |
| `nie-data` | Modèles de données du jeu (skills, auras, chara_param, items, growth) | 1445 |
| `nie-core` | Logique reversée (ballon, IA tactique, FSM de match, gardien, stats, CRand) | 311 |
| `nie-geom` | Types géométriques POD partagés — source unique `Vec2`/`Vec3` | 9 |
| `nie-lua` | VM Lua 5.2 réelle (mlua, PUC-Rio 5.2.4 vendored) + analyse statique tree-sitter | 107 |
| `nie-camera` | Modèle et contrôleurs de caméra portés (`CCameraCtrl*`), codec G4CM, pilotage live | 33 |
| `nie-app` | Machine à états d'écran (`GameState`) + rendu abstrait (trait `Renderer`) | 17 |
| `nie-game` | Hôte GUI natif wgpu — rend les vrais assets | 24 |
| `nie-render3d` | Renderer 3D : charge un GLB réel et le rend en perspective | 17 |
| `nie-runtime` | Boucle intégrée monde + physique + rendu top-down → frames/MP4 | 6 |
| `nie-play` | Front headless/golden : rejoue `nie-app`, écrit PNG/MP4 déterministes | 0 |
| `nie-headless` | Front headless sans fenêtre, résumé JSON par format | 18 |
| `nie-save` | Déchiffrement, lecture et édition des saves (XOR clé CRC32) | 57 |
| `nie-explore` | Aperçu/description des entrées VFS par format | 41 |
| `nie-viola` | Modding Level-5 (dump/pack/merge/crypto Criware), périmètre outil « Viola » | 51 |
| `nie-ui` | Source unique typée des jetons de design du jeu (OKLCH, géométrie, mouvement) → CSS | 35 |
| `nie-aphrody` | Runtime typé du pet « Codex Aphrody v2 » (atlas RGBA, animations, directions) | 56 |
| `nie-ffi` | Frontière C-ABI — **seul natif chargé côté TS** | 13 |
| `nie-wasm` | Bindings WebAssembly du savoir vérifié | 32 |

### `crates/tools/*` — outillage (9)

| Crate | Rôle | Tests |
|---|---|---:|
| `nie-cli` | Binaire `niers` — la seule CLI utilisateur, pilote aussi la boucle RE et la frontière redis | 24 |
| `nie-site` | Serveur HTTP Aphrody (Axum 0.8) : bundle `nie-web`, `/api/v1`, VFS `/f` `/b`, proxy `nie-model-serve` | 275 |
| `nie-model-serve` | Serveur HTTP live d'assemblage GLB IEVR (corps+face+uniforme depuis CPK, cache disque) | 13 |
| `nie-steam` | Acquisition Steam native (download/dump de dépôts IEVR), remplace SteamKit2 | 35 |
| `nie-zukan` | Ingesteur de l'encyclopédie officielle Level-5 Inagle (JP/FR/EN) | 53 |
| `nie-wiki` | Exploration game-data IEVR depuis le miroir SQLite (personnages, skills, items, équipes) | 0 |
| `nie-editor` | Éditeur 3D NIE natif, viewport GPU partagé DirectX 12/Vulkan/OpenGL | 1 |
| `nie-bench` | Banc d'essai inter-langages : mesure les hot paths Rust, échantillons pour C++/C#/TS | 2 |
| `nie-tasks` | Orchestration de jobs asynchrones annulables/pausables avec progression | 0 |

## Les ponts

| Pont | Sens | Point d'entrée |
|---|---|---|
| `nie-forge cc` | Rust → C | `src/decomp/functions/*.c`, annotés `/* @nie 0x… */` |
| `iecode export-knowledge` | C# → Rust | JSON → `crates/forge/nie-seed/src/format_catalog.rs` |
| `packages/nie` | Rust → TS | `nie_ffi` via `bun:ffi` (préchargé par `bunfig.toml`) — **seul** natif chargé côté TS |
| `niers cpp` / `niers cs` | Rust → C++ / C# | délégation par sous-processus, `crates/tools/nie-cli/src/delegate.rs` |
| `scripts/sync-gamedata.ts` | TS → C# | `dotnet` puis `iecode.dll` |
| `packages/nie-bridge` | TS ↔ TS | protocole `nie-mcp` ↔ `nie-explorer` |

Non ponté : C# ↔ natif (la couche `csharp/IECODE.Core/Native` est du SIMD .NET pur). Le C++
n'expose **aucune FFI** : il ne se parle avec les autres arbres que par la façade CLI, en
sous-processus. `crates/archive/nie-rs` est du décompilé porté en Rust, hors workspace et compilé
par personne : matière de RE, pas un pont.

## Fusions interdites

Quatre duplications sont **volontaires**. Les collapser corrompt le byte-exact en silence, avec
des tests qui restent verts.

1. **`crc32` vs `crc32_nie`** — deux fonctions distinctes. `crc32` (complément final : noms
   `cfg.bin`, clés de fichier CPK, type-id ECS, CRC de save) ≠ `crc32_nie` (accumulateur brut
   sans complément : model-id CPK, lookup g4tx). Les fusionner corrompt silencieusement l'un des
   deux chemins.
2. **`g4sk::mat_mul`** reste scalaire local, jamais glam/FMA : il est validé golden sur fixtures
   réelles (skinning), un réordonnancement f32 casse le golden.
3. **`StatBlock` de `nie-wiki`** (2 segments f64) diverge volontairement de celui de `nie-core`
   (3 segments f32) : le miroir SQLite n'a pas le palier lv30.
4. **Conventions d'axe vertical opposées** — `nie-core` traite `y` comme hauteur, `nie-runtime`
   traite `z` comme hauteur. `nie-geom::Vec3` unifie le *type* mais **pas** la sémantique : chaque
   crate garde sa convention dans son code. Ne jamais convertir implicitement d'un système vers
   l'autre — la similarité de layout ne vaut pas équivalence sémantique. Idem `Vec2` :
   `g4mg::{u,v}` (UV) ≠ `{x,y}` (terrain).

## Contraintes de structure

- `src/CMakeLists.txt` fait un `GLOB_RECURSE` sur tout `src/` pour `iecode_core` : les sous-arbres
  à target propre (`cli`, `tests`, `ffi`, `decomp`, `driver`, `include`) en sont exclus par
  `list(FILTER … EXCLUDE REGEX ".*/src/<nom>/.*")`. En ajouter un sans son filtre met plusieurs
  `main()` dans la lib.
- Bun ne charge **que** `nie_ffi` (Rust). C'est délibéré : `bunfig.toml` précharge `nie-plugin`,
  donc tout natif joint à cette chaîne ferait échouer n'importe quelle commande `bun` du dépôt dès
  qu'il n'est pas construit. Le C++ s'atteint par la CLI (`niers cpp`), jamais en process.
- vcpkg n'est pas installé par défaut : la chaîne C++ ne compile pas tant que `just cpp-bootstrap`
  n'a pas tourné. `just all-check` exclut donc le C++.
