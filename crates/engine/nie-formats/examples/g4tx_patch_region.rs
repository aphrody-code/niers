//! Remplace **une seule région** d'un atlas `.g4tx` par une image PNG, en réécrivant les pixels
//! dans leur **format d'origine et à taille identique**.
//!
//! Pourquoi ce détour plutôt que `niers mod texture` : cet atlas est compressé (BC7) et porte
//! plusieurs régions. Réencoder en BGRA8 changerait la taille des données, donc les offsets, donc
//! casserait la table des régions. Ici, la taille des blocs BC7 ne dépend que des dimensions —
//! inchangées — donc `data_size` est rigoureusement identique et **seuls les octets de pixels
//! bougent**. L'en-tête, la table des textures et les 6 régions restent bit pour bit les mêmes.
//!
//! Usage :
//!   `cargo run -p nie-formats --features textures-encode --example g4tx_patch_region -- \`
//!   `  <in.g4tx> <region> <remplacement.png> <out.g4tx>`

use image_dds::{ImageFormat, Mipmaps, Quality, Surface, SurfaceRgba8};

/// En-tête `DDS_HEADER` (128) + extension `DDS_HEADER_DXT10` (20).
const DDS_DX10_HEADER: usize = 148;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        eprintln!("usage: g4tx_patch_region <in.g4tx> <region> <png> <out.g4tx>");
        std::process::exit(2);
    }
    let (src, region_name, png_path, dst) = (&a[0], &a[1], &a[2], &a[3]);

    let original = std::fs::read(src).expect("lecture g4tx");
    let g = nie_formats::g4tx::parse(&original).expect("parse g4tx");
    let tex = g.textures.first().expect("aucune texture");
    let region = tex
        .sub_textures
        .iter()
        .find(|s| s.name == *region_name)
        .unwrap_or_else(|| panic!("région « {region_name} » absente"));

    let (tw, th) = (tex.width as u32, tex.height as u32);
    println!(
        "atlas     {tw}x{th}  data_offset=0x{:X} data_size={}",
        tex.data_offset, tex.data_size
    );
    println!(
        "région    « {} » x={} y={} {}x{}",
        region.name, region.x, region.y, region.width, region.height
    );

    // ── 1. décoder les pixels BC7 de l'atlas ────────────────────────────────────
    let pixels = &original[tex.data_offset + DDS_DX10_HEADER..tex.data_offset + tex.data_size];
    let surface = Surface {
        width: tw,
        height: th,
        depth: 1,
        layers: 1,
        mipmaps: 1,
        image_format: ImageFormat::BC7RgbaUnorm,
        data: pixels,
    };
    let mut rgba = surface.decode_rgba8().expect("décodage BC7").data;
    println!("décodé    {} octets RGBA", rgba.len());

    // ── 2. coller le PNG dans la région ─────────────────────────────────────────
    // Le PNG doit être AUX DIMENSIONS EXACTES de la région : aucun redimensionnement ici, donc
    // aucune déformation silencieuse. Au besoin, redimensionner en amont en conservant le ratio.
    let png_bytes = std::fs::read(png_path).expect("lecture png");
    let (pw, ph, src) =
        nie_formats::g4tx_encode::decode_png_to_rgba8(&png_bytes).expect("décodage png");
    let (rw, rh) = (region.width as u32, region.height as u32);
    assert_eq!(
        (pw, ph),
        (rw, rh),
        "le PNG fait {pw}x{ph} mais la région « {} » fait {rw}x{rh}",
        region.name
    );
    let (rx, ry) = (region.x as u32, region.y as u32);
    for y in 0..rh {
        let so = ((y * rw) * 4) as usize;
        let dof = (((ry + y) * tw + rx) * 4) as usize;
        rgba[dof..dof + (rw * 4) as usize].copy_from_slice(&src[so..so + (rw * 4) as usize]);
    }
    println!("collé     {rw}x{rh} en ({rx},{ry})");

    // ── 3. réencoder en BC7, même dimensions donc même taille ───────────────────
    let encoded = SurfaceRgba8 {
        width: tw,
        height: th,
        depth: 1,
        layers: 1,
        mipmaps: 1,
        data: rgba.as_slice(),
    }
    .encode(ImageFormat::BC7RgbaUnorm, Quality::Fast, Mipmaps::Disabled)
    .expect("encodage BC7");

    let expected = tex.data_size - DDS_DX10_HEADER;
    assert_eq!(
        encoded.data.len(),
        expected,
        "taille BC7 réencodée ({}) ≠ taille d'origine ({expected}) — refus d'écrire",
        encoded.data.len()
    );
    println!(
        "réencodé  {} octets BC7 (identique à l'origine)",
        encoded.data.len()
    );

    // ── 4. patch en place : seuls les octets de pixels changent ─────────────────
    let mut out = original.clone();
    let start = tex.data_offset + DDS_DX10_HEADER;
    out[start..start + encoded.data.len()].copy_from_slice(&encoded.data);
    assert_eq!(
        out.len(),
        original.len(),
        "taille de fichier modifiée — refus d'écrire"
    );

    // ── 5. relecture de contrôle : la structure doit être inchangée ─────────────
    let g2 = nie_formats::g4tx::parse(&out).expect("le fichier produit doit se reparser");
    let t2 = g2.textures.first().expect("texture");
    assert_eq!(
        g2.header.sub_texture_count, g.header.sub_texture_count,
        "nb de régions modifié"
    );
    assert_eq!(t2.data_offset, tex.data_offset, "offset de données modifié");
    assert_eq!(t2.data_size, tex.data_size, "taille de données modifiée");
    for (x, y) in t2.sub_textures.iter().zip(tex.sub_textures.iter()) {
        assert_eq!(
            (x.id, x.x, x.y, x.width, x.height),
            (y.id, y.x, y.y, y.width, y.height),
            "région « {} » déplacée",
            y.name
        );
        assert_eq!(x.name, y.name, "nom de région modifié");
    }
    // Hors zone de pixels, le fichier doit être identique octet pour octet.
    assert_eq!(out[..start], original[..start], "en-tête modifié");
    let tail = start + encoded.data.len();
    assert_eq!(out[tail..], original[tail..], "queue de fichier modifiée");
    println!(
        "contrôle  structure identique, {} régions préservées",
        t2.sub_textures.len()
    );

    std::fs::write(dst, &out).expect("écriture");
    println!("écrit     {dst} ({} octets)", out.len());
}
