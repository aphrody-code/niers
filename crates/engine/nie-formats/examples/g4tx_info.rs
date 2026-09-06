//! Dump la structure d'un `.g4tx` : en-tête, textures, régions, et le format réel des
//! données de pixels (magic DDS + FourCC / DXGI format).
//!
//! Sert à préparer un remplacement **en place** : si le format et la taille des données sont
//! conservés, les offsets et les régions restent valides et le fichier ne peut pas être corrompu.
//!
//! Usage : `cargo run -p nie-formats --example g4tx_info -- <fichier.g4tx>`

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: g4tx_info <fichier.g4tx>");
    let data = std::fs::read(&path).expect("lecture");
    let g = nie_formats::g4tx::parse(&data).expect("parse g4tx");

    println!("fichier          {path}");
    println!("octets           {}", data.len());
    println!("header_size      {}", g.header.header_size);
    println!("file_type        0x{:04X}", g.header.file_type);
    println!("table_size       {}", g.header.table_size);
    println!("texture_count    {}", g.header.texture_count);
    println!("total_count      {}", g.header.total_count);
    println!("sub_texture_count {}", g.header.sub_texture_count);
    println!("texture_data_size {}", g.header.texture_data_size);

    for t in &g.textures {
        println!("\n── texture id={} « {} »", t.id, t.name);
        println!("   {}x{}  is_dds={}", t.width, t.height, t.is_dds);
        println!(
            "   data_offset=0x{:X} data_size={}",
            t.data_offset, t.data_size
        );
        let px = i64::from(t.width) * i64::from(t.height);
        if px > 0 {
            #[allow(clippy::cast_precision_loss)]
            let bpp = t.data_size as f64 / px as f64;
            println!("   octets/pixel={bpp:.3}");
        }
        // En-tête DDS : magic 'DDS ', puis dwSize=124 ; le FourCC est à +84,
        // et pour 'DX10' un en-tête étendu suit avec le DXGI_FORMAT à +128.
        let d = &data[t.data_offset..(t.data_offset + t.data_size).min(data.len())];
        if d.len() >= 148 && &d[0..4] == b"DDS " {
            let fourcc = String::from_utf8_lossy(&d[84..88]).to_string();
            println!("   DDS magic=OK fourcc=« {fourcc} »");
            if &d[84..88] == b"DX10" {
                let dxgi = u32::from_le_bytes([d[128], d[129], d[130], d[131]]);
                println!("   DX10 dxgi_format={dxgi} ({})", dxgi_name(dxgi));
            }
            let h = u32::from_le_bytes([d[12], d[13], d[14], d[15]]);
            let w = u32::from_le_bytes([d[16], d[17], d[18], d[19]]);
            let mips = u32::from_le_bytes([d[28], d[29], d[30], d[31]]);
            println!("   DDS {w}x{h} mipmaps={mips}");
        } else if d.len() >= 4 {
            println!(
                "   pas d'en-tête DDS — 4 premiers octets : {:02X?}",
                &d[0..4]
            );
        }
        for s in &t.sub_textures {
            println!(
                "   région id={} « {} » x={} y={} {}x{}",
                s.id, s.name, s.x, s.y, s.width, s.height
            );
        }
    }
}

/// Noms des `DXGI_FORMAT` utiles pour les textures de ce jeu.
fn dxgi_name(v: u32) -> &'static str {
    match v {
        28 => "R8G8B8A8_UNORM",
        71 => "BC1_UNORM",
        74 => "BC2_UNORM",
        77 => "BC3_UNORM",
        80 => "BC4_UNORM",
        83 => "BC5_UNORM",
        95 => "BC6H_UF16",
        98 => "BC7_UNORM",
        99 => "BC7_UNORM_SRGB",
        _ => "?",
    }
}
