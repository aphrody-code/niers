# Stack runtime

Les briques du moteur, et la règle qui décide de leur admission.

> **Règle de sélection** : chaque brique sert l'identité au jeu réel (déterminisme, fidélité
> byte/pixel), pas la commodité. Une dépendance qui injecte son propre mixeur, resampler, raster
> ou ordre d'itération est écartée dès qu'elle empêche de matcher l'octet.

Workspace `edition = "2024"`, toolchain épinglée `nightly-2026-05-17`. La toolchain n'affecte pas
les sorties du moteur (IEEE-754 déterministe, pas de fast-math) : un bump ne perturbe aucun golden
byte/pixel, il change seulement le hash du binaire.

## Ce qui est dans l'arbre

Versions lues dans `Cargo.lock`, pas déclarées d'intention.

| Sous-système | Brique | Version | Pourquoi celle-là |
|---|---|---|---|
| GPU / fenêtrage | `wgpu` + `winit` + `pollster` + `bytemuck` | 29.0.3 · 0.30.13 · 0.4.0 · 1.25 | Backend unifié Vulkan/D3D12/Metal/GL/WebGPU ; readback déterministe `texture→buffer` (alignement 256 o) ; `#![forbid(unsafe_code)]` tenable |
| VM Lua | `mlua` (`lua52` + `vendored`) | 0.11.6 | PUC-Rio **5.2.4**, la VM du jeu : mêmes ordre d'itération de tables, coercions `lua_Number`, `bit32`, GC |
| Analyse Lua statique | `tree-sitter` + `tree-sitter-lua` | 0.26 · 0.5 | Lit les scripts décompilés sans les exécuter (la grammaire n'expose qu'un `LanguageFn`) |
| Audio | `cridecoder` + `audio-decode` | — | HCA/ADX/AWB décodés ; le mixeur CRI Atom Ex reste maison |
| Textures | `image_dds` + `bcdec_rs` + `png` | — | BCn/DDS → RGBA8 ; source unique `nie_formats::g4tx_decode` |
| Éditeur de scène | `fyrox` + `fyroxed_base` | — | `nie-editor` seul — hors chemin de fidélité |
| Sérialisation | `serde` + `serde_json` | 1 | Toujours feature-gated, **jamais** sur le chemin de l'octet |
| Base de connaissance | `rusqlite` (bundled) | 0.37 | `var/niers.sqlite` |
| Désassemblage | `iced-x86` + `goblin` | — | `nie-re`, `nie-asm` — pas de dépendance externe à r2/objdump |

### Reverse engineering et Computer Use

La chaîne canonique est `nie-re` (analyse statique et base SQLite) → `nie-trace` (processus vivant)
→ `nie-computer-use` (façade d’orchestration). Les consommateurs sont `nie-cli re`, `nie mem`, les
exemples de dumps et Inacord en lecture de `var/niers.sqlite`. Les fichiers locaux correspondants
sont les crates sous `crates/forge/` et `crates/tools/`; ils correspondent à `origin/main` du dépôt
[aphrody-code/nie](https://github.com/aphrody-code/nie) sur l’audit du 2026-09-07.

Les algorithmes Rust sont conservés. La seule migration recommandée est une couche de session
read-only typée dans `nie-computer-use`, avec hash du binaire, `binary_id`, RVA/VA, backend et
artefact de preuve. La surface d’écriture (`write_*`, recettes, patch EAC) et le lancement de
processus ne doivent pas être admis implicitement. La parité actuelle est partielle : les tests
unitaires passent, mais il manque encore le test Windows live, les fixtures PE/SQLite, le handshake
Ghidra MCP et les tests de limites.

Physique de match, boucle de jeu, skinning, IA, police et compositeur 2D n'ont **aucune
dépendance** : ce sont des ports du décompilé (`nie-core`, `nie-formats::menu`, `nie-formats::font`).

## Deux environnements, un binaire

niers tourne sur un serveur Linux sans GPU (indexation RE, forge, services HTTP) et sur un poste
Windows avec GPU (rendu, capture, jeu). Rien n'est compilé pour l'un au détriment de l'autre :

| | Serveur Linux | Poste Windows |
|---|---|---|
| Backend | Vulkan — lavapipe quand il n'y a pas de matériel | **D3D12** d'abord, Vulkan en repli |
| Adaptateur | l'unique, logiciel | `HighPerformance` → la carte discrète |
| Rendu de référence | le rasteriseur logiciel | `NIE_WGPU_FORCE_FALLBACK=1` pour le reproduire |

Le choix se fait dans `nie-game/src/gpu_select.rs`. Les backends sont essayés **un à un, dans
l'ordre** : passer un masque combiné à `Instance::new` laisse wgpu trancher, et son ordre n'est
pas celui qu'on veut. `NIE_WGPU_BACKEND` (`dx12`, `vulkan`, `metal`, `gl`, `all`) surcharge.

Le pipeline est vérifié byte-identique sur les trois chemins — D3D12 matériel, Vulkan matériel et
rendu logiciel produisent le **même SHA256** de capture. C'est ce qui permet de tenir une gate
pixel sur un serveur sans GPU et de la reproduire sur un poste équipé.

**Aucun chemin de machine n'est compilé dans un binaire.** La racine du jeu se résout à
l'exécution (`nie_formats::vfs::resolve_game_dir` : `NIE_GAME_DIR`, sinon le répertoire courant ou
un ancêtre portant `data/cpk_list.cfg.bin`, sinon le répertoire de l'exécutable) — sur une
installation Steam, la racine du jeu **est** le répertoire courant. Les artefacts régénérables
vivent sous `<racine>/var/`. Les goldens adossés au corpus de dumps passent par
`NIE_GAMEDATA_JSON` et **annoncent leur saut** quand le corpus est absent.

## Ce qui est écarté, et pourquoi

Ces rejets sont doctrinaux : ils tiennent tant que l'objectif byte/pixel tient.

| Écarté | Raison |
|---|---|
| **rapier / parry / salva** | Solveur TGS + broadphase/islands/substepping réinjectent un ordre flottant et un ordonnanceur. « Enhanced determinism » = reproductibilité de rapier, pas identité de `nie.exe`. Le jeu a `gravité 2.0/frame²`, `max_collisions_per_frame=5`, 10 `BallMoveKind` extraits des vftables |
| **bevy_ecs / hecs / legion / specs** (pour le cœur) | Le byte-exact exige des structs 1:1 avec le layout C++ (offsets `0x700`, strides `0x570`) ; un ECS éclate ces structs et casse la correspondance champ-par-champ |
| **rstar / kdtree / bvh / pathfinding / navmesh** | N≤23 joueurs : zéro besoin de perf, et toute structure spatiale réordonne les ex-æquo d'un scan linéaire |
| **fontdue / swash / ttf-parser / rustybuzz** | Aucun `.ttf` livré : le texte est un **atlas bitmap pré-cuit** (`font_def/font.g4tx`, AA bakée). Tout rasteriseur diverge |
| **rend3 / rafx / lyon** | Les menus sont 100 % sprite/atlas : aucun tracé vectoriel au runtime |
| **rust-skia** | Raster propre (AA, sous-pixel, gamma) ≠ raster D3D11 du jeu. Le rendu de référence doit être bit-identique, pas ressemblant |
| **rodio / kira** | Surcouches qui imposent leur mixeur et leur resampler, là où l'identité audio est le PCM du mixeur CRI |
| **openh264 / ffmpeg pour la vidéo** | Le RE prouve qu'aucun chemin H.264 n'existe dans `nie.exe` — c'est VP9 (libvpx via criVvp9) |
| **dssim-core** | AGPL-3.0, incompatible avec la licence MIT du workspace |
| **sdl2 / glfw** | Dépendance C, hostile à wasm |
| **tokio `rt-multi-thread`** sur le chemin de jeu | L'ordonnanceur réordonne et casse le golden. `rt` current-thread seulement |
| **bincode / borsh / speedy** | Imposent leur propre layout binaire : ne matchent ni le format save ni `cfg.bin` |

## Règles de la boucle moteur

- **Timestep fixe.** La logique tourne au tick réel du moteur Lives, les frames longues sont
  bornées, le rendu part d'un état interpolé. C'est la condition du reproductible. Jamais de
  logique pilotée par un delta-time variable.
- **Pipeline de frame** : entrées → update simulation → préparer les données de rendu → présenter.
  Le rendu **lit** l'état et ne contient aucune règle de jeu.
- **Rendu, entrées et physique sont des adaptateurs** autour de la donnée gameplay. La physique
  portée est une boîte noire déterministe avec des points de synchronisation explicites.
- **Tout aléa passe par `lives::CRand`** (MT19937 byte-exact), jamais l'horloge.
- **Validation** : les systèmes se testent comme des fonctions pures sur la donnée ; `nie-headless`
  rejoue la même `step()` sans rendu — c'est la gate de déterminisme. Le rendu se valide par
  égalité d'octets d'abord (hash du RGBA8 dépaddé), tolérance SSIM/PSNR ensuite, jamais l'inverse.

## Règles Lua (`nie-lua`)

- Les fonctions hôtes passent par `lua.create_function` + `globals().set`. Ne jamais toucher la
  pile Lua C : mlua protège chaque longjmp via `lua_pcall`.
- **Erreurs, pas panics** : retourner `Err(mlua::Error…)`, réserver le panic aux invariants
  impossibles.
- Charger un `.lua.bin` exige `Lua::unsafe_new` (un bytecode malformé corrompt la VM). Le bytecode
  du jeu est de confiance, l'usage est isolé dans `nie-lua`, hors `forbid(unsafe_code)`. Vérifier
  la signature `1b 4c 75 61 52` avant.
- **Piège de cycle** : placer un handle Lua dans un `UserData` ou une closure crée un cycle de
  références qui empêche de détruire la VM. Passer par des IDs, pas des handles stockés.
- VM mono-thread : garder mlua `!Send` (pas la feature `send`) et partager par `Rc<RefCell<…>>`.
  Réutiliser une seule `Lua` — la création est coûteuse.
