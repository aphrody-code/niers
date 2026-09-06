// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::pedantic)]
//! Tests golden `soccer_chara_unique_rarity` — entrées réelles de la liste
//! `m_soccerCharaUniqueRarityList` (type `SOCCER_CHARA_UNIQUE_RARITY`, 71 entrées) tirées de :
//! `data/common/gamedata/soccer/soccer_chara_unique_rarity_config_1.03.00.00.cfg.bin`
//! (VFS du jeu Steam ; extrait via `nie_formats::cfgbin` → forme iecode `lists`).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/hero-config.ts` (l.81-104). Détail
//! Level-5 vérifié : le `charaParamIdHero*` n'est exposé que si `isHeroX == true` ET id non
//! nul — plusieurs entrées réelles ont un id non nul avec le flag à `false` (index 0/1/2/4/6).

use nie_data::hash::HashId;
use nie_data::soccer_chara_unique_rarity::{
    HeroType, HeroVariantInfo, hero_type_for_param, parse_hero_config, variants_for_chara,
};
use serde_json::json;

/// Fixture = 6 entrées réelles (index 0,1,2,4,6,70 du dump), couvrant tous les cas :
/// flag vrai+id, flag faux+id non nul, flag vrai+id nul, flag faux+id nul.
fn node_fixture() -> serde_json::Value {
    json!({
        "lists": [{
            "name": "m_soccerCharaUniqueRarityList",
            "typeName": "SOCCER_CHARA_UNIQUE_RARITY",
            "values": [
                // [0] 0x81789A26 — Feu faux+nul, Noir vrai, Rose vrai.
                { "charaId": "0x81789A26",
                  "isHeroFire": false, "charaParamIdHeroFire": "0x00000000",
                  "isHeroBlack": true, "charaParamIdHeroBlack": "0x9634AFB0",
                  "isHeroPink": true,  "charaParamIdHeroPink": "0xBFFC1B42" },
                // [1] 0x13B82253 — Feu vrai, Noir FAUX mais id 0x04F417C5 (doit rester None), Rose faux+nul.
                { "charaId": "0x13B82253",
                  "isHeroFire": true,  "charaParamIdHeroFire": "0x2D3CA337",
                  "isHeroBlack": false, "charaParamIdHeroBlack": "0x04F417C5",
                  "isHeroPink": false, "charaParamIdHeroPink": "0x00000000" },
                // [2] 0x6CFE2D23 — Feu vrai, Noir vrai, Rose faux+nul.
                { "charaId": "0x6CFE2D23",
                  "isHeroFire": true,  "charaParamIdHeroFire": "0x527AAC47",
                  "isHeroBlack": true, "charaParamIdHeroBlack": "0x7BB218B5",
                  "isHeroPink": false, "charaParamIdHeroPink": "0x00000000" },
                // [4] 0x977FF3FD — Feu FAUX mais id 0x8033C66B (None), Noir faux+nul, Rose vrai.
                { "charaId": "0x977FF3FD",
                  "isHeroFire": false, "charaParamIdHeroFire": "0x8033C66B",
                  "isHeroBlack": false, "charaParamIdHeroBlack": "0x00000000",
                  "isHeroPink": true,  "charaParamIdHeroPink": "0xA9FB7299" },
                // [6] 0xC6FCB2B3 — Feu vrai, Noir faux+nul, Rose FAUX mais id 0xD1B08725 (None).
                { "charaId": "0xC6FCB2B3",
                  "isHeroFire": true,  "charaParamIdHeroFire": "0xF87833D7",
                  "isHeroBlack": false, "charaParamIdHeroBlack": "0x00000000",
                  "isHeroPink": false, "charaParamIdHeroPink": "0xD1B08725" },
                // [70] 0x835222D3 — Feu faux+nul, Noir vrai, Rose vrai (dernière entrée).
                { "charaId": "0x835222D3",
                  "isHeroFire": false, "charaParamIdHeroFire": "0x00000000",
                  "isHeroBlack": true, "charaParamIdHeroBlack": "0xBDD6A3B7",
                  "isHeroPink": true,  "charaParamIdHeroPink": "0x13BE3226" }
            ]
        }]
    })
}

#[test]
fn parse_compte_et_chara_ids() {
    let list = parse_hero_config(&node_fixture());
    assert_eq!(list.len(), 6, "6 entrées de fixture");
    assert_eq!(list[0].chara_id, HashId(0x8178_9A26));
    assert_eq!(list[5].chara_id, HashId(0x8352_22D3));
}

#[test]
fn entree_0_flags_et_param_ids() {
    let list = parse_hero_config(&node_fixture());
    let e: &HeroVariantInfo = &list[0];
    // Feu : flag faux + id nul → None.
    assert!(!e.has_fire);
    assert_eq!(e.fire_param_id, None);
    // Noir : flag vrai + id → exposé.
    assert!(e.has_black);
    assert_eq!(e.black_param_id, Some(HashId(0x9634_AFB0)));
    // Rose : flag vrai + id → exposé.
    assert!(e.has_pink);
    assert_eq!(e.pink_param_id, Some(HashId(0xBFFC_1B42)));
    // Accès générique.
    assert_eq!(e.param_id(HeroType::Black), Some(HashId(0x9634_AFB0)));
    assert_eq!(e.param_id(HeroType::Fire), None);
}

#[test]
fn flag_faux_mais_id_non_nul_reste_none() {
    // Détail Level-5 critique (positions dans la fixture, pas indices du dump) :
    // pos1 0x13B82253 Noir=false+0x04F417C5, pos3 0x977FF3FD Feu=false+0x8033C66B,
    // pos4 0xC6FCB2B3 Rose=false+0xD1B08725 → tous None (le flag gate l'id), comme inagle.
    let list = parse_hero_config(&node_fixture());
    assert!(!list[1].has_black);
    assert_eq!(list[1].black_param_id, None);
    assert!(!list[3].has_fire);
    assert_eq!(list[3].fire_param_id, None);
    assert!(!list[4].has_pink);
    assert_eq!(list[4].pink_param_id, None);
    // Mais les variantes actives de ces mêmes entrées restent exposées.
    assert_eq!(list[1].fire_param_id, Some(HashId(0x2D3C_A337)));
    assert_eq!(list[3].pink_param_id, Some(HashId(0xA9FB_7299)));
    assert_eq!(list[4].fire_param_id, Some(HashId(0xF878_33D7)));
}

#[test]
fn hero_type_for_param_resout_les_variantes_exposees() {
    let list = parse_hero_config(&node_fixture());
    // Param ids réellement exposés → type correct.
    assert_eq!(
        hero_type_for_param(&list, HashId(0x9634_AFB0)),
        Some(HeroType::Black)
    );
    assert_eq!(
        hero_type_for_param(&list, HashId(0xBFFC_1B42)),
        Some(HeroType::Pink)
    );
    assert_eq!(
        hero_type_for_param(&list, HashId(0x2D3C_A337)),
        Some(HeroType::Fire)
    );
    // Param id présent dans le dump mais NON exposé (flag faux) → introuvable.
    assert_eq!(hero_type_for_param(&list, HashId(0x04F4_17C5)), None);
    assert_eq!(hero_type_for_param(&list, HashId(0x8033_C66B)), None);
    // Inconnu → None.
    assert_eq!(hero_type_for_param(&list, HashId(0xDEAD_BEEF)), None);
}

#[test]
fn variants_for_chara_retrouve_par_chara_id() {
    let list = parse_hero_config(&node_fixture());
    let v = variants_for_chara(&list, HashId(0x6CFE_2D23)).expect("entrée 0x6CFE2D23");
    assert_eq!(v.fire_param_id, Some(HashId(0x527A_AC47)));
    assert_eq!(v.black_param_id, Some(HashId(0x7BB2_18B5)));
    assert_eq!(v.pink_param_id, None);
    assert!(variants_for_chara(&list, HashId(0x0000_0001)).is_none());
}
