# Bibliothèques de la chaîne « image → SVG → raster »

Versions et dates **mesurées le 2026-09-05** (API crates.io + `gh api repos/…`), pas citées de
mémoire. Rejouer la mesure avant de recopier un numéro :

```bash
curl -s https://crates.io/api/v1/crates/resvg | jq '{max_stable_version,downloads,updated_at}'
gh api repos/linebender/resvg --jq '.pushed_at, .stargazers_count, .license.spdx_id'
```

## Retenus

| Rôle | Crate | Version | Licence | Dernier push |
|---|---|---|---|---|
| Rastérisation SVG | `resvg` | 0.48.1 | Apache-2.0 OR MIT | 2026-08-16 |
| Parse/normalisation SVG → arbre | `usvg` | 0.48.1 | Apache-2.0 OR MIT | 2026-08-16 |
| Back-end 2D CPU | `tiny-skia` | 0.12.0 | **BSD-3-Clause** | 2026-08-20 |
| Types SVG (`d`, transform, couleur) | `svgtypes` | 0.16.1 | MIT OR Apache-2.0 | 2026-05-13 |
| XML | `roxmltree` | 0.21.1 | MIT OR Apache-2.0 | 2026-05-23 |
| Décodage/encodage image | `image` | 0.25.10 | MIT OR Apache-2.0 | 2026-08-28 |
| Couleur Oklab/Oklch typée | `palette` | 0.7.7 | MIT OR Apache-2.0 | 2026-08-29 |
| Parse couleur CSS (`oklch()`, `lab()`) | `csscolorparser` | 0.8.4 | MIT OR Apache-2.0 | 2026-09-05 |
| Quantification de palette (Wu + k-means Oklab, SIMD) | `quantette` | 0.6.0 | MIT OR Apache-2.0 | 2026-09-02 |
| Comparaison perceptuelle (SSIM/MS-SSIM/RMS) | `image-compare` | 0.5.0 | MIT | 2025-08-18 |
| Vectorisation, socle (optionnel) | `visioncortex` | 0.9.3 | MIT OR Apache-2.0 | 2026-08-14 |

`usvg` tire déjà `roxmltree` et `svgtypes` : un seul parseur XML dans l'arbre, ne pas en ajouter
un second.

## Rejetés, et pourquoi

| Candidat | Raison du rejet |
|---|---|
| **`dssim`** 3.5.1 | **AGPL-3.0** — contamine tout binaire distribué. C'est la seule licence non permissive du lot ; `image-compare` rend le même SSIM en MIT. |
| **potrace** | Pas de crate sur crates.io, et l'original est **GPL-2.0** : contamination + binding C. |
| `vtracer` (comme bibliothèque) | Tire `image ^0.23` (un **second** `image` dans l'arbre à côté du 0.25), plus `clap 2` et `pyo3` — c'est un binaire déguisé en crate. Dépendre de `visioncortex` directement. |
| `vello` 0.10.0 | Exige `wgpu`, et ne lit pas le SVG nativement (il faut `vello_svg`). Inutile pour un pipeline hors-écran. |
| `femtovg` 0.27.0 | Exige un contexte OpenGL ; orienté UI temps réel. |
| `kmeans_colors` 0.7.1 | 12 mois sans commit, plus lent que `quantette`. |
| `color_quant` 2.0.0 | NeuQuant seul, aucun contrôle perceptuel. |
| `exoquant` 0.2.0 | Abandonné (dernière publication 2016). |
| `oklab` 1.1.2 | 2 ans sans commit, Oklab seul — ni Oklch, ni gamut mapping. |
| `pixelmatch` (rs) 0.1.0 | Mort (2021, 6 ★). `dify` 0.8.0 (MIT) est l'alternative vivante si l'on veut un **diff** plutôt qu'un score. |
| `svg` (bodoni) 0.18.0 | Acceptable en écriture seule, mais 20 mois sans commit ; `format!` ou `xmlwriter` évite la dette pour émettre des `path`. |
| `sharp` (Bun) 0.35.4 | Binding libvips lourd ; `image` + `resvg` couvrent le besoin en pur Rust. À ne tirer que pour AVIF/HEIF. |

## Points durs

- **La vectorisation est un décalque.** `vtracer`/`visioncortex` ne comprennent rien : ils tracent
  des isolignes après quantification et rendent des milliers de Bézier collés à la grille de
  pixels — SVG lourd, non éditable, escalier sur les dégradés. Acceptable pour un logo plat de
  ≤ 8 couleurs ; jamais pour prétendre « propre ». Depuis un atlas de jeu, un SVG écrit à la main
  bat le tracé automatique dans tous les cas.
- **`tiny-skia` est BSD-3-Clause** (pas MIT/Apache) : permissif, mais impose la clause de
  non-endossement dans les mentions de licence.
- **wasm32** : `resvg`, `usvg`, `tiny-skia`, `image`, `palette` sont pur Rust et y
  compilent. Le texte SVG demande `usvg` + feature `text`, et de charger les fontes à la main
  (retirer `system-fonts`, alimenter `usvg::fontdb` en mémoire).
- **Optimisation SVG** : rien de mûr en Rust. `oxvg` 0.0.7 (MIT, port de svgo, très actif) est le
  plus prometteur mais reste en 0.0.x ; repli `svgo@4.1.0` (MIT) côté Bun.

## Lignes de Cargo.toml

```toml
resvg          = { version = "0.48", default-features = false, features = ["text", "system-fonts"] }
usvg           = { version = "0.48", default-features = false, features = ["text", "system-fonts"] }
tiny-skia      = "0.12"
image          = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }
palette        = { version = "0.7", default-features = false, features = ["std"] }
csscolorparser = "0.8"
image-compare  = "0.5"
visioncortex   = { version = "0.9", optional = true }   # feature `vectoriser`
```

## Ce que la crate utilise réellement

`nie-aphrody` tire `image`, `image-compare`, `palette`, `resvg`/`usvg`/`tiny-skia` et
`visioncortex` (derrière la feature `vectoriser`). **`quantette` n'est pas tiré** : le k-means de
`pixel::mesurer` tient en une cinquantaine de lignes et travaille déjà en Oklab via `palette` —
une dépendance de plus pour la même chose ne se justifiait pas. Le reprendre si la
quantification devient un goulot (il est SIMD) ou s'il faut du dithering.
