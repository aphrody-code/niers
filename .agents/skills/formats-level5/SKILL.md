---
name: formats-level5
description: Formats de fichiers Level-5 et Criware d'Inazuma Eleven Victory Road — CPK, cfg.bin (RDBN et T2B), G4TX, G4MD, G4MG, G4SK, G4MT, G4MA, G4CM, G4LA, G4VS, G4PK, G4PKM, NAVM/g4nv, COL, LIP, OBJBIN, MEVBIN, NXTCH, HCA, ACB, AWB, USM, CRILAYLA, @UTF, DXBC, font. À charger pour lire, décoder, écrire ou porter un de ces formats, identifier un fichier par son magic, ou comprendre une extension du VFS.
---

# Formats Level-5 / Criware

Le parseur de référence est toujours `crates/engine/nie-formats/src/<format>.rs`. Ce skill dit
lequel ouvrir et ce qui s'y cache ; **il ne remplace pas la lecture du module**, qui seul fait
foi sur la disposition des octets.

## Identifier un fichier

L'extension ne suffit pas — `.g4nv` porte le magic `NAVM`, et deux formats très différents
partagent `.cfg.bin`. Détecter par les octets de tête :

```ts
import { detectFormat } from "nie";
detectFormat(bytes);            // { kind, name }
```

Ou, sans écrire de code : outil MCP `vfs_stat` (donne le format détecté et le mode de décodage),
ou `niers vfs stat <chemin>`.

## Décoder

| Voie | Quand | Comment |
|---|---|---|
| MCP `asset_get` | Explorer, inspecter | `decode: "cfg" \| "tex" \| "raw"` en process ; `"audio"` et `"model"` via `nie-model-serve` |
| `packages/nie` (FFI) | Depuis TypeScript | `decode(bytes)` → JSON ; `decodeToPng(bytes)` pour G4TX |
| Crate Rust | Depuis Rust | `nie_formats::<module>` |
| CLI | En ligne de commande | `niers vfs cat <chemin>` (décodage structuré, repli hexdump) |
| Probes | Explorer une famille | `target/debug/examples/probe_rdbn <prefix>` · `probe_t2b <prefix>` |

## Familles

- **Conteneurs** : CPK, CRILAYLA, @UTF → `references/conteneurs.md`
- **Configuration** : cfg.bin (RDBN et T2B), OBJBIN, MEVBIN → `references/configuration.md`
- **Modèles 3D et animation** : G4MD, G4MG, G4SK, G4MT, G4MA, G4CM, G4LA, G4VS, G4PK, G4PKM →
  `references/modeles-animation.md`
- **Textures et rendu 2D** : G4TX, DXBC, font, raster2d → `references/textures.md`
- **Monde** : NAVM/g4nv, COL, NXTCH → `references/monde.md`
- **Audio et vidéo** : HCA, ACB, AWB, USM → `references/audio-video.md`

Chaque fiche donne, par format : le magic réel, l'extension, le module Rust, ce que le format
contient, et les pièges connus.

## Règles qui valent pour tous

- **Vérifier le magic, pas l'extension.** Les deux divergent dans le VFS.
- **Ne jamais inventer un offset.** Si la disposition n'est pas dans le module, la lire dans le
  fichier (`niers vfs cat`, hexdump) avant d'écrire du code.
- **Un décodage qui « marche » peut être faux** : un `.cfg.bin` lu avec la mauvaise branche
  (RDBN au lieu de T2B) rend des valeurs plausibles et fausses. Contrôler contre le jeu ou un
  golden.
- **Écriture** : seuls quelques formats ont un encodeur (`cpk_encode`, `g4tx_encode`). Pour les
  autres, l'aller-retour n'est pas garanti — le vérifier avant de proposer une écriture.
- **Golden tests** : `cargo test -p nie-data --test <fam>_golden`. Une famille portée sans golden
  n'est pas portée.

## Porter une nouvelle famille de données

La quasi-totalité est déjà portée. Avant d'en ajouter une, **chercher par marqueur, pas par nom
de fichier** — les modules de `nie-data` sont nommés par concept :

```bash
grep -rl "<MARKER_LIST>" crates/engine/nie-data/src/
```
