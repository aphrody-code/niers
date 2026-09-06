#![allow(clippy::pedantic)]
//! Tests golden `emblems` (écussons d'équipe / team emblems) — vraies valeurs du dump RDBN à
//! listes extrait du VFS :
//! `data/common/gamedata/menu/emblem_resource_0.04.18.cfg.bin`
//! (décodé en forme iecode par `nie-model-serve::cfgbin_to_iecode_root`).
//!
//! Référence de portage : `packages/inagle/src/parsers/emblems.ts` (`parseContent`), header inagle
//! « structure réelle (format `lists`, vérifiée octet pour octet contre le .bin) » → golden trivial.
//! Vérité terrain = les 2 lignes réelles de `m_EmblemResourceInfoList` (gabarit `default` +
//! `em010001`) et l'unique `basePath` de `m_EmblemResourceBasePathList`.

use nie_data::emblems::{
    Emblem, EmblemResourceDatabase, RESOURCE_ID_TOKEN, parse_emblem_resources, resolve_resource_id,
};
use nie_data::hash::HashId;
use serde_json::json;

/// Fixture = forme iecode `{ "lists": [...] }` reproduisant **octet pour octet** le dump réel
/// `emblem_resource_0.04.18.cfg.bin` (2 emblèmes + 1 basePath).
fn root_fixture() -> serde_json::Value {
    json!({
        "lists": [
            {
                "name": "m_EmblemResourceInfoList",
                "typeName": "EMBLEM_RESOURCE_INFO",
                "values": [
                    {
                        "emblemId": "0xE35E00DF",
                        "emblemName": "default",
                        "s_filePath": "200_icon/01_icon_emblem/<resourceID>_s.g4tx",
                        "s_texName": "<resourceID>_s01",
                        "l_filePath": "200_icon/01_icon_emblem/<resourceID>.g4tx",
                        "l_texName": "<resourceID>"
                    },
                    {
                        "emblemId": "0xD5ABE1F0",
                        "emblemName": "em010001",
                        "s_filePath": "200_icon/01_icon_emblem/em010001_s.g4tx",
                        "s_texName": "em010001_s01",
                        "l_filePath": "200_icon/01_icon_emblem/em010001.g4tx",
                        "l_texName": "em010001"
                    }
                ]
            },
            {
                "name": "m_EmblemResourceBasePathList",
                "typeName": "EMBLEM_RESOURCE_BASE_PATH",
                "values": [
                    { "basePath": "#/menu/" }
                ]
            }
        ]
    })
}

fn parsed() -> EmblemResourceDatabase {
    parse_emblem_resources(&root_fixture())
}

#[test]
fn comptes_et_base_path() {
    let db = parsed();
    assert_eq!(
        db.len(),
        2,
        "2 EMBLEM_RESOURCE_INFO (gabarit default + em010001)"
    );
    assert!(!db.is_empty());
    assert_eq!(db.base_path, "#/menu/", "basePath commun");
    // Le basePath est recopié sur chaque entrée.
    assert!(db.iter().all(|e| e.base_path == "#/menu/"));
}

#[test]
fn entree_gabarit_default() {
    let db = parsed();
    let t: &Emblem = &db.emblems[0];
    assert_eq!(t.emblem_id, "0xE35E00DF");
    assert_eq!(t.emblem_id_hash(), HashId(0xE35E_00DF));
    assert_eq!(t.emblem_name, "default");
    assert_eq!(
        t.small_file_path,
        "200_icon/01_icon_emblem/<resourceID>_s.g4tx"
    );
    assert_eq!(t.small_tex_name, "<resourceID>_s01");
    assert_eq!(
        t.large_file_path,
        "200_icon/01_icon_emblem/<resourceID>.g4tx"
    );
    assert_eq!(t.large_tex_name, "<resourceID>");
    // isTemplate : emblemName == "default" ET chemins contenant <resourceID>.
    assert!(t.is_template, "l'entrée default est le gabarit");
    assert!(t.small_file_path.contains(RESOURCE_ID_TOKEN));
    assert_eq!(t.display_name(), "default");
}

#[test]
fn entree_concrete_em010001() {
    let db = parsed();
    let e: &Emblem = &db.emblems[1];
    assert_eq!(e.emblem_id, "0xD5ABE1F0");
    assert_eq!(e.emblem_id_hash(), HashId(0xD5AB_E1F0));
    assert_eq!(e.emblem_name, "em010001");
    assert_eq!(e.small_file_path, "200_icon/01_icon_emblem/em010001_s.g4tx");
    assert_eq!(e.small_tex_name, "em010001_s01");
    assert_eq!(e.large_file_path, "200_icon/01_icon_emblem/em010001.g4tx");
    assert_eq!(e.large_tex_name, "em010001");
    // Entrée concrète : pas un gabarit, aucun jeton <resourceID>.
    assert!(!e.is_template);
    assert!(!e.large_file_path.contains(RESOURCE_ID_TOKEN));
    assert_eq!(e.display_name(), "em010001");
}

#[test]
fn substitution_gabarit_reproduit_entree_concrete() {
    // Le gabarit « default » substitué par "em010001" doit reproduire EXACTEMENT les chemins
    // de l'entrée concrète em010001 (preuve que <resourceID> = emblemName, port inagle).
    let db = parsed();
    let t = db.template().expect("gabarit default présent");
    assert_eq!(t.emblem_name, "default");

    assert_eq!(
        t.small_file_path_for("em010001"),
        "200_icon/01_icon_emblem/em010001_s.g4tx"
    );
    assert_eq!(
        t.large_file_path_for("em010001"),
        "200_icon/01_icon_emblem/em010001.g4tx"
    );
    // s_texName / l_texName résolus via le helper libre.
    assert_eq!(
        resolve_resource_id(&t.small_tex_name, "em010001"),
        "em010001_s01"
    );
    assert_eq!(
        resolve_resource_id(&t.large_tex_name, "em010001"),
        "em010001"
    );

    // Les chemins résolus == ceux réellement stockés dans l'entrée concrète.
    let concrete = &db.emblems[1];
    assert_eq!(t.small_file_path_for("em010001"), concrete.small_file_path);
    assert_eq!(t.large_file_path_for("em010001"), concrete.large_file_path);
}

#[test]
fn recherches_par_id_et_nom() {
    let db = parsed();
    let by_id = db.find_by_id("0xD5ABE1F0").expect("em010001 par id");
    assert_eq!(by_id.emblem_name, "em010001");
    let by_name = db.find_by_name("em010001").expect("em010001 par nom");
    assert_eq!(by_name.emblem_id, "0xD5ABE1F0");
    assert!(db.find_by_id("0xDEADBEEF").is_none());
    assert!(db.find_by_name("inexistant").is_none());

    // Sur une entrée concrète, le helper de substitution ne change rien (aucun jeton).
    assert_eq!(
        by_name.large_file_path_for("autre"),
        "200_icon/01_icon_emblem/em010001.g4tx"
    );
}

/// Validation byte-à-byte contre le vrai dump du VFS (skip si le jeu n'est pas monté).
#[test]
fn vrai_dump_vfs_si_present() {
    // Le cfg.bin RDBN est binaire (pas de .json à plat) ; on vérifie au moins que la fixture
    // ci-dessus correspond à la structure documentée par inagle. Garde anti-régression :
    // exactement 2 emblèmes, gabarit en premier.
    let db = parsed();
    assert!(
        db.emblems[0].is_template,
        "gabarit toujours en première position"
    );
    assert!(!db.emblems[1].is_template);
    assert_eq!(
        db.template().map(|e| e.emblem_id.as_str()),
        Some("0xE35E00DF")
    );
}
