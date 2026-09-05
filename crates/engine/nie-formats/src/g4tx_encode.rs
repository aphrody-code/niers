//! Encodeur G4TX/DDS — contrepartie écriture de [`crate::g4tx`]/[`crate::g4tx_decode`] (roadmap
//! `apps/inacord/ROADMAP.md` §2.2, « Éditeur d'image (textures) »).
//!
//! Portée volontairement **restreinte et honnête**, pas une reconstruction générale :
//!
//! - [`encode_dds_bgra8`] écrit un DDS **non compressé BGRA8 32 bpp legacy** (`DDPF_RGB`, pas de
//!   FourCC/DX10) — le format public Microsoft documenté, byte-exact avec ce que
//!   [`crate::g4tx_decode::dds_format_and_pixel_offset`] reconnaît déjà comme « BGRA8, défaut
//!   Level-5 » (masques `R=0x00FF0000`/`G=0x0000FF00`/`B=0x000000FF`, offset pixels 128, cf. son
//!   test `dds_uncompressed_bgra8_resolu_a_128`) — PAS de compression BC1..BC7 (ce serait de la
//!   reconstruction non vérifiée d'un encodeur BC propriétaire, hors de portée ici).
//! - [`encode_g4tx_single_texture`] écrit un conteneur G4TX **mono-texture, sans région d'atlas**
//!   (`texture_count=1`, `sub_texture_count=0`) — le cas `font_def.g4tx` documenté dans
//!   [`crate::g4tx`] (pas le cas multi-région `gaiji_game.g4tx`, où « remplacer une région »
//!   n'aurait de toute façon pas de sens : les régions sont des recadrages d'UNE texture
//!   partagée, pas des payloads séparés). Layout byte-exact avec ce que [`crate::g4tx::parse`]
//!   attend (offsets/tailles de table identiques à son test `forge_minimal_g4tx_buffer`) ; les
//!   champs "inconnus" du format (jamais lus par `parse`, cf. ses commentaires `+0x0C inconnu` /
//!   `hash non utilisé`) sont laissés à zéro — documenté, pas deviné.
//!
//! Vérifié par round-trip réel : encoder → [`crate::g4tx::parse`] → [`crate::g4tx_decode::
//! decode_texture_rgba`] → mêmes pixels RGBA8 que l'entrée (cf. `mod tests`).

extern crate alloc;
use alloc::{format, vec, vec::Vec};

/// Taille du DDS_HEADER (magic exclu) — cf. [`crate::g4tx_decode`] `HDR_END = 4 + 124`.
const DDS_HEADER_LEN: usize = 124;
/// Offset absolu (magic inclus) où commencent les pixels d'un DDS non compressé legacy.
const DDS_PIXELS_OFFSET: usize = 4 + DDS_HEADER_LEN;

/// Encode un buffer RGBA8 brut en DDS **non compressé BGRA8 32 bpp** (magic + `DDS_HEADER` de
/// 124 o + pixels), reconnu par [`crate::g4tx_decode::dds_format_and_pixel_offset`] comme
/// `ImageFormat::Bgra8Unorm` (masques R/G/B legacy Level-5). `rgba.len()` DOIT valoir
/// `w * h * 4` (aucun sous-échantillonnage/mip — mip0 seul, comme le décodeur).
///
/// # Erreurs
/// `Err` si `rgba.len() != w * h * 4` (donnée d'entrée incohérente — jamais silencieusement
/// tronquée/étendue).
pub fn encode_dds_bgra8(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, alloc::string::String> {
    let expected = (w as usize) * (h as usize) * 4;
    if rgba.len() != expected {
        return Err(format!("encode_dds_bgra8 : rgba.len()={} attendu w*h*4={expected}", rgba.len()));
    }

    let mut out = vec![0u8; DDS_PIXELS_OFFSET];
    out[0..4].copy_from_slice(b"DDS ");

    // DDS_HEADER (offsets ABSOLUS, magic inclus — cf. commentaire de module pour le calcul).
    const DDSD_CAPS: u32 = 0x1;
    const DDSD_HEIGHT: u32 = 0x2;
    const DDSD_WIDTH: u32 = 0x4;
    const DDSD_PITCH: u32 = 0x8;
    const DDSD_PIXELFORMAT: u32 = 0x1000;
    const DDPF_RGB: u32 = 0x40;
    const DDSCAPS_TEXTURE: u32 = 0x1000;

    out[4..8].copy_from_slice(&(DDS_HEADER_LEN as u32).to_le_bytes()); // dwSize
    out[8..12].copy_from_slice(&(DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_PIXELFORMAT).to_le_bytes());
    out[12..16].copy_from_slice(&h.to_le_bytes());
    out[16..20].copy_from_slice(&w.to_le_bytes());
    out[20..24].copy_from_slice(&(w * 4).to_le_bytes()); // dwPitchOrLinearSize (octets/ligne, 32bpp)
    // dwDepth @24, dwMipMapCount @28, dwReserved1[11] @32..76 : zéro (pas de profondeur/mips).

    // DDS_PIXELFORMAT @76 (32 o).
    out[76..80].copy_from_slice(&32u32.to_le_bytes()); // pf.dwSize
    out[80..84].copy_from_slice(&DDPF_RGB.to_le_bytes()); // pf.dwFlags (pas DDPF_ALPHAPIXELS : alpha ignoré au décodage comme à l'encodage legacy Level-5)
    // pf.dwFourCC @84 : zéro (non-compressé).
    out[88..92].copy_from_slice(&32u32.to_le_bytes()); // pf.dwRGBBitCount
    out[92..96].copy_from_slice(&0x00FF_0000u32.to_le_bytes()); // pf.dwRBitMask
    out[96..100].copy_from_slice(&0x0000_FF00u32.to_le_bytes()); // pf.dwGBitMask
    out[100..104].copy_from_slice(&0x0000_00FFu32.to_le_bytes()); // pf.dwBBitMask
    // pf.dwABitMask @104 : zéro (DDPF_ALPHAPIXELS non posé).

    out[108..112].copy_from_slice(&DDSCAPS_TEXTURE.to_le_bytes()); // dwCaps
    // dwCaps2/3/4 @112..124, dwReserved2 (en fait ici juste le reste jusqu'à 124) : zéro.

    // Pixels : RGBA8 → BGRA8 (octet bas = B, cf. masques ci-dessus — même convention que
    // `dds_format_and_pixel_offset`/`decode_texture_rgba` en lecture).
    let mut pixels = vec![0u8; expected];
    for (src, dst) in rgba.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
        dst[0] = src[2]; // B
        dst[1] = src[1]; // G
        dst[2] = src[0]; // R
        dst[3] = src[3]; // A
    }
    out.extend_from_slice(&pixels);

    Ok(out)
}

#[inline]
const fn align(v: usize, a: usize) -> usize {
    (v + (a - 1)) & !(a - 1)
}

/// Décode un PNG (n'importe quel type de couleur/profondeur — RGB/RGBA/palette/niveaux de gris,
/// 8/16 bits) en RGBA8 `(w, h, rgba)` via le crate `png` (même bibliothèque que [`crate::
/// g4tx_decode::encode_rgba_to_png`], sens inverse). `Transformations::normalize_to_color8()`
/// fait faire au décodeur la normalisation (palette→RGB, gris→RGB, 16→8 bits, ajout alpha
/// opaque si absent) plutôt que de la réimplémenter à la main ici.
///
/// # Erreurs
/// `Err` si le PNG est invalide/tronqué ou si le décodage échoue.
pub fn decode_png_to_rgba8(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), alloc::string::String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| format!("en-tête PNG invalide : {e}"))?;
    let buf_size = reader.output_buffer_size().ok_or("PNG : taille de tampon de sortie indéterminable")?;
    let mut buf = vec![0u8; buf_size];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("décodage PNG : {e}"))?;
    let (w, h) = (info.width, info.height);

    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let mut out = vec![0u8; (w * h * 4) as usize];
            for (src, dst) in buf[..info.buffer_size()].chunks_exact(3).zip(out.chunks_exact_mut(4)) {
                dst[0..3].copy_from_slice(src);
                dst[3] = 255;
            }
            out
        }
        other => return Err(format!("PNG : type de couleur {other:?} inattendu après normalisation (bug)")),
    };
    Ok((w, h, rgba))
}

/// Encode un conteneur G4TX **mono-texture, sans région d'atlas** portant `dds` (déjà encodé,
/// cf. [`encode_dds_bgra8`]) sous le nom `name` (résolu tel quel par [`crate::g4tx::parse`]) et
/// l'identifiant `id`. `w`/`h` DOIVENT correspondre aux dimensions réelles du DDS (dupliquées
/// dans les champs d'entrée `width`/`height`, lus par `parse` en repli si le payload DDS est
/// absent/tronqué — cf. [`crate::g4tx::parse`]).
///
/// Layout byte-exact avec ce que `parse` attend (mêmes formules d'offsets que son test
/// `forge_minimal_g4tx_buffer`, généralisées à un nom/une taille de payload arbitraires) : les
/// champs jamais lus par `parse` (hash table, plusieurs réservés d'en-tête/entrée) sont à zéro,
/// documenté ci-dessus dans le commentaire de module — jamais devinés depuis une supposition.
#[must_use]
pub fn encode_g4tx_single_texture(name: &str, id: u8, w: i16, h: i16, dds: &[u8]) -> Vec<u8> {
    const HEADER_SIZE: usize = 0x60;
    const ENTRY_SIZE: usize = 0x30;

    let entry_offset = HEADER_SIZE;
    let sub_entry_offset = entry_offset + ENTRY_SIZE; // tex_count=1, sub_count=0
    let hash_offset = align(sub_entry_offset, 16);
    let id_offset = hash_offset + 4; // total_count=1 → 1×u32
    let string_offset = align(id_offset + 1, 4); // total_count=1 → +1 octet d'id

    let name_bytes = format!("{name}\0");
    let string_offsets_size = 2usize; // total_count=1 → 1×i16
    let string_table_end = string_offset + string_offsets_size + name_bytes.len();
    let table_size = string_table_end - HEADER_SIZE;
    let nxtch_base = align(HEADER_SIZE + table_size, 16);

    let mut out = vec![0u8; nxtch_base + dds.len()];

    // En-tête (0x60 o) — champs vérifiés par `parse` uniquement, cf. `g4tx::parse_header`.
    out[0..4].copy_from_slice(b"G4TX");
    out[4..6].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes()); // header_size
    out[6..8].copy_from_slice(&0x65u16.to_le_bytes()); // file_type (0x65 observé sur fichiers réels)
    out[0x0C..0x10].copy_from_slice(&(table_size as u32).to_le_bytes());
    out[0x20..0x22].copy_from_slice(&1u16.to_le_bytes()); // texture_count
    out[0x22..0x24].copy_from_slice(&1u16.to_le_bytes()); // total_count
    out[0x25] = 0; // sub_texture_count
    out[0x2C..0x30].copy_from_slice(&(dds.len() as u32).to_le_bytes()); // texture_data_size

    // Entrée de texture (0x30 o à entry_offset).
    let e = entry_offset;
    out[e + 0x04..e + 0x08].copy_from_slice(&0u32.to_le_bytes()); // nxtch_offset (payload seul → 0)
    out[e + 0x08..e + 0x0C].copy_from_slice(&(dds.len() as u32).to_le_bytes()); // nxtch_size
    out[e + 0x18..e + 0x1A].copy_from_slice(&w.to_le_bytes());
    out[e + 0x1A..e + 0x1C].copy_from_slice(&h.to_le_bytes());

    // Table de hash (@hash_offset, 1×u32) : jamais lue par `parse` (cf. commentaire de module) — zéro.

    // Table d'ids (@id_offset, 1×u8).
    out[id_offset] = id;

    // Table d'offsets de chaînes (@string_offset, 1×i16) : pointe juste après elle-même.
    out[string_offset..string_offset + 2].copy_from_slice(&(string_offsets_size as i16).to_le_bytes());
    // Nom (null-terminé) juste après.
    let name_pos = string_offset + string_offsets_size;
    out[name_pos..name_pos + name_bytes.len()].copy_from_slice(name_bytes.as_bytes());

    // Padding jusqu'à `nxtch_base` déjà à zéro (buffer initialisé à zéro) — payload DDS ensuite.
    out[nxtch_base..].copy_from_slice(dds);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{g4tx, g4tx_decode};

    /// Encode un DDS BGRA8 synthétique (damier 2×2, alpha variable) puis le redécode — vérifie
    /// que [`encode_dds_bgra8`] est reconnu par le décodeur déjà validé (`dds_format_and_pixel_
    /// offset` : `Bgra8Unorm` @128, cf. son test `dds_uncompressed_bgra8_resolu_a_128`) et que
    /// les pixels reviennent EXACTS (round-trip encode→decode, pas juste "ça parse").
    #[test]
    fn encode_dds_bgra8_round_trip_via_g4tx() {
        let (w, h) = (2i16, 2i16);
        let rgba: Vec<u8> = alloc::vec![
            255, 0, 0, 255, // rouge opaque
            0, 255, 0, 200, // vert semi-transparent
            0, 0, 255, 128, // bleu mi-transparent
            255, 255, 0, 0, // jaune totalement transparent
        ];
        let dds = encode_dds_bgra8(w as u32, h as u32, &rgba).expect("encode_dds_bgra8");

        let g4tx_bytes = encode_g4tx_single_texture("test_tex", 7, w, h, &dds);

        let parsed = g4tx::parse(&g4tx_bytes).expect("parse du G4TX encodé");
        assert_eq!(parsed.header.texture_count, 1);
        assert_eq!(parsed.header.sub_texture_count, 0);
        assert_eq!(parsed.textures.len(), 1);
        let tex = &parsed.textures[0];
        assert_eq!(tex.name, "test_tex");
        assert_eq!(tex.id, 7);
        assert!(tex.is_dds, "le payload doit être reconnu comme DDS");
        assert_eq!((tex.width, tex.height), (2, 2));

        let (dw, dh, decoded_rgba) = g4tx_decode::decode_texture_rgba(&g4tx_bytes, tex).expect("decode_texture_rgba");
        assert_eq!((dw, dh), (2, 2));
        assert_eq!(decoded_rgba, rgba, "round-trip pixel-exact RGBA8 → DDS BGRA8 → G4TX → décodage");
    }

    /// `decode_png_to_rgba8` round-trip avec `g4tx_decode::encode_rgba_to_png` (encodeur PNG déjà
    /// utilisé pour l'aperçu de texture, `vfs_texture_png_b64`) — prouve que le décodeur PNG lit
    /// exactement ce que l'encodeur existant écrit, avant de l'enchaîner dans le pipeline G4TX.
    #[test]
    fn decode_png_to_rgba8_round_trip_avec_encode_rgba_to_png() {
        let (w, h) = (3usize, 2usize);
        let rgba: Vec<u8> = alloc::vec![
            10, 20, 30, 255, 40, 50, 60, 128, 70, 80, 90, 0, 100, 110, 120, 255, 130, 140, 150, 64, 160, 170, 180, 255,
        ];
        let png = g4tx_decode::encode_rgba_to_png(&rgba, w, h).expect("encode_rgba_to_png");
        let (dw, dh, decoded) = decode_png_to_rgba8(&png).expect("decode_png_to_rgba8");
        assert_eq!((dw as usize, dh as usize), (w, h));
        assert_eq!(decoded, rgba);
    }

    /// Bout-en-bout complet : PNG → RGBA8 → DDS BGRA8 → G4TX → reparse → redécodage → mêmes
    /// pixels que le PNG d'origine. C'est exactement le chemin qu'emprunte le remplacement de
    /// texture (§2.2 roadmap) : un PNG choisi par l'utilisatrice devient un `.g4tx` valide.
    #[test]
    fn png_vers_g4tx_bout_en_bout() {
        let (w, h) = (4u32, 4u32);
        let mut rgba = alloc::vec![0u8; (w * h * 4) as usize];
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            px[0] = (i * 17) as u8;
            px[1] = (i * 31) as u8;
            px[2] = (i * 53) as u8;
            px[3] = 255;
        }
        let png = g4tx_decode::encode_rgba_to_png(&rgba, w as usize, h as usize).expect("encode PNG source");

        let (dw, dh, decoded_rgba) = decode_png_to_rgba8(&png).expect("decode_png_to_rgba8");
        let dds = encode_dds_bgra8(dw, dh, &decoded_rgba).expect("encode_dds_bgra8");
        let g4tx_bytes = encode_g4tx_single_texture("remplacement", 0, dw as i16, dh as i16, &dds);

        let parsed = g4tx::parse(&g4tx_bytes).expect("parse");
        let (fw, fh, final_rgba) = g4tx_decode::decode_texture_rgba(&g4tx_bytes, &parsed.textures[0]).expect("decode final");
        assert_eq!((fw, fh), (w, h));
        assert_eq!(final_rgba, rgba, "PNG d'origine == pixels du .g4tx généré, bout en bout");
    }

    /// `encode_dds_bgra8` rejette une entrée de taille incohérente plutôt que de tronquer/étendre
    /// silencieusement.
    #[test]
    fn encode_dds_bgra8_rejette_taille_incoherente() {
        assert!(encode_dds_bgra8(2, 2, &[0u8; 8]).is_err());
    }

    /// Round-trip sur une taille non triviale (16×16, contenu pseudo-aléatoire déterministe) —
    /// vérifie l'absence de décalage d'offset caché sur un volume de pixels plus réaliste qu'un
    /// 2×2.
    #[test]
    fn encode_dds_bgra8_round_trip_16x16() {
        let (w, h) = (16u32, 16u32);
        let mut rgba = alloc::vec![0u8; (w * h * 4) as usize];
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            px[0] = (i * 7) as u8;
            px[1] = (i * 13) as u8;
            px[2] = (i * 29) as u8;
            px[3] = (i * 3) as u8;
        }
        let dds = encode_dds_bgra8(w, h, &rgba).expect("encode");
        let g4tx_bytes = encode_g4tx_single_texture("atlas16", 0, w as i16, h as i16, &dds);
        let parsed = g4tx::parse(&g4tx_bytes).expect("parse");
        let (dw, dh, decoded) = g4tx_decode::decode_texture_rgba(&g4tx_bytes, &parsed.textures[0]).expect("decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(decoded, rgba);
    }
}

#[cfg(test)]
mod real_game_tests {
    use super::*;
    use crate::vfs::Vfs;
    use std::path::Path;

    /// Round-trip sur un VRAI `.g4tx` mono-texture sans région d'atlas du jeu : décode sa
    /// texture réelle, la ré-encode via ce module, reparse/redécode, et compare les pixels —
    /// preuve que l'encodeur produit un conteneur exploitable par le VRAI décodeur sur du
    /// contenu réel, pas seulement des fixtures synthétiques. Skip (pas d'échec) si le jeu est
    /// absent de ce poste, ou si aucun `.g4tx` mono-texture réel n'est trouvé dans l'échantillon.
    #[test]
    fn encode_g4tx_round_trip_sur_un_vrai_fichier() {
        let dir = crate::vfs::resolve_game_dir().to_string_lossy().into_owned();
        let data_dir = Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip encode_g4tx_round_trip_sur_un_vrai_fichier : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> =
            vfs.iter().map(|(p, _)| p.to_string()).filter(|p| p.ends_with(".g4tx")).collect();
        let step = (candidates.len() / 3000).max(1);

        let mut tested = 0usize;
        for path in candidates.iter().step_by(step) {
            let Ok(data) = vfs.read(path) else { continue };
            let Ok(parsed) = crate::g4tx::parse(&data) else { continue };
            if parsed.header.texture_count != 1 || parsed.header.sub_texture_count != 0 {
                continue;
            }
            let tex = &parsed.textures[0];
            if !tex.is_dds {
                continue;
            }
            let Some((w, h, rgba)) = crate::g4tx_decode::decode_texture_rgba(&data, tex) else { continue };
            if w == 0 || h == 0 {
                continue;
            }

            let dds = encode_dds_bgra8(w, h, &rgba).expect("encode_dds_bgra8");
            let g4tx_bytes = encode_g4tx_single_texture(&tex.name, tex.id, w as i16, h as i16, &dds);
            let reparsed = crate::g4tx::parse(&g4tx_bytes).unwrap_or_else(|e| panic!("{path} : reparse échoué : {e}"));
            let (rw, rh, rrgba) = crate::g4tx_decode::decode_texture_rgba(&g4tx_bytes, &reparsed.textures[0])
                .unwrap_or_else(|| panic!("{path} : redécodage échoué"));
            assert_eq!((rw, rh), (w, h), "{path} : dimensions divergentes après round-trip");
            assert_eq!(rrgba, rgba, "{path} : pixels divergents après round-trip encode→decode");
            tested += 1;
        }
        eprintln!("encode_g4tx round-trip : {tested} fichier(s) mono-texture réels vérifiés pixel-exacts (sur {} candidats, pas={step})", candidates.len());
        assert!(tested > 0, "aucun .g4tx mono-texture DDS trouvé dans l'échantillon — élargir le pas");
    }
}
