#![allow(clippy::pedantic)]
//! Tests golden `players_universe` — valeurs réelles tirées du VFS du jeu :
//! `data/common/gamedata/players_universe/players_universe_config_1.03.59.00.cfg.bin`
//! et `players_universe_event_config.cfg.bin`
//! (montés depuis `/mnt/c/Program Files (x86)/Steam/steamapps/common/INAZUMA ELEVEN
//! Victory Road`, extraits via `nie_formats::cfgbin` → forme iecode `lists`).
//!
//! Port 1:1 d'inagle `parsers/constellation.ts` + `parsers/star-sign.ts`. Layout `lists` :
//! 10 listes (config) + 1 liste (event). Les valeurs ci-dessous sont les **vraies** lignes
//! du dump (premières/dernières entrées), embarquées en fixture.

mod common;

use nie_data::hash::HashId;
use nie_data::players_universe::{
    PlayersUniverseConfig, StarInfo, StarKeyInfo, parse_players_universe_config,
    parse_players_universe_event_config,
};
use serde_json::json;

// ════════════════════════════════════════════════════════════════════════════════
//  Fixture embarquée — vraies lignes extraites du dump (sous-ensemble représentatif).
// ════════════════════════════════════════════════════════════════════════════════

/// Reconstruit un `root` iecode `{ lists: [...] }` avec les vraies lignes du dump.
fn fixture() -> serde_json::Value {
    json!({
        "lists": [
            { "name": "m_universeInfoList", "values": [
                { "enableCond": "0xFFFFFFFF", "locatorNameHash": "0x3752BB92",
                  "universeIdCrc": "0xC26FDF3D", "universeInfoTextId": "0x1937FDB5",
                  "universeLockTextId": "0x144805DA", "universeNameId": "0x35101955" }
            ]},
            { "name": "m_starKeyRouteInfoList", "values": [
                { "starKeyNameHash": "0x011F09E8", "starKeyNameHashLinkDown": "0x0672CDF1",
                  "starKeyNameHashLinkLeft": "0x00000000", "starKeyNameHashLinkRight": "0x9F7B9C4B",
                  "starKeyNameHashLinkUp": "0x98165852" },
                { "starKeyNameHash": "0x12F422CA", "starKeyNameHashLinkDown": "0x00000000",
                  "starKeyNameHashLinkLeft": "0x00000000", "starKeyNameHashLinkRight": "0x00000000",
                  "starKeyNameHashLinkUp": "0x00000000" }
            ]},
            { "name": "m_starInfoList", "values": [
                { "numTextureNameHash": "0x603EB1D7", "rareStarTextureNameHash": "0x170351A0",
                  "starAfterTextureNameHash": "0x61812B9A", "starKeyInfoList": [0, 8],
                  "starLayerNameHash": "0x0831BA97", "starLocatorNameHash": "0xF8EAC758",
                  "starNameHash": "0x193E6BEA", "starSignLayerNameHash": "0xB6662475",
                  "starTextureNameHash": "0xFE773B4A" },
                { "numTextureNameHash": "0x3C14D282", "rareStarTextureNameHash": "0x4B2932F5",
                  "starAfterTextureNameHash": "0x0146A27F", "starKeyInfoList": [232, 8],
                  "starLayerNameHash": "0x7E11B7A8", "starLocatorNameHash": "0x347FB99C",
                  "starNameHash": "0xC5BAF95F", "starSignLayerNameHash": "0x4CD2ED4D",
                  "starTextureNameHash": "0xA25D581F" }
            ]},
            { "name": "m_starKeyInfoList", "values": [
                { "flagIndex": 0, "isRarePackStar": false, "lotteryStarSignIdCrc": "0x00000000",
                  "starKeyLocatorNameHash": "0x43C1786E", "starKeyNameHash": "0x011F09E8",
                  "winContentTextureNameHash": "0x00000000" },
                { "flagIndex": 8, "isRarePackStar": false, "lotteryStarSignIdCrc": "0x00000000",
                  "starKeyLocatorNameHash": "0x43C1786E", "starKeyNameHash": "0x13140F1B",
                  "winContentTextureNameHash": "0x00000000" },
                { "flagIndex": 247, "isRarePackStar": true, "lotteryStarSignIdCrc": "0x00000000",
                  "starKeyLocatorNameHash": "0x3A1DC0CA", "starKeyNameHash": "0x12F422CA",
                  "winContentTextureNameHash": "0x00000000" }
            ]},
            { "name": "m_universeStarSignInfoList", "values": [
                { "starSignIdCrc": "0xCED5BE45" },
                { "starSignIdCrc": "0x8975C495" },
                { "starSignIdCrc": "0xB415ED25" },
                { "starSignIdCrc": "0x7521E55B" }
            ]},
            { "name": "m_universeStarSignSetDataList", "values": [
                { "universeIdCrc": "0xC26FDF3D", "universeStarSignInfoList": [0, 30] }
            ]},
            { "name": "m_starSignInfoList", "values": [
                { "clearFlagIndex": 0, "dropCharacterNum": 12, "enableCond": "0xFFFFFFFF",
                  "keyItemId": "0x64EB1725", "keyItemNum": 5, "starKeyNameHash": "0x193E6BEA",
                  "starSignIdCrc": "0xCED5BE45", "starSignInfoTextId": "0xE0FDE179",
                  "starSignNameId": "0xE0C8FB97", "starSignNo": 1 },
                { "clearFlagIndex": 29, "dropCharacterNum": 12, "enableCond": "0xFFFFFFFF",
                  "keyItemId": "0x64EB1725", "keyItemNum": 5, "starKeyNameHash": "0xC5BAF95F",
                  "starSignIdCrc": "0x7521E55B", "starSignInfoTextId": "0x1A492841",
                  "starSignNameId": "0xBCE298C2", "starSignNo": 30 }
            ]},
            { "name": "m_starSignCharaInfoList", "values": [
                { "charaParamId": "0xDCD5DB61", "charaRarity": 7, "charaRateBoostA": 0,
                  "charaRateBoostB": 0, "charaRateBoostC": 0, "charaRateBoostD": 0,
                  "charaRateDefault": 1, "enableCond": "0xFFFFFFFF", "isRemarkable": true },
                { "charaParamId": "0x43AB265F", "charaRarity": 7, "charaRateBoostA": 0,
                  "charaRateBoostB": 0, "charaRateBoostC": 0, "charaRateBoostD": 0,
                  "charaRateDefault": 1, "enableCond": "0xFFFFFFFF", "isRemarkable": true },
                { "charaParamId": "0x2DB0BE2E", "charaRarity": 0, "charaRateBoostA": 0,
                  "charaRateBoostB": 0, "charaRateBoostC": 0, "charaRateBoostD": 0,
                  "charaRateDefault": 1, "enableCond": "0xFFFFFFFF", "isRemarkable": false }
            ]},
            { "name": "m_starSignRarityRateInfoList", "values": [
                { "rarityRateBoostA": 0, "rarityRateBoostB": 0, "rarityRateBoostC": 0,
                  "rarityRateBoostD": 0, "rarityRateDefault": 1, "rarityType": 2,
                  "starSignCharaInfoList": [0, 4] },
                { "rarityRateBoostA": 0, "rarityRateBoostB": 0, "rarityRateBoostC": 0,
                  "rarityRateBoostD": 0, "rarityRateDefault": 19, "rarityType": 1,
                  "starSignCharaInfoList": [4, 5] },
                { "rarityRateBoostA": 0, "rarityRateBoostB": 0, "rarityRateBoostC": 0,
                  "rarityRateBoostD": 0, "rarityRateDefault": 180, "rarityType": 0,
                  "starSignCharaInfoList": [9, 181] }
            ]},
            { "name": "m_starSignCharaSetDataList", "values": [
                { "starSignIdCrc": "0xCED5BE45", "starSignRarityRateInfoList": [0, 3] },
                { "starSignIdCrc": "0x313DC35E", "starSignRarityRateInfoList": [87, 3] }
            ]}
        ]
    })
}

/// Fixture event : 3 vraies lignes de `m_playersUniverseResultEffectInfoList`.
fn event_fixture() -> serde_json::Value {
    json!({
        "lists": [
            { "name": "m_playersUniverseResultEffectInfoList", "values": [
                { "id": 1, "moveDegree": 160.0, "moveDistance": 25, "targetLocatorCrc": "0xED2AA647" },
                { "id": 6, "moveDegree": 20.0, "moveDistance": 25, "targetLocatorCrc": "0x734E33E4" },
                { "id": 12, "moveDegree": -20.0, "moveDistance": 25, "targetLocatorCrc": "0x61FB9C0A" }
            ]}
        ]
    })
}

// ════════════════════════════════════════════════════════════════════════════════
//  Tests sur la fixture embarquée.
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn universe_info_champs() {
    let cfg = parse_players_universe_config(&fixture());
    assert_eq!(cfg.universe_info.len(), 1);
    let u = &cfg.universe_info[0];
    assert_eq!(u.universe_id_crc, HashId(0xC26F_DF3D));
    assert_eq!(u.universe_name_id, HashId(0x3510_1955));
    assert_eq!(u.universe_info_text_id, HashId(0x1937_FDB5));
    assert_eq!(u.universe_lock_text_id, HashId(0x1448_05DA));
    assert_eq!(u.locator_name_hash, HashId(0x3752_BB92));
    assert_eq!(u.enable_cond, "0xFFFFFFFF");
}

#[test]
fn star_key_route_liens() {
    let cfg = parse_players_universe_config(&fixture());
    let r0 = &cfg.star_key_routes[0];
    assert_eq!(r0.star_key_name_hash, HashId(0x011F_09E8));
    assert_eq!(r0.link_up, HashId(0x9816_5852));
    assert_eq!(r0.link_down, HashId(0x0672_CDF1));
    assert_eq!(r0.link_right, HashId(0x9F7B_9C4B));
    assert_eq!(r0.link_left, HashId::ZERO);
    // Dernière route (239) : feuille sans voisin.
    let last = cfg.star_key_routes.last().unwrap();
    assert_eq!(last.star_key_name_hash, HashId(0x12F4_22CA));
    assert_eq!(last.link_up, HashId::ZERO);
    assert_eq!(last.link_right, HashId::ZERO);
}

#[test]
fn star_info_textures_et_plage() {
    let cfg = parse_players_universe_config(&fixture());
    let s0 = &cfg.stars[0];
    assert_eq!(s0.star_name_hash, HashId(0x193E_6BEA));
    assert_eq!(s0.star_texture_name_hash, HashId(0xFE77_3B4A));
    assert_eq!(s0.star_after_texture_name_hash, HashId(0x6181_2B9A));
    assert_eq!(s0.rare_star_texture_name_hash, HashId(0x1703_51A0));
    assert_eq!(s0.num_texture_name_hash, HashId(0x603E_B1D7));
    assert_eq!(s0.star_key_offset, 0);
    assert_eq!(s0.star_key_count, 8);
    // Dernière étoile (29).
    let s29 = &cfg.stars[1];
    assert_eq!(s29.star_name_hash, HashId(0xC5BA_F95F));
    assert_eq!(s29.star_key_offset, 232);
    assert_eq!(s29.star_key_count, 8);
}

#[test]
fn star_key_info_rare_pack() {
    let cfg = parse_players_universe_config(&fixture());
    let k0 = &cfg.star_keys[0];
    assert_eq!(k0.star_key_name_hash, HashId(0x011F_09E8));
    assert_eq!(k0.star_key_locator_name_hash, HashId(0x43C1_786E));
    assert_eq!(k0.flag_index, 0);
    assert!(!k0.is_rare_pack_star);
    assert_eq!(k0.lottery_star_sign_id_crc, HashId::ZERO);
    // Étoile rare en fin de liste.
    let last = cfg.star_keys.last().unwrap();
    assert_eq!(last.star_key_name_hash, HashId(0x12F4_22CA));
    assert_eq!(last.flag_index, 247);
    assert!(last.is_rare_pack_star);
}

#[test]
fn star_sign_info_champs() {
    let cfg = parse_players_universe_config(&fixture());
    let s0 = &cfg.star_signs[0];
    assert_eq!(s0.star_sign_id_crc, HashId(0xCED5_BE45));
    assert_eq!(s0.star_sign_name_id, HashId(0xE0C8_FB97));
    assert_eq!(s0.star_sign_info_text_id, HashId(0xE0FD_E179));
    assert_eq!(s0.star_sign_no, 1);
    assert_eq!(s0.key_item_id, HashId(0x64EB_1725));
    assert_eq!(s0.key_item_num, 5);
    assert_eq!(s0.drop_character_num, 12);
    assert_eq!(s0.star_key_name_hash, HashId(0x193E_6BEA));
    assert_eq!(s0.clear_flag_index, 0);
    assert_eq!(s0.enable_cond, "0xFFFFFFFF");
    // signe 30 : starSignNo=30, clearFlagIndex=29.
    let s29 = &cfg.star_signs[1];
    assert_eq!(s29.star_sign_id_crc, HashId(0x7521_E55B));
    assert_eq!(s29.star_sign_no, 30);
    assert_eq!(s29.clear_flag_index, 29);
    // Recoupe StarInfo : starSignInfo[0].starKeyNameHash == starInfo[0].starNameHash.
    assert_eq!(s0.star_key_name_hash, cfg.stars[0].star_name_hash);
}

#[test]
fn star_sign_chara_rarete_et_taux() {
    let cfg = parse_players_universe_config(&fixture());
    let c0 = &cfg.star_sign_charas[0];
    assert_eq!(c0.chara_param_id, HashId(0xDCD5_DB61));
    assert_eq!(c0.chara_rarity, 7);
    assert_eq!(c0.chara_rate_default, 1);
    assert_eq!(c0.chara_rate_boost_a, 0);
    assert!(c0.is_remarkable);
    assert_eq!(c0.enable_cond, "0xFFFFFFFF");
    // Dernier perso de la fixture (entrée 5009 du dump) : Normal, non-remarquable.
    let last = cfg.star_sign_charas.last().unwrap();
    assert_eq!(last.chara_param_id, HashId(0x2DB0_BE2E));
    assert_eq!(last.chara_rarity, 0);
    assert!(!last.is_remarkable);
}

#[test]
fn rarity_rate_plages() {
    let cfg = parse_players_universe_config(&fixture());
    let r0 = &cfg.star_sign_rarity_rates[0];
    assert_eq!(r0.rarity_type, 2);
    assert_eq!(r0.rarity_rate_default, 1);
    assert_eq!(r0.chara_offset, 0);
    assert_eq!(r0.chara_count, 4);
    let r1 = &cfg.star_sign_rarity_rates[1];
    assert_eq!(r1.rarity_type, 1);
    assert_eq!(r1.rarity_rate_default, 19);
    assert_eq!(r1.chara_offset, 4);
    assert_eq!(r1.chara_count, 5);
    let r2 = &cfg.star_sign_rarity_rates[2];
    assert_eq!(r2.rarity_type, 0);
    assert_eq!(r2.rarity_rate_default, 180);
    assert_eq!(r2.chara_offset, 9);
    assert_eq!(r2.chara_count, 181);
}

#[test]
fn chara_set_data_et_universe_set() {
    let cfg = parse_players_universe_config(&fixture());
    let set0 = &cfg.star_sign_chara_sets[0];
    assert_eq!(set0.star_sign_id_crc, HashId(0xCED5_BE45));
    assert_eq!(set0.rate_offset, 0);
    assert_eq!(set0.rate_count, 3);
    // Set du dernier signe (29) : raretés [87, 3].
    let set29 = &cfg.star_sign_chara_sets[1];
    assert_eq!(set29.star_sign_id_crc, HashId(0x313D_C35E));
    assert_eq!(set29.rate_offset, 87);
    assert_eq!(set29.rate_count, 3);
    // Univers → 30 signes.
    let us = &cfg.universe_star_sign_sets[0];
    assert_eq!(us.universe_id_crc, HashId(0xC26F_DF3D));
    assert_eq!(us.star_sign_offset, 0);
    assert_eq!(us.star_sign_count, 30);
    // 1er signe de l'univers.
    assert_eq!(
        cfg.universe_star_signs[0].star_sign_id_crc,
        HashId(0xCED5_BE45)
    );
}

#[test]
fn accesseurs_offset_count() {
    // Fixture auto-cohérente (vraies valeurs, plages contiguës) pour tester le slicing.
    let cfg = parse_players_universe_config(&fixture());
    // rates_of(set0) → 3 raretés [0,3] dans la fixture (qui en contient 3).
    let set0 = &cfg.star_sign_chara_sets[0];
    let rates = cfg.rates_of(set0);
    assert_eq!(rates.len(), 3);
    assert_eq!(rates[0].rarity_type, 2);
    // charas_of(rate0) → [0,4] mais la fixture n'a que 3 personnages → tranche bornée vide.
    let charas = cfg.charas_of(&rates[0]);
    assert!(
        charas.is_empty(),
        "plage hors bornes → tranche vide (saturée)"
    );

    // Petit config synthétique aux vraies valeurs, plages valides.
    let mini = json!({ "lists": [
        { "name": "m_starInfoList", "values": [
            { "starNameHash": "0x193E6BEA", "starKeyInfoList": [0, 2] }
        ]},
        { "name": "m_starKeyInfoList", "values": [
            { "starKeyNameHash": "0x011F09E8", "flagIndex": 0, "isRarePackStar": false },
            { "starKeyNameHash": "0x13140F1B", "flagIndex": 8, "isRarePackStar": false }
        ]}
    ]});
    let m: PlayersUniverseConfig = parse_players_universe_config(&mini);
    let star: &StarInfo = &m.stars[0];
    let keys: &[StarKeyInfo] = m.star_keys_of(star);
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].star_key_name_hash, HashId(0x011F_09E8));
    assert_eq!(keys[1].flag_index, 8);
}

#[test]
fn event_effets_resultat() {
    let effects = parse_players_universe_event_config(&event_fixture());
    assert_eq!(effects.len(), 3);
    let e0 = &effects[0];
    assert_eq!(e0.id, 1);
    assert!((e0.move_degree - 160.0).abs() < 1e-6);
    assert_eq!(e0.move_distance, 25);
    assert_eq!(e0.target_locator_crc, HashId(0xED2A_A647));
    // Angle négatif sur le dernier effet (id 12).
    let e12 = effects.last().unwrap();
    assert_eq!(e12.id, 12);
    assert!((e12.move_degree - (-20.0)).abs() < 1e-6);
    assert_eq!(e12.target_locator_crc, HashId(0x61FB_9C0A));
}

// ════════════════════════════════════════════════════════════════════════════════
//  Test optionnel sur le vrai dump complet (sauté si absent du disque, comme les
//  autres golden : le cfg.bin vit dans le VFS, pas extrait en .json par défaut).
//
//  Vérité terrain = le .json de référence (5010 charas). NB : le .cfg.bin RDBN posé à
//  côté est un snapshot ANTÉRIEUR (4793 charas, parsé byte-exact par nie-formats — les
//  9 autres listes coïncident) ; le pool de tirage a grossi entre les deux extractions,
//  même nom de version dans le fichier. Le golden valide le parsing nie-data du .json.
// ════════════════════════════════════════════════════════════════════════════════

const REAL_PATH: &str = "players_universe/players_universe_config_1.03.59.00.cfg.bin.json";

#[test]
fn comptes_listes_vrai_dump() {
    if !std::path::Path::new(REAL_PATH).exists() {
        return;
    }
    let content = std::fs::read_to_string(REAL_PATH)
        .unwrap_or_else(|e| panic!("Impossible de lire {REAL_PATH}: {e}"));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    let cfg = parse_players_universe_config(&root);
    assert_eq!(cfg.universe_info.len(), 1, "m_universeInfoList");
    assert_eq!(cfg.star_key_routes.len(), 240, "m_starKeyRouteInfoList");
    assert_eq!(cfg.stars.len(), 30, "m_starInfoList");
    assert_eq!(cfg.star_keys.len(), 240, "m_starKeyInfoList");
    assert_eq!(
        cfg.universe_star_signs.len(),
        30,
        "m_universeStarSignInfoList"
    );
    assert_eq!(
        cfg.universe_star_sign_sets.len(),
        1,
        "m_universeStarSignSetDataList"
    );
    assert_eq!(cfg.star_signs.len(), 30, "m_starSignInfoList");
    assert_eq!(cfg.star_sign_charas.len(), 5010, "m_starSignCharaInfoList");
    assert_eq!(
        cfg.star_sign_rarity_rates.len(),
        90,
        "m_starSignRarityRateInfoList"
    );
    assert_eq!(
        cfg.star_sign_chara_sets.len(),
        30,
        "m_starSignCharaSetDataList"
    );
    // Spot-check profond cohérent avec la fixture embarquée.
    assert_eq!(cfg.stars[0].star_name_hash, HashId(0x193E_6BEA));
    assert_eq!(
        cfg.star_sign_charas[5009].chara_param_id,
        HashId(0x2DB0_BE2E)
    );
}
