# Conteneurs et compression

| Format | Magic | Module | Contenu |
|---|---|---|---|
| CPK | `CPK ` (avec espace) | `cpk.rs`, `cpk_encode.rs` | Archive Criware — les packs du jeu (~57 Go) |
| @UTF | `@UTF` | `cpk.rs` | Table de métadonnées Criware, structure interne des CPK et ACB |
| CRILAYLA | `CRILAYLA` | `crilayla.rs` | Compression Criware appliquée aux entrées de CPK |

## Le VFS plutôt que les CPK

Ne pas ouvrir les CPK à la main pour lire un fichier : le VFS (`nie_formats::vfs`) présente les
255 308 fichiers comme une arborescence, résout le pack conteneur et décompresse.

```rust
let vfs = Vfs::init(game_dir.join("data"))?;   // <racine>/data, PAS la racine
```

```ts
import { vfsOpen } from "nie";
using vfs = vfsOpen("./data")!;
vfs.count();                    // 255 308 — total réel
vfs.listAll();                  // index complet, paginé côté Rust (~1 s)
vfs.read("data/…/x.cfg.bin");   // octets bruts
```

`list()` existe encore mais **plafonne à 50 000 entrées et tronque en silence** : préférer
`listAll()` ou `listRange(offset, limit)`.

## Écriture

`cpk_encode.rs` produit un CPK **autonome, non chiffré, non compressé** — c'est ce que
`nie-explorer` utilise pour exporter un mod. Ce n'est pas un CPK identique à ceux du jeu.

## Pièges

- Le magic est `CPK ` avec un **espace** final.
- `Vfs::init()` prend `<racine>/data`, pas la racine — sinon « impossible d'ouvrir
  cpk_list.cfg.bin », message qui n'indique pas la vraie cause.
- Certains CPK sont **absents de `cpk_list.cfg.bin`** (films, sound_asset) : le VFS les découvre
  séparément (`discover_extra_cpks`, compté par `extra_count()`).
- Des fichiers « loose » (CPK vide dans la liste) sont lus depuis le disque, pas depuis un pack.
