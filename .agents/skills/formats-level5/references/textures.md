# Textures, shaders et rendu 2D

| Format | Magic | Extension | Module | Contenu |
|---|---|---|---|---|
| G4TX | `G4TX` | `.g4tx` | `g4tx.rs`, `g4tx_decode.rs`, `g4tx_encode.rs` | Texture Level-5 |
| DXBC | `DXBC` (chunk `SHEX`) | `.vfxo`, `.pfxo`, `.cfxo`, `.gfxo` | `dxbc.rs` | Shaders compilés DirectX |
| NXTCH | `NXTCH` | — | `nxtch.rs` | Chunk de texture Switch + déswizzle GOB Tegra X1 |
| font | (via `font.cfg.bin`) | — | `font.rs` | Métriques de glyphes + blit de la police bitmap |
| raster2d | — | — | `raster2d.rs` | Rastérisation 2D |

## Décoder une texture

```ts
import { decodeToPng } from "nie";
const png = decodeToPng(bytes);        // null si ce n'est pas un G4TX décodable
```

Ou l'outil MCP `asset_get` avec `decode: "tex"`.

## La convention `/tex` — piège classique

La route HTTP de `nie-model-serve` **remplace `.png` par `.g4tx`** : lui passer
`…/x.g4tx.png` est une erreur. L'URL correcte est `…/x.png`, et le service va chercher
`…/x.g4tx`.

En voie FFI (décodage en process), il n'y a pas de substitution : le chemin garde son `.g4tx`.
La réponse de `asset_get` indique laquelle des deux voies a servi, via `source: "ffi"` ou
`"model-serve"` — s'y fier plutôt que de supposer.

## Écriture

`g4tx_encode.rs` est un des rares encodeurs du dépôt : l'aller-retour G4TX est possible. Le
vérifier sur un fichier réel avant de s'en servir dans un mod.

## Police

`font.rs` interprète `font.cfg.bin` (métriques) et l'atlas `font.g4tx`. Côté TypeScript :

```ts
using vfs  = vfsOpen("./data")!;
using font = vfs.openFont()!;
const png  = font.renderText("COMMENCER");   // PNG RGBA8
```

Il n'y a **pas** de format `.g4tg` à reverser : c'était une hypothèse fausse, levée le
2026-06-16 (cf. l'en-tête de `font.rs`).

## Switch

`nxtch.rs` (magic `NXTCH`) et le déswizzle Block-Linear Tegra X1 concernent les assets de la
version Switch, pas le build PC dx11.
