#![allow(clippy::pedantic)]
//! Tests golden `ai::resolve_strategy_name`/`resolve_strategy_description` — jointure
//! `m_StrategyAIInfoList.name_id`/`desc_id` → `ai_text`.
//!
//! `ai.rs` n'a **pas** de parseur inagle de référence ; le modèle de données est confirmé
//! **end-to-end** : `cargo run -p nie-game --example extract_ai_strategy_names` résout **62/62**
//! stratégies via `ai_text` (noms internes JA : « 基本戦略 » = stratégie de base, « オフェンス時基本戦略 »
//! = stratégie d'attaque ; les 3 premières sont des entrées de test). Ce fichier fige la jointure.

use nie_data::ai::{StrategyAiInfo, resolve_strategy_description, resolve_strategy_name};
use nie_data::hash::HashId;

fn strategy(name_id: u32, desc_id: u32) -> StrategyAiInfo {
    StrategyAiInfo {
        strategy_id: HashId(0x1DF9_83AD),
        name_id: HashId(name_id),
        desc_id: HashId(desc_id),
        phase_id: HashId::ZERO,
        evaluation_adjust_id: HashId::ZERO,
        is_confirm: false,
        tactics_offset: 0,
        tactics_count: 0,
    }
}

fn ai_text() -> Vec<(HashId, String)> {
    vec![
        (HashId(0x0000_1001), String::from("基本戦略")),
        (HashId(0x0000_1002), String::from("基本戦略説明テキスト")),
    ]
}

#[test]
fn resolves_strategy_name_and_description() {
    let t = ai_text();
    let s = strategy(0x0000_1001, 0x0000_1002);
    assert_eq!(resolve_strategy_name(&s, &t), Some("基本戦略"));
    assert_eq!(
        resolve_strategy_description(&s, &t),
        Some("基本戦略説明テキスト")
    );
}

#[test]
fn missing_hash_is_none() {
    let t = ai_text();
    let s = strategy(0xDEAD_BEEF, 0xFEED_FACE);
    assert_eq!(resolve_strategy_name(&s, &t), None);
    assert_eq!(resolve_strategy_description(&s, &t), None);
}
