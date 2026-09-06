#![allow(clippy::pedantic)]
//! Tests golden `vsroute` — valeurs réelles tirées de :
//! - `data/common/gamedata/vsroute/chronicle_vs_route_config_5.00.30.cfg.bin.json` (golden)
//! - `data/common/gamedata/vsroute/chronicle_vs_route_config_2.00.16.cfg.bin.json` (version ancienne)
//! - `data/common/gamedata/vsroute/chronicle_vs_route_opponent_info_0.00.00.cfg.bin.json`
//! - `data/common/gamedata/vsroute/chronicle_vs_route_trigger_0.04.78.cfg.bin.json`
//!
//! Toutes les valeurs ci-dessous sont lues octet-pour-octet dans les dumps JSON réels.

mod common;

extern crate std;

use nie_data::hash::HashId;
use nie_data::vsroute::{parse_chronicle_vs_route_config, parse_opponent_info, parse_trigger};

const REAL_CONFIG_5: &str = "vsroute/chronicle_vs_route_config_5.00.30.cfg.bin.json";
const REAL_CONFIG_2: &str = "vsroute/chronicle_vs_route_config_2.00.16.cfg.bin.json";
const REAL_OPPONENT: &str = "vsroute/chronicle_vs_route_opponent_info_0.00.00.cfg.bin.json";
const REAL_TRIGGER: &str = "vsroute/chronicle_vs_route_trigger_0.04.78.cfg.bin.json";

fn load_json(path: &str) -> Option<serde_json::Value> {
    let path = common::chemin(path)?;
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

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG 5.00.30 (golden)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cfg5_comptes_des_listes() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // Comptes réels confirmés sur le dump 5.00.30.
    assert_eq!(c.unlock_pieces.len(), 235, "m_unlockPieceInfoList = 235");
    assert_eq!(c.move_routes.len(), 238, "m_pieceMoveRouteInfoList = 238");
    assert_eq!(c.events.len(), 159, "m_pieceEventInfoList = 159");
    assert_eq!(c.pieces.len(), 238, "m_pieceInfoList = 238");
    assert_eq!(c.routes.len(), 10, "m_chronicleVsRouteInfoList = 10");
    assert_eq!(c.icons.len(), 143, "m_pieceIconInfoList = 143");
    assert_eq!(c.gates.len(), 1, "m_gateOpenInfoList = 1");
}

#[test]
fn cfg5_unlock_piece_0_et_10() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [0] = {unlockPieceId: 0x009FC304, unlockEvent: true, dlcNo: 0}
    assert_eq!(c.unlock_pieces[0].unlock_piece_id, HashId(0x009F_C304));
    assert!(c.unlock_pieces[0].unlock_event);
    assert_eq!(c.unlock_pieces[0].dlc_no, 0);
    // [10] = {unlockPieceId: 0x015DA933, unlockEvent: true, dlcNo: 0}
    assert_eq!(c.unlock_pieces[10].unlock_piece_id, HashId(0x015D_A933));
    assert!(c.unlock_pieces[10].unlock_event);
}

#[test]
fn cfg5_move_route_1() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [1] = {moveRouteLU: 0x009FC304, moveRouteLD: 0, moveRouteRU: 0x32A9A186, moveRouteRD: 0}
    let m = &c.move_routes[1];
    assert_eq!(m.move_route_lu, HashId(0x009F_C304));
    assert_eq!(m.move_route_ld, HashId::ZERO);
    assert_eq!(m.move_route_ru, HashId(0x32A9_A186));
    assert_eq!(m.move_route_rd, HashId::ZERO);
}

#[test]
fn cfg5_event_0() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [0] = {paramCrc1: 0xA04868C3, paramCrc2: 0, paramNum1: 0, paramNum2: 0,
    //        bustupEventPreSoccer: "ev21_01000", bustupEventPostWinSoccer: "ev21_01100"}
    let e = &c.events[0];
    assert_eq!(e.param_crc1, HashId(0xA048_68C3));
    assert_eq!(e.param_crc2, HashId::ZERO);
    assert_eq!(e.param_num1, 0);
    assert_eq!(e.param_num2, 0);
    assert_eq!(e.bustup_event_pre_soccer, "ev21_01000");
    assert_eq!(e.bustup_event_post_win_soccer, "ev21_01100");
}

#[test]
fn cfg5_event_5() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [5] bustupEventPreSoccer = "ev21_06000", bustupEventPostWinSoccer = "ev21_06100"
    let e = &c.events[5];
    assert_eq!(e.param_crc1, HashId(0xBD4D_587B));
    assert_eq!(e.bustup_event_pre_soccer, "ev21_06000");
    assert_eq!(e.bustup_event_post_win_soccer, "ev21_06100");
}

#[test]
fn cfg5_event_last_sentinelle_normalisee() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // Dernière entrée : bustup* sont des sentinelles (U+FFFD) → normalisées en "".
    let e = c.events.last().unwrap();
    assert_eq!(e.param_crc1, HashId(0x5108_6EF0));
    assert_eq!(e.bustup_event_pre_soccer, "", "sentinelle → vide");
    assert_eq!(e.bustup_event_post_win_soccer, "", "sentinelle → vide");
}

#[test]
fn cfg5_piece_0() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [0] = {routeType: 0, pieceId: 0x32A9A186, eventType: 6, statusFlagIndex: 1,
    //        routeProgressIndex: 1, addDlcNo: 0, piecePos: "000040C00000A0C0",
    //        IconInfoId: 0xDD5767B2, bgmId: 0x3E772F0A, moveRouteInfo: [0,1],
    //        unlockPieceInfo: [0,0], eventInfoRef: [0,0], condition: ""}
    let p = &c.pieces[0];
    assert_eq!(p.route_type, 0);
    assert_eq!(p.piece_id, HashId(0x32A9_A186));
    assert_eq!(p.event_type, 6);
    assert_eq!(p.status_flag_index, 1);
    assert_eq!(p.route_progress_index, 1);
    assert_eq!(p.add_dlc_no, 0);
    assert_eq!(p.piece_pos, "000040C00000A0C0");
    assert_eq!(p.piece_pos_offset, "0000000000000000");
    assert_eq!(p.icon_info_id, HashId(0xDD57_67B2));
    assert_eq!(p.bgm_id, HashId(0x3E77_2F0A));
    assert_eq!(p.move_route_offset, 0);
    assert_eq!(p.move_route_count, 1);
    assert_eq!(p.unlock_piece_offset, 0);
    assert_eq!(p.unlock_piece_count, 0);
    assert_eq!(p.event_info_offset, 0);
    assert_eq!(p.event_info_count, 0);
    assert_eq!(p.condition, "", "condition sentinelle → vide");
    assert!(!p.can_obtain_gate_key);
    assert!(!p.show_new_route_effect);
    assert!(!p.need_update);
}

#[test]
fn cfg5_piece_3_route_type_255() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [3] = {routeType: 255, pieceId: 0x4FDE55C3, statusFlagIndex: 4,
    //        piecePos: "000000C1000000C1", IconInfoId: 0x80868A84,
    //        moveRouteInfo: [3,1], unlockPieceInfo: [2,1], eventInfoRef: [2,1]}
    let p = &c.pieces[3];
    assert_eq!(p.route_type, 255);
    assert_eq!(p.piece_id, HashId(0x4FDE_55C3));
    assert_eq!(p.status_flag_index, 4);
    assert_eq!(p.route_progress_index, 4);
    assert_eq!(p.piece_pos, "000000C1000000C1");
    assert_eq!(p.icon_info_id, HashId(0x8086_8A84));
    assert_eq!(p.move_route_offset, 3);
    assert_eq!(p.move_route_count, 1);
    assert_eq!(p.unlock_piece_offset, 2);
    assert_eq!(p.unlock_piece_count, 1);
    assert_eq!(p.event_info_offset, 2);
    assert_eq!(p.event_info_count, 1);
}

#[test]
fn cfg5_piece_last() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // Dernière case = {pieceId: 0x90185427, routeType: 255, statusFlagIndex: 254,
    //   routeProgressIndex: 13, addDlcNo: 4, bgmId: 0x517E456D,
    //   bgmId_futureMapCleared: 0x4865742C, moveRouteInfo: [237,1], eventInfoRef: [158,1]}
    let p = c.pieces.last().unwrap();
    assert_eq!(p.piece_id, HashId(0x9018_5427));
    assert_eq!(p.route_type, 255);
    assert_eq!(p.status_flag_index, 254);
    assert_eq!(p.route_progress_index, 13);
    assert_eq!(p.add_dlc_no, 4);
    assert_eq!(p.bgm_id, HashId(0x517E_456D));
    assert_eq!(p.bgm_id_future_map_cleared, HashId(0x4865_742C));
    assert_eq!(p.move_route_offset, 237);
    assert_eq!(p.move_route_count, 1);
    assert_eq!(p.event_info_offset, 158);
    assert_eq!(p.event_info_count, 1);
}

#[test]
fn cfg5_routes_toutes() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // 10 routes, valeurs (id, routeMapType, pieceInfo[offset,count]) confirmées.
    assert_eq!(c.routes[0].id, HashId(0x7297_8C53));
    assert_eq!(c.routes[0].route_map_type, 0);
    assert_eq!(c.routes[0].piece_offset, 0);
    assert_eq!(c.routes[0].piece_count, 15);
    assert_eq!(c.routes[1].id, HashId(0xEB9E_DDE9));
    assert_eq!(c.routes[1].piece_offset, 15);
    assert_eq!(c.routes[1].piece_count, 28);
    // Dernière route : routeMapType = 1.
    let last = c.routes.last().unwrap();
    assert_eq!(last.id, HashId(0x2771_3169));
    assert_eq!(last.route_map_type, 1);
    assert_eq!(last.piece_offset, 225);
    assert_eq!(last.piece_count, 13);
}

#[test]
fn cfg5_icon_2_et_last() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [2] = {id: 0xB659ACF7, textureName: 0x24988232, texturePath: 0x3C637EBE}
    assert_eq!(c.icons[2].id, HashId(0xB659_ACF7));
    assert_eq!(c.icons[2].texture_name, HashId(0x2498_8232));
    assert_eq!(c.icons[2].texture_path, HashId(0x3C63_7EBE));
    // dernière = {id: 0x65EEE33D, textureName: 0x46612B7F, texturePath: 0x37AA4A5C}
    let last = c.icons.last().unwrap();
    assert_eq!(last.id, HashId(0x65EE_E33D));
    assert_eq!(last.texture_name, HashId(0x4661_2B7F));
    assert_eq!(last.texture_path, HashId(0x37AA_4A5C));
}

#[test]
fn cfg5_gate_0() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [0] = {id: 0xF9076F90, unlockFlag1: 0x0DFFC8B6, unlockFlag2: 0x94F6990C,
    //        unlockFlag3: 0, condition1: "AAAAACEFNcl4...", condition2: "...", condition3: ""}
    let g = &c.gates[0];
    assert_eq!(g.id, HashId(0xF907_6F90));
    assert_eq!(g.unlock_flag1, HashId(0x0DFF_C8B6));
    assert_eq!(g.unlock_flag2, HashId(0x94F6_990C));
    assert_eq!(g.unlock_flag3, HashId::ZERO);
    assert_eq!(
        g.condition1, "AAAAACEFNcl4Pb8AEwIoAAYCNK0nb6ooAAYCMgAAAAIyAAAAAXg=",
        "condition1 base64 brut"
    );
    assert_eq!(
        g.condition2,
        "AAAAACEFNcl4Pb8AEwIoAAYCNLiYp6YoAAYCMgAAAAIyAAAAAXg="
    );
    assert_eq!(g.condition3, "", "condition3 sentinelle → vide");
}

#[test]
fn cfg5_resolution_pieces_of() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // Route 0 : pieceInfo = [0, 15] → 15 cases, la première étant pieces[0].
    let route0 = &c.routes[0];
    let pieces = c.pieces_of(route0);
    assert_eq!(pieces.len(), 15);
    assert_eq!(pieces[0].piece_id, HashId(0x32A9_A186));
}

#[test]
fn cfg5_resolution_move_routes_of() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // pieces[3] : moveRouteInfo = [3, 1] → 1 arête (move_routes[3]).
    let p = &c.pieces[3];
    let mr = c.move_routes_of(p);
    assert_eq!(mr.len(), 1);
}

#[test]
fn cfg5_find_piece_et_icon() {
    let Some(root) = load_json(REAL_CONFIG_5) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    assert!(c.find_piece(HashId(0x32A9_A186)).is_some());
    assert!(c.find_piece(HashId(0xDEAD_BEEF)).is_none());
    assert!(c.find_icon(HashId(0xB659_ACF7)).is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG 2.00.16 (version ancienne — bgmId_futureMapCleared absent)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cfg2_comptes() {
    let Some(root) = load_json(REAL_CONFIG_2) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    assert_eq!(c.unlock_pieces.len(), 220);
    assert_eq!(c.move_routes.len(), 222);
    assert_eq!(c.events.len(), 151);
    assert_eq!(c.pieces.len(), 223);
    assert_eq!(c.routes.len(), 8);
    assert_eq!(c.icons.len(), 136);
    assert_eq!(c.gates.len(), 1);
}

#[test]
fn cfg2_piece_0_sans_bgm_future() {
    let Some(root) = load_json(REAL_CONFIG_2) else {
        return;
    };
    let c = parse_chronicle_vs_route_config(&root);
    // [0] = {routeType: 0, pieceId: 0x32A9A186, eventType: 6, statusFlagIndex: 1,
    //        piecePos: "000040C00000A0C0", bgmId: 0x3E772F0A} — pas de bgmId_futureMapCleared.
    let p = &c.pieces[0];
    assert_eq!(p.piece_id, HashId(0x32A9_A186));
    assert_eq!(p.event_type, 6);
    assert_eq!(p.piece_pos, "000040C00000A0C0");
    assert_eq!(p.bgm_id, HashId(0x3E77_2F0A));
    // Champ absent dans cette version → HashId::ZERO.
    assert_eq!(p.bgm_id_future_map_cleared, HashId::ZERO);
}

// ═══════════════════════════════════════════════════════════════════════════════
// OPPONENT_INFO
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn opponent_compte_et_entree_0() {
    let Some(root) = load_json(REAL_OPPONENT) else {
        return;
    };
    let opp = parse_opponent_info(&root);
    // 4 entrées (dont 3 à battleId nul, conservées).
    assert_eq!(opp.len(), 4, "m_vsRouteOpponentInfoList = 4");
    // [0] = {category: 0, battleId: 0xD750C819, sortOrder: 1, cond1: "AAAAABgFNaNVuJ8...", cond2..4: ""}
    let o = &opp[0];
    assert_eq!(o.category, 0);
    assert_eq!(o.battle_id, HashId(0xD750_C819));
    assert_eq!(o.sort_order, 1);
    assert_eq!(
        o.cond1, "AAAAABgFNaNVuJ8ACgEoAAYCNEHkr7IyAAAAAXg=",
        "cond1 base64"
    );
    assert_eq!(o.cond2, "", "cond2 sentinelle → vide");
    assert_eq!(o.cond3, "");
    assert_eq!(o.cond4, "");
}

#[test]
fn opponent_entree_2_battle_id_nul() {
    let Some(root) = load_json(REAL_OPPONENT) else {
        return;
    };
    let opp = parse_opponent_info(&root);
    // [2] = {category: 0, battleId: 0, sortOrder: 1, cond*: ""}
    let o = &opp[2];
    assert_eq!(o.battle_id, HashId::ZERO);
    assert_eq!(o.sort_order, 1);
    assert_eq!(o.cond1, "");
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIGGER (format entries)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn trigger_compte() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let t = parse_trigger(&root);
    // DATA_COUNT_0.var[0] = 63 → 63 DATA_ITEM.
    assert_eq!(t.len(), 63, "63 DATA_ITEM");
}

#[test]
fn trigger_item_0() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let t = parse_trigger(&root);
    // DATA_ITEM_0 = [505, 0, 0, "AAAAABgFNZ6ZhIwACgEoAAYCNLvfuDAyAAAAAHg=", 0, 0, 1]
    let d = &t[0];
    assert_eq!(d.kind, 505);
    assert_eq!(d.param1, HashId::ZERO);
    assert_eq!(d.param2, HashId::ZERO);
    assert_eq!(d.condition, "AAAAABgFNZ6ZhIwACgEoAAYCNLvfuDAyAAAAAHg=");
    assert_eq!(d.arg1, 0);
    assert_eq!(d.arg2, 0);
    assert_eq!(d.arg3, 1);
}

#[test]
fn trigger_item_1() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let t = parse_trigger(&root);
    // DATA_ITEM_1 = [504, 428143173, -186917087, "AAAAABgFNZ6ZhIwACgEoAAYCNLvfuDAyAAAAA28=", 2, 0, 2]
    let d = &t[1];
    assert_eq!(d.kind, 504);
    assert_eq!(d.param1, HashId::from_i64(428_143_173));
    assert_eq!(d.param2, HashId::from_i64(-186_917_087));
    assert_eq!(d.condition, "AAAAABgFNZ6ZhIwACgEoAAYCNLvfuDAyAAAAA28=");
    assert_eq!(d.arg1, 2);
    assert_eq!(d.arg2, 0);
    assert_eq!(d.arg3, 2);
}

#[test]
fn trigger_item_5_condition_zero() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let t = parse_trigger(&root);
    // DATA_ITEM_5 = [504, -505175872, -186917087, "0", 0, 0, 6] — var[3] = Int "0".
    let d = &t[5];
    assert_eq!(d.kind, 504);
    assert_eq!(d.param1, HashId::from_i64(-505_175_872));
    assert_eq!(d.param2, HashId::from_i64(-186_917_087));
    assert_eq!(d.condition, "0", "var[3]=Int 0 → chaîne brute \"0\"");
    assert_eq!(d.arg3, 6);
}

#[test]
fn trigger_item_last() {
    let Some(root) = load_json(REAL_TRIGGER) else {
        return;
    };
    let t = parse_trigger(&root);
    // DATA_ITEM_62 (dernier) = [504, 0, -186917087, "0", 63, 0, 63]
    let d = t.last().unwrap();
    assert_eq!(d.kind, 504);
    assert_eq!(d.param1, HashId::ZERO);
    assert_eq!(d.param2, HashId::from_i64(-186_917_087));
    assert_eq!(d.condition, "0");
    assert_eq!(d.arg1, 63);
    assert_eq!(d.arg2, 0);
    assert_eq!(d.arg3, 63);
}
