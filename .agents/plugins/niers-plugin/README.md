# Plugin Claude Code — niers

Outillage Claude Code du projet **niers** (réécriture byte-perfect d'*Inazuma Eleven: Victory
Road*) : le serveur MCP du jeu, plus quatre skills qui ancrent les réponses sur les sources du
dépôt au lieu de la mémoire du modèle.

## Contenu

| Composant | Rôle |
|---|---|
| MCP `niers-game` | 14 outils : VFS (255 308 fichiers), assets décodés, base de connaissance RE, lecture du dépôt, pilotage de `nie-explorer`, lancement du jeu |
| Skill `ievr-terminologie` | Empêche d'inventer un format, un code, un symbole ou un chemin — dit où vérifier chaque nature de terme |
| Skill `niers-architecture` | Les 34 crates une par une, les 5 paquets Bun, le C++ `src/` et le C# IECODE — et comment les quatre arbres se répondent |
| Skill `niers-monorepo` | Conventions, emplacements, commandes de build et de test, règles avant commit |
| Skill `formats-level5` | Les formats Level-5 et Criware, par famille, avec magic réel, module Rust et pièges |
| Skill `jouer-ievr` | Lancer, simuler, rendre et observer le jeu — et ce qui n'est **pas** possible |
| Skill `aphrody-pet` | Découvrir et charger le package Aphrody v2, valider son atlas, extraire une frame et choisir une direction |
| Agent `vfs-scout` | Localise des assets dans les 255 308 entrées sans déverser les chemins |
| Agent `forge-analyst` | Diagnostique un plateau de la forge : blocages ventilés, octets divergents, cible chiffrée |
| Agent `re-lookup` | Interroge la KB de reverse-engineering (52 783 fonctions, lecture seule) |
| Agent `port-scout` | Dit si une logique est déjà portée, et dans lequel des quatre arbres |
| Agent `build-doctor` | Vérifie les deux arbres et sépare les régressions des pièges d'environnement connus |

Les agents existent pour les tâches qui, sinon, remplissent la conversation de sorties
intermédiaires : parcourir 255 000 chemins, ventiler des blocages, requêter une base de 52 783
fonctions. Ils rendent une conclusion, pas un journal.

## Installation

```bash
claude --plugin-dir plugins/niers-plugin        # essai local
```

Le serveur MCP est déclaré dans `.mcp.json` et résolu par `${CLAUDE_PLUGIN_ROOT}` : il pointe
sur `apps/nie-mcp/src/index.ts` du dépôt, sans chemin absolu.

## Prérequis

- **Bun ≥ 1.3** et un `bun install` à la racine du dépôt.
- **`libnie_ffi` construite** : `bun run build:ffi` (ou `cargo build -p nie-ffi`). Sans elle, le
  serveur ne peut pas monter les CPK — et, plus largement, toute commande `bun` lancée depuis le
  dépôt échoue, à cause du `preload` de `bunfig.toml`.
- Le VFS du jeu (`data/cpk_list.cfg.bin`). Sur l'installation Steam, c'est la racine du dépôt et
  rien n'est à configurer ; ailleurs, poser `NIE_GAME_DIR`.

Optionnels, chacun avec une dégradation propre :

| Absent | Conséquence |
|---|---|
| `var/niers.sqlite` | `re_query`, `re_function`, `re_coverage` renvoient une erreur explicite |
| `nie-model-serve` | `asset_get` échoue pour `audio` et `model` ; `raw`/`cfg`/`tex` passent en FFI |
| Redis | Sans effet : les CPK locaux sont la voie par défaut |
| `nie-explorer` non lancé | Les outils `explorer_*` disent quoi faire ; le reste fonctionne |

## Variables d'environnement

| Variable | Défaut | Rôle |
|---|---|---|
| `NIERS_REPO` | racine déduite | Racine du dépôt pour `repo_read` |
| `NIE_GAME_DIR` | racine du dépôt | Dossier contenant `data/` |
| `NIERS_SQLITE` | `<repo>/var/niers.sqlite` | Base de connaissance RE |
| `MODEL_SERVE_URL` | `http://127.0.0.1:8790` | Service de décodage audio/modèles |
| `NIERS_BRIDGE_PORT` | `8791` | Port du pont vers `nie-explorer` |
| `NIERS_GAME_EXE` | `nie.exe` | Exécutable lancé par `game_launch` |

## Pourquoi ces skills

Les identifiants d'IEVR sont opaques et se ressemblent : `G4MD` et `G4MG` sont deux formats
distincts, `.g4nv` porte le magic `NAVM`, `.col` porte `PXCL`, et deux formats incompatibles
partagent l'extension `.cfg.bin`. Un nom inventé se propage ensuite dans le code, les commits et
la base de connaissance sans que rien ne le signale. Ces skills ne récitent pas ces faits : ils
donnent la commande qui tranche.
