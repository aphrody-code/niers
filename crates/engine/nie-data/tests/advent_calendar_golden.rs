#![allow(clippy::pedantic)]
//! Tests golden `post::advent_calendar` — valeurs réelles tirées de :
//! `post/advent_calendar_config_2.00.17.00.cfg.bin.json`
//!
//! Layout `lists` : 5 listes (calendrier, news, items/récompenses de connexion).

mod common;

use nie_data::hash::HashId;
use nie_data::post::{AdventCalendarConfig, parse_advent_calendar_config};

const REAL_PATH: &str = "post/advent_calendar_config_2.00.17.00.cfg.bin.json";

fn load_real() -> Option<AdventCalendarConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_advent_calendar_config(&root))
}

#[test]
fn comptes_listes() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.calendar.len(), 5, "m_AdventCalendarInfoList");
    assert_eq!(cfg.news.len(), 6, "m_NewsInfoList");
    assert_eq!(
        cfg.login_bonus_reward_items.len(),
        1,
        "m_LoginBonusRewardItem"
    );
    assert_eq!(cfg.login_bonus_rewards.len(), 1, "m_LoginBonusReward");
    assert_eq!(cfg.login_bonus_infos.len(), 1, "m_LoginBonusInfo");
}

#[test]
fn calendar_entree0() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.calendar[0];
    assert_eq!(e.id_crc, HashId(0xF43E_EA20));
    assert_eq!(e.cell_type, 1);
    assert_eq!(e.flag_no, 0);
    assert_eq!(e.time_start, 2025);
    assert_eq!(e.time_end, 11);
    assert_eq!(e.repeat_type, 11);
    assert_eq!(e.data_id_crc, HashId::ZERO);
    assert_eq!(e.item_info_id_crc, HashId(0x000B_07E9));
    // valid_cond opaque (fragment de typeName sur le dump réel)
    assert_eq!(e.valid_cond, "NDAR_INFO");
}

#[test]
fn calendar_entree2() {
    let Some(cfg) = load_real() else { return };
    let e = &cfg.calendar[2];
    assert_eq!(e.id_crc, HashId(0xF1F0_BAAB));
    assert_eq!(e.flag_no, 2);
    assert_eq!(e.time_start, 2026);
    assert_eq!(e.time_end, 1);
    assert_eq!(e.repeat_type, 28);
    assert_eq!(e.item_info_id_crc, HashId(0x0001_07EA));
}

#[test]
fn news_entrees() {
    let Some(cfg) = load_real() else { return };
    // m_NewsInfoList[0] : essentiellement vide hormis id_crc + news_type=1
    let n0 = &cfg.news[0];
    assert_eq!(n0.id_crc, HashId(0xF43E_EA20));
    assert_eq!(n0.news_type, 1);
    assert_eq!(n0.banner_start_res_name, "");
    // m_NewsInfoList[1]
    let n1 = &cfg.news[1];
    assert_eq!(n1.id_crc, HashId(0x7A12_D002));
    assert_eq!(n1.headline_small_text_id, HashId(0xA0EC_5DEA));
    assert_eq!(n1.headline_large_text_id, HashId(0x2DF4_7B41));
    assert_eq!(n1.detail_text_id, HashId(0x6C26_9B9F));
    assert_eq!(n1.banner_start_res_name, "calendar_update_img_01_001");
    assert_eq!(n1.news_type, 0);
    assert_eq!(n1.detail_window_res_name, "box_win_update_img01_001");
    // m_NewsInfoList[5] : news_type=3
    assert_eq!(cfg.news[5].id_crc, HashId(0x385E_32A5));
    assert_eq!(cfg.news[5].news_type, 3);
}

#[test]
fn login_bonus() {
    let Some(cfg) = load_real() else { return };
    let item = &cfg.login_bonus_reward_items[0];
    assert_eq!(item.item_id, HashId(0x0260_1663));
    assert_eq!(item.item_num, 1);

    let r = &cfg.login_bonus_rewards[0];
    assert_eq!(r.day_count, 1);
    assert_eq!(r.reward_item_offset, 0);
    assert_eq!(r.reward_item_count, 1);

    let info = &cfg.login_bonus_infos[0];
    assert_eq!(info.id_crc, HashId(0x447A_797C));
    assert_eq!(info.reward_offset, 0);
    assert_eq!(info.reward_count, 1);
}
