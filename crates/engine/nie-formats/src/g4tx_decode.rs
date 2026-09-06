//! Décodage des textures **G4TX / DDS → RGBA8 / PNG** — *source unique* du workspace.
//!
//! Phase 1b de `docs/ARCHITECTURE.md` : ce module enrobe la variante **la plus complète**
//! (celle de `nie-model-serve`) et remplace les 4 copies divergentes (model-serve, wasm, ffi,
//! game). On **enrobe sans réécrire** la logique de décodage validée en prod sur le CDN.
//!
//! Trois familles d'en-tête DDS rencontrées dans IEVR sont gérées :
//! - extension **DX10** : `dxgiFormat` (BC1..BC7) aux 4 octets suivant le `DDS_HEADER` ;
//! - **FourCC legacy** : `DXT1/3/5`, `ATI1/2` (= BC4/BC5), sans extension DX10 ;
//! - **non compressé** 32 bpp : RGBA8 / BGRA8 distingués par les masques du `DDS_PIXELFORMAT`.
//!
//! Le sélecteur anti-dummy [`crate::g4tx::select_main_texture`] est réutilisé (et non un
//! `max_by_key` ad hoc) pour choisir la texture principale réelle d'un atlas.
//!
//! ## Pourquoi derrière la feature `textures` (off par défaut)
//!
//! Le décodage tire `image_dds` (std). `nie-formats` est no_std-friendly + wasm-portable :
//! gater ce module derrière la feature Cargo `textures` préserve le build no_std/wasm par
//! défaut. `image_dds` en `default-features = false` compile bien en wasm32 (cf. `nie-wasm`).
#![cfg(feature = "textures")]

use image_dds::{ImageFormat, Surface};

use crate::g4tx::{self, G4txTexture};

/// Magic DDS (`"DDS "`, LE `0x2053_4444`).
const DDS_MAGIC: u32 = 0x2053_4444;

/// Table de correspondance DXGI format → `image_dds::ImageFormat` (BC1..BC7).
#[must_use]
pub fn dxgi_to_image_format(dxgi: u32) -> Option<ImageFormat> {
    match dxgi {
        // BC1
        71 => Some(ImageFormat::BC1RgbaUnorm),
        72 => Some(ImageFormat::BC1RgbaUnormSrgb),
        // BC2
        73 => Some(ImageFormat::BC2RgbaUnorm),
        74 => Some(ImageFormat::BC2RgbaUnormSrgb),
        // BC3
        77 => Some(ImageFormat::BC3RgbaUnorm),
        78 => Some(ImageFormat::BC3RgbaUnormSrgb),
        // BC4
        79 | 80 => Some(ImageFormat::BC4RUnorm),
        // BC5
        83 | 84 => Some(ImageFormat::BC5RgUnorm),
        // BC6H
        95 => Some(ImageFormat::BC6hRgbUfloat),
        96 => Some(ImageFormat::BC6hRgbSfloat),
        // BC7
        98 => Some(ImageFormat::BC7RgbaUnorm),
        99 => Some(ImageFormat::BC7RgbaUnormSrgb),
        _ => None,
    }
}

/// FourCC legacy (`DDS_PIXELFORMAT.dwFourCC`, sans extension DX10) → `image_dds::ImageFormat`.
///
/// L'outillage Level-5 écrit certaines textures (visages `_face/*/_base`, mips de secours)
/// en DDS **legacy** (DXT1/3/5, ATI1/2 = BC4/BC5) au lieu de l'extension DX10. `DXT1` est
/// décodé comme `BC1RgbaUnorm` (le bit alpha 1-bit est porté par le bloc lui-même).
#[must_use]
pub fn fourcc_to_image_format(fourcc: &[u8; 4]) -> Option<ImageFormat> {
    match fourcc {
        b"DXT1" => Some(ImageFormat::BC1RgbaUnorm),
        b"DXT2" | b"DXT3" => Some(ImageFormat::BC2RgbaUnorm),
        b"DXT4" | b"DXT5" => Some(ImageFormat::BC3RgbaUnorm),
        b"ATI1" | b"BC4U" => Some(ImageFormat::BC4RUnorm),
        b"ATI2" | b"BC5U" => Some(ImageFormat::BC5RgUnorm),
        _ => None,
    }
}

/// Détermine `(format image_dds, offset des pixels)` depuis un DDS brut (slice débutant au
/// magic `DDS `). Gère les **trois** familles d'en-tête :
/// - **extension DX10** : `dxgiFormat` aux 4 octets suivant le `DDS_HEADER` (offset 128),
///   pixels après l'extension de 20 octets (offset 148) ;
/// - **legacy FourCC** : format lu dans `DDS_PIXELFORMAT.dwFourCC`, pixels juste après le
///   `DDS_HEADER` (offset 128) ;
/// - **non compressé** : masques RGB du `DDS_PIXELFORMAT`, pixels à l'offset 128.
///
/// Offsets `DDS_HEADER` (124 o) + `DDS_PIXELFORMAT` (ddspf à l'offset fichier 76) conformes à
/// la spec Microsoft DDS.
#[must_use]
pub fn dds_format_and_pixel_offset(dds_slice: &[u8]) -> Option<(ImageFormat, usize)> {
    const HDR_END: usize = 4 + 124; // magic(4) + DDS_HEADER(124) = 128
    const DX10_PIXELS: usize = HDR_END + 20; // + DDS_HEADER_DXT10(20) = 148
    // Offsets fichier dans DDS_PIXELFORMAT (struct à l'offset 76 : 4 magic + 72 dans le header).
    const PF_FLAGS: usize = 80;
    const PF_FOURCC: usize = 84;
    const PF_BITCOUNT: usize = 88;
    const PF_RMASK: usize = 92;
    const PF_BMASK: usize = 100;
    const DDPF_FOURCC: u32 = 0x4;
    const DDPF_RGB: u32 = 0x40;

    if dds_slice.len() < HDR_END {
        return None;
    }
    let pf_flags = u32::from_le_bytes(dds_slice[PF_FLAGS..PF_FLAGS + 4].try_into().ok()?);
    let fourcc: [u8; 4] = dds_slice[PF_FOURCC..PF_FOURCC + 4].try_into().ok()?;

    if pf_flags & DDPF_FOURCC != 0 {
        if &fourcc == b"DX10" {
            if dds_slice.len() < DX10_PIXELS {
                return None;
            }
            let dxgi = u32::from_le_bytes(dds_slice[HDR_END..HDR_END + 4].try_into().ok()?);
            return dxgi_to_image_format(dxgi).map(|f| (f, DX10_PIXELS));
        }
        return fourcc_to_image_format(&fourcc).map(|f| (f, HDR_END));
    }

    if pf_flags & DDPF_RGB != 0 {
        // Non compressé : bitcount + masques distinguent BGRA8/RGBA8 (cas 32 bpp Level-5).
        let bitcount = u32::from_le_bytes(dds_slice[PF_BITCOUNT..PF_BITCOUNT + 4].try_into().ok()?);
        let r_mask = u32::from_le_bytes(dds_slice[PF_RMASK..PF_RMASK + 4].try_into().ok()?);
        let b_mask = u32::from_le_bytes(dds_slice[PF_BMASK..PF_BMASK + 4].try_into().ok()?);
        if bitcount == 32 {
            // R en octet bas (0x0000_00ff) ⇒ RGBA ; sinon B en octet bas ⇒ BGRA (défaut L5).
            let fmt = if r_mask == 0x0000_00ff && b_mask == 0x00ff_0000 {
                ImageFormat::Rgba8Unorm
            } else {
                ImageFormat::Bgra8Unorm
            };
            return Some((fmt, HDR_END));
        }
    }
    None
}

/// Décode une entrée `G4txTexture` (payload DDS) en RGBA8 brut `(w, h, data)`.
///
/// Couvre DX10 + FourCC legacy + non compressé. Renvoie `None` si la texture n'est pas DDS,
/// si le payload est tronqué, si le magic `DDS ` est absent, ou si le format n'est pas géré.
#[must_use]
pub fn decode_texture_rgba(g4tx_data: &[u8], tex: &G4txTexture) -> Option<(u32, u32, Vec<u8>)> {
    if !tex.is_dds {
        return None;
    }

    let offset = tex.data_offset;
    const HDR_END: usize = 4 + 124; // magic(4) + DDS_HEADER(124) = 128

    if offset + HDR_END > g4tx_data.len() {
        return None;
    }

    let dds_slice = g4tx_data.get(offset..)?;

    // Vérifie le magic DDS.
    let magic = u32::from_le_bytes(dds_slice.get(..4)?.try_into().ok()?);
    if magic != DDS_MAGIC {
        return None;
    }

    // Résout le format ET l'offset des pixels (DX10 @148 ou legacy/uncompressed @128).
    let (image_format, pixel_offset) = dds_format_and_pixel_offset(dds_slice)?;

    let w = tex.width as u32;
    let h = tex.height as u32;
    if w == 0 || h == 0 {
        return None;
    }

    if pixel_offset > dds_slice.len() {
        return None;
    }
    let pixel_data = dds_slice.get(pixel_offset..)?;

    // Construit une `Surface` image_dds avec mip0 seulement.
    let surface = Surface {
        width: w,
        height: h,
        depth: 1,
        layers: 1,
        mipmaps: 1,
        image_format,
        data: pixel_data,
    };

    let rgba = surface.decode_rgba8().ok()?;
    Some((w, h, rgba.data))
}

/// Décode la texture principale **réelle** d'un G4TX en RGBA8 `(w, h, data)`.
///
/// Parse le conteneur puis sélectionne la texture via [`crate::g4tx::select_main_texture`]
/// (anti-dummy) plutôt qu'un `max_by_key` ad hoc.
///
/// `basename` = nom du fichier `.g4tx` **sans dossier ni extension** (ex. `title02_07`,
/// [`basename_of`] le calcule). C'est le nom que porte la texture utile dans les conteneurs
/// mono-texture, et le seul moyen de ne pas rendre une texture arbitraire dans les conteneurs
/// qui en portent plusieurs : un `""` désactive l'étape 1 de la sélection et fait retomber
/// directement sur « la plus grande non-dummy » (ce que faisait cette fonction avant, à tort).
/// Passer `""` reste licite quand l'appelant n'a QUE des octets (FFI, wasm) et aucun nom.
#[must_use]
pub fn decode_best_to_rgba(g4tx_data: &[u8], basename: &str) -> Option<(u32, u32, Vec<u8>)> {
    let parsed = g4tx::parse(g4tx_data).ok()?;
    let tex = g4tx::select_main_texture(&parsed, basename)?;
    decode_texture_rgba(g4tx_data, tex)
}

/// Variante PNG de [`decode_best_to_rgba`] : décode la texture principale et la réencode en PNG.
#[must_use]
pub fn decode_best_to_png(g4tx_data: &[u8], basename: &str) -> Option<Vec<u8>> {
    let (w, h, rgba) = decode_best_to_rgba(g4tx_data, basename)?;
    encode_rgba_to_png(&rgba, w as usize, h as usize)
}

/// Nom de base d'un chemin (VFS ou disque) : dossier et extension retirés.
///
/// `data/dx11/menu/200_icon/02_icon_item/icon_item05.g4tx` → `icon_item05`. C'est l'argument
/// attendu par [`decode_best_to_rgba`] / [`decode_best_to_png`].
#[must_use]
pub fn basename_of(path: &str) -> &str {
    let fichier = path.rsplit(['/', '\\']).next().unwrap_or(path);
    fichier.rsplit_once('.').map_or(fichier, |(tronc, _)| tronc)
}

/// Décode une texture **nommée** d'un conteneur G4TX en RGBA8 `(w, h, data)`.
///
/// Deux dispositions coexistent dans les conteneurs IEVR, et elles ne se résolvent pas pareil :
///
/// 1. **multi-textures principales** (`sub_texture_count == 0`) — chaque nom a son **propre
///    payload DDS complet**. C'est le cas des icônes d'objets : `icon_item05.g4tx` porte 80
///    textures 256×256 BC7 nommées `eq_ac0100101`… Il faut donc **sélectionner** la texture,
///    surtout pas rogner quoi que ce soit.
/// 2. **atlas spatial** (`sub_texture_count > 0`) — un seul payload et une table de rectangles
///    nommés ([`crate::g4tx::find_sub_texture`]). Là il faut décoder la texture porteuse **puis
///    rogner** le rectangle.
///
/// La comparaison de nom est insensible à la casse, comme le reste du module. Rend `None` si
/// aucun des deux chemins ne trouve `nom` (le conteneur ne contient pas cette texture).
#[must_use]
pub fn decode_named_to_rgba(g4tx_data: &[u8], nom: &str) -> Option<(u32, u32, Vec<u8>)> {
    let parsed = g4tx::parse(g4tx_data).ok()?;

    // 1. Texture principale portant ce nom → payload autonome, décodage direct.
    if let Some(tex) = parsed
        .textures
        .iter()
        .find(|t| t.is_dds && t.name.eq_ignore_ascii_case(nom))
    {
        return decode_texture_rgba(g4tx_data, tex);
    }

    // 2. Région d'atlas → décode la texture porteuse, puis rogne le rectangle.
    let (tex, sub) = g4tx::find_sub_texture(&parsed, nom)?;
    let (w, h, rgba) = decode_texture_rgba(g4tx_data, tex)?;
    crop_rgba(&rgba, w, h, sub.x, sub.y, sub.width, sub.height)
}

/// Variante PNG de [`decode_named_to_rgba`].
#[must_use]
pub fn decode_named_to_png(g4tx_data: &[u8], nom: &str) -> Option<Vec<u8>> {
    let (w, h, rgba) = decode_named_to_rgba(g4tx_data, nom)?;
    encode_rgba_to_png(&rgba, w as usize, h as usize)
}

/// Rogne un rectangle dans un buffer RGBA8 `(w, h)`.
///
/// Les rectangles d'atlas viennent du fichier : ils sont signés (`i16`) et peuvent déborder la
/// texture porteuse d'un ou deux pixels. Un rectangle hors champ ou vide rend `None` ; un
/// rectangle qui déborde est **borné** à la texture plutôt que rejeté — sinon une icône parfaitement
/// utilisable disparaîtrait pour un pixel de marge.
#[must_use]
fn crop_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    x: i16,
    y: i16,
    cw: i16,
    ch: i16,
) -> Option<(u32, u32, Vec<u8>)> {
    if x < 0 || y < 0 || cw <= 0 || ch <= 0 {
        return None;
    }
    let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
    if x >= w || y >= h {
        return None;
    }
    let cw = u32::try_from(cw).ok()?.min(w - x);
    let ch = u32::try_from(ch).ok()?.min(h - y);
    if rgba.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }

    let mut out = Vec::with_capacity((cw as usize) * (ch as usize) * 4);
    for ligne in 0..ch {
        let debut = (((y + ligne) as usize) * (w as usize) + x as usize) * 4;
        let fin = debut + (cw as usize) * 4;
        out.extend_from_slice(rgba.get(debut..fin)?);
    }
    Some((cw, ch, out))
}

/// Encode un buffer RGBA8 brut en PNG (8 bits, RGBA).
#[must_use]
pub fn encode_rgba_to_png(rgba: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let mut out_buf: Vec<u8> = Vec::with_capacity(w * h);
    {
        let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut out_buf), w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un en-tête DDS minimal (magic + DDS_HEADER de 124 o, + extension si DX10),
    /// avec `ddspf.flags`/`fourCC` posés aux offsets spec. Suffit à tester la résolution
    /// de format ; aucune donnée de pixels réelle requise.
    fn dds_header(pf_flags: u32, fourcc: &[u8; 4], extra: &[(usize, u32)], len: usize) -> Vec<u8> {
        let mut h = vec![0u8; len];
        h[0..4].copy_from_slice(b"DDS ");
        h[80..84].copy_from_slice(&pf_flags.to_le_bytes());
        h[84..88].copy_from_slice(fourcc);
        for &(off, val) in extra {
            h[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }
        h
    }

    #[test]
    fn dds_legacy_dxt1_resolu_en_bc1_a_128() {
        // FourCC DXT1 sans extension DX10 (le cas des visages EDIT `base_normal_00`).
        let h = dds_header(0x4, b"DXT1", &[], 256);
        let (fmt, off) = dds_format_and_pixel_offset(&h).expect("DXT1 doit être reconnu");
        assert!(matches!(fmt, ImageFormat::BC1RgbaUnorm));
        assert_eq!(off, 128, "pixels legacy juste après le DDS_HEADER");
    }

    #[test]
    fn dds_legacy_ati2_resolu_en_bc5() {
        let h = dds_header(0x4, b"ATI2", &[], 256);
        let (fmt, off) = dds_format_and_pixel_offset(&h).expect("ATI2 doit être reconnu");
        assert!(matches!(fmt, ImageFormat::BC5RgUnorm));
        assert_eq!(off, 128);
    }

    #[test]
    fn dds_dx10_bc7_resolu_a_148() {
        // FourCC DX10 + dxgiFormat 98 (BC7RgbaUnorm) à l'offset 128 ; pixels à 148.
        let h = dds_header(0x4, b"DX10", &[(128, 98)], 256);
        let (fmt, off) = dds_format_and_pixel_offset(&h).expect("DX10 BC7 doit être reconnu");
        assert!(matches!(fmt, ImageFormat::BC7RgbaUnorm));
        assert_eq!(off, 148, "pixels DX10 après l'extension de 20 o");
    }

    #[test]
    fn dds_uncompressed_bgra8_resolu_a_128() {
        // DDPF_RGB, 32 bpp, B en octet bas (masque B=0xff) → BGRA8 (défaut Level-5).
        let h = dds_header(
            0x40,
            &[0; 4],
            &[(88, 32), (92, 0x00ff_0000), (100, 0x0000_00ff)],
            256,
        );
        let (fmt, off) = dds_format_and_pixel_offset(&h).expect("BGRA8 doit être reconnu");
        assert!(matches!(fmt, ImageFormat::Bgra8Unorm));
        assert_eq!(off, 128);
    }

    #[test]
    fn dds_uncompressed_rgba8_resolu_a_128() {
        let h = dds_header(
            0x40,
            &[0; 4],
            &[(88, 32), (92, 0x0000_00ff), (100, 0x00ff_0000)],
            256,
        );
        let (fmt, _) = dds_format_and_pixel_offset(&h).expect("RGBA8 doit être reconnu");
        assert!(matches!(fmt, ImageFormat::Rgba8Unorm));
    }

    #[test]
    fn dds_fourcc_inconnu_rejete() {
        let h = dds_header(0x4, b"ZZZZ", &[], 256);
        assert!(dds_format_and_pixel_offset(&h).is_none());
    }

    #[test]
    fn basename_retire_dossier_et_extension() {
        assert_eq!(
            basename_of("data/dx11/menu/200_icon/02_icon_item/icon_item05.g4tx"),
            "icon_item05"
        );
        assert_eq!(basename_of("icon_item05.g4tx"), "icon_item05");
        assert_eq!(basename_of("icon_item05"), "icon_item05");
        assert_eq!(basename_of(""), "");
    }

    #[test]
    fn crop_borne_et_rejette() {
        // 4×2 RGBA, chaque pixel = son index répété.
        let rgba: Vec<u8> = (0u8..8).flat_map(|i| [i, i, i, 255]).collect();
        let (w, h, out) = crop_rgba(&rgba, 4, 2, 1, 1, 2, 1).expect("rect valide");
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, vec![5, 5, 5, 255, 6, 6, 6, 255]);

        // Débordement → borné à la texture, pas rejeté.
        let (w, h, _) = crop_rgba(&rgba, 4, 2, 2, 0, 99, 99).expect("rect borné");
        assert_eq!((w, h), (2, 2));

        // Hors champ / vide → None.
        assert!(crop_rgba(&rgba, 4, 2, 4, 0, 1, 1).is_none());
        assert!(crop_rgba(&rgba, 4, 2, 0, 0, 0, 1).is_none());
        assert!(crop_rgba(&rgba, 4, 2, -1, 0, 1, 1).is_none());
    }

    /// Sélection par NOM sur un vrai conteneur du jeu : `icon_item05.g4tx` porte 80 textures
    /// principales 256×256 (une par objet, `sub_texture_count == 0`). Le décodeur « meilleure
    /// texture » n'en rend qu'une, toujours la même — c'est précisément ce que la route
    /// `/tex/<…>.g4tx/<nom>.png` doit dépasser.
    ///
    /// Le corpus (`data/`, © Level-5) est absent du clone public : le test ANNONCE son saut
    /// plutôt que de passer en silence.
    #[test]
    fn selection_nommee_sur_vrai_conteneur_icon_item05() {
        const CHEMIN: &str = "data/dx11/menu/200_icon/02_icon_item/icon_item05.g4tx";

        let racine = crate::vfs::resolve_game_dir();
        let mut vfs = crate::vfs::Vfs::new();
        if vfs.init(racine.join("data")).is_err() {
            eprintln!(
                "SAUTÉ : VFS réel indisponible sous {} (corpus du jeu absent)",
                racine.display()
            );
            return;
        }
        let Ok(data) = vfs.read(CHEMIN) else {
            eprintln!("SAUTÉ : {CHEMIN} absent du VFS monté");
            return;
        };

        let parsed = crate::g4tx::parse(&data).expect("G4TX parsable");
        let noms: Vec<String> = parsed
            .textures
            .iter()
            .filter(|t| t.is_dds)
            .map(|t| t.name.clone())
            .collect();
        assert!(
            noms.len() >= 2,
            "conteneur multi-textures attendu, vu {} texture(s)",
            noms.len()
        );

        // Deux noms DIFFÉRENTS doivent rendre deux images DIFFÉRENTES.
        let a = decode_named_to_png(&data, &noms[0]).expect("1re texture nommée décodée");
        let b = decode_named_to_png(&data, &noms[noms.len() - 1])
            .expect("dernière texture nommée décodée");
        assert_ne!(
            a, b,
            "deux noms distincts rendent le même PNG — sélection par nom inopérante"
        );

        // La casse ne doit pas compter.
        let a_maj =
            decode_named_to_png(&data, &noms[0].to_ascii_uppercase()).expect("nom en majuscules");
        assert_eq!(
            a, a_maj,
            "la comparaison de nom doit être insensible à la casse"
        );

        // Un nom absent ne doit PAS retomber sur une texture arbitraire.
        assert!(
            decode_named_to_png(&data, "nom_qui_nexiste_pas_dans_ce_conteneur").is_none(),
            "un nom inconnu doit rendre None, pas une texture au hasard"
        );

        // Le décodeur « meilleure texture » rend UNE des textures nommées : il ne peut donc pas
        // servir à adresser les 79 autres.
        let best =
            decode_best_to_png(&data, basename_of(CHEMIN)).expect("meilleure texture décodée");
        let egal_a_une_nommee = noms
            .iter()
            .filter_map(|n| decode_named_to_png(&data, n))
            .any(|png| png == best);
        assert!(
            egal_a_une_nommee,
            "la meilleure texture doit être l'une des textures nommées"
        );
    }
}
