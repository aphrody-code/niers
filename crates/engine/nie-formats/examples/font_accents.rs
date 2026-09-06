//! Diagnostic : la police du jeu connaît-elle les caractères accentués français ?
//!
//! Le rendu de texte du moteur passe par `LatinAtlas`, un edge-scan de la seule rangée ASCII
//! contiguë — tout ce qui est au-delà de `~` y devient une espace, et les menus français
//! s'affichent « Composition d' quipe ». Avant de bricoler un rendu d'accents, il faut savoir si
//! les glyphes existent : s'ils sont dans les métriques, ils sont dans l'atlas, et c'est un
//! problème d'accès, pas de contenu.

use nie_formats::vfs;

/// Chemin logique des métriques de police du jeu.
const FONT_CFG: &str = "data/common/font/font/font_def/font.cfg.bin";

fn main() -> Result<(), String> {
    let vfs = vfs::open_game().map_err(|e| format!("VFS : {e:?}"))?;
    let octets = vfs
        .read(FONT_CFG)
        .map_err(|e| format!("{FONT_CFG} : {e:?}"))?;
    let cfg = nie_formats::cfgbin::parse_t2b(&octets).map_err(|e| format!("cfg : {e:?}"))?;
    let metrics = nie_formats::font::parse_metrics(&cfg);

    println!("glyphes (police 0) : {}", metrics.glyph_count());
    println!(
        "cell_height={} ascent={}",
        metrics.dims.cell_height, metrics.dims.ascent
    );

    // Les caractères que le français exige réellement, minuscules et majuscules.
    const FR: &str = "éèêëàâäîïôöùûüçÉÈÊËÀÂÄÎÏÔÖÙÛÜÇœŒ«»…–—’";
    /// Clé brute d'un caractère dans la table : son point de code, ou ses octets UTF-8
    /// empaquetés en big-endian — la forme réellement stockée par le jeu au-delà de l'ASCII
    /// (cf. `decode_packed_codepoint`).
    fn cle_empaquetee(c: char) -> u32 {
        let mut buf = [0u8; 4];
        c.encode_utf8(&mut buf)
            .as_bytes()
            .iter()
            .fold(0u32, |acc, &b| (acc << 8) | u32::from(b))
    }

    let (mut presents, mut absents) = (Vec::new(), Vec::new());
    for c in FR.chars() {
        let direct = metrics.glyph(c as u32).is_some();
        let empaquete = metrics.glyph(cle_empaquetee(c)).is_some();
        if direct || empaquete {
            presents.push((c, if direct { "direct" } else { "empaqueté" }));
        } else {
            absents.push(c);
        }
    }
    let presents: Vec<char> = {
        for (c, forme) in &presents {
            println!("  '{c}' trouvé en {forme} (clé {:#x})", cle_empaquetee(*c));
        }
        presents.iter().map(|(c, _)| *c).collect()
    };
    println!(
        "\naccents PRÉSENTS dans les métriques ({}) : {}",
        presents.len(),
        presents.iter().collect::<String>()
    );
    println!(
        "accents ABSENTS ({}) : {}",
        absents.len(),
        absents.iter().collect::<String>()
    );

    // Où vivent-ils dans l'atlas ? Un Y différent de la rangée ASCII expliquerait que l'edge-scan
    // ne les voie pas — et donnerait l'adresse pour les atteindre.
    let ascii_y: Vec<u16> = "AZaz09"
        .chars()
        .filter_map(|c| metrics.glyph(c as u32).map(|g| g.y))
        .collect();
    println!("\nY des glyphes ASCII : {ascii_y:?}");
    for c in presents.iter().take(8) {
        if let Some(g) = metrics
            .glyph(*c as u32)
            .or_else(|| metrics.glyph(cle_empaquetee(*c)))
        {
            println!(
                "  '{c}' → x={} y={} avance={} bearing_x={}",
                g.x, g.y, g.advance, g.bearing_x
            );
        }
    }
    // L'atlas latin (`font_def`) est edge-scanné depuis la rangée ASCII : si celle-ci contient
    // PLUS de 94 glyphes (0x21..=0x7E), la suite est peut-être le Latin-1 accentué — auquel cas
    // les accents s'atteignent par le même mécanisme, sans passer par des métriques qui décrivent
    // visiblement un autre atlas (un `é` rendu par ses métriques donne un idéogramme).
    let g4tx = vfs
        .read("data/dx11/font/font_def/font.g4tx")
        .map_err(|e| format!("atlas : {e:?}"))?;
    let tx = nie_formats::g4tx::parse(&g4tx).map_err(|e| format!("g4tx : {e:?}"))?;
    let t = tx.textures.first().ok_or("pas de texture")?;
    let dds = &g4tx[t.data_offset..];
    let off = if dds.len() >= 88 && &dds[84..88] == b"DX10" {
        148
    } else {
        128
    };
    let atlas = &dds[off..];
    let la = nie_formats::font::LatinAtlas::from_atlas(
        atlas,
        t.width as usize,
        t.height as usize,
        946,
        metrics.dims.cell_height,
    );
    println!(
        "\natlas {}x{} — spans edge-scannés : {}",
        t.width,
        t.height,
        la.spans.len()
    );
    println!("ASCII imprimable = 0x21..=0x7E = 94 glyphes ; au-delà, la rangée continue avec :");
    for (i, (x0, x1)) in la.spans.iter().enumerate().skip(94).take(12) {
        let cp = 0x21 + i as u32;
        println!("  span #{i} (cp supposé {cp:#x}) x={x0}..{x1}");
    }
    // Où l'encre se trouve-t-elle DANS la cellule ? La mise en page des listes suppose que le
    // texte occupe le haut de ce qu'on lui donne ; si l'encre vit au milieu ou en bas d'une
    // cellule de 71 px, tout libellé déborde sous sa barre de surlignage.
    let stride = t.width as usize * 4;
    let (mut haut, mut bas) = (None::<usize>, 0usize);
    for gy in 0..metrics.dims.cell_height as usize {
        let ay = 946 + gy;
        let encre = (0..t.width as usize)
            .any(|ax| atlas.get(ay * stride + ax * 4 + 3).copied().unwrap_or(0) > 32);
        if encre {
            haut.get_or_insert(gy);
            bas = gy;
        }
    }
    match haut {
        Some(h) => println!(
            "\nencre dans la cellule (hauteur {}) : lignes {h}..={bas} → hauteur utile {}",
            metrics.dims.cell_height,
            bas - h + 1,
        ),
        None => println!("\naucune encre trouvée dans la rangée"),
    }
    Ok(())
}
