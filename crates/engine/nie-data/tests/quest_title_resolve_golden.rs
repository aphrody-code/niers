#![allow(clippy::pedantic)]
//! Tests golden `quest::resolve_title` — port de la jointure titre de `buildQuestDatabase`
//! (`quest-config.ts`, `loadTextFile("quest_title").get(titleHash)`).
//!
//! Vérité terrain (via `cargo run -p nie-game --example extract_quest_titles`) : **182/182**
//! titres résolus en fr ; échantillons réels figés ci-dessous :
//! - quest `0x673DC707`, titleHash `0x0B0F1787` → « Tu n'as pas perdu le foot » ;
//! - quest `0x103AF791`, titleHash `0xA04C7405` → « Interrogez les élèves des alentours. ».

use nie_data::hash::HashId;
use nie_data::quest::{ParsedQuest, resolve_title};

fn quest(id: u32, title: u32) -> ParsedQuest {
    ParsedQuest {
        quest_id: HashId(id),
        title_hash: HashId(title),
        phase: 0,
        quest_type: 0,
        image: None,
    }
}

fn titles() -> Vec<(HashId, String)> {
    vec![
        (
            HashId(0x0B0F_1787),
            String::from("Tu n'as pas perdu le foot"),
        ),
        (
            HashId(0xA04C_7405),
            String::from("Interrogez les élèves des alentours."),
        ),
        // hash dupliqué : la DERNIÈRE valeur l'emporte (last-wins de buildNameMap).
        (HashId(0x0000_00AA), String::from("ancien")),
        (HashId(0x0000_00AA), String::from("récent")),
    ]
}

#[test]
fn resolves_real_titles() {
    let t = titles();
    assert_eq!(
        resolve_title(&quest(0x673D_C707, 0x0B0F_1787), &t),
        Some("Tu n'as pas perdu le foot")
    );
    assert_eq!(
        resolve_title(&quest(0x103A_F791, 0xA04C_7405), &t),
        Some("Interrogez les élèves des alentours.")
    );
}

#[test]
fn unmatched_hash_is_none() {
    let t = titles();
    assert_eq!(resolve_title(&quest(0x1, 0xDEAD_BEEF), &t), None);
}

#[test]
fn duplicate_hash_last_wins() {
    let t = titles();
    assert_eq!(resolve_title(&quest(0x2, 0x0000_00AA), &t), Some("récent"));
}
