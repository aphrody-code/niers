#![allow(clippy::pedantic)]
//! Tests golden `post::delivery_list` — valeurs réelles tirées de :
//! `post/delivery_list_config.cfg.bin.json`
//!
//! Layout `entries` : `DELIVERY_INFO_LIST_BEG_0` → 20 `DELIVERY_INFO_N` (10 vars chacun),
//! chacun portant un sous-arbre `DELIVERY_INFO_DATA_LIST_BEG_N` → `DELIVERY_INFO_DATA_M`
//! (3 vars). 74 contenus au total.

mod common;

use nie_data::hash::HashId;
use nie_data::post::{DeliveryListInfo, parse_delivery_list_config};

const REAL_PATH: &str = "post/delivery_list_config.cfg.bin.json";

fn load_real() -> Option<Vec<DeliveryListInfo>> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_delivery_list_config(&root))
}

#[test]
fn compte_livraisons_et_contenus() {
    let Some(infos) = load_real() else { return };
    assert_eq!(
        infos.len(),
        20,
        "20 DELIVERY_INFO sous DELIVERY_INFO_LIST_BEG_0"
    );
    let total_data: usize = infos.iter().map(|i| i.data.len()).sum();
    assert_eq!(total_data, 74, "74 DELIVERY_INFO_DATA au total");
}

#[test]
fn info0_champs_et_contenus() {
    let Some(infos) = load_real() else { return };
    let i0 = &infos[0];
    assert_eq!(i0.id_crc, HashId(0xDD72_D708));
    assert_eq!(i0.title, HashId(0x595F_86E4));
    assert_eq!(i0.received_flag, HashId(0xE922_D390));
    // var[3] = "0" (pas de base64 ici)
    assert_eq!(i0.open_cond, "0");
    // var[4..=9] bruts
    assert_eq!(i0.raw_tail, [1, -1375673200, 0, 1, 1, 0]);
    // 4 contenus (DELIVERY_INFO_DATA_LIST_BEG_0 compte = 4)
    assert_eq!(i0.data.len(), 4);
    let d0 = &i0.data[0];
    assert_eq!(d0.item_id_crc, HashId(0xC5A2_7A3C));
    assert_eq!(d0.num, 1);
    assert_eq!(d0.sub_id_crc, HashId(0xED5F_1133));
}

#[test]
fn info9_champs() {
    let Some(infos) = load_real() else { return };
    let i9 = &infos[9];
    assert_eq!(i9.id_crc, HashId(0xA4AE_6FAC));
    assert_eq!(i9.title, HashId(0xD33D_22CC));
    assert_eq!(i9.received_flag, HashId(0x8D4E_1694));
    assert_eq!(i9.raw_tail, [1, -673424332, 0, 8, 43, 0]);
}

#[test]
fn info19_open_cond_base64() {
    let Some(infos) = load_real() else { return };
    let i19 = &infos[19];
    assert_eq!(i19.id_crc, HashId(0x2D0A_437C));
    assert_eq!(i19.title, HashId(0x889D_3DAD));
    assert_eq!(i19.received_flag, HashId(0x38E0_68D9));
    // var[3] est ici un base64 opaque
    assert_eq!(i19.open_cond, "AAAAABgFNSo9RUMACgEoAAYCNE/IO9syAAAAAXg=");
    assert_eq!(i19.raw_tail, [0, 0, 0, 17, 91, 0]);
    // 2 contenus, premier : [-337509745, 5, -735452770]
    assert_eq!(i19.data.len(), 2);
    assert_eq!(i19.data[0].num, 5);
}
