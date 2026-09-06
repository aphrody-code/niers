#![allow(clippy::pedantic)]
//! Tests golden `stadium` — noeud réel `SOCCER_OPTION_FIELD_INFO_LIST_BEG_0` (81 enfants)
//! extrait du VFS de jeu :
//! `data/common/gamedata/soccer/soccer_game_option.cfg.bin`
//! (Steam : `INAZUMA ELEVEN Victory Road/data/...`, format T2B `entries`, 12464 octets).
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/stadium-config.ts`
//! (`parseFieldEntry` / `parseContent`). Vérité terrain = les 81 entrées
//! `SOCCER_OPTION_FIELD_INFO_N`, chacune : `[fieldId(Int), index(Int),
//! condition(Int=0 ou String), imagePath(String), _(Int), _(Int)]`.
//!
//! La fixture `RAW` ci-dessous EST le dump réel (extrait via `nie_formats::cfgbin` +
//! la conversion iecode de `nie-model-serve`). Aucune valeur inventée.

use nie_data::hash::HashId;
use nie_data::stadium::{Stadium, parse_stadium_config};
use serde_json::{Value, json};

/// Les 81 entrées réelles : `(fieldId, index, condition, imagePath_brut, var4, var5)`.
/// `condition` vide ⇒ la `var[2]` du dump était l'entier 0 (cas de la seule entrée 0).
const RAW: &[(i64, i64, &str, &str, i64, i64)] = &[
    (
        -826611768,
        0,
        "",
        "#/menu/220_img/stadium/img_room_s90g001.g4tx",
        1045184524,
        -1488645706,
    ),
    (
        585721253,
        1,
        "AAAAABgFNRftNPcACgEoAAYCNI7qCisyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s10g001.g4tx",
        -2053840364,
        480097198,
    ),
    (
        149202806,
        2,
        "AAAAABgFNRftNPcACgEoAAYCNJfxO2oyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s42g001.g4tx",
        -1926848474,
        338686364,
    ),
    (
        897879750,
        3,
        "AAAAABgFNRftNPcACgEoAAYCNA74atAyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s43g001.g4tx",
        1099735224,
        -662310654,
    ),
    (
        -2019274026,
        4,
        "AAAAABgFNRftNPcACgEoAAYCNHn/WkYyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s44g001.g4tx",
        1263118753,
        -767494117,
    ),
    (
        -1161528474,
        5,
        "AAAAABgFNRftNPcACgEoAAYCNOebz+UyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s45g001.g4tx",
        -2015123137,
        518715525,
    ),
    (
        679591550,
        6,
        "AAAAABgFNRftNPcACgEoAAYCNBfjW5EyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s33g001.g4tx",
        636341617,
        -1125868341,
    ),
    (
        -680934555,
        7,
        "AAAAABgFNRftNPcACgEoAAYCNJCc/3MyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s14g001.g4tx",
        31386862,
        -1730691756,
    ),
    (
        529091605,
        8,
        "AAAAABgFNRftNPcACgEoAAYCNKFsFQkyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s11g001.g4tx",
        1228857994,
        -801656016,
    ),
    (
        1329913254,
        9,
        "AAAAABgFNRftNPcACgEoAAYCNKKqnfEyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s41g001.g4tx",
        61450181,
        -1700727169,
    ),
    (
        -458894223,
        10,
        "AAAAABgFNRftNPcACgEoAAYCNGDkawcyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s09g001.g4tx",
        -1918833284,
        346668230,
    ),
    (
        1915024406,
        11,
        "AAAAABgFNRftNPcACgEoAAYCNH6Snl8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s40g001.g4tx",
        -821844133,
        1443559137,
    ),
    (
        -43777610,
        12,
        "AAAAABgFNRftNPcACgEoAAYCNO4tg84yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s46g001.g4tx",
        158117596,
        -1872363674,
    ),
    (
        1864484014,
        13,
        "AAAAABgFNRftNPcACgEoAAYCNJkqs1gyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s30g001.g4tx",
        -1419588974,
        845912872,
    ),
    (
        -1073469434,
        14,
        "AAAAABgFNRftNPcACgEoAAYCNMvbWD8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s47g001.g4tx",
        -976835006,
        1557135352,
    ),
    (
        1112850391,
        15,
        "AAAAABgFNRftNPcACgEoAAYCNLzcaKkyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s48g001.g4tx",
        484883694,
        -2047873708,
    ),
    (
        2134152807,
        16,
        "AAAAABgFNRftNPcACgEoAAYCNCXVORMyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s49g001.g4tx",
        -800277392,
        1229319626,
    ),
    (
        -1183307853,
        17,
        "AAAAABgFNRftNPcACgEoAAYCNP6A/qQyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s50g001.g4tx",
        -663178472,
        1097982626,
    ),
    (
        -2078798333,
        18,
        "AAAAABgFNRftNPcACgEoAAYCNFLSCYUyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s51g001.g4tx",
        349352838,
        -1914969540,
    ),
    (
        -1011321645,
        19,
        "AAAAABgFNRftNPcACgEoAAYCNMy2nCYyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s52g001.g4tx",
        -1705135003,
        55895519,
    ),
    (
        1699290997,
        20,
        "AAAAABgFNRftNPcACgEoAAYCNImHzjIyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s13g001.g4tx",
        186377719,
        -1844267955,
    ),
    (
        1479096005,
        21,
        "AAAAABgFNRftNPcACgEoAAYCNAmVrskyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s12g001.g4tx",
        -944642711,
        1589163219,
    ),
    (
        -19374749,
        22,
        "AAAAABgFNRftNPcACgEoAAYCNLuxrLAyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s53g001.g4tx",
        1459074299,
        -805378751,
    ),
    (
        1905830083,
        23,
        "AAAAABgFNRftNPcACgEoAAYCNFW/zZwyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s55g001.g4tx",
        -1869038212,
        160558278,
    ),
    (
        909693459,
        24,
        "AAAAABgFNRftNPcACgEoAAYCNMUA0A0yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s56g001.g4tx",
        504871583,
        -2028016859,
    ),
    (
        190380963,
        25,
        "AAAAABgFNRftNPcACgEoAAYCNBCOn4gyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s57g001.g4tx",
        -759318015,
        1270148027,
    ),
    (
        -1995936654,
        26,
        "AAAAABgFNRftNPcACgEoAAYCNLIH4JsyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s58g001.g4tx",
        194881709,
        -1835730665,
    ),
    (
        -1268234814,
        27,
        "AAAAABgFNRftNPcACgEoAAYCNNLAaX4yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s59g001.g4tx",
        -952653773,
        1581185417,
    ),
    (
        1072452893,
        28,
        "AAAAABgFNRftNPcACgEoAAYCNKXHWegyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s60g001.g4tx",
        -504000547,
        2029837927,
    ),
    (
        42761389,
        29,
        "AAAAABgFNRftNPcACgEoAAYCNDzOCFIyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s61g001.g4tx",
        761237315,
        -1269375239,
    ),
    (
        1160513149,
        30,
        "AAAAABgFNRftNPcACgEoAAYCNEvJOMQyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s62g001.g4tx",
        -1546614624,
        987355418,
    ),
    (
        1291354483,
        31,
        "AAAAABgFNRftNPcACgEoAAYCNCK4/QoyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s54g001.g4tx",
        1546829282,
        -985928616,
    ),
    (
        -368448811,
        32,
        "AAAAABgFNRftNPcACgEoAAYCNNWtrWcyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s15g001.g4tx",
        -847975312,
        1417526730,
    ),
    (
        -558021525,
        33,
        "AAAAABgFNRftNPcACgEoAAYCNGeJrx4yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s22g001.g4tx",
        -29566548,
        1731561494,
    ),
    (
        725476011,
        34,
        "AAAAABgFNRftNPcACgEoAAYCNDujzEsyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s26g001.g4tx",
        2054608726,
        -478181652,
    ),
    (
        375245595,
        35,
        "AAAAABgFNRftNPcACgEoAAYCNEyk/N0yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s27g001.g4tx",
        -1227040824,
        802522738,
    ),
    (
        2018258893,
        36,
        "AAAAABgFNRftNPcACgEoAAYCNNwb4UwyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s63g001.g4tx",
        1870301246,
        -160179836,
    ),
    (
        -898829347,
        37,
        "AAAAABgFNRftNPcACgEoAAYCNKsc0doyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s64g001.g4tx",
        1707049255,
        -55127907,
    ),
    (
        -150152595,
        38,
        "AAAAABgFNRftNPcACgEoAAYCNJ2B/7kyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s65g001.g4tx",
        -1458208327,
        807194627,
    ),
    (
        -1865827915,
        39,
        "AAAAABgFNRftNPcACgEoAAYCNOqGzy8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s17g001.g4tx",
        -1890456819,
        374913719,
    ),
    (
        -1330863939,
        43,
        "AAAAABgFNRftNPcACgEoAAYCNO3rCzYyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s66g001.g4tx",
        664438362,
        -1097607200,
    ),
    (
        -1915975411,
        44,
        "AAAAABgFNRftNPcACgEoAAYCNHTiWowyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s67g001.g4tx",
        -349141308,
        1916393342,
    ),
    (
        261889756,
        45,
        "AAAAABgFNRftNPcACgEoAAYCNPc2so8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s68g001.g4tx",
        839947368,
        -1424374318,
    ),
    (
        312028772,
        48,
        "AAAAABgFNRftNPcACgEoAAYCNORdRx0yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s18g001.g4tx",
        1450310049,
        -814176229,
    ),
    (
        1647421151,
        49,
        "AAAAABgFNRftNPcACgEoAAYCNISazvgyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s39g001.g4tx",
        -1272065607,
        757366787,
    ),
    (
        1380052254,
        50,
        "AAAAABgFNRftNPcACgEoAAYCNPOd/m4yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s31g001.g4tx",
        1741212172,
        -20866122,
    ),
    (
        855390060,
        51,
        "AAAAABgFNRftNPcACgEoAAYCNGqUr9QyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s69g001.g4tx",
        -21326602,
        1739834700,
    ),
    (
        -189757768,
        52,
        "AAAAABgFNRftNPcACgEoAAYCNB2Tn0IyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s70g001.g4tx",
        -158426210,
        1871170084,
    ),
    (
        -1811063606,
        53,
        "AAAAABgFNRftNPcACgEoAAYCNIP3CuEyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s28g001.g4tx",
        1878836580,
        -151677730,
    ),
    (
        -909070584,
        54,
        "AAAAABgFNRftNPcACgEoAAYCNPTwOncyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s71g001.g4tx",
        975477504,
        -1557280070,
    ),
    (
        -1905207848,
        55,
        "AAAAABgFNRftNPcACgEoAAYCNIAxghkyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s72g001.g4tx",
        -1263821597,
        765644121,
    ),
    (
        -1452444294,
        56,
        "AAAAABgFNRftNPcACgEoAAYCNOD2C/wyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s29g001.g4tx",
        -1554594310,
        979342400,
    ),
    (
        -1290732440,
        57,
        "AAAAABgFNRftNPcACgEoAAYCNG35a80yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s73g001.g4tx",
        2013371517,
        -519516729,
    ),
    (
        1011878344,
        58,
        "AAAAABgFNRftNPcACgEoAAYCNIpBRsoyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s75g001.g4tx",
        -1100048902,
        661111872,
    ),
    (
        19931256,
        59,
        "AAAAABgFNRftNPcACgEoAAYCNBr+W1syAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s74g001.g4tx",
        1925485924,
        -338836258,
    ),
    (
        804849620,
        60,
        "AAAAABgFNRftNPcACgEoAAYCNP1GdlwyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s19g001.g4tx",
        -1696859841,
        64137349,
    ),
    (
        -1211701123,
        61,
        "AAAAABgFNRftNPcACgEoAAYCNF8osWIyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s87g001.g4tx",
        603086166,
        -1157952276,
    ),
    (
        -1825962416,
        62,
        "AAAAABgFNRftNPcACgEoAAYCNK+3nTsyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_b10g004.g4tx",
        2049374448,
        -483415734,
    ),
    (
        1365257935,
        63,
        "AAAAABgFNRftNPcACgEoAAYCNN/dabQyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_b17g001.g4tx",
        1195180507,
        -566996895,
    ),
    (
        1708425329,
        64,
        "AAAAABgFNRftNPcACgEoAAYCNEbUOA4yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_b20g001.g4tx",
        1953838087,
        -310614595,
    ),
    (
        -1820990112,
        65,
        "AAAAABgFNRftNPcACgEoAAYCNNiwra0yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s02g001.g4tx",
        -792006358,
        1237557392,
    ),
    (
        -1374303024,
        66,
        "AAAAABgFNRftNPcACgEoAAYCNEG5/BcyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s03g001.g4tx",
        476115380,
        -2056675314,
    ),
    (
        2027008201,
        67,
        "AAAAABgFNRftNPcACgEoAAYCNKjaWSIyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_w50.g4tx",
        409586822,
        -2124374724,
    ),
    (
        -724197456,
        68,
        "AAAAABgFNRftNPcACgEoAAYCNDHTCJgyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s01g001.g4tx",
        1581643465,
        -951277709,
    ),
    (
        2079355672,
        69,
        "AAAAABgFNRftNPcACgEoAAYCNDE0sLUyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s76g001.g4tx",
        820089369,
        -1444363357,
    ),
    (
        1183865512,
        70,
        "AAAAABgFNRftNPcACgEoAAYCNEYzgCMyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s77g001.g4tx",
        -62156153,
        1698874173,
    ),
    (
        -1968782899,
        71,
        "AAAAABgFNRftNPcACgEoAAYCNCgvgfQyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s86g001.g4tx",
        -279421496,
        1985023090,
    ),
    (
        -993997447,
        72,
        "AAAAABgFNRftNPcACgEoAAYCNN860ZkyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s78g001.g4tx",
        627802155,
        -1134374511,
    ),
    (
        -106896183,
        78,
        "AAAAABgFNRftNPcACgEoAAYCNKg94Q8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s79g001.g4tx",
        -373194571,
        1892208911,
    ),
    (
        98972781,
        79,
        "AAAAABgFNRftNPcACgEoAAYCNDZZdKwyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s80g001.g4tx",
        691491919,
        -1338112523,
    ),
    (
        367105998,
        80,
        "AAAAABgFNRftNPcACgEoAAYCNM9wFN4yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s32g001.g4tx",
        -381170193,
        1884200021,
    ),
    (
        1375580619,
        81,
        "AAAAABgFNRftNPcACgEoAAYCNCjIOdkyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s24g001.g4tx",
        945016875,
        -1587904111,
    ),
    (
        -472027685,
        82,
        "AAAAABgFNRftNPcACgEoAAYCNF/PCU8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s23g001.g4tx",
        848746802,
        -1415608184,
    ),
    (
        -1426003347,
        83,
        "AAAAABgFNRftNPcACgEoAAYCNMbGWPUyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s22g001b.g4tx",
        76784013,
        -1650790345,
    ),
    (
        948313565,
        84,
        "AAAAABgFNRftNPcACgEoAAYCNEFeRDoyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s81g001.g4tx",
        -442673967,
        2090075499,
    ),
    (
        2133223181,
        85,
        "AAAAABgFNRftNPcACgEoAAYCNNhXFYAyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s82g001.g4tx",
        1796363058,
        -233110904,
    ),
    (
        1822267515,
        86,
        "AAAAABgFNRftNPcACgEoAAYCNLh3JEgyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s25g001.g4tx",
        -184954699,
        1844478223,
    ),
    (
        1111903933,
        87,
        "AAAAABgFNRftNPcACgEoAAYCNK9QJRYyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s83g001.g4tx",
        -1481092180,
        1051787798,
    ),
    (
        -261713235,
        88,
        "AAAAABgFNRftNPcACgEoAAYCND/vOIcyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s84g001.g4tx",
        -1384293707,
        880020239,
    ),
    (
        -855196899,
        89,
        "AAAAABgFNRftNPcACgEoAAYCNEjoCBEyAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s85g001.g4tx",
        1641503275,
        -119665775,
    ),
    (
        1727349664,
        90,
        "AAAAABgFNRftNPcACgEoAAYCNNZrJZ8yAAAAAXg=",
        "#/menu/220_img/stadium/img_room_s06g001.g4tx",
        1417740240,
        -846614934,
    ),
];

/// Reconstruit la disposition réelle : un noeud `SOCCER_OPTION_FIELD_INFO_LIST_BEG_0`
/// dont chaque enfant `SOCCER_OPTION_FIELD_INFO_N` porte les 6 variables positionnelles.
fn node_fixture() -> Value {
    let children: Vec<Value> = RAW
        .iter()
        .enumerate()
        .map(|(i, (field_id, index, cond, img, v4, v5))| {
            // var[2] : entier 0 si pas de condition, sinon la chaîne base64.
            let var2 = if cond.is_empty() {
                json!({ "type": "Int", "value": "0" })
            } else {
                json!({ "type": "String", "value": cond })
            };
            json!({
                "name": format!("SOCCER_OPTION_FIELD_INFO_{i}"),
                "variables": [
                    { "type": "Int", "value": field_id.to_string() },
                    { "type": "Int", "value": index.to_string() },
                    var2,
                    { "type": "String", "value": img },
                    { "type": "Int", "value": v4.to_string() },
                    { "type": "Int", "value": v5.to_string() }
                ],
                "children": []
            })
        })
        .collect();
    json!({
        "entries": [{
            "name": "SOCCER_OPTION_FIELD_INFO_LIST_BEG_0",
            "variables": [{ "type": "Int", "value": "81" }],
            "children": children
        }]
    })
}

#[test]
fn stadium_comptes_reels() {
    let stadiums = parse_stadium_config(&node_fixture());
    // 81 stades, tous avec une image ; 80 portent une condition (l'entrée 0 n'en a pas).
    assert_eq!(stadiums.len(), 81, "81 entrées SOCCER_OPTION_FIELD_INFO_N");
    assert!(
        stadiums.iter().all(|s| !s.image_path.is_empty()),
        "tous ont une image"
    );
    let avec_cond = stadiums.iter().filter(|s| !s.condition.is_empty()).count();
    assert_eq!(avec_cond, 80, "80 stades avec condition de déverrouillage");
}

#[test]
fn stadium_entree_0_sans_condition() {
    // SOCCER_OPTION_FIELD_INFO_0 : fieldId -826611768 → 0xCEBAE7C8, var[2] = Int 0 → pas de condition.
    let stadiums = parse_stadium_config(&node_fixture());
    let s: &Stadium = &stadiums[0];
    assert_eq!(s.field_id, HashId::from_i64(-826611768));
    assert_eq!(s.field_id, HashId(0xCEBA_E7C8));
    assert_eq!(s.field_id_hex(), "0xCEBAE7C8");
    assert_eq!(s.index, 0);
    assert_eq!(s.image_path, "stadium/img_room_s90g001");
    assert_eq!(s.name, "img_room_s90g001");
    assert!(s.condition.is_empty());
}

#[test]
fn stadium_entree_1_avec_condition() {
    // SOCCER_OPTION_FIELD_INFO_1 : fieldId 585721253 → 0x22E965A5, condition base64 (40 car.).
    let stadiums = parse_stadium_config(&node_fixture());
    let s = &stadiums[1];
    assert_eq!(s.field_id, HashId(0x22E9_65A5));
    assert_eq!(s.index, 1);
    assert_eq!(s.image_path, "stadium/img_room_s10g001");
    assert_eq!(s.name, "img_room_s10g001");
    assert_eq!(s.condition, "AAAAABgFNRftNPcACgEoAAYCNI7qCisyAAAAAXg=");
    assert_eq!(s.condition.len(), 40);
}

#[test]
fn stadium_derniere_entree_index_non_contigu() {
    // SOCCER_OPTION_FIELD_INFO_80 : dernier stade. L'index (90) ≠ la position (80) :
    // les index in-game ne sont PAS contigus (sauts à 40-42, 46-47, 73-77…).
    let stadiums = parse_stadium_config(&node_fixture());
    let s = stadiums.last().unwrap();
    assert_eq!(s.field_id, HashId(0x66F5_43A0));
    assert_eq!(
        s.index, 90,
        "index in-game non contigu (90 pour la 81e entrée)"
    );
    assert_eq!(s.image_path, "stadium/img_room_s06g001");
    assert_eq!(s.name, "img_room_s06g001");
}

#[test]
fn stadium_strip_image_et_nom_coherents() {
    // Port de extractImageName + name = dernier segment, vérifié sur les 81 entrées.
    let stadiums = parse_stadium_config(&node_fixture());
    for s in &stadiums {
        assert!(
            s.image_path.starts_with("stadium/"),
            "préfixe #/menu/220_img/ retiré"
        );
        assert!(!s.image_path.contains("#/menu/220_img/"), "préfixe absent");
        assert!(!s.image_path.ends_with(".g4tx"), "suffixe .g4tx retiré");
        let last = s.image_path.rsplit('/').next().unwrap();
        assert_eq!(s.name, last, "name = dernier segment du chemin d'image");
    }
}

#[test]
fn stadium_field_id_2eme_entree() {
    // Garde supplémentaire : SOCCER_OPTION_FIELD_INFO_2, fieldId 149202806 → 0x08E4A776.
    let stadiums = parse_stadium_config(&node_fixture());
    assert_eq!(stadiums[2].field_id, HashId(0x08E4_A776));
    assert_eq!(stadiums[2].name, "img_room_s42g001");
}
