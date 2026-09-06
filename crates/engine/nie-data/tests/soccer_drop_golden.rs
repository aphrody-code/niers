#![allow(clippy::pedantic)]
//! Tests golden `soccer_drop` — système de **butin de match** d'IEVR, sur les vrais dumps
//! (skip silencieux si absents du VPS — fragments copyright Level-5 non committés) :
//!
//! - `data/common/gamedata/soccer/soccer_drop_config_5.00.27.00.cfg.bin.json`
//! - `data/common/gamedata/soccer/soccer_drop_config_1.03.20.00.cfg.bin.json`
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/drop-rates.ts` (`loadSpiritDropRates`) pour
//! la jointure de taux d'esprit ; le reste des 12 listes est nouveau (famille jusqu'ici non
//! couverte par nie-data). Chaque valeur assérée est copiée octet pour octet du dump réel.

mod common;

use nie_data::hash::HashId;
use nie_data::soccer_drop::{ItemDropData, parse_soccer_drop_config};

const BASE: &str = "soccer";

fn load_json(filename: &str) -> Option<serde_json::Value> {
    let path = std::format!("{BASE}/{filename}");
    let path = common::chemin(&path)?;
    if !path.is_file() {
        eprintln!("skip : {} absent du corpus", path.display());
        return None;
    }
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path.display()));
    Some(
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide {}: {e}", path.display())),
    )
}

const V5: &str = "soccer_drop_config_5.00.27.00.cfg.bin.json";
const V1: &str = "soccer_drop_config_1.03.20.00.cfg.bin.json";

#[test]
fn v5_comptes_des_douze_listes() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(cfg.rarity_decide_tables.len(), 6);
    assert_eq!(cfg.lottery_nums.len(), 10);
    assert_eq!(cfg.item_drop_data.len(), 94);
    assert_eq!(cfg.item_drop_tables.len(), 26);
    assert_eq!(cfg.spirit_drop_data.len(), 54);
    assert_eq!(cfg.spirit_drop_tables.len(), 22);
    assert_eq!(cfg.spirit_table_data.len(), 568);
    assert_eq!(cfg.spirit_chara_tables.len(), 34);
    assert_eq!(cfg.decide_table_data.len(), 15);
    assert_eq!(cfg.item_table_decide_tables.len(), 2);
    assert_eq!(cfg.exception_drop_charas.len(), 113);
    assert_eq!(cfg.exception_drop_lists.len(), 1);
}

#[test]
fn v5_rarity_decide_table_typo_et_valeurs() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    // [0] : clé JSON réelle `rairtyId` (typo Level-5) — doit être lue.
    let r0 = &cfg.rarity_decide_tables[0];
    assert_eq!(r0.rarity_id, HashId(0xD17D_AF62));
    assert_eq!(r0.normal, HashId(0x81A0_8AD6));
    assert_eq!(r0.legendary, HashId(0x18A9_DB6C));
    // [5] : dernière entrée.
    let r5 = &cfg.rarity_decide_tables[5];
    assert_eq!(r5.rarity_id, HashId(0x675B_8332));
    assert_eq!(r5.normal, HashId(0x18A9_DB6C));
    assert_eq!(r5.growing, HashId(0x6FAE_EBFA));
    assert_eq!(r5.hero, HashId(0x6FAE_EBFA));
}

#[test]
fn v5_lottery_num_bornes() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(cfg.lottery_nums[0].id, HashId(0x43E9_9C1C));
    assert_eq!(cfg.lottery_nums[0].min_num, 3);
    assert_eq!(cfg.lottery_nums[0].max_num, 7);
    // [9] : dernière — tirage fixe de 1.
    assert_eq!(cfg.lottery_nums[9].id, HashId(0xCEE3_7778));
    assert_eq!(cfg.lottery_nums[9].min_num, 1);
    assert_eq!(cfg.lottery_nums[9].max_num, 1);
}

#[test]
fn v5_item_drop_data_et_tranches() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(
        cfg.item_drop_data[0],
        ItemDropData {
            drop_rarity: 0,
            weight: 5
        }
    );
    assert_eq!(
        cfg.item_drop_data[1],
        ItemDropData {
            drop_rarity: 1,
            weight: 5
        }
    );
    assert_eq!(
        cfg.item_drop_data[93],
        ItemDropData {
            drop_rarity: 2,
            weight: 80
        }
    );
    // Tables : [0] id 0x9CCAB1C6 tranche [0,4] ; [25] id 0xC9633DED tranche [93,1].
    assert_eq!(cfg.item_drop_tables[0].id, HashId(0x9CCA_B1C6));
    assert_eq!(cfg.item_drop_tables[0].data, [0, 4]);
    assert_eq!(cfg.item_drop_tables[25].id, HashId(0xC963_3DED));
    assert_eq!(cfg.item_drop_tables[25].data, [93, 1]);
    // Résolution de tranche : table[25] → 1 item (le 94ᵉ, poids 80).
    let slice = cfg
        .item_drop_slice(HashId(0xC963_3DED))
        .expect("table présente");
    assert_eq!(slice.len(), 1);
    assert_eq!(
        slice[0],
        ItemDropData {
            drop_rarity: 2,
            weight: 80
        }
    );
}

#[test]
fn v5_spirit_drop_data_poids_flottants_byte_exact() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    // Poids flottants — comparés au bit près (20.75 / 4.25 / 5.0 exacts en f64).
    assert_eq!(cfg.spirit_drop_data[0].drop_rarity, 0);
    assert_eq!(cfg.spirit_drop_data[0].rarity, 1);
    assert_eq!(
        cfg.spirit_drop_data[0].weight.to_bits(),
        20.75_f64.to_bits()
    );
    assert_eq!(cfg.spirit_drop_data[1].weight.to_bits(), 4.25_f64.to_bits());
    assert_eq!(cfg.spirit_drop_data[2].weight.to_bits(), 20.0_f64.to_bits());
    assert_eq!(cfg.spirit_drop_data[53].drop_rarity, 20);
    assert_eq!(cfg.spirit_drop_data[53].rarity, 4);
    assert_eq!(cfg.spirit_drop_data[53].weight.to_bits(), 5.0_f64.to_bits());
}

#[test]
fn v5_spirit_drop_rates_jointure_proprietaire() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    let rates = cfg.spirit_drop_rates();
    assert_eq!(
        rates.len(),
        54,
        "une ligne par entrée de m_spiritDropDataList"
    );
    // table[0] data[0,0] n'owne rien ; table[1] data[0,2] owne idx 0,1 ; table[2] data[2,3] owne idx 2.
    assert_eq!(rates[0].source_id, Some(HashId(0xBFF8_60D5)));
    assert_eq!(rates[0].weight.to_bits(), 20.75_f64.to_bits());
    assert_eq!(rates[2].source_id, Some(HashId(0xC8FF_5043)));
    // idx 53 appartient à la dernière table (0xBEDED281, data [51,3]).
    assert_eq!(rates[53].source_id, Some(HashId(0xBEDE_D281)));
    // Toutes les lignes sont rattachées (chaque entrée référencée exactement une fois).
    assert!(
        rates.iter().all(|r| r.source_id.is_some()),
        "aucune ligne orpheline"
    );
}

#[test]
fn v5_spirit_et_chara_tables() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(cfg.spirit_table_data[0].chara_id, HashId(0xFDCD_0454));
    assert_eq!(cfg.spirit_table_data[0].weight, 0);
    assert_eq!(cfg.spirit_table_data[0].run_cond, "");
    assert_eq!(cfg.spirit_table_data[567].chara_id, HashId(0x8964_AD01));
    assert_eq!(cfg.spirit_table_data[567].weight, 1);
    // spirit_chara_tables : [0] [0,6], [1] [6,16], [33] [564,4].
    assert_eq!(cfg.spirit_chara_tables[0].id, HashId(0x6CDD_8A22));
    assert_eq!(cfg.spirit_chara_tables[0].data, [0, 6]);
    assert_eq!(cfg.spirit_chara_tables[33].data, [564, 4]);
}

#[test]
fn v5_decide_et_exceptions() {
    let Some(root) = load_json(V5) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(
        cfg.decide_table_data[0].drop_rarity_name_crc,
        HashId(0x7EFD_704E)
    );
    assert_eq!(cfg.decide_table_data[0].item_table_id, HashId(0xF0F3_FEE3));
    assert_eq!(cfg.item_table_decide_tables[0].data, [0, 12]);
    assert_eq!(cfg.item_table_decide_tables[1].data, [12, 3]);
    assert_eq!(
        cfg.exception_drop_charas[0].exception_drop_chara_id,
        HashId(0x8352_22D3)
    );
    assert_eq!(
        cfg.exception_drop_charas[112].exception_drop_chara_id,
        HashId(0xD089_9786)
    );
    // L'unique liste d'exception couvre les 113 persos exclus.
    assert_eq!(cfg.exception_drop_lists[0].id, HashId(0xEEAA_6F2E));
    assert_eq!(cfg.exception_drop_lists[0].data, [0, 113]);
}

#[cfg(feature = "serde")]
#[test]
fn dispatch_typed_atteint_azalee() {
    // Chemin azalee : `family_key` dérive la clé du nom de fichier versionné, puis
    // `decode_by_key` route vers le parseur typé → JSON servable (route /typed, nie-wasm).
    use nie_data::typed::{decode_by_key, family_key};
    let Some(root) = load_json(V5) else { return };
    assert_eq!(
        family_key("soccer_drop_config_5.00.27.00.cfg.bin"),
        "soccer_drop_config"
    );
    let (label, json) = decode_by_key("soccer_drop_config", &root).expect("famille typée câblée");
    assert_eq!(label, "soccer_drop");
    // Le JSON typé expose bien les listes (≠ décodage générique).
    assert_eq!(json["spirit_drop_data"].as_array().map(Vec::len), Some(54));
    assert_eq!(json["item_drop_data"].as_array().map(Vec::len), Some(94));
}

#[test]
fn v1_version_anterieure_sans_exceptions() {
    // Le dump 1.03.20.00 a les MÊMES tables de base mais 0 liste d'exception et moins de spirits.
    let Some(root) = load_json(V1) else { return };
    let cfg = parse_soccer_drop_config(&root);
    assert_eq!(cfg.rarity_decide_tables.len(), 6);
    assert_eq!(cfg.item_drop_data.len(), 94);
    assert_eq!(cfg.spirit_drop_data.len(), 54);
    assert_eq!(
        cfg.spirit_table_data.len(),
        484,
        "moins d'esprits qu'en 5.00.27.00 (568)"
    );
    assert_eq!(cfg.spirit_chara_tables.len(), 30);
    assert_eq!(
        cfg.exception_drop_charas.len(),
        0,
        "pas d'exceptions dans cette version"
    );
    assert_eq!(cfg.exception_drop_lists.len(), 0);
    // La jointure de taux reste cohérente (54 lignes, toutes rattachées).
    let rates = cfg.spirit_drop_rates();
    assert_eq!(rates.len(), 54);
    assert!(rates.iter().all(|r| r.source_id.is_some()));
}
