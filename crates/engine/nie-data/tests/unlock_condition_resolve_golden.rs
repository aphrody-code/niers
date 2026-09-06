#![allow(clippy::pedantic)]
//! Golden `unlock_condition` — **résolution event-flag → event_id par CRC32**.
//!
//! Découverte validée (2026-06-23) : les feuilles event-flag des conditions réelles référencent
//! l'event_id par `crc32(event_id)` (poly 0xEDB88320). Vérifié sur le corpus : **156/373** event_id
//! candidats (noms de fichiers `qsb*`/`ev*`) matchent un CRC présent dans une vraie condition.
//! Ce test fige 5 paires confirmées + la résolution end-to-end (blob event-flag → event_id).

use nie_data::unlock_condition::{
    UnlockType, build_event_crc_lookup, crc32_str, decode_unlock_condition_bytes,
};

#[test]
fn crc_event_id_matche_les_conditions_reelles() {
    // (event_id, CRC) — CRC présents dans de vraies feuilles event-flag (analyse corpus).
    let pairs: [(&str, u32); 5] = [
        ("qsb010200", 0x8947_511C),
        ("qsb080100", 0x8611_8D34),
        ("ev07_00125", 0x8D4C_304C),
        ("ev08_04100", 0xB63A_41FF),
        ("qsb090300", 0xB8F5_70EA),
    ];
    for (id, crc) in pairs {
        assert_eq!(crc32_str(id), crc, "crc32({id})");
    }
}

#[test]
fn resolution_event_flag_end_to_end() {
    // Blob event-flag au FORMAT VALIDÉ (cadrage + tokens) : version 0, len 0x10, opcode 0x05,
    // feuille 0x35 ns=0x2A3D4543 (event-flag) + 0x34 crc=crc32("qsb010200") + 0x32 cmp=1.
    let crc = crc32_str("qsb010200");
    let mut blob = vec![0x00, 0x00, 0x00, 0x00, 0x10, 0x05, 0x35];
    blob.extend_from_slice(&0x2A3D_4543u32.to_be_bytes()); // ns event-flag
    blob.push(0x34);
    blob.extend_from_slice(&crc.to_be_bytes()); // val = crc de l'event
    blob.push(0x32);
    blob.extend_from_slice(&1u32.to_be_bytes()); // cmp = 1 occurrence

    let mut cond = decode_unlock_condition_bytes(&blob, String::new());
    assert_eq!(cond.kind, UnlockType::EventFlag);
    assert_eq!(cond.required_events.len(), 1);
    assert_eq!(cond.required_events[0].crc, crc);
    assert_eq!(cond.required_events[0].count, 1);
    assert!(
        cond.required_events[0].event_id.is_none(),
        "non résolu avant lookup"
    );

    // Résolution via le lookup CRC→event_id.
    let lookup = build_event_crc_lookup(["qsb010200", "qsb080100", "ev07_00125"]);
    cond.resolve_events(&lookup);
    assert_eq!(
        cond.required_events[0].event_id.as_deref(),
        Some("qsb010200")
    );
}
