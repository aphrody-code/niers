//! **Renderer 3D niers** — charge un GLB réel (modèle reconstruit depuis les CPK par
//! `nie_formats::assemble`) et le rend en **perspective 3D texturée** (rastérisation CPU : z-buffer,
//! backface culling, échantillonnage des atlas PNG embarqués + éclairage Lambert). C'est le maillon
//! « rendu 3D » qui manquait : le vrai jeu est en 3D, et ce module affiche les **vrais maillages et
//! textures** du jeu (pas des primitives abstraites).
//!
//! Deux chemins de rendu, même contrat (`Model` → RGBA8) :
//! - [`render`] — rastériseur **CPU** de référence : headless, déterministe, sans pilote graphique.
//!   C'est lui qui sert de vérité terrain aux tests golden.
//! - [`gpu`] (feature `gpu`) — pipeline **wgpu** : le modèle est téléversé une fois en mémoire GPU
//!   puis chaque image ne coûte qu'un appel de dessin. C'est ce qui rend une caméra manipulable à
//!   la souris possible, là où le CPU convient pour une vignette mais pas pour un viewport.
//!   Mesuré sur RTX 4070 (DX12, release, 1920×1080) : **2,16 ms/image contre 9,38** au CPU.
//!
//! ## Ce que « même contrat » ne dit pas
//!
//! Les deux chemins cadrent la même vue — champ de vision, distance, inclinaison et sens de
//! rotation sont partagés (`render::FOCALE`, `DISTANCE_CAMERA`, `TILT`), et
//! `gpu::tests::gpu_et_cpu_cadrent_la_meme_vue` le vérifie. Ils ne rendent pas les mêmes octets
//! pour autant, et trois écarts sont **assumés** :
//!
//! | | CPU | GPU |
//! |---|---|---|
//! | ombrage | plat, normale de face | lissé, normale de sommet interpolée |
//! | filtrage de texture | plus proche voisin | linéaire |
//! | faces arrière | écartées | conservées (maillages à orientation incohérente) |
//! | fond | dégradé opaque | transparent (l'interface décide) |
//!
//! Comparer les deux se fait donc sur la **silhouette**, pas pixel à pixel :
//! `nie-render3d --gpu --verify` rapporte le recouvrement et le réserve au verdict, l'écart de
//! couleur restant informatif. Les conventions ont divergé une fois sans que rien ne le signale
//! (sens de rotation inversé, champ de vision différent) — d'où ce test.

#![forbid(unsafe_code)]

pub mod document;
pub mod glb;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod render;
pub mod scene;
mod vecmath;
#[cfg(feature = "webgpu")]
pub mod web;
