# Modèles 3D et animation

Tous les modules sont dans `crates/engine/nie-formats/src/`.

| Format | Magic | Extension | Module | Contenu |
|---|---|---|---|---|
| G4MD | `G4MD` (LE ou BE) | `.g4md` | `g4md.rs` | **Métadonnées** de modèle : en-tête, submeshes (records 0x50), table d'attributs vertex (8 o/attribut) |
| G4MG | **aucun** | `.g4mg` | `g4mg.rs` | **Géométrie** : l'offset 0 est directement de la donnée vertex |
| G4SK | `G4SK` | `.g4sk` | `g4sk.rs` | Squelette (hiérarchie de bones) |
| G4MT | `G4MT` | `.g4mt` | `g4mt.rs` | Motion / animation squelettique |
| G4MA | `G4MA` | `.g4ma` | `g4ma.rs` | Animation matérielle |
| G4CM | `G4CM` | `.g4cm` | `g4cm.rs` | Animation de caméra |
| G4LA | `G4LA` | `.g4la` | `g4la.rs` | Animation de lumière |
| G4VS | `G4VS` | `.g4vs` | `g4vs.rs` | Visibilité / états |
| G4PK | `G4PK` | `.g4pk` | `g4pk.rs` | Conteneur de plusieurs sous-fichiers |
| G4PKM | (via G4PK) | `.g4pkm` | `g4pkm.rs`, `g4pkm_motion.rs` | Layout 2D de menu : transforms de chaque bone d'un G4SK encapsulé dans un G4PK |

## Pièges

- **`G4MD` ≠ `G4MG`.** Le module `g4md.rs` teste explicitement `assert!(!is_g4md(b"G4MG"))`.
  Les deux se lisent ensemble : G4MD décrit, G4MG contient les sommets.
- **G4MG n'a pas de magic** : impossible de l'identifier par ses octets de tête. Se fier au
  chemin et au G4MD associé. `detectFormat` ne le reconnaîtra pas.
- **G4MD accepte deux boutismes** (`MAGIC_LE` et `MAGIC_BE`) — ne pas comparer à une seule
  constante.
- **G4PKM** : le magic `G4PK` peut être absent selon le point d'entrée ; lire le module avant de
  supposer un en-tête.

## Assemblage

`assemble.rs` recompose un modèle texturé à partir de ses pièces. Côté service,
`nie-model-serve` expose `/model-full/<code>.glb` (assemblage + texturage), et l'outil MCP
`asset_get` avec `decode: "model"` prend un **code personnage nu** (ex. `c01000010`), pas un
chemin VFS.
