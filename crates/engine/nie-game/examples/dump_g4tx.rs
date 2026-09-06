//! Décode toutes les textures DDS d'un `.g4tx` du VFS en PNG (diagnostic). Usage :
//! `cargo run -p nie-game --example dump_g4tx -- title02_00` → /tmp/g4tx/<name>.png par texture.
use image_dds::Surface;
use nie_formats::{g4tx, vfs::Vfs};

fn dxgi_to_fmt(dxgi: u32) -> Option<image_dds::ImageFormat> {
    use image_dds::ImageFormat::*;
    Some(match dxgi {
        71 | 72 => BC1RgbaUnorm,
        74 | 75 => BC2RgbaUnorm,
        77 | 78 => BC3RgbaUnorm,
        80 | 81 => BC4RUnorm,
        83 | 84 => BC5RgUnorm,
        95 | 96 => BC6hRgbUfloat,
        98 | 99 => BC7RgbaUnorm,
        28 | 29 => Rgba8Unorm,
        _ => return None,
    })
}

fn decode(g4tx_data: &[u8], tex: &g4tx::G4txTexture) -> Option<(u32, u32, Vec<u8>)> {
    if !tex.is_dds {
        return None;
    }
    let off = tex.data_offset;
    const PIXELS: usize = 148;
    if off + PIXELS > g4tx_data.len() {
        return None;
    }
    let dds = &g4tx_data[off..];
    if u32::from_le_bytes(dds[..4].try_into().ok()?) != 0x2053_4444 {
        return None;
    }
    let dxgi = u32::from_le_bytes(dds[128..132].try_into().ok()?);
    let fmt = dxgi_to_fmt(dxgi)?;
    let (w, h) = (tex.width as u32, tex.height as u32);
    if w == 0 || h == 0 {
        return None;
    }
    let surface = Surface {
        width: w,
        height: h,
        depth: 1,
        layers: 1,
        mipmaps: 1,
        image_format: fmt,
        data: &dds[PIXELS..],
    };
    Some((w, h, surface.decode_rgba8().ok()?.data))
}

fn main() {
    let arg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "title02_00".into());
    let dir = nie_formats::vfs::resolve_game_dir();
    let mut vfs = Vfs::new();
    vfs.init(dir.join("data").as_path()).expect("vfs init");
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(&format!("{arg}.g4tx")))
        .min()
        .expect("g4tx introuvable");
    println!("g4tx: {path}");
    let bytes = vfs.read(&path).unwrap();
    let g = g4tx::parse(&bytes).unwrap();
    std::fs::create_dir_all("/tmp/g4tx").unwrap();
    for t in &g.textures {
        match decode(&bytes, t) {
            Some((w, h, rgba)) => {
                let out = format!("/tmp/g4tx/{}_{}.png", arg, t.name);
                let mut buf = Vec::new();
                {
                    let mut enc = png::Encoder::new(&mut buf, w, h);
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
                }
                std::fs::write(&out, &buf).unwrap();
                println!("  tex {:24} {w}x{h} -> {out}", t.name);
            }
            None => println!(
                "  tex {:24} {}x{} (non-DDS/skip)",
                t.name, t.width, t.height
            ),
        }
    }
}
