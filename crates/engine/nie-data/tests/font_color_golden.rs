#![allow(clippy::pedantic)]
//! Tests golden `font_color` — la palette de texte du jeu.
//!
//! Valeurs réelles tirées de `data/common/font/font_color.cfg.bin.json` : une liste
//! `m_FontColorDataList` de 70 entrées, chacune portant deux triplets RVB (texte et rubis).
//!
//! Ce fichier vit dans `common/font/`, **hors** de la racine `gamedata` que résout le helper
//! partagé : le chemin remonte donc d'un cran. Comme les autres goldens adossés au corpus, il
//! annonce son saut quand le dump est absent plutôt que de passer au vert sans rien prouver.

mod common;

extern crate std;

use nie_data::font_color::{find_color, parse_font_colors};
use nie_data::hash::HashId;
use serde_json::json;

const REEL: &str = "../font/font_color.cfg.bin.json";

fn charger() -> Option<serde_json::Value> {
    let chemin = common::chemin(REEL)?;
    if !chemin.is_file() {
        eprintln!("skip : {} absent du corpus", chemin.display());
        return None;
    }
    let contenu = std::fs::read_to_string(&chemin)
        .unwrap_or_else(|e| panic!("lecture {} : {e}", chemin.display()));
    Some(serde_json::from_str(&contenu).unwrap_or_else(|e| panic!("JSON invalide : {e}")))
}

#[test]
fn fixture_palette() {
    let root = json!({
        "lists": [{ "name": "m_FontColorDataList", "typeName": "FONT_COLOR", "values": [
            { "fontColorId": "0x270D2BDA", "red": 245, "green": 230, "blue": 245,
              "rubiRed": 245, "rubiGreen": 245, "rubiBlue": 230 },
            // Bornage : une composante hors 0..255 est ramenée dans l'intervalle, jamais
            // repliée par transtypage (256 ne doit pas devenir 0).
            { "fontColorId": "0x00000001", "red": 300, "green": -5, "blue": 128,
              "rubiRed": 0, "rubiGreen": 0, "rubiBlue": 0 }
        ]}]
    });
    let palette = parse_font_colors(&root);
    assert_eq!(palette.len(), 2);
    assert_eq!(palette[0].rgb, (245, 230, 245));
    assert_eq!(palette[0].rubi_rgb, (245, 245, 230));
    assert_eq!(palette[0].hex(), "#F5E6F5");
    assert_eq!(palette[0].rubi_hex(), "#F5F5E6");
    assert_eq!(palette[1].rgb, (255, 0, 128), "bornage 0..255");

    let id = HashId::parse("0x270D2BDA").unwrap();
    assert_eq!(
        find_color(&palette, id).map(|c| c.rgb),
        Some((245, 230, 245))
    );
    assert!(find_color(&palette, HashId::parse("0xDEADBEEF").unwrap()).is_none());
}

#[test]
fn golden_palette_reelle() {
    let Some(root) = charger() else { return };
    let palette = parse_font_colors(&root);
    assert_eq!(palette.len(), 70, "70 couleurs de texte");

    // Première entrée du dump, à l'octet.
    assert_eq!(palette[0].id, HashId::parse("0x270D2BDA").unwrap());
    assert_eq!(palette[0].rgb, (245, 230, 245));
    assert_eq!(palette[0].rubi_rgb, (245, 245, 230));

    // Aucun identifiant nul, et aucun doublon : la palette est bien un index par nom.
    assert!(palette.iter().all(|c| !c.id.is_zero()), "identifiant nul");
    let mut ids: Vec<u32> = palette.iter().map(|c| c.id.get()).collect();
    ids.sort_unstable();
    let avant = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), avant, "identifiants dupliqués dans la palette");

    // Chaque couleur rend un hexadécimal CSS de 7 caractères — c'est ce qui la rend utilisable
    // hors du moteur (thème web).
    assert!(
        palette
            .iter()
            .all(|c| c.hex().len() == 7 && c.hex().starts_with('#'))
    );
}
