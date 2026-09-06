#![allow(clippy::pedantic)]
//! Tests golden `movie` — vraies valeurs du dump RDBN à listes extrait du VFS :
//! `data/common/gamedata/movie/movie_playing_config_1.02.28.cfg.bin`
//! (décodé en forme iecode par `nie-model-serve::cfgbin_to_iecode_root`).
//!
//! Aucun parseur inagle dédié `movie` n'existe : la référence est le parseur générique
//! `packages/inagle/src/parsers/universal-gamedata.ts` ; les structs sont dérivées 1:1 des
//! noms de champs RDBN. Vérité terrain = les valeurs réellement lues dans le fichier.

use nie_data::hash::HashId;
use nie_data::movie::{
    MoviePlayingConfig, MoviePlayingInfo, MovieSubtitleMenuData, NONE_SENTINEL,
    parse_movie_playing_config,
};
use serde_json::json;

/// Fixture = forme iecode `{ "lists": [...] }`. Les 3 lignes réelles de
/// `m_MovieSubtitleMenuDataList`, la liste `m_MovieSongCaptionDataList` vide (0 ligne du dump),
/// et 5 lignes représentatives de `m_MoviePlayingInfoList` (indices réels 0, 4, 5, 19, 218).
fn root_fixture() -> serde_json::Value {
    json!({
        "lists": [
            {
                "name": "m_MovieSubtitleMenuDataList",
                "typeName": "MOVIE_SUBTITLE_MENU_DATA",
                "values": [
                    { "subtitleId": "0xC8A5975A", "menuCrc": "0x518597B1", "layerCrc": "0x518597B1",
                      "textLocateCrc": "0xB028913E", "usedGroupCrc": "0x00000000",
                      "layerBGCrc": "0x00000000", "meshBGLocateCrc": "0x00000000" },
                    { "subtitleId": "0x819B0631", "menuCrc": "0x8F0DCDB8", "layerCrc": "0x8F0DCDB8",
                      "textLocateCrc": "0x8CDEAB03", "usedGroupCrc": "0x00000000",
                      "layerBGCrc": "0xD71437D8", "meshBGLocateCrc": "0xCBBAD7BF" },
                    { "subtitleId": "0x32636B36", "menuCrc": "0x8F0DCDB8", "layerCrc": "0x8F0DCDB8",
                      "textLocateCrc": "0x15D7FAB9", "usedGroupCrc": "0x00000000",
                      "layerBGCrc": "0xD71437D8", "meshBGLocateCrc": "0x52B38605" }
                ]
            },
            {
                "name": "m_MovieSongCaptionDataList",
                "typeName": "MOVIE_SONG_CAPTION_DATA",
                "values": []
            },
            {
                "name": "m_MoviePlayingInfoList",
                "typeName": "MOVIE_PLAYING_INFO",
                "values": [
                    { "movieId": "0x15F883E4", "menuId": "0x00000000", "captionId": "0x00000000",
                      "moviePath": "common/movie/ev90_00100.usm", "bgmName": "0x15F883E4",
                      "fedeInTime": 1.0, "fedeOutTime": 1.0, "staffrollDataName": "ed_01",
                      "subtitleTextPath": "0xFFFFFFFF", "subtitleSettingPath": "0xFFFFFFFF",
                      "notStopBgmOnMovieEnd": false, "fadeMethodType": 0 },
                    { "movieId": "0xD1B7673E", "menuId": "0xC8A5975A", "captionId": "0x00000000",
                      "moviePath": "common/movie/ev01_00150.usm", "bgmName": "0xD1B7673E",
                      "fedeInTime": 1.0, "fedeOutTime": 1.0, "staffrollDataName": "0xFFFFFFFF",
                      "subtitleTextPath": "common/text/<LG>/event/ev01_00150.cfg.bin",
                      "subtitleSettingPath": "common/gamedata/event/subtitle/<VLG>/Subtitle_ev01_00150.cfg.bin",
                      "notStopBgmOnMovieEnd": false, "fadeMethodType": 0 },
                    { "movieId": "0xAE862D22", "menuId": "0xC8A5975A", "captionId": "0x00000000",
                      "moviePath": "common/movie/ev01_00200.usm", "bgmName": "0xAE862D22",
                      "fedeInTime": 1.0, "fedeOutTime": 1.0, "staffrollDataName": "ev01_00200",
                      "subtitleTextPath": "common/text/<LG>/event/ev01_00200.cfg.bin",
                      "subtitleSettingPath": "common/gamedata/event/subtitle/<VLG>/Subtitle_ev01_00200.cfg.bin",
                      "notStopBgmOnMovieEnd": false, "fadeMethodType": 0 },
                    { "movieId": "0x15BE9E29", "menuId": "0xC8A5975A", "captionId": "0x00000000",
                      "moviePath": "common/movie/ev01_01000.usm", "bgmName": "0x15BE9E29",
                      "fedeInTime": 1.0, "fedeOutTime": 1.0, "staffrollDataName": "0xFFFFFFFF",
                      "subtitleTextPath": "common/text/<LG>/event/ev01_01000.cfg.bin",
                      "subtitleSettingPath": "common/gamedata/event/subtitle/<VLG>/Subtitle_ev01_01000.cfg.bin",
                      "notStopBgmOnMovieEnd": true, "fadeMethodType": 0 },
                    { "movieId": "0x24176E17", "menuId": "0x00000000", "captionId": "0x00000000",
                      "moviePath": "common/movie/ev09_01300.usm", "bgmName": "0x00000000",
                      "fedeInTime": 1.0, "fedeOutTime": 1.0, "staffrollDataName": "0xFFFFFFFF",
                      "subtitleTextPath": "0xFFFFFFFF", "subtitleSettingPath": "0xFFFFFFFF",
                      "notStopBgmOnMovieEnd": false, "fadeMethodType": 1 }
                ]
            }
        ]
    })
}

fn parsed() -> MoviePlayingConfig {
    parse_movie_playing_config(&root_fixture())
}

#[test]
fn comptes_des_trois_listes() {
    let cfg = parsed();
    assert_eq!(cfg.subtitle_menus.len(), 3, "3 MOVIE_SUBTITLE_MENU_DATA");
    assert_eq!(
        cfg.song_captions.len(),
        0,
        "0 MOVIE_SONG_CAPTION_DATA (liste vide réelle)"
    );
    assert_eq!(
        cfg.playing_infos.len(),
        5,
        "5 lignes MOVIE_PLAYING_INFO échantillonnées"
    );
}

#[test]
fn subtitle_menu_data_champs_hash() {
    let cfg = parsed();
    let s0: &MovieSubtitleMenuData = &cfg.subtitle_menus[0];
    assert_eq!(s0.subtitle_id, HashId(0xC8A5_975A));
    assert_eq!(s0.menu_crc, HashId(0x5185_97B1));
    assert_eq!(s0.layer_crc, HashId(0x5185_97B1)); // layerCrc == menuCrc dans le dump réel
    assert_eq!(s0.text_locate_crc, HashId(0xB028_913E));
    assert_eq!(s0.used_group_crc, HashId::ZERO);
    assert_eq!(s0.layer_bg_crc, HashId::ZERO);
    assert_eq!(s0.mesh_bg_locate_crc, HashId::ZERO);

    // Ligne 2 (dernière) : arrière-plan renseigné.
    let s2 = &cfg.subtitle_menus[2];
    assert_eq!(s2.subtitle_id, HashId(0x3263_6B36));
    assert_eq!(s2.layer_bg_crc, HashId(0xD714_37D8));
    assert_eq!(s2.mesh_bg_locate_crc, HashId(0x52B3_8605));
}

#[test]
fn playing_info_ligne0_staffroll() {
    let cfg = parsed();
    let m: &MoviePlayingInfo = &cfg.playing_infos[0];
    assert_eq!(m.movie_id, HashId(0x15F8_83E4));
    assert_eq!(m.bgm_name, HashId(0x15F8_83E4)); // movieId == bgmName ici
    assert_eq!(m.menu_id, HashId::ZERO);
    assert_eq!(m.caption_id, HashId::ZERO);
    assert_eq!(m.movie_path, "common/movie/ev90_00100.usm");
    assert_eq!(m.movie_usm_path(), Some("common/movie/ev90_00100.usm"));
    assert_eq!(m.staffroll_data_name, "ed_01");
    assert_eq!(m.staffroll_path(), Some("ed_01"));
    // Fondus = 1.0 s, fade method 0, BGM coupée à la fin.
    assert!((m.fede_in_time - 1.0).abs() < f32::EPSILON);
    assert!((m.fede_out_time - 1.0).abs() < f32::EPSILON);
    assert_eq!(m.fade_method_type, 0);
    assert!(!m.not_stop_bgm_on_movie_end);
    // Pas de sous-titres : sentinelle 0xFFFFFFFF conservée byte-exact, accesseur -> None.
    assert_eq!(m.subtitle_text_path, NONE_SENTINEL);
    assert_eq!(m.subtitle_text(), None);
    assert_eq!(m.subtitle_setting(), None);
}

#[test]
fn playing_info_avec_sous_titres_et_menu() {
    let cfg = parsed();
    // Ligne réelle 4 : menu de sous-titres + chemins réels, sans staffroll.
    let m = &cfg.playing_infos[1];
    assert_eq!(m.movie_id, HashId(0xD1B7_673E));
    assert_eq!(m.menu_id, HashId(0xC8A5_975A));
    assert_eq!(
        m.subtitle_text(),
        Some("common/text/<LG>/event/ev01_00150.cfg.bin")
    );
    assert_eq!(
        m.subtitle_setting(),
        Some("common/gamedata/event/subtitle/<VLG>/Subtitle_ev01_00150.cfg.bin")
    );
    assert_eq!(m.staffroll_path(), None); // 0xFFFFFFFF

    // Le menu_id recoupe MovieSubtitleMenuData.subtitle_id[0] -> résolution croisée.
    let menu = cfg.subtitle_menu_of(m).expect("menu de sous-titres résolu");
    assert_eq!(menu.subtitle_id, HashId(0xC8A5_975A));
    assert_eq!(menu.menu_crc, HashId(0x5185_97B1));
}

#[test]
fn playing_info_not_stop_bgm_et_fade_method() {
    let cfg = parsed();
    // Ligne réelle 19 : notStopBgmOnMovieEnd = true.
    let m = &cfg.playing_infos[3];
    assert_eq!(m.movie_id, HashId(0x15BE_9E29));
    assert!(m.not_stop_bgm_on_movie_end);

    // Ligne réelle 218 (dernière) : fadeMethodType = 1, tous chemins sentinelle.
    let last = &cfg.playing_infos[4];
    assert_eq!(last.movie_id, HashId(0x2417_6E17));
    assert_eq!(last.fade_method_type, 1);
    assert_eq!(last.bgm_name, HashId::ZERO);
    assert_eq!(last.movie_usm_path(), Some("common/movie/ev09_01300.usm"));
    assert_eq!(last.staffroll_path(), None);
    assert_eq!(last.subtitle_text(), None);
}

#[test]
fn find_movie_par_hash() {
    let cfg = parsed();
    let found = cfg
        .find_movie(HashId(0xAE86_2D22))
        .expect("ev01_00200 trouvé");
    assert_eq!(found.movie_path, "common/movie/ev01_00200.usm");
    assert_eq!(found.staffroll_data_name, "ev01_00200");
    assert!(cfg.find_movie(HashId(0xDEAD_BEEF)).is_none());
}
