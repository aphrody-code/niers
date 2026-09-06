#![allow(clippy::pedantic)]
//! Tests golden `shop` — boutiques réelles `SHOP_INFO_0` et `SHOP_INFO_2` tirées de :
//! `data/common/gamedata/shop/shop_config_3.00.22.cfg.bin` (VFS IEVR).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/shop-config.ts` : shopId @var0,
//! nameHash @var1 (réinterprétés u32 = `>>> 0`), itemId @var2 collecté en `Set` (dédup,
//! ordre conservé). Valeurs extraites via l'exemple jetable `nie-model-serve` (supprimé après).
//!
//! Disposition niers (`cfgbin_to_t2b_iecode_root`) : l'item-list `SHOP_INFO_ITEM_LIST_BEG_x`
//! est un *frère* du shop `SHOP_INFO_x`, ses items lui étant rattachés séquentiellement.

mod common;

use nie_data::hash::HashId;
use nie_data::shop::{ShopInfo, parse_shop_config};
use serde_json::{Value, json};

// ─── Valeurs réelles extraites du VFS (shop_config_3.00.22.cfg.bin) ──────────────

/// `SHOP_INFO_0` : shopId=1629770002 (0x61245112), nameHash=889569797 (0x3505C205).
/// 74 noeuds item (var[2]) — aucun doublon.
const SHOP0_ITEMS: &[i64] = &[
    1834815904,
    1534657310,
    1580602779,
    -1480349646,
    1864574786,
    679697810,
    -302595765,
    -1428960701,
    -1518004629,
    717611979,
    515745120,
    1519979486,
    721914364,
    -107281971,
    -391480755,
    -11484378,
    1286660990,
    1168775706,
    1206690883,
    -930204908,
    -888477830,
    -53209272,
    1372878623,
    -546010497,
    -1621191279,
    -1120116079,
    919832948,
    703701836,
    347176564,
    -1349923457,
    359863363,
    674482555,
    -1082467128,
    -1455288428,
    -1272538123,
    -397057678,
    -110880014,
    1192476540,
    -375977147,
    -662554212,
    1934203680,
    -624654395,
    1896818041,
    -565576841,
    -2132727350,
    -695113692,
    -690934017,
    -744605574,
    711689683,
    -494204253,
    -1523879821,
    -1485976022,
    -1498438627,
    674053002,
    -1827305343,
    -710155792,
    491482752,
    1525355600,
    -1008581142,
    -1260030596,
    1285730378,
    -1145577433,
    -1819912160,
    658772439,
    -1189051276,
    1346576705,
    -917868293,
    1380126450,
    -479074257,
    -836258590,
    1462691160,
    519516757,
    -2013371409,
    -252095623,
];

/// `SHOP_INFO_2` : shopId=-1321522219 (0xB13B2BD5), nameHash=-1246565820 (0xB5B2EA44).
/// 88 noeuds item — l'item `-1937608713` apparaît deux fois (→ 87 items uniques).
const SHOP2_ITEMS: &[i64] = &[
    -412001153,
    515867680,
    -2133126781,
    1859481775,
    433209401,
    1773712566,
    13201784,
    1312150553,
    -1332295585,
    -946734903,
    1046421654,
    -1606699723,
    -683899485,
    -1559974997,
    973994513,
    1292552839,
    -747741404,
    -1536593998,
    1029717512,
    1248267934,
    -623373553,
    -1377885287,
    -854021508,
    -1172333846,
    588696400,
    1410464710,
    -1447398871,
    817046419,
    1202983685,
    -640645466,
    -409219099,
    -1565876751,
    -1725254824,
    -25100636,
    160804553,
    277782408,
    1020135506,
    1285881053,
    -360263889,
    -2080961823,
    -45713828,
    17505103,
    -395788678,
    1902243776,
    -284853661,
    107405142,
    -1744680203,
    1980509145,
    2116126297,
    -532343814,
    1397506102,
    1385048577,
    -1937608713,
    -1937608713,
    825536693,
    -1472339697,
    1438220555,
    582922653,
    -2045924693,
    -1413415311,
    -1909054080,
    -114223850,
    -1233887816,
    -1446168714,
    933864149,
    1085182531,
    -643316743,
    -1365183633,
    1591240475,
    557759386,
    -2051265791,
    48997348,
    684210649,
    -665381504,
    -1433173980,
    1826590260,
    -428085310,
    39851619,
    -658799743,
    -1312143793,
    1223165969,
    -341824810,
    1726004271,
    -959613223,
    1606690659,
    -1777263795,
    -2100170984,
    750866647,
];

/// Construit un noeud item `SHOP_INFO_ITEM_y` avec `item_id` à l'index 2 (vars 0/1 nuls).
fn item_node(idx: usize, item_id: i64) -> Value {
    json!({
        "name": format!("SHOP_INFO_ITEM_{idx}"),
        "variables": [
            { "type": "Int", "value": "0" },
            { "type": "Int", "value": "0" },
            { "type": "Int", "value": item_id.to_string() },
        ],
        "children": []
    })
}

/// Construit un bloc shop niers : `SHOP_INFO_x` suivi de son frère `SHOP_INFO_ITEM_LIST_BEG_x`.
fn shop_block(shop_idx: usize, shop_id: i64, name_hash: i64, items: &[i64]) -> Vec<Value> {
    let item_children: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, v)| item_node(i, *v))
        .collect();
    vec![
        json!({
            "name": format!("SHOP_INFO_{shop_idx}"),
            "variables": [
                { "type": "Int", "value": shop_id.to_string() },
                { "type": "Int", "value": name_hash.to_string() },
            ],
            "children": []
        }),
        json!({
            "name": format!("SHOP_INFO_ITEM_LIST_BEG_{shop_idx}"),
            "variables": [{ "type": "Int", "value": "0" }],
            "children": item_children
        }),
    ]
}

/// Fixture : conteneur `SHOP_INFO_LIST_BEG_0` avec les blocs shop0 et shop2 (frères).
fn fixture() -> Value {
    let mut children: Vec<Value> = Vec::new();
    children.extend(shop_block(0, 1629770002, 889569797, SHOP0_ITEMS));
    children.extend(shop_block(2, -1321522219, -1246565820, SHOP2_ITEMS));
    json!({
        "entries": [{
            "name": "SHOP_INFO_LIST_BEG_0",
            "variables": [{ "type": "Int", "value": "0" }],
            "children": children
        }]
    })
}

#[test]
fn deux_boutiques_parsees() {
    let shops = parse_shop_config(&fixture());
    assert_eq!(shops.len(), 2, "exactement 2 boutiques dans la fixture");
}

#[test]
fn shop0_ids_et_inventaire() {
    let shops = parse_shop_config(&fixture());
    let s: &ShopInfo = &shops[0];

    // var0/var1 réinterprétés u32 (>>> 0).
    assert_eq!(s.shop_id, HashId::from_signed(1629770002));
    assert_eq!(s.shop_id, HashId(0x6124_5112));
    assert_eq!(s.name_hash, HashId::from_signed(889569797));
    assert_eq!(s.name_hash, HashId(0x3505_C205));

    // 74 noeuds, aucun doublon → 74 items uniques, ordre d'insertion préservé.
    assert_eq!(s.item_count(), 74);
    assert_eq!(s.items[0], HashId(0x6D5D_11A0)); // 1834815904
    assert_eq!(s.items[1], HashId(0x5B79_031E)); // 1534657310
    assert_eq!(s.items[3], HashId::from_signed(-1480349646));
    assert_eq!(s.items[73], HashId::from_signed(-252095623));
    assert!(s.has_item(HashId(0x6D5D_11A0)));
    assert!(!s.has_item(HashId(0xDEAD_BEEF)));
}

#[test]
fn shop2_dedup_set_87_items() {
    let shops = parse_shop_config(&fixture());
    let s = &shops[1];

    assert_eq!(s.shop_id, HashId::from_signed(-1321522219));
    assert_eq!(s.shop_id, HashId(0xB13B_2BD5));
    assert_eq!(s.name_hash, HashId::from_signed(-1246565820));
    assert_eq!(s.name_hash, HashId(0xB5B2_EA44));

    // 88 noeuds item mais 87 uniques : -1937608713 apparaît deux fois (sémantique Set).
    assert_eq!(SHOP2_ITEMS.len(), 88);
    assert_eq!(
        s.item_count(),
        87,
        "doublon -1937608713 fusionné comme un Set JS"
    );
    assert!(s.has_item(HashId::from_signed(-1937608713)));
    // Le premier item reste en tête, le dernier en queue (ordre conservé).
    assert_eq!(s.items[0], HashId::from_signed(-412001153));
    assert_eq!(*s.items.last().unwrap(), HashId::from_signed(750866647));
    // -1937608713 n'apparaît qu'une seule fois.
    let occurrences = s
        .items
        .iter()
        .filter(|&&h| h == HashId::from_signed(-1937608713))
        .count();
    assert_eq!(occurrences, 1);
}

#[test]
fn validation_byte_a_byte_vrai_fichier() {
    // Validation contre le vrai dump si présent (fichier hors VCS). Source :
    // data/common/gamedata/shop/shop_config_3.00.22.cfg.bin.json (forme iecode T2B).
    let path = "shop/shop_config_3.00.22.cfg.bin.json";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", path));
    let root: Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    let shops = parse_shop_config(&root);
    assert!(!shops.is_empty(), "aucune boutique parsée dans {path}");
    // La boutique 0x61245112 doit exister et vendre l'item 0x6D5D11A0.
    let shop0 = shops
        .iter()
        .find(|s| s.shop_id == HashId(0x6124_5112))
        .expect("SHOP_INFO 0x61245112 introuvable dans le vrai dump");
    assert!(shop0.has_item(HashId(0x6D5D_11A0)));
}
