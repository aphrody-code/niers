#![allow(clippy::pedantic)]
//! Tests golden `unlock_condition` — port 1:1 d'inagle
//! `packages/inagle/src/parsers/unlock-condition.test.ts`.
//!
//! Les fixtures sont **identiques** à celles d'inagle : deux blobs base64 réels extraits de
//! `gallery_config` / `scene_archive_config` (`STORY_EV01`, `SCENE_EV01_00050`) et trois blobs
//! définis en hexadécimal (`TRIVIAL`, `COMPOSITE_0B`, `COMPOSITE_17`). Les sorties attendues
//! (type, op, seuil, épisode, CRC, namespace) sont celles asserées par la suite TS.

use nie_data::unlock_condition::{
    STORY_EPISODE_BASE, STORY_EPISODE_STEP, UnlockOp, UnlockType, build_event_crc_lookup,
    crc32_str, decode_unlock_condition, decode_unlock_condition_bytes, story_threshold_to_episode,
};

/// `gallery openCond`, seuil 20010 (ev01).
const STORY_EV01: &str = "AAAAAA8FNbkZNtoAAQAyAABOKnE=";
/// `scene_archive condition` → ev01_00150.
const SCENE_EV01_00050: &str = "AAAAABgFNSo9RUMACgEoAAYCNNG3Zz4yAAAAAXg=";

/// Hex → octets (les fixtures `TRIVIAL`/`COMPOSITE_*` sont définies en hex côté inagle).
fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

#[test]
fn story_threshold_to_episode_grid() {
    assert_eq!(story_threshold_to_episode(STORY_EPISODE_BASE), Some(1));
    assert_eq!(
        story_threshold_to_episode(STORY_EPISODE_BASE + STORY_EPISODE_STEP),
        Some(2)
    );
    assert_eq!(story_threshold_to_episode(90010), Some(8));
    // Seuils non alignés.
    assert_eq!(story_threshold_to_episode(12345), None);
    assert_eq!(story_threshold_to_episode(0), None);
    // La grille +10000/épisode jusqu'à ev08.
    for ep in 1..=8u32 {
        let threshold = STORY_EPISODE_BASE + (ep - 1) * STORY_EPISODE_STEP;
        assert_eq!(story_threshold_to_episode(threshold), Some(ep));
    }
}

#[test]
fn story_episode_1() {
    let c = decode_unlock_condition(STORY_EV01);
    assert_eq!(c.kind, UnlockType::Story);
    assert_eq!(c.op, UnlockOp::Single);
    assert_eq!(c.story_threshold, Some(20010));
    assert_eq!(c.story_episode, Some(1));
    assert!(c.required_events.is_empty());
}

#[test]
fn event_flag_crc_resolved() {
    let lookup = build_event_crc_lookup(["ev01_00150"]);
    let mut c = decode_unlock_condition(SCENE_EV01_00050);
    c.resolve_events(&lookup);
    assert_eq!(c.kind, UnlockType::EventFlag);
    assert_eq!(c.required_events.len(), 1);
    let ev = &c.required_events[0];
    // CRC32 (poly 0xEDB88320) de "ev01_00150".
    assert_eq!(ev.crc, crc32_str("ev01_00150"));
    assert_eq!(ev.crc_hex(), "0xD1B7673E");
    assert_eq!(ev.count, 1);
    assert_eq!(ev.event_id.as_deref(), Some("ev01_00150"));
}

#[test]
fn event_flag_without_lookup() {
    let c = decode_unlock_condition(SCENE_EV01_00050);
    assert!(c.required_events[0].event_id.is_none());
    assert_eq!(c.required_events[0].crc, crc32_str("ev01_00150"));
}

#[test]
fn composite_story_plus_event_0b() {
    let blob = hex(
        "00000000300b35b91936da000100320000c35a7135dafab70a0013022800060234c05d8c3b2800060232000000013200000001788f",
    );
    let c = decode_unlock_condition_bytes(&blob, String::new());
    assert_eq!(c.kind, UnlockType::Composite);
    assert_eq!(c.op, UnlockOp::And);
    assert_eq!(c.story_threshold, Some(50010));
    assert_eq!(c.story_episode, Some(4));
    assert_eq!(c.required_events.len(), 1);
    assert_eq!(c.required_events[0].namespace.to_hex(), "0xDAFAB70A");
}

#[test]
fn composite_two_event_flags_17() {
    let blob = hex(
        "0000000048173200000001352a3d4543000a0128000602344dea45163200000001788f32000000018f3200000001352a3d4543000a01280006023456b7ca0e3200000001788f32000000018f90",
    );
    let c = decode_unlock_condition_bytes(&blob, String::new());
    assert_eq!(c.kind, UnlockType::EventFlag);
    assert_eq!(c.op, UnlockOp::And);
    assert_eq!(c.required_events.len(), 2);
    assert_eq!(c.required_events[0].crc, crc32_str("ev04_02110"));
    assert_eq!(c.required_events[0].crc_hex(), "0x4DEA4516");
    assert!(
        c.required_events
            .iter()
            .all(|e| e.namespace.to_hex() == "0x2A3D4543")
    );
}

#[test]
fn trivial_opcode_3f() {
    let blob = hex("1802cb11163f20d14e");
    let c = decode_unlock_condition_bytes(&blob, String::new());
    assert_eq!(c.kind, UnlockType::Always);
    assert_eq!(c.op, UnlockOp::None);
    assert!(c.required_events.is_empty());
}

#[test]
fn empty_is_always() {
    assert_eq!(decode_unlock_condition("").kind, UnlockType::Always);
}
