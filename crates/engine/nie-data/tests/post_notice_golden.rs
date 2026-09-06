#![allow(clippy::pedantic)]
//! Tests golden `post::post_notice` — valeurs réelles tirées de :
//! `post/post_notice_config_1.03.93.00.cfg.bin.json`
//!
//! Layout `lists` : 6 listes (compositions de bannières + fonds/badges/icônes/textes
//! graphiques + annonces).

mod common;

use nie_data::hash::HashId;
use nie_data::post::{PostNoticeConfig, parse_post_notice_config};

const REAL_PATH: &str = "post/post_notice_config_1.03.93.00.cfg.bin.json";

fn load_real() -> Option<PostNoticeConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_post_notice_config(&root))
}

#[test]
fn comptes_listes() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.banner_imgs.len(), 44, "m_PostNoticeBannerImgInfoList");
    assert_eq!(cfg.banner_bgs.len(), 16, "m_PostNoticeBannerBgInfoList");
    assert_eq!(
        cfg.banner_badges.len(),
        3,
        "m_PostNoticeBannerBadgeInfoList"
    );
    assert_eq!(cfg.banner_icons.len(), 9, "m_PostNoticeBannerIconInfoList");
    assert_eq!(
        cfg.banner_graphics_texts.len(),
        8,
        "m_PostNoticeBannerGraphicsTextInfo"
    );
    assert_eq!(cfg.infos.len(), 9, "m_PostNoticeInfoList");
}

#[test]
fn banner_img_echantillons() {
    let Some(cfg) = load_real() else { return };
    let b0 = &cfg.banner_imgs[0];
    assert_eq!(b0.id_crc, HashId(0xEA8A_163E));
    assert_eq!(b0.banner_bg_id_crc, HashId(0x4E6E_E6A6));
    assert_eq!(b0.banner_badge_id_crc, HashId(0x73FB_83C5));
    assert_eq!(b0.banner_icon_id_crc, HashId(0x2B4F_ECA6));
    // dernier [43]
    let b43 = &cfg.banner_imgs[43];
    assert_eq!(b43.id_crc, HashId(0x6F33_6B28));
    assert_eq!(b43.banner_icon_id_crc, HashId(0x0062_BF65));
}

#[test]
fn banner_bg_chemin_texture() {
    let Some(cfg) = load_real() else { return };
    let bg0 = &cfg.banner_bgs[0];
    assert_eq!(bg0.id_crc, HashId(0x4E6E_E6A6));
    assert_eq!(bg0.banner_bg_texture_name_crc, HashId(0x60FF_D0B3));
    assert_eq!(
        bg0.banner_bg_texture_path,
        "#/menu/220_img/banner_img/banner01_0001.g4tx"
    );
    let bg2 = &cfg.banner_bgs[2];
    assert_eq!(bg2.id_crc, HashId(0x7C58_8424));
    assert_eq!(
        bg2.banner_bg_texture_path,
        "#/menu/220_img/banner_img/banner_img_early.g4tx"
    );
}

#[test]
fn badges_et_icones() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.banner_badges[0].id_crc, HashId(0x73FB_83C5));
    assert_eq!(
        cfg.banner_badges[0].banner_badge_texture_name_crc,
        HashId(0xE586_163F)
    );
    assert_eq!(cfg.banner_badges[2].id_crc, HashId(0x41CD_E147));
    assert_eq!(
        cfg.banner_badges[2].banner_badge_texture_name_crc,
        HashId(0x0B88_7713)
    );

    assert_eq!(cfg.banner_icons[0].id_crc, HashId(0x2B4F_ECA6));
    assert_eq!(
        cfg.banner_icons[0].banner_icon_texture_name_crc,
        HashId(0x24DE_C470)
    );
    assert_eq!(cfg.banner_icons[8].id_crc, HashId(0x4F23_29A2));
}

#[test]
fn graphics_text_marqueur_final() {
    let Some(cfg) = load_real() else { return };
    let g0 = &cfg.banner_graphics_texts[0];
    assert_eq!(g0.id_crc, HashId(0x2B77_25CF));
    assert_eq!(
        g0.banner_graphics_text_texture_path,
        "#/menu/220_img/banner_img/banner02_0001.g4tx"
    );
    // dernier [7] : chemin marqueur opaque (caractère de remplacement U+FFFD)
    let g7 = &cfg.banner_graphics_texts[7];
    assert_eq!(g7.id_crc, HashId(0xFAB5_9E86));
    assert_eq!(g7.banner_graphics_text_texture_name_crc, HashId::ZERO);
    assert_eq!(
        g7.banner_graphics_text_texture_path,
        "\u{FFFD}POST_NOTICE_INFO"
    );
}

#[test]
fn post_notice_info_echantillons() {
    let Some(cfg) = load_real() else { return };
    let n0 = &cfg.infos[0];
    assert_eq!(n0.id_crc, HashId(0x3A54_9FEC));
    assert_eq!(n0.flag_no, 19);
    assert!(!n0.is_advance_notice);
    assert_eq!(n0.banner_img_id_crc, HashId(0x7D86_C4C6));
    assert_eq!(n0.banner_graphics_text_id_crc, HashId(0x7D2D_8249));
    assert_eq!(n0.banner_title_text_font_style_type, 1);
    assert_eq!(n0.detail_window_main_txt_id_crc, HashId(0x75B5_144B));
    assert!(n0.is_use_utc);
    assert_eq!(n0.valid_cond, "POST_NOTICE_INFO");

    // [3] : valid_cond opaque tronqué "O", time bornes renseignées
    let n3 = &cfg.infos[3];
    assert_eq!(n3.id_crc, HashId(0x7F65_CDF8));
    assert_eq!(n3.banner_overview_two_line_text_font_style_type, 3);
    assert_eq!(n3.start_enable_time, 2025);
    assert_eq!(n3.end_enable_time, 11);
    assert_eq!(n3.valid_cond, "O");

    // dernier [8]
    let n8 = &cfg.infos[8];
    assert_eq!(n8.id_crc, HashId(0x916B_ACD4));
    assert_eq!(n8.flag_no, 2);
    assert_eq!(n8.banner_title_text_id_crc, HashId(0x68BB_F6EB));
    assert_eq!(n8.detail_window_title_txt_id_crc, HashId(0x7873_CF46));
}
