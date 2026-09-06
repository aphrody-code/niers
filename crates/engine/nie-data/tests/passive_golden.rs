#![allow(clippy::pedantic)]
//! Tests golden `passive` — noeud réel `PASSIVE_SKILL_INFO_0` tiré de :
//! `skill/passive_skill_config_0.08.86.cfg.bin.json`.
//!
//! PASSIVE_SKILL_INFO_0 (6 vars Int) : `[975948532, 1105141741, 0, 0, 6, 0]`
//! → passiveId=0x3A2BCAF4, effectId=0x41DF1FED, nameId=0x00000000, descId=0x00000000, rarity=6.
//! PASSIVE_SKILL_EFFECT : Variables[0]=hash (effectId), suivantes=params (Int/Float).

mod common;

use nie_data::hash::HashId;
use nie_data::passive::{
    PassiveBoostType, PassiveScope, detect_boost_type, detect_scope, parse_effects, parse_passives,
};
use serde_json::json;

fn passive_fixture() -> serde_json::Value {
    json!({
        "entries": [
            {
                "name": "PASSIVE_SKILL_INFO_BEGIN",
                "variables": [],
                "children": [{
                    "name": "PASSIVE_SKILL_INFO_0",
                    "variables": [
                        { "type": "Int", "value": "975948532" },
                        { "type": "Int", "value": "1105141741" },
                        { "type": "Int", "value": "0" },
                        { "type": "Int", "value": "0" },
                        { "type": "Int", "value": "6" },
                        { "type": "Int", "value": "0" }
                    ],
                    "children": []
                }]
            },
            {
                "name": "PASSIVE_SKILL_EFFECT_BEGIN",
                "variables": [],
                "children": [{
                    "name": "PASSIVE_SKILL_EFFECT_0",
                    "variables": [
                        { "type": "Int", "value": "1105141741" },
                        { "type": "Float", "value": "0.5" },
                        { "type": "Int", "value": "10" }
                    ],
                    "children": []
                }]
            }
        ]
    })
}

#[test]
fn passive_skill_info_0() {
    let passives = parse_passives(&passive_fixture());
    assert_eq!(passives.len(), 1);
    let p = &passives[0];

    assert_eq!(p.passive_id, HashId::from_signed(975948532));
    assert_eq!(p.passive_id.to_hex(), "0x3A2BCAF4");
    assert_eq!(p.effect_id, HashId::from_signed(1105141741));
    assert_eq!(p.effect_id.to_hex(), "0x41DF1FED");
    assert_eq!(p.name_id, HashId::ZERO);
    assert_eq!(p.desc_id, HashId::ZERO);
    assert_eq!(p.rarity, 6);

    // Jointure effectId → params (0.5 Float, 10 Int).
    let params = p
        .effect_params
        .as_ref()
        .expect("params joints via effectId");
    assert_eq!(params.len(), 2);
    assert!((params[0] - 0.5).abs() < 1e-9);
    assert!((params[1] - 10.0).abs() < 1e-9);
}

#[test]
fn effets_collectes() {
    let effects = parse_effects(&passive_fixture());
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect_id, HashId(0x41DF_1FED));
    assert_eq!(effects[0].params.len(), 2);
}

#[test]
fn detect_scope_golden() {
    // Source : detectScope (mapper-passive.ts l.131-161).
    assert_eq!(
        detect_scope(Some("boost de toute l'équipe"), None),
        PassiveScope::Team
    );
    assert_eq!(
        detect_scope(Some("réduit la vitesse de l'adversaire"), None),
        PassiveScope::Enemy
    );
    assert_eq!(
        detect_scope(Some("pour les joueurs de feu"), None),
        PassiveScope::Player
    );
    assert_eq!(
        detect_scope(Some("effet indéterminé xyz"), None),
        PassiveScope::Unknown
    );
    // fallback EN si FR absent.
    assert_eq!(
        detect_scope(None, Some("boosts the whole team")),
        PassiveScope::Team
    );
}

#[test]
fn detect_boost_type_golden() {
    // Ordre de priorité porté de detectBoostType (premier match gagne).
    assert_eq!(
        detect_boost_type(Some("augmente la brèche"), None),
        PassiveBoostType::Breach
    );
    assert_eq!(
        detect_boost_type(Some("taux de justice"), None),
        PassiveBoostType::Justice
    );
    assert_eq!(
        detect_boost_type(Some("bonus d'affrontement"), None),
        PassiveBoostType::Confrontation
    );
    assert_eq!(
        detect_boost_type(Some("gain de tension"), None),
        PassiveBoostType::Tension
    );
    assert_eq!(
        detect_boost_type(Some("récupération accrue"), None),
        PassiveBoostType::Recovery
    );
    assert_eq!(
        detect_boost_type(Some("boost de vitesse"), None),
        PassiveBoostType::Speed
    );
    assert_eq!(
        detect_boost_type(Some("plus d'endurance"), None),
        PassiveBoostType::Stamina
    );
    assert_eq!(
        detect_boost_type(Some("tir +<VALUE>%"), None),
        PassiveBoostType::Shoot
    );
    assert_eq!(
        detect_boost_type(Some("dribble renforcé"), None),
        PassiveBoostType::Dribble
    );
    // "défense" → block (testé avant defense dans la cascade).
    assert_eq!(
        detect_boost_type(Some("hausse la défense physique"), None),
        PassiveBoostType::Block
    );
    assert_eq!(
        detect_boost_type(Some("arrêt amélioré"), None),
        PassiveBoostType::Catch
    );
    // mot unique sans espace → special (branche !contains(' ')).
    assert_eq!(
        detect_boost_type(Some("Passion"), None),
        PassiveBoostType::Special
    );
    assert_eq!(
        detect_boost_type(Some("xyzzy multi mots inconnus"), None),
        PassiveBoostType::Unknown
    );
}

#[test]
fn boost_type_labels() {
    assert_eq!(PassiveBoostType::Breach.names(), ("Breach", "Brèche"));
    assert_eq!(PassiveBoostType::Shoot.names(), ("Shoot", "Tir"));
    assert_eq!(PassiveScope::Team.names(), ("Team", "Équipe"));
}
