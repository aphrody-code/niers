# Monde : navigation et collision

| Format | Magic | Extension | Module | Contenu |
|---|---|---|---|---|
| NAVM | `NAVM` | **`.g4nv`** | `navm.rs` | Navmesh |
| PXCL | `PXCL` | `.col` | `col.rs` | Collision — conteneur Level-5 enveloppant du PhysX 3.x *cooked* |

## Pièges

- **`.g4nv` porte le magic `NAVM`.** L'extension et le magic ne coïncident pas ; c'est le cas le
  plus trompeur du VFS. Ne jamais chercher un magic `G4NV`, il n'existe pas.
- **`.col` porte le magic `PXCL`**, pas `COL`. C'est le même en-tête de conteneur Level-5 que
  `g4cm` (cf. `level5.rs`), avec `header_size = 0x30` (48) et `type_id = 0x65`.
- Le contenu utile d'un `.col` est du **PhysX 3.x cooked** : le footer porte une version
  (`s03_1`, `s05_2`…). Le module valide l'enveloppe, il ne décode pas la géométrie PhysX.

## Validation

`col.rs` a été validé à l'octet sur **1 143 `.col` réels** du VFS : magic `PXCL` plus
l'invariant `header_size + data_size == file_size`. Un fichier qui viole cet invariant n'est pas
un `.col` valide — le signaler plutôt que de forcer le parsing.

Emplacements : `common/map/**/*.col` et `dx11/map/**/*.col`.

## En-tête commun Level-5

`level5.rs` porte l'en-tête de conteneur partagé par plusieurs formats (`g4cm`, `col`…). Le lire
avant d'écrire un parseur pour un format inconnu qui commence par un en-tête de 0x30 octets :
c'est probablement le même.
