#![allow(clippy::pedantic)]
//! Tests golden `post::delivery` — valeurs réelles tirées de :
//! `post/delivery_config_1.03.63.00.cfg.bin.json`
//!
//! Layout `lists` : 4 listes (contenus, livraisons, codes, données de mot de passe).

mod common;

use nie_data::hash::HashId;
use nie_data::post::{DeliveryConfig, parse_delivery_config};

const REAL_PATH: &str = "post/delivery_config_1.03.63.00.cfg.bin.json";

fn load_real() -> Option<DeliveryConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_delivery_config(&root))
}

#[test]
fn comptes_listes() {
    let Some(cfg) = load_real() else { return };
    assert_eq!(cfg.contents.len(), 73, "m_DeliveryContentsDataList");
    assert_eq!(cfg.infos.len(), 30, "m_DeliveryInfoList");
    assert_eq!(cfg.password_codes.len(), 14, "m_PasswordCodesList");
    assert_eq!(cfg.password_data.len(), 14, "m_PasswordDataList");
}

#[test]
fn contents_echantillons() {
    let Some(cfg) = load_real() else { return };
    let c0 = &cfg.contents[0];
    assert_eq!(c0.contents_type, 1);
    assert_eq!(c0.item_id_crc, HashId(0x6BB2_4A2D));
    assert_eq!(c0.num, 1);
    assert!(!c0.is_passphrase);

    // [4] : aocIdCrc + replaceItemIdCrc non nuls
    let c4 = &cfg.contents[4];
    assert_eq!(c4.item_id_crc, HashId(0x29F1_A34C));
    assert_eq!(c4.aoc_id_crc, HashId(0x3709_B52A));
    assert_eq!(c4.replace_item_id_crc, HashId(0x3B44_0CA2));

    // dernier [72]
    let c72 = &cfg.contents[72];
    assert_eq!(c72.item_id_crc, HashId(0x149D_BE6B));
    assert_eq!(c72.num, 1);
}

#[test]
fn infos_echantillons() {
    let Some(cfg) = load_real() else { return };
    let i0 = &cfg.infos[0];
    assert_eq!(i0.id_crc, HashId(0x626B_2A47));
    assert_eq!(i0.title, HashId(0x6145_669B));
    assert_eq!(i0.received_flag, HashId(0x066D_378F));
    assert_eq!(i0.new_flag, 1);
    assert_eq!(i0.send_target_type, 2);
    assert_eq!(i0.contents_offset, 0);
    assert_eq!(i0.contents_count, 1);
    assert_eq!(i0.open_cond, "");

    let i1 = &cfg.infos[1];
    assert_eq!(i1.id_crc, HashId(0xFB62_7BFD));
    assert_eq!(i1.contents_offset, 1);
    assert_eq!(i1.contents_count, 5);
    assert_eq!(i1.open_cond, "AAAAABgFNZ81hrQACgEoAAYCNHzR0l4yAAAAAXg=");

    let i29 = &cfg.infos[29];
    assert_eq!(i29.id_crc, HashId(0x3E41_4912));
    assert_eq!(i29.new_flag, 30);
    assert_eq!(i29.contents_offset, 72);
    assert_eq!(i29.contents_count, 1);
}

#[test]
fn contents_of_resout_plage() {
    let Some(cfg) = load_real() else { return };
    // infos[1].deliveryContents = [1, 5] → 5 contenus à partir de l'index 1.
    let slice = cfg.contents_of(&cfg.infos[1]);
    assert_eq!(slice.len(), 5);
    assert_eq!(slice[0].item_id_crc, cfg.contents[1].item_id_crc);
}

#[test]
fn password_codes_litteraux() {
    let Some(cfg) = load_real() else { return };
    // [0] : code japonais en kana, anglais vide
    assert_eq!(cfg.password_codes[0].japanese, "カハワヒコツイノモメオサ");
    assert_eq!(cfg.password_codes[0].english, "");
    // [3] : anglais renseigné, japonais vide
    assert_eq!(cfg.password_codes[3].japanese, "");
    assert_eq!(cfg.password_codes[3].english, "HPKCYJVQWTWX");
    // dernier [13]
    assert_eq!(cfg.password_codes[13].english, "CPVHNYBRBGNR");
}

#[test]
fn password_data_echantillons() {
    let Some(cfg) = load_real() else { return };
    let p0 = &cfg.password_data[0];
    assert_eq!(p0.id, HashId(0x4E32_3AA7));
    assert_eq!(p0.text_id, HashId(0x4D1C_767B));
    assert_eq!(p0.flag_id, HashId(0x9090_8C13));
    assert_eq!(p0.codes_offset, 0);
    assert_eq!(p0.codes_count, 1);

    let p13 = &cfg.password_data[13];
    assert_eq!(p13.id, HashId(0x2743_FF69));
    assert_eq!(p13.flag_id, HashId(0xF9E1_49DD));
    assert_eq!(p13.codes_offset, 13);
    assert_eq!(p13.codes_count, 1);
}
