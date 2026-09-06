//! Catalogue les 9 fontes du jeu (`data/common/font/font/font_*/font.cfg.bin`) : glyphes,
//! dimensions d'atlas, répartition par page. Une seule (`font_def`, Latin) était exploitée
//! jusqu'ici (rendu via [`nie_formats::font::LatinAtlas`], scène de dialogue) ; les 8 autres
//! (CJK + génériques crédits) ont un `font.cfg.bin` que [`nie_formats::font::parse_metrics`]
//! décode déjà — générique, rien de spécifique-Latin dans le parseur — mais qui n'a jamais été
//! appelé dessus. Ce catalogue le fait, et écrit un PNG de sonde (un glyphe blitté via les
//! métriques brutes, PAS l'edge-scan `LatinAtlas`) pour chaque fonte, afin de vérifier si le
//! blit metrics-based generalise au-delà de `font_def` ou si ces fontes ont le même problème de
//! « repacking » que `font.rs` documente pour le Latin.
//!
//! Usage : `cargo run -p nie-formats --example font_catalog --features std`
//! (`NIE_GAME_DIR` pour pointer une install hors du répertoire courant.)

use nie_formats::{cfgbin, font, g4tx, vfs::Vfs};

const FONTS: &[&str] = &[
    "font_def",
    "font_ja",
    "font_ja2",
    "font_ja_endroll",
    "font_ja_endroll2",
    "font_zh_endroll",
    "font_zh_hans",
    "font_zh_hans2",
    "font_zh_hant",
    "font_zh_hant2",
];

fn main() {
    let game_dir = nie_formats::vfs::resolve_game_dir();
    let data_dir = game_dir.join("data");
    let mut vfs = Vfs::new();
    vfs.init(&data_dir)
        .expect("init VFS depuis cpk_list.cfg.bin");

    println!(
        "{:<20} {:>8} {:>6} {:>6} {:>10} {:>10}",
        "fonte", "glyphes", "pages", "petite", "atlas_w", "atlas_h"
    );

    for name in FONTS {
        let cfg_path = format!("data/common/font/font/{name}/font.cfg.bin");
        let g4tx_path = format!("data/dx11/font/{name}/font.g4tx");

        let Ok(cfg_bytes) = vfs.read(&cfg_path) else {
            println!("{name:<20} — absent ({cfg_path})");
            continue;
        };
        let Ok(cfg) = cfgbin::parse_t2b(&cfg_bytes) else {
            println!("{name:<20} — parse T2B échoué");
            continue;
        };
        let metrics = font::parse_metrics(&cfg);

        // Répartition par page (0..=3) : le CJK est-il étalé sur plusieurs couches d'atlas ?
        let mut par_page = [0usize; 4];
        for g in metrics.glyphs.values() {
            let p = (g.page as usize).min(3);
            par_page[p] += 1;
        }
        let pages_utilisees = par_page.iter().filter(|&&n| n > 0).count();

        println!(
            "{name:<20} {:>8} {:>6} {:>6} {:>10} {:>10}   pages={:?}",
            metrics.glyph_count(),
            pages_utilisees,
            metrics.glyphs_small.len(),
            metrics.atlas_width,
            metrics.atlas_height,
            par_page,
        );

        let Ok(g4tx_bytes) = vfs.read(&g4tx_path) else {
            println!("  atlas absent ({g4tx_path})");
            continue;
        };
        let Ok(tx) = g4tx::parse(&g4tx_bytes) else {
            println!("  atlas illisible");
            continue;
        };
        let Some(t) = tx.textures.first() else {
            println!("  aucune texture dans l'atlas");
            continue;
        };
        if !t.is_dds {
            println!("  atlas non-DDS, sonde sautée");
            continue;
        }
        let dds = &g4tx_bytes[t.data_offset..];
        let px_off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
            148
        } else {
            128
        };
        let Some(atlas) = dds.get(px_off..) else {
            println!("  payload atlas hors limites");
            continue;
        };
        let cell_h = metrics.dims.cell_height.max(1);

        // Sonde de blit : essaie plusieurs candidats (ASCII '!', hiragana 'あ' U+3042, puis TOUS
        // les glyphes de la table dans l'ordre) via `glyph_blitter` (métriques brutes x/y, PAS
        // d'edge-scan `LatinAtlas`) jusqu'à en trouver un qui rend un pixel non-vide — un premier
        // candidat blanc peut être un glyphe légitimement vide (espace insécable…), pas une preuve
        // que le blit metrics-based ne generalise pas à cette fonte.
        let candidats: Vec<u32> = [0x21u32, 0x3042]
            .into_iter()
            .chain(metrics.glyphs.keys().copied().filter(|&cp| cp > 0x20))
            .collect();
        let mut trouve = false;
        for &cp in &candidats {
            let Some(m) = metrics.glyphs.get(&cp) else {
                continue;
            };
            let (cw, ch) = (u32::from(m.width).max(1) + 8, u32::from(cell_h) + 8);
            let mut canvas = vec![0u8; (cw * ch * 4) as usize];
            font::glyph_blitter(
                atlas,
                t.width as u32,
                m,
                cell_h,
                &mut canvas,
                cw * 4,
                4,
                4,
                [255, 255, 255, 255],
            );
            let lit: usize = canvas.chunks_exact(4).filter(|p| p[3] > 8).count();
            if lit == 0 {
                continue;
            }
            let out_path = format!("/tmp/font_catalog_{name}.png");
            let mut buf = Vec::new();
            {
                let mut e = png::Encoder::new(std::io::Cursor::new(&mut buf), cw, ch);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header().unwrap().write_image_data(&canvas).unwrap();
            }
            std::fs::write(&out_path, &buf).unwrap();
            let ch = font::decode_packed_codepoint(cp);
            println!(
                "  sonde raw={cp:#X} ({ch:?}) x={} y={} w={} page={} → {lit} px allumés/{}, {out_path}",
                m.x,
                m.y,
                m.width,
                m.page,
                canvas.len() / 4
            );
            trouve = true;
            break;
        }
        if !trouve {
            println!(
                "  AUCUN candidat n'a rendu de pixel (metrics-based blit peut-être cassé pour cette fonte)"
            );
        }
    }
}
