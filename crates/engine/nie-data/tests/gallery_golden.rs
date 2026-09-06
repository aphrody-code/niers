#![allow(clippy::pedantic)]
//! Tests golden `gallery` — valeurs réelles tirées de :
//! `gallery/gallery_config_1.03.71.00.cfg.bin.json`
//!
//! ## Vérifications champ par champ (fichier → valeur confirmée)
//!
//! ### m_GalleryInfoList[0]
//! - `galleryId`    = `"0x33AF67B0"` → `HashId(0x33AF67B0)`
//! - `imgPath`      = `"img_story_ev01_main_0010"`
//! - `thumbPath`    = `"thumb_story_ev01_main_0010"`
//! - `needTokenNum` = 1
//! - `flgNo`        = 0
//! - `openCond`     = `"AAAAAA8FNbkZNtoAAQAyAABOKnE="`
//!
//! ### m_GalleryInfoList[1]
//! - `galleryId`    = `"0xAA4D01B1"` → `HashId(0xAA4D01B1)`
//! - `imgPath`      = `"img_story_ev02_main_0010"`
//! - `flgNo`        = 1
//! - `openCond`     = `"AAAAAA8FNbkZNtoAAQAyAAB1OnE="`
//!
//! ### m_GalleryInfoList[10]
//! - `galleryId`    = `"0x248248B6"` → `HashId(0x248248B6)`
//! - `imgPath`      = `"img_story_ev03_main_0060"`
//! - `flgNo`        = 10
//! - `openCond`     = `"AAAAAA8FNbkZNtoAAQAyAACcSnE="`
//!
//! ### m_GalleryInfoList[100]
//! - `galleryId`    = `"0x2A1C3108"` → `HashId(0x2A1C3108)`
//! - `imgPath`      = `"img_story_character_0240"`
//! - `flgNo`        = 101   ← flgNo ≠ index (gap à flgNo=48)
//!
//! ### m_GalleryInfoList[359] (dernier)
//! - `galleryId`    = `"0x9A2F0E7B"` → `HashId(0x9A2F0E7B)`
//! - `imgPath`      = `"img_other_promotion_0180"`
//! - `flgNo`        = 355
//! - `openCond`     = `"\x01GALLERY_INFO"` (marqueur interne, pas de base64)

mod common;

use nie_data::gallery::{GalleryConfig, GalleryInfo, parse_gallery_config};
use nie_data::hash::HashId;
use serde_json::json;

// ─── Fixture ──────────────────────────────────────────────────────────────────
// Valeurs extraites octet-pour-octet du dump réel — voir commentaires ci-dessus.

fn fixture() -> serde_json::Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_GalleryInfoList",
                "typeName": "GALLERY_INFO",
                "values": [
                    // entrée 0 — gallery_config_1.03.71.00, m_GalleryInfoList[0]
                    {
                        "galleryId":    "0x33AF67B0",
                        "imgPath":      "img_story_ev01_main_0010",
                        "thumbPath":    "thumb_story_ev01_main_0010",
                        "needTokenNum": 1,
                        "flgNo":        0,
                        "openCond":     "AAAAAA8FNbkZNtoAAQAyAABOKnE="
                    },
                    // entrée 1 — gallery_config_1.03.71.00, m_GalleryInfoList[1]
                    {
                        "galleryId":    "0xAA4D01B1",
                        "imgPath":      "img_story_ev02_main_0010",
                        "thumbPath":    "thumb_story_ev02_main_0010",
                        "needTokenNum": 1,
                        "flgNo":        1,
                        "openCond":     "AAAAAA8FNbkZNtoAAQAyAAB1OnE="
                    },
                    // entrée avec flgNo=10 — gallery_config_1.03.71.00, m_GalleryInfoList[10]
                    {
                        "galleryId":    "0x248248B6",
                        "imgPath":      "img_story_ev03_main_0060",
                        "thumbPath":    "thumb_story_ev03_main_0060",
                        "needTokenNum": 1,
                        "flgNo":        10,
                        "openCond":     "AAAAAA8FNbkZNtoAAQAyAACcSnE="
                    },
                    // entrée avec marqueur interne (dernier type de openCond)
                    {
                        "galleryId":    "0x9A2F0E7B",
                        "imgPath":      "img_other_promotion_0180",
                        "thumbPath":    "thumb_other_promotion_0180",
                        "needTokenNum": 1,
                        "flgNo":        355,
                        "openCond":     "GALLERY_INFO"
                    }
                ]
            }
        ]
    })
}

// ─── Tests sur la fixture ─────────────────────────────────────────────────────

#[test]
fn fixture_compte_entrees() {
    let cfg = parse_gallery_config(&fixture());
    assert_eq!(cfg.entries.len(), 4);
}

#[test]
fn fixture_entree0_champs_complets() {
    let cfg = parse_gallery_config(&fixture());
    let e: &GalleryInfo = &cfg.entries[0];

    // gallery_config_1.03.71.00 : m_GalleryInfoList[0]
    assert_eq!(e.gallery_id, HashId(0x33AF_67B0));
    assert_eq!(e.img_path, "img_story_ev01_main_0010");
    assert_eq!(e.thumb_path, "thumb_story_ev01_main_0010");
    assert_eq!(e.need_token_num, 1);
    assert_eq!(e.flg_no, 0);
    assert_eq!(e.open_cond, "AAAAAA8FNbkZNtoAAQAyAABOKnE=");
}

#[test]
fn fixture_entree1_gallery_id() {
    let cfg = parse_gallery_config(&fixture());
    let e = &cfg.entries[1];

    // gallery_config_1.03.71.00 : m_GalleryInfoList[1]
    assert_eq!(e.gallery_id, HashId(0xAA4D_01B1));
    assert_eq!(e.img_path, "img_story_ev02_main_0010");
    assert_eq!(e.flg_no, 1);
    assert_eq!(e.open_cond, "AAAAAA8FNbkZNtoAAQAyAAB1OnE=");
}

#[test]
fn fixture_flg_no_non_sequentiel() {
    let cfg = parse_gallery_config(&fixture());
    // La 3e entrée fixture (index 2) a flgNo=10 (non-séquentiel par rapport à l'index).
    let e = &cfg.entries[2];
    assert_eq!(
        e.flg_no, 10,
        "flgNo != index (gap à flgNo=48 dans le dump réel)"
    );
    assert_eq!(e.gallery_id, HashId(0x2482_48B6));
}

#[test]
fn fixture_open_cond_marqueur_interne() {
    let cfg = parse_gallery_config(&fixture());
    // Dernière entrée fixture — openCond = marqueur interne "\x01GALLERY_INFO"
    // gallery_config_1.03.71.00 : m_GalleryInfoList[359]
    let e = &cfg.entries[3];
    assert_eq!(e.gallery_id, HashId(0x9A2F_0E7B));
    assert_eq!(e.open_cond, "\x01GALLERY_INFO");
    assert_eq!(e.flg_no, 355);
}

#[test]
fn fixture_find_by_id_present() {
    let cfg = parse_gallery_config(&fixture());
    let found = cfg.find_by_id(HashId(0x33AF_67B0));
    assert!(found.is_some(), "find_by_id doit trouver 0x33AF67B0");
    assert_eq!(found.unwrap().img_path, "img_story_ev01_main_0010");
}

#[test]
fn fixture_find_by_id_absent() {
    let cfg = parse_gallery_config(&fixture());
    assert!(
        cfg.find_by_id(HashId(0xDEAD_BEEF)).is_none(),
        "find_by_id doit renvoyer None pour un id absent"
    );
}

#[test]
fn fixture_find_by_flg_no_present() {
    let cfg = parse_gallery_config(&fixture());
    let found = cfg.find_by_flg_no(10);
    assert!(
        found.is_some(),
        "find_by_flg_no(10) doit trouver une entrée"
    );
    assert_eq!(found.unwrap().gallery_id, HashId(0x2482_48B6));
}

#[test]
fn fixture_find_by_flg_no_absent() {
    let cfg = parse_gallery_config(&fixture());
    // flgNo=48 est un gap dans le dump réel — absent aussi de la fixture.
    assert!(cfg.find_by_flg_no(999).is_none());
}

#[test]
fn fixture_liste_manquante_renvoie_vide() {
    // Un JSON sans m_GalleryInfoList doit renvoyer une config vide sans paniquer.
    let root = serde_json::json!({ "version": 100, "lists": [] });
    let cfg = parse_gallery_config(&root);
    assert_eq!(cfg.entries.len(), 0);
}

// ─── Test sur le vrai fichier (skip si absent du VPS) ────────────────────────

const REAL_PATH: &str = "gallery/gallery_config_1.03.71.00.cfg.bin.json";

fn load_real() -> Option<GalleryConfig> {
    let chemin_abs = common::chemin(REAL_PATH)?;
    if !chemin_abs.is_file() {
        eprintln!("skip : {} absent du corpus", chemin_abs.display());
        return None;
    }
    let content = std::fs::read_to_string(&chemin_abs)
        .unwrap_or_else(|e| panic!("Impossible de lire {}: {e}", chemin_abs.display()));
    let root: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("JSON invalide: {e}"));
    Some(parse_gallery_config(&root))
}

#[test]
fn real_file_compte_total() {
    let Some(cfg) = load_real() else { return };
    // gallery_config_1.03.71.00 : 360 entrées dans m_GalleryInfoList
    assert_eq!(
        cfg.entries.len(),
        360,
        "360 entrées GALLERY_INFO dans le dump réel"
    );
}

#[test]
fn real_file_entree0_valeurs() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[0]
    let e = &cfg.entries[0];
    assert_eq!(e.gallery_id, HashId(0x33AF_67B0), "galleryId[0]");
    assert_eq!(e.img_path, "img_story_ev01_main_0010", "imgPath[0]");
    assert_eq!(e.thumb_path, "thumb_story_ev01_main_0010", "thumbPath[0]");
    assert_eq!(e.need_token_num, 1, "needTokenNum[0]");
    assert_eq!(e.flg_no, 0, "flgNo[0]");
    assert_eq!(e.open_cond, "AAAAAA8FNbkZNtoAAQAyAABOKnE=", "openCond[0]");
}

#[test]
fn real_file_entree1_valeurs() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[1]
    let e = &cfg.entries[1];
    assert_eq!(e.gallery_id, HashId(0xAA4D_01B1), "galleryId[1]");
    assert_eq!(e.img_path, "img_story_ev02_main_0010", "imgPath[1]");
    assert_eq!(e.flg_no, 1, "flgNo[1]");
    assert_eq!(e.open_cond, "AAAAAA8FNbkZNtoAAQAyAAB1OnE=", "openCond[1]");
}

#[test]
fn real_file_entree10_valeurs() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[10]
    let e = &cfg.entries[10];
    assert_eq!(e.gallery_id, HashId(0x2482_48B6), "galleryId[10]");
    assert_eq!(e.img_path, "img_story_ev03_main_0060", "imgPath[10]");
    assert_eq!(e.flg_no, 10, "flgNo[10]");
    assert_eq!(e.open_cond, "AAAAAA8FNbkZNtoAAQAyAACcSnE=", "openCond[10]");
}

#[test]
fn real_file_entree100_flg_no_decale() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[100]
    // flgNo=101 ≠ index 100 : il existe un gap à flgNo=48 avant cet index.
    let e = &cfg.entries[100];
    assert_eq!(e.gallery_id, HashId(0x2A1C_3108), "galleryId[100]");
    assert_eq!(e.img_path, "img_story_character_0240", "imgPath[100]");
    assert_eq!(
        e.flg_no, 101,
        "flgNo[100] = 101 (gap à flgNo=48 provoque décalage)"
    );
}

#[test]
fn real_file_dernier_entree_marqueur() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[359] (dernier)
    let e = &cfg.entries[359];
    assert_eq!(e.gallery_id, HashId(0x9A2F_0E7B), "galleryId[359]");
    assert_eq!(e.img_path, "img_other_promotion_0180", "imgPath[359]");
    assert_eq!(e.flg_no, 355, "flgNo[359]");
    assert_eq!(
        e.open_cond, "\x01GALLERY_INFO",
        "openCond[359] = marqueur interne"
    );
}

#[test]
fn real_file_need_token_num_toujours_1() {
    let Some(cfg) = load_real() else { return };

    // Dans tout le dump, needTokenNum = 1 pour toutes les entrées.
    let tous_un = cfg.entries.iter().all(|e| e.need_token_num == 1);
    assert!(
        tous_un,
        "needTokenNum = 1 pour toutes les 360 entrées du dump réel"
    );
}

#[test]
fn real_file_find_by_id() {
    let Some(cfg) = load_real() else { return };

    // gallery_config_1.03.71.00 : m_GalleryInfoList[0].galleryId = "0x33AF67B0"
    let found = cfg.find_by_id(HashId(0x33AF_67B0));
    assert!(
        found.is_some(),
        "find_by_id(0x33AF67B0) doit trouver l'entrée 0"
    );
    assert_eq!(found.unwrap().flg_no, 0);
}

#[test]
fn real_file_find_by_flg_no() {
    let Some(cfg) = load_real() else { return };

    // flgNo=10 → m_GalleryInfoList[10], galleryId = "0x248248B6"
    let found = cfg.find_by_flg_no(10);
    assert!(
        found.is_some(),
        "find_by_flg_no(10) doit trouver une entrée"
    );
    assert_eq!(found.unwrap().gallery_id, HashId(0x2482_48B6));
}

#[test]
fn real_file_flg_no_gap_48_absent() {
    let Some(cfg) = load_real() else { return };

    // flgNo=48 est un gap connu dans le dump réel — aucune entrée ne doit l'avoir.
    assert!(
        cfg.find_by_flg_no(48).is_none(),
        "flgNo=48 est un gap dans le dump réel (non assigné)"
    );
}

// ─── Condition d'ouverture décodée (wiring cond → gallery, 2026-06-23) ───────────
// Chaîne end-to-end sur le vrai dump : entrée gallery → openCond (blob) → UnlockCondition.
const GALLERY_PATH: &str = "gallery/gallery_config_1.03.71.00.cfg.bin.json";

#[test]
fn open_cond_decode_story_episodes_reels() {
    use nie_data::gallery::parse_gallery_config;
    use nie_data::unlock_condition::UnlockType;
    if !std::path::Path::new(GALLERY_PATH).exists() {
        return;
    }
    let txt = std::fs::read_to_string(GALLERY_PATH).unwrap();
    let root: serde_json::Value = serde_json::from_str(&txt).unwrap();
    let cfg = parse_gallery_config(&root);
    assert_eq!(cfg.entries.len(), 360);

    // [0] : story, seuil 20010 = épisode 1 (vérité terrain).
    let c0 = cfg.entries[0].decode_open_cond();
    assert_eq!(c0.kind, UnlockType::Story);
    assert_eq!(c0.story_threshold, Some(20010));
    assert_eq!(c0.story_episode, Some(1));
    // [1] : story, seuil 30010 = épisode 2.
    let c1 = cfg.entries[1].decode_open_cond();
    assert_eq!(c1.story_threshold, Some(30010));
    assert_eq!(c1.story_episode, Some(2));
    // [5] : story, seuil 40010 = épisode 3.
    assert_eq!(
        cfg.entries[5].decode_open_cond().story_threshold,
        Some(40010)
    );

    // Toutes les entrées décodent sans panique en une condition non-triviale story/event.
    let non_trivial = cfg
        .entries
        .iter()
        .filter(|e| e.decode_open_cond().kind != UnlockType::Always)
        .count();
    assert!(
        non_trivial > 300,
        "la plupart des entrées ont une vraie condition (eu {non_trivial})"
    );
}
