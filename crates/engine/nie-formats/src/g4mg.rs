//! Extraction de géométrie G4MG (Level-5 « Graphics 4 Mesh Geometry »).
//!
//! Port Rust de `IECODE.Core/Formats/Level5/G4mgParser.cs` (`ExtractGeometry` L178,
//! `DecodeNormal` L364, `DecodeUv` L392, `Snorm16` L415, `Vec3ByteSize`/`Vec2ByteSize`
//! L341/L352). Commits de référence : `e59f0b3` (normales SNORM16 + UV0) et `e444a88`.
//!
//! ## Les .g4mg réels sont SANS EN-TÊTE
//!
//! L'offset 0 d'un .g4mg est directement de la donnée vertex (un float, pas de magic). TOUTES
//! les métadonnées (stride, offsets/comptes des buffers, table d'attributs, sous-mailles)
//! viennent du `.g4md` compagnon de même basename. On ne peut donc PAS décoder un .g4mg seul :
//! [`extract_geometry`] prend un [`crate::g4md::G4md`] déjà parsé.
//!
//! ## Règles de décodage (vérifiées sur chr/_uniform u11130090)
//!
//! - **stride** = `submesh.stride` (sinon dérivé `face_data_base / total_verts`) ;
//! - **positions** = float3 LE à +0 de chaque vertex (`v_offset + i*stride`) ;
//! - **normale** (vtype=2) à son offset réel : float32×3, ou SNORM16 short×3 → `max(s/32767,-1)`,
//!   renormalisée ;
//! - **UV0** (vtype=10) : float32×2, ushort UNORM16 → `u/65535`, short SNORM16 → `s/32767` ;
//! - **indices** à `face_data_base + submesh.index_offset`, u16 par défaut, u32 si
//!   `vertex_count > 65535` ; ils sont normalisés en sortie en indices locaux, y compris quand
//!   un fichier réel les exprime dans l'espace vertex global.
//!
//! On ne fabrique JAMAIS de normale/UV si l'attribut est absent ou ne tient pas dans le stride
//! (liste vide → le writer GLB n'émet pas l'accessor correspondant).
//!
//! Référence externe : loader Noesis `inazuma_switch.py` (AFGRocha), seul loader IEVR public.
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::g4md::{G4md, VertexAttribute};

/// Vecteur 3 composantes (positions / normales).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec3 {
    /// Composante X.
    pub x: f32,
    /// Composante Y.
    pub y: f32,
    /// Composante Z.
    pub z: f32,
}

/// Vecteur 2 composantes (UV).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    /// Composante U.
    pub u: f32,
    /// Composante V.
    pub v: f32,
}

/// Vecteur 4 composantes (couleurs).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec4 {
    /// Composante X (R).
    pub x: f32,
    /// Composante Y (G).
    pub y: f32,
    /// Composante Z (B).
    pub z: f32,
    /// Composante W (A).
    pub w: f32,
}

/// Géométrie d'une sous-maille extraite.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubmeshGeometry {
    /// Index de la sous-maille dans le .g4md.
    pub index: usize,
    /// Nombre de vertices.
    pub vertex_count: usize,
    /// Stride effectif (octets).
    pub stride: usize,
    /// Index de matériau.
    pub material_index: u8,
    /// Indices 32 bits ? (vertex_count > 65535).
    pub index32: bool,
    /// Positions (float3 à +0).
    pub positions: Vec<Vec3>,
    /// Normales décodées (vide si attribut absent/illisible).
    pub normals: Vec<Vec3>,
    /// UV0 décodés (vide si attribut absent/illisible).
    pub uv0: Vec<Vec2>,
    /// Couleurs décodées (vide si attribut absent/illisible).
    pub colors: Vec<Vec4>,
    /// Indices locaux à la sous-maille.
    pub indices: Vec<u32>,
}

/// Skinning par vertex : jusqu'à 8 influences (os + poids). Décodé byte-exact (example
/// `validate_skin`, c01000010 : 880/880 vertices poids = 1.0). WEIGHTS = `8× u16` UNORM (vtype 5),
/// INDICES = `8× u8` (vtype 6) ; stride perso skinné = 68.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VertexSkin {
    /// Indices d'os (LOCAUX au mesh — la palette os local→global g4sk reste à valider).
    pub bones: [u8; 8],
    /// Poids associés (somment à ≈1).
    pub weights: [f32; 8],
}

/// Extrait le skinning par vertex d'une sous-maille (`None` si le mesh n'a pas d'attributs
/// WEIGHTS/INDICES, ou si les données sortent des limites).
#[must_use]
pub fn extract_skin(g4mg: &[u8], g4md: &G4md, submesh: usize) -> Option<Vec<VertexSkin>> {
    let sm = g4md.submeshes.get(submesh)?;
    // Le layout propre à la sous-maille : sur `c01001900`, la chevelure (layout 2) et les yeux
    // (layout 0) partagent les offsets de skinning, mais rien ne garantit ce partage ailleurs.
    let w = g4md.find_attribute_of(sm, 5)?;
    let idx = g4md.find_attribute_of(sm, 6)?;
    // stride perso skinné = 68 ; le champ historique vaut souvent 0, le champ réel (+0x3E) le
    // porte ; à défaut des deux, plancher documenté.
    let declared = sm.declared_stride();
    let stride = if declared >= 12 { declared } else { 68 };
    // Nombre d'influences réellement stockées : les tranches WEIGHTS (u16) et INDICES (u8)
    // peuvent être plus courtes que 8 sur certains layouts. Les emplacements au-delà restent à
    // poids 0.
    let n_w = (g4md.attribute_extent(sm, w, stride) / 2).min(8);
    let n_i = g4md.attribute_extent(sm, idx, stride).min(8);
    let n = n_w.min(n_i);
    if n == 0 {
        return None;
    }
    let vbase = sm.vertex_offset as usize;
    let (wo0, io0) = (w.offset as usize, idx.offset as usize);
    let mut out = Vec::with_capacity(sm.vertex_count as usize);
    for v in 0..sm.vertex_count as usize {
        let vo = vbase + v * stride;
        let (wo, io) = (vo + wo0, vo + io0);
        if wo + n * 2 > g4mg.len() || io + n > g4mg.len() {
            return None;
        }
        let weights: [f32; 8] = core::array::from_fn(|k| {
            if k < n {
                f32::from(u16::from_le_bytes([g4mg[wo + k * 2], g4mg[wo + k * 2 + 1]])) / 65535.0
            } else {
                0.0
            }
        });
        let bones: [u8; 8] = core::array::from_fn(|k| if k < n { g4mg[io + k] } else { 0 });
        out.push(VertexSkin { bones, weights });
    }
    Some(out)
}

/// Extrait la géométrie de toutes les sous-mailles d'un .g4mg piloté par le .g4md compagnon.
#[must_use]
pub fn extract_geometry(g4mg: &[u8], g4md: &G4md) -> Vec<SubmeshGeometry> {
    let face_data_base = g4md.header.face_data_base as usize;
    let normal_attr = g4md.find_attribute(2);
    let uv_attr = g4md.find_attribute(10);
    let color_attr = g4md.find_attribute(8);

    // Stride dérivé : région vertex [0, face_data_base) / nombre total de vertices.
    let total_verts: usize = g4md.submeshes.iter().map(|s| s.vertex_count as usize).sum();
    let derived_stride = if face_data_base > 0 && total_verts > 0 {
        face_data_base / total_verts
    } else {
        0
    };

    // `total_verts` n'est fiable que si les vertex_offsets sont DISTINCTS. Sur les meshes de menu,
    // des submeshes PARTAGENT leurs vertices (offsets répétés) → `total_verts` double-compte →
    // `derived_stride` trop petit, d'où le plancher `attr_extent`. Sur les **maps**, les offsets
    // sont distincts et chaque submesh peut avoir son PROPRE stride (36/40/48 selon le format) :
    // on le dérive alors de l'écart au prochain vertex_offset (pavage exact, sans padding).
    let mut order: Vec<usize> = (0..g4md.submeshes.len()).collect();
    order.sort_by_key(|&i| g4md.submeshes[i].vertex_offset);
    let shared_vertices = order
        .windows(2)
        .any(|w| g4md.submeshes[w[0]].vertex_offset == g4md.submeshes[w[1]].vertex_offset);
    // Stride par submesh = (vertex_offset suivant − courant) / vertex_count (borne finale =
    // face_data_base, début du bloc d'index). 0 si indéterminable.
    let mut gap_stride = vec![0usize; g4md.submeshes.len()];
    for (k, &i) in order.iter().enumerate() {
        let cnt = g4md.submeshes[i].vertex_count as usize;
        if cnt == 0 {
            continue;
        }
        let next = order.get(k + 1).map_or(face_data_base, |&j| {
            g4md.submeshes[j].vertex_offset as usize
        });
        gap_stride[i] = next.saturating_sub(g4md.submeshes[i].vertex_offset as usize) / cnt;
    }

    // Extent minimal d'un vertex = max(offset + taille) sur les attributs (position float3 @0 = 12 ;
    // normale vec3 ; UV vec2 ; couleur vec4). Le stride NE PEUT PAS être inférieur. Or sur les meshes
    // de menu MULTI-submesh, `total_verts` double-compte les vertices PARTAGÉS entre submeshes (ex.
    // title02_00 : `vertex_offset` répétés [0,124,236,236,124]) → `derived_stride = 384/20 = 19 < 32`
    // (UV0 @24 + 8). On borne donc le stride dérivé par cet extent (vrai stride confirmé = 32 via dump
    // octets : submesh[0] = quad propre (1,0)(0,0)(0,-1)(1,-1)). **Divergence ASSUMÉE d'iecode** —
    // RE originale, ce layout multi-submesh étant non résolu dans iecode (cf. DESIGN.md §6). Sans
    // effet sur les meshes de perso / mono-submesh (où `derived_stride >= attr_extent`).
    let attr_extent = {
        let mut e = 12usize;
        if let Some(a) = normal_attr {
            e = e.max(a.offset as usize + vec3_byte_size(a.datatype));
        }
        if let Some(a) = uv_attr {
            e = e.max(a.offset as usize + vec2_byte_size(a.datatype));
        }
        if let Some(a) = color_attr {
            e = e.max(a.offset as usize + vec4_byte_size(a.datatype));
        }
        e
    };

    let mut out = Vec::with_capacity(g4md.submeshes.len());
    let mut global_bases = Vec::with_capacity(g4md.submeshes.len());

    for (idx, sm) in g4md.submeshes.iter().enumerate() {
        let vertex_count = sm.vertex_count as usize;
        if vertex_count == 0 {
            continue;
        }

        // Priorité au stride déclaré : champ historique (+0x2E, fixtures et maps), puis champ réel
        // (+0x3E, tous les modèles de personnage mesurés) ; les dérivations ne servent qu'à défaut.
        let stride = if sm.declared_stride() >= 12 {
            sm.declared_stride()
        } else if shared_vertices {
            derived_stride.max(attr_extent) // vertices partagés (menu) : derived sous-estime
        } else if gap_stride[idx] >= 12 {
            gap_stride[idx] // offsets distincts (maps/perso) : stride réel par l'écart au suivant
        } else {
            derived_stride.max(attr_extent)
        };
        if stride < 12 {
            continue; // pas de place pour une position float3
        }

        let v_offset = sm.vertex_offset as usize;
        let v_end = v_offset + vertex_count * stride;
        if v_end > g4mg.len() {
            continue;
        }

        // Décodage conditionnel des normales/UV/couleurs : l'attribut doit tenir dans le stride.
        // Le layout propre à la sous-maille prime sur le premier layout du fichier.
        let normal_attr = g4md.find_attribute_of(sm, 2).or(normal_attr);
        let uv_attr = g4md.find_attribute_of(sm, 10).or(uv_attr).map(|a| {
            // `datatype = 2` est ambigu : ushort×2 sur les maps (4 octets), float×2 sur les
            // personnages (`u011001` layout 1 : UV @0x40, stride 72 → 8 octets). Ce n'est pas
            // le code qui tranche mais la place réservée dans le vertex : huit octets ou plus,
            // c'est du float. Lu en ushort, le short de Byron avait tous ses V dans une bande de
            // 1 % de la planche — chaussettes et nœud disparus.
            if a.datatype == 2 && g4md.attribute_extent(sm, a, stride) >= 8 {
                VertexAttribute { datatype: 3, ..a }
            } else {
                a
            }
        });
        let color_attr = g4md.find_attribute_of(sm, 8).or(color_attr);
        let decode_normal = normal_attr.filter(|a| {
            let sz = vec3_byte_size(a.datatype);
            sz > 0 && a.offset as usize + sz <= stride
        });
        let decode_uv = uv_attr.filter(|a| {
            let sz = vec2_byte_size(a.datatype);
            sz > 0 && a.offset as usize + sz <= stride
        });
        let decode_color = color_attr.filter(|a| {
            let sz = vec4_byte_size(a.datatype);
            sz > 0 && a.offset as usize + sz <= stride
        });

        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(if decode_normal.is_some() {
            vertex_count
        } else {
            0
        });
        let mut uv0 = Vec::with_capacity(if decode_uv.is_some() { vertex_count } else { 0 });
        let mut colors = Vec::with_capacity(if decode_color.is_some() {
            vertex_count
        } else {
            0
        });

        for i in 0..vertex_count {
            let p = v_offset + i * stride;
            positions.push(Vec3 {
                x: sanitize(read_f32(g4mg, p)),
                y: sanitize(read_f32(g4mg, p + 4)),
                z: sanitize(read_f32(g4mg, p + 8)),
            });
            if let Some(a) = decode_normal {
                normals.push(decode_normal_at(g4mg, p + a.offset as usize, a.datatype));
            }
            if let Some(a) = decode_uv {
                uv0.push(decode_uv_at(g4mg, p + a.offset as usize, a.datatype));
            }
            if let Some(a) = decode_color {
                colors.push(decode_color_at(g4mg, p + a.offset as usize, a.datatype));
            }
        }

        // Indices.
        let index_count = sm.index_count as usize;
        let index32 = vertex_count > 65535;
        let idx_size = if index32 { 4 } else { 2 };
        let i_offset = face_data_base + sm.index_offset as usize;
        let i_end = i_offset + index_count * idx_size;

        let mut indices = Vec::with_capacity(index_count);
        if index_count > 0 && i_end <= g4mg.len() {
            for i in 0..index_count {
                let p = i_offset + i * idx_size;
                let v = if index32 {
                    read_u32(g4mg, p)
                } else {
                    u32::from(read_u16(g4mg, p))
                };
                indices.push(v);
            }
        }
        // Les fichiers multi-mailles de keshin (et certains objets) stockent les indices dans
        // l'espace vertex GLOBAL du G4MG. Les positions que cette fonction expose sont, elles,
        // locales à chaque sous-maille : le GLB doit donc recevoir le même index rebasé. On ne
        // corrige que la signature mesurée [base, base+count) ; un index réellement corrompu
        // reste visible et sera rejeté par le parseur GLB au lieu d'être masqué.
        let vertex_base =
            (stride > 0 && v_offset.is_multiple_of(stride)).then_some(v_offset / stride);

        out.push(SubmeshGeometry {
            index: idx,
            vertex_count,
            stride,
            material_index: sm.material_index,
            index32,
            positions,
            normals,
            uv0,
            colors,
            indices,
        });
        global_bases.push(vertex_base);
    }

    compact_global_indices(&mut out, &global_bases);
    out
}

/// Compacte les indices qui référencent l'espace vertex global du G4MG.
///
/// Les buffers exposés par [`extract_geometry`] sont locaux à chaque sous-maille, mais certains
/// G4MG (notamment les meshes keshin) écrivent des indices vers n'importe quelle sous-maille.
/// Quand une liste n'est pas locale, on reconstruit seulement les sommets effectivement référencés
/// et on réécrit la liste en indices locaux. Un triangle dont une référence reste introuvable est
/// écarté afin de ne jamais émettre un GLB contenant un indice hors de son accessor POSITION.
fn compact_global_indices(geometries: &mut [SubmeshGeometry], global_bases: &[Option<usize>]) {
    let mut global_vertices = BTreeMap::new();
    let source_geometries = geometries.to_vec();
    for (geometry_index, geometry) in source_geometries.iter().enumerate() {
        let Some(&Some(base)) = global_bases.get(geometry_index) else {
            continue;
        };
        for local_index in 0..geometry.positions.len() {
            global_vertices.insert(base + local_index, (geometry_index, local_index));
        }
    }

    for geometry_index in 0..geometries.len() {
        let geometry = &source_geometries[geometry_index];
        if geometry.indices.is_empty()
            || geometry.indices.iter().all(|&index| {
                usize::try_from(index).is_ok_and(|index| index < geometry.positions.len())
            })
        {
            continue;
        }
        let old = geometry.clone();
        let mut remap = BTreeMap::new();
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uv0 = Vec::new();
        let mut colors = Vec::new();
        let has_normals = !old.normals.is_empty();
        let has_uv0 = !old.uv0.is_empty();
        let has_colors = !old.colors.is_empty();
        let mut indices = Vec::with_capacity(old.indices.len());

        for triangle in old.indices.chunks(3) {
            if triangle.len() != 3 {
                continue;
            }
            let references = triangle.iter().map(|&index| {
                usize::try_from(index).ok().and_then(|index| {
                    if index < old.positions.len() {
                        Some((geometry_index, index))
                    } else {
                        global_vertices.get(&index).copied()
                    }
                })
            });
            let Some(references) = references.collect::<Option<Vec<_>>>() else {
                continue;
            };
            for (source_geometry_index, source_local_index) in references {
                let vertex_key = (source_geometry_index, source_local_index);
                let local_index = if let Some(&local_index) = remap.get(&vertex_key) {
                    local_index
                } else {
                    let source = &source_geometries[source_geometry_index];
                    let local_index = positions.len() as u32;
                    positions.push(source.positions[source_local_index]);
                    if has_normals {
                        normals.push(source.normals.get(source_local_index).copied().unwrap_or(
                            Vec3 {
                                x: 0.0,
                                y: 0.0,
                                z: 1.0,
                            },
                        ));
                    }
                    if has_uv0 {
                        uv0.push(
                            source
                                .uv0
                                .get(source_local_index)
                                .copied()
                                .unwrap_or(Vec2 { u: 0.0, v: 0.0 }),
                        );
                    }
                    if has_colors {
                        colors.push(source.colors.get(source_local_index).copied().unwrap_or(
                            Vec4 {
                                x: 1.0,
                                y: 1.0,
                                z: 1.0,
                                w: 1.0,
                            },
                        ));
                    }
                    remap.insert(vertex_key, local_index);
                    local_index
                };
                indices.push(local_index);
            }
        }

        let target = &mut geometries[geometry_index];
        target.vertex_count = positions.len();
        target.index32 = positions.len() > 65535;
        target.positions = positions;
        target.normals = normals;
        target.uv0 = uv0;
        target.colors = colors;
        target.indices = indices;
    }
}

/// Nom de texture base-color pour la sous-maille (via `material_index` → `material_base_names`).
#[must_use]
pub fn material_base_name<'a>(g4md: &'a G4md, sm: &SubmeshGeometry) -> Option<&'a String> {
    g4md.material_base_names.get(sm.material_index as usize)
}

/// Taille (octets) d'un vecteur 4 composantes (2/3=float→16 ; 12=ubyte4→4 ; 14/18/20=short/ushort→8).
#[must_use]
pub fn vec4_byte_size(datatype: u32) -> usize {
    match datatype {
        2 | 3 => 16,
        12 => 4,
        14 | 18 | 20 => 8,
        _ => 0,
    }
}

/// Taille (octets) d'un vecteur 3 composantes (2/3=float→12 ; 18/20=short SNORM16→6).
#[must_use]
pub fn vec3_byte_size(datatype: u32) -> usize {
    match datatype {
        2 | 3 => 12,
        18 | 20 => 6,
        _ => 0,
    }
}

/// Taille (octets) d'un vecteur 2 composantes (2=ushort2→4 ; 3=float2→8 ; 14=ushort→4 ; 18/20=short→4).
///
/// **datatype 2 = ushort UNORM** (u16/65535 par composante, 4 o), distinct de datatype 3 = float32.
/// Validé sur les maps : UV0 vtype=10 dt=2 @32 tient dans le stride (36) ⇒ 4 octets, et lu en
/// ushort UNORM donne des UV sains [0,1] (lu en float/half = garbage). Les perso utilisent dt=14
/// pour l'UV (pas dt=2) → ce changement ne les affecte pas.
#[must_use]
pub fn vec2_byte_size(datatype: u32) -> usize {
    match datatype {
        2 | 14 | 18 | 20 => 4,
        3 => 8,
        _ => 0,
    }
}

/// Décode une couleur (4 composantes) à `off` (port de `DecodeColor`).
fn decode_color_at(data: &[u8], off: usize, datatype: u32) -> Vec4 {
    match datatype {
        2 | 3 => Vec4 {
            x: sanitize(read_f32(data, off)),
            y: sanitize(read_f32(data, off + 4)),
            z: sanitize(read_f32(data, off + 8)),
            w: sanitize(read_f32(data, off + 12)),
        },
        12 => {
            if off + 4 <= data.len() {
                Vec4 {
                    x: data[off] as f32 / 255.0,
                    y: data[off + 1] as f32 / 255.0,
                    z: data[off + 2] as f32 / 255.0,
                    w: data[off + 3] as f32 / 255.0,
                }
            } else {
                Vec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                }
            }
        }
        14 => Vec4 {
            x: f32::from(read_u16(data, off)) / 65535.0,
            y: f32::from(read_u16(data, off + 2)) / 65535.0,
            z: f32::from(read_u16(data, off + 4)) / 65535.0,
            w: f32::from(read_u16(data, off + 6)) / 65535.0,
        },
        _ => Vec4 {
            x: snorm16(read_i16(data, off)),
            y: snorm16(read_i16(data, off + 2)),
            z: snorm16(read_i16(data, off + 4)),
            w: snorm16(read_i16(data, off + 6)),
        },
    }
}

/// SNORM16 : `max(s/32767, -1)` (convention glTF/OpenGL).
#[must_use]
pub fn snorm16(s: i16) -> f32 {
    (s as f32 / 32767.0).max(-1.0)
}

/// Décode une normale à `off` et la renormalise (port de `DecodeNormal`).
fn decode_normal_at(data: &[u8], off: usize, datatype: u32) -> Vec3 {
    let (mut x, mut y, mut z) = match datatype {
        2 | 3 => (
            read_f32(data, off),
            read_f32(data, off + 4),
            read_f32(data, off + 8),
        ),
        _ => (
            snorm16(read_i16(data, off)),
            snorm16(read_i16(data, off + 2)),
            snorm16(read_i16(data, off + 4)),
        ),
    };
    x = sanitize(x);
    y = sanitize(y);
    z = sanitize(z);
    let len = (x * x + y * y + z * z).sqrt();
    if len > 1e-6 {
        Vec3 {
            x: x / len,
            y: y / len,
            z: z / len,
        }
    } else {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        } // normale dégénérée : repli sûr
    }
}

/// Décode un UV0 à `off` (port de `DecodeUv`).
fn decode_uv_at(data: &[u8], off: usize, datatype: u32) -> Vec2 {
    let (u, v) = match datatype {
        3 => (read_f32(data, off), read_f32(data, off + 4)),
        2 | 14 => (
            // ushort UNORM : u16/65535 (dt=2 sur les maps ; dt=14 sur les perso).
            f32::from(read_u16(data, off)) / 65535.0,
            f32::from(read_u16(data, off + 2)) / 65535.0,
        ),
        _ => (
            snorm16(read_i16(data, off)),
            snorm16(read_i16(data, off + 2)),
        ),
    };
    Vec2 {
        u: sanitize(u),
        v: sanitize(v),
    }
}

fn sanitize(f: f32) -> f32 {
    if f.is_finite() { f } else { 0.0 }
}

fn read_f32(data: &[u8], off: usize) -> f32 {
    data.get(off..off + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map_or(0.0, f32::from_le_bytes)
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    data.get(off..off + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map_or(0, u16::from_le_bytes)
}

fn read_i16(data: &[u8], off: usize) -> i16 {
    read_u16(data, off) as i16
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    data.get(off..off + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map_or(0, u32::from_le_bytes)
}

/// Expose la table d'attributs résolue (pour les writers GLB).
#[must_use]
pub fn attributes(g4md: &G4md) -> &[VertexAttribute] {
    &g4md.attributes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g4md;

    /// Construit un g4md + g4mg synthétiques cohérents avec le layout chr/_uniform :
    /// stride 72, normale short×3 @12, uv ushort×2 @64, couleur ubyte4 @68, face_data_base placé après les vertices.
    fn build_pair(vertex_count: usize) -> (g4md::G4md, Vec<u8>) {
        let stride = 72usize;
        let face_data_base = vertex_count * stride; // bloc d'index juste après les vertices
        let submesh_info = 0x60usize;
        let attr_table = submesh_info + SUBMESH_RECORD_SIZE_LOCAL + 8;
        let str_region = attr_table + 4 * 8;

        let mut md = alloc::vec![0u8; str_region];
        md[0..4].copy_from_slice(&g4md::MAGIC_LE.to_le_bytes());
        md[0x04..0x06].copy_from_slice(&(submesh_info as u16).to_le_bytes());
        md[0x20..0x22].copy_from_slice(&1u16.to_le_bytes()); // submesh_count
        md[0x22..0x24].copy_from_slice(&1u16.to_le_bytes()); // material_count
        md[0x26] = 4; // vlayout_count (position + normal + uv + color)
        md[0x5C..0x60].copy_from_slice(&(face_data_base as u32).to_le_bytes());

        let r = submesh_info;
        md[r..r + 4].copy_from_slice(&0u32.to_le_bytes()); // vertex_offset
        md[r + 0x04..r + 0x08].copy_from_slice(&0u32.to_le_bytes()); // index_offset (relatif)
        md[r + 0x08..r + 0x0C].copy_from_slice(&(vertex_count as u32).to_le_bytes());
        md[r + 0x0C..r + 0x10].copy_from_slice(&3u32.to_le_bytes()); // index_count = 3 (1 tri)
        md[r + 0x2E] = stride as u8;

        let put = |buf: &mut [u8], base: usize, vt: u8, off: u16, dt: u32| {
            buf[base] = vt;
            buf[base + 1..base + 3].copy_from_slice(&off.to_le_bytes());
            buf[base + 4..base + 8].copy_from_slice(&dt.to_le_bytes());
        };
        put(&mut md, attr_table, 1, 0, 3); // position float
        put(&mut md, attr_table + 8, 2, 12, 18); // normale short SNORM16
        put(&mut md, attr_table + 16, 10, 64, 14); // uv ushort UNORM16
        put(&mut md, attr_table + 24, 8, 68, 12); // color ubyte4 @68
        md.extend_from_slice(b"mat_10M\0mat_10\0");

        let parsed = g4md::parse(&md).expect("g4md synthétique");

        // g4mg : vertices puis 3 indices u16.
        let mut mg = alloc::vec![0u8; face_data_base + 3 * 2];
        for i in 0..vertex_count {
            let p = i * stride;
            // position (i, 0, 0)
            mg[p..p + 4].copy_from_slice(&(i as f32).to_le_bytes());
            // normale @12 : (0, 0, 32767) → SNORM16 (0,0,1) après renorm
            mg[p + 12..p + 14].copy_from_slice(&0i16.to_le_bytes());
            mg[p + 14..p + 16].copy_from_slice(&0i16.to_le_bytes());
            mg[p + 16..p + 18].copy_from_slice(&32767i16.to_le_bytes());
            // uv @64 : (65535, 0) → UNORM16 (1.0, 0.0)
            mg[p + 64..p + 66].copy_from_slice(&65535u16.to_le_bytes());
            mg[p + 66..p + 68].copy_from_slice(&0u16.to_le_bytes());
            // color @68 : (255, 128, 0, 255) → RGBA float
            mg[p + 68] = 255;
            mg[p + 69] = 128;
            mg[p + 70] = 0;
            mg[p + 71] = 255;
        }
        // indices 0,1,2
        for (k, v) in [0u16, 1, 2].into_iter().enumerate() {
            let q = face_data_base + k * 2;
            mg[q..q + 2].copy_from_slice(&v.to_le_bytes());
        }

        (parsed, mg)
    }

    const SUBMESH_RECORD_SIZE_LOCAL: usize = 0x50;

    #[test]
    fn extract_positions_normales_uv_golden() {
        let (md, mg) = build_pair(3);
        let geo = extract_geometry(&mg, &md);
        assert_eq!(geo.len(), 1);
        let g = &geo[0];
        assert_eq!(g.vertex_count, 3);
        assert_eq!(g.stride, 72);

        // Positions = (0,0,0),(1,0,0),(2,0,0).
        assert_eq!(
            g.positions[0],
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(g.positions[1].x, 1.0);
        assert_eq!(g.positions[2].x, 2.0);

        // Normales décodées & renormalisées : |n| = 1, ≈ (0,0,1).
        assert_eq!(g.normals.len(), 3);
        for n in &g.normals {
            let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normale non unitaire: {len}");
            assert!((n.z - 1.0).abs() < 1e-3);
        }

        // UV0 ∈ [0,1] : (1.0, 0.0).
        assert_eq!(g.uv0.len(), 3);
        for uv in &g.uv0 {
            assert!((uv.u - 1.0).abs() < 1e-4);
            assert!(uv.v.abs() < 1e-4);
        }

        // Couleurs décodées ubyte4 -> f32.
        assert_eq!(g.colors.len(), 3);
        for c in &g.colors {
            assert!((c.x - 1.0).abs() < 1e-4); // R
            assert!((c.y - 128.0 / 255.0).abs() < 1e-4); // G
            assert!(c.z.abs() < 1e-4); // B
            assert!((c.w - 1.0).abs() < 1e-4); // A
        }

        // Indices locaux 0,1,2.
        assert_eq!(g.indices, alloc::vec![0u32, 1, 2]);
        assert!(!g.index32);
    }

    #[test]
    fn pas_de_fabrication_si_attribut_hors_stride() {
        // Construit une paire dont le stride (20) ne couvre PAS l'UV (@64) : aucun UV émis,
        // mais la normale (@12, short×3 = 6 octets, 12+6=18 ≤ 20) reste décodée. Vérifie qu'on
        // ne FABRIQUE rien hors stride (liste vide), tout en gardant les positions.
        let stride = 20u8;
        let vertex_count = 2usize;
        let face_data_base = vertex_count * stride as usize;
        let submesh_info = 0x60usize;
        let attr_table = submesh_info + SUBMESH_RECORD_SIZE_LOCAL + 8;
        let str_region = attr_table + 4 * 8;

        let mut md = alloc::vec![0u8; str_region];
        md[0..4].copy_from_slice(&g4md::MAGIC_LE.to_le_bytes());
        md[0x04..0x06].copy_from_slice(&(submesh_info as u16).to_le_bytes());
        md[0x20..0x22].copy_from_slice(&1u16.to_le_bytes());
        md[0x22..0x24].copy_from_slice(&1u16.to_le_bytes());
        md[0x5C..0x60].copy_from_slice(&(face_data_base as u32).to_le_bytes());
        let r = submesh_info;
        md[r + 0x08..r + 0x0C].copy_from_slice(&(vertex_count as u32).to_le_bytes());
        md[r + 0x0C..r + 0x10].copy_from_slice(&0u32.to_le_bytes());
        md[r + 0x2E] = stride;
        let put = |buf: &mut [u8], base: usize, vt: u8, off: u16, dt: u32| {
            buf[base] = vt;
            buf[base + 1..base + 3].copy_from_slice(&off.to_le_bytes());
            buf[base + 4..base + 8].copy_from_slice(&dt.to_le_bytes());
        };
        put(&mut md, attr_table, 1, 0, 3);
        put(&mut md, attr_table + 8, 2, 12, 18);
        put(&mut md, attr_table + 16, 10, 64, 14); // UV @64 : hors d'un stride de 20
        md.extend_from_slice(b"mat_10M\0mat_10\0");
        let md = g4md::parse(&md).unwrap();

        let mg = alloc::vec![0u8; face_data_base];
        let geo = extract_geometry(&mg, &md);
        assert_eq!(geo.len(), 1);
        let g = &geo[0];
        assert_eq!(g.positions.len(), 2);
        // Normale dans le stride → décodée ; UV hors stride → liste vide (pas de fabrication).
        assert_eq!(g.normals.len(), 2);
        assert!(g.uv0.is_empty(), "UV hors stride NE doit PAS être fabriqué");
    }

    #[test]
    fn snorm16_borne() {
        assert_eq!(snorm16(32767), 1.0);
        assert_eq!(snorm16(0), 0.0);
        // -32768/32767 = -1.00003 → clampé à -1.
        assert_eq!(snorm16(-32768), -1.0);
    }

    #[test]
    fn material_base_name_via_index() {
        let (md, mg) = build_pair(3);
        let geo = extract_geometry(&mg, &md);
        let base = material_base_name(&md, &geo[0]);
        assert_eq!(base.map(String::as_str), Some("mat_10"));
    }

    #[test]
    fn indices_globaux_sont_compactes_depuis_toutes_les_sous_mailles() {
        let geometry = |index, x, indices| SubmeshGeometry {
            index,
            vertex_count: 2,
            stride: 68,
            material_index: 0,
            index32: false,
            positions: alloc::vec![
                Vec3 { x, y: 0.0, z: 0.0 },
                Vec3 {
                    x: x + 1.0,
                    y: 0.0,
                    z: 0.0
                }
            ],
            normals: Vec::new(),
            uv0: Vec::new(),
            colors: Vec::new(),
            indices,
        };
        let mut geometries = alloc::vec![
            geometry(0, 10.0, alloc::vec![]),
            geometry(1, 20.0, alloc::vec![2, 3, 2]),
        ];
        compact_global_indices(&mut geometries, &[Some(2), Some(4)]);
        assert_eq!(geometries[1].positions[0].x, 10.0);
        assert_eq!(geometries[1].positions[1].x, 11.0);
        assert_eq!(geometries[1].indices, alloc::vec![0, 1, 0]);
        assert_eq!(geometries[1].vertex_count, 2);

        let mut invalid = alloc::vec![
            geometry(0, 10.0, alloc::vec![]),
            geometry(1, 20.0, alloc::vec![2, 3, 999]),
        ];
        compact_global_indices(&mut invalid, &[Some(2), Some(4)]);
        assert!(invalid[1].indices.is_empty());
        assert!(invalid[1].positions.is_empty());
    }

    // ── Quad de menu réel : `mainmenu90_02_2.g4mg` ────────────────────────────
    //
    // Les 192 premiers octets du fichier
    // `data/common/menu/100_mainmenu/mainmenu90/mainmenu90_02_2/mainmenu90_02_2.g4mg` —
    // 4 sommets de stride 32, puis 6 indices u16 à 0x80. Le test
    // `le_littéral_menu_correspond_au_fichier_du_jeu` (plus bas) vérifie que ce littéral EST
    // bien le début du fichier du jeu : sans lui ce ne serait qu'un dogme recopié.
    //
    // Les `.g4mg` de menu n'ont pas de `.g4md` sur disque (leur géométrie est pilotée par le
    // paquet menu) : le compagnon est donc construit champ par champ ci-dessous, comme le
    // faisait `G4mgGeometryTests.cs`.
    #[rustfmt::skip]
    const MENU_G4MG: [u8; 192] = [
        0, 0, 128, 63,   0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        255, 127, 255, 127, 255, 255, 0, 0,  0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 0, 0,     255, 127, 255, 127, 0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 128, 191, 0, 0, 0, 0,     0, 0, 0, 0,
        255, 127, 255, 127, 0, 0, 255, 255,  0, 0, 128, 63, 0, 0, 128, 191,
        0, 0, 0, 0,     0, 0, 0, 0,     255, 127, 255, 127, 255, 255, 255, 255,
        0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        // 0x80 : indices u16 du quad — deux triangles [0,1,2] et [0,2,3].
        0, 0, 1, 0,     2, 0, 0, 0,     2, 0, 3, 0,     0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
    ];

    /// G4MD compagnon du quad de menu : 1 sous-maille, 4 sommets de stride 32, 6 indices,
    /// bloc d'index à 0x80, position en `float3` à l'offset 0.
    fn build_menu_g4md() -> g4md::G4md {
        let submesh_info = 0x60usize;
        let attr_table = submesh_info + SUBMESH_RECORD_SIZE_LOCAL + 8;
        let str_region = attr_table + 4 * 8;

        let mut md = alloc::vec![0u8; str_region];
        md[0..4].copy_from_slice(&g4md::MAGIC_LE.to_le_bytes());
        md[0x04..0x06].copy_from_slice(&(submesh_info as u16).to_le_bytes());
        md[0x20..0x22].copy_from_slice(&1u16.to_le_bytes()); // submesh_count
        md[0x22..0x24].copy_from_slice(&1u16.to_le_bytes()); // material_count
        md[0x26] = 1; // vlayout_count : position seule
        md[0x5C..0x60].copy_from_slice(&0x80u32.to_le_bytes()); // face_data_base

        let r = submesh_info;
        md[r..r + 4].copy_from_slice(&0u32.to_le_bytes()); // vertex_offset
        md[r + 0x04..r + 0x08].copy_from_slice(&0u32.to_le_bytes()); // index_offset
        md[r + 0x08..r + 0x0C].copy_from_slice(&4u32.to_le_bytes()); // vertex_count
        md[r + 0x0C..r + 0x10].copy_from_slice(&6u32.to_le_bytes()); // index_count
        md[r + 0x2E] = 32; // stride

        md[attr_table] = 1; // vtype position
        md[attr_table + 1..attr_table + 3].copy_from_slice(&0u16.to_le_bytes()); // offset
        md[attr_table + 4..attr_table + 8].copy_from_slice(&3u32.to_le_bytes()); // float3
        md.extend_from_slice(b"mat_menu\0");

        g4md::parse(&md).expect("g4md compagnon du quad de menu")
    }

    #[test]
    fn geometrie_du_quad_menu_reel() {
        let md = build_menu_g4md();
        assert_eq!(md.header.submesh_count, 1);
        assert_eq!(md.header.face_data_base, 0x80);

        let geo = extract_geometry(&MENU_G4MG, &md);
        assert_eq!(geo.len(), 1, "une sous-maille");
        let g = &geo[0];
        assert_eq!(g.vertex_count, 4);
        assert_eq!(g.stride, 32);
        assert_eq!(
            g.indices,
            alloc::vec![0u32, 1, 2, 0, 2, 3],
            "quad = deux triangles"
        );
        assert!(!g.index32, "indices sur 16 bits");
        assert_eq!(g.positions.len(), 4);
        assert_eq!(g.positions[0].x, 1.0, "premier sommet en x = 1.0");
    }

    /// Le littéral ci-dessus doit être **le début du vrai fichier**. C'est ce test qui distingue
    /// une vérité terrain d'une constante recopiée.
    #[test]
    fn le_litteral_menu_correspond_au_fichier_du_jeu() {
        let Some((chemin, data)) = crate::g4pk::tests_vfs::lire_par_suffixe("mainmenu90_02_2.g4mg")
        else {
            return;
        };
        assert!(data.len() >= 192, "{chemin} : moins de 192 octets");
        assert_eq!(
            &data[..192],
            &MENU_G4MG[..],
            "{chemin} : les 192 premiers octets"
        );

        // Et la géométrie extraite du fichier réel est celle du quad.
        let md = build_menu_g4md();
        let geo = extract_geometry(&data, &md);
        assert_eq!(geo[0].indices, alloc::vec![0u32, 1, 2, 0, 2, 3]);
        std::eprintln!("{chemin} : quad de menu conforme (4 sommets, 6 indices)");
    }
}
