#![allow(clippy::pedantic)]
//! Tests golden `video_waza` — port 1:1 d'inagle `packages/inagle/src/parsers/video-waza.ts`.
//!
//! Vérité terrain = le vrai `event/event_movie_config_0.00.00.cfg.bin` (VFS IEVR, 3 listes RDBN),
//! via `cargo run -p nie-game --example extract_video_waza` (videos=4, subtitles=3, captions=2 ;
//! ligne movie 0 = `0xDB464E8F`, path `yw-y1_evop_0010.usm`, bgm `0xC1EEF2A7`, fede 1.0). Les
//! valeurs réelles ci-dessous (movies, caption, subtitle) sont figées comme fixtures hors-ligne ;
//! la jointure movie↔caption (par `captionId`) est exercée explicitement.

use nie_data::hash::HashId;
use nie_data::video_waza::{EventSongCaptionData, EventSubtitleMenuData, parse_video_waza};
use serde_json::{Value, json};

fn movie(
    event: &str,
    caption: &str,
    path: &str,
    bgm: &str,
    fin: f64,
    fout: f64,
    staff: &str,
) -> Value {
    json!({
        "eventId": event, "menuId": "0x00000000", "captionId": caption,
        "moviePath": path, "bgmName": bgm,
        "fedeInTime": fin, "fedeOutTime": fout, "staffrollDataName": staff
    })
}

fn root_fixture() -> Value {
    json!({
        "lists": [
            { "name": "m_EventMovieInfoList", "values": [
                // movie réel 0 : captionId nul → pas de légende.
                movie("0xDB464E8F", "0x00000000", "common/movie/yw-y1_evop_0010.usm",
                      "0xC1EEF2A7", 1.0, 1.0, "0xFFFFFFFF"),
                // movie réel 1 : staffroll nommé "op_01", captionId pointant la légende réelle.
                movie("0x147CF41E", "0x1A323042", "common/movie/yw-y1_evop_0020.usm",
                      "0x6BA84291", 1.0, 1.0, "op_01"),
            ]},
            { "name": "m_EventSongCaptionDataList", "values": [
                { "captionId": "0x1A323042", "startFrame": 220, "endFrame": 453,
                  "captionName": "telop_ev01_2501" },
            ]},
            { "name": "m_EventSubtitleMenuDataList", "values": [
                { "subtitleId": "0xC8A5975A", "menuCrc": "0x518597B1", "layerCrc": "0x518597B1",
                  "textLocateCrc": "0xE40E3B28", "usedGroupCrc": "0xF4DBDF21" },
            ]},
        ]
    })
}

#[test]
fn movie_zero_real_no_caption() {
    let db = parse_video_waza(&root_fixture());
    assert_eq!(db.videos.len(), 2);
    let v0 = &db.videos[0];
    assert_eq!(v0.movie.event_id, HashId(0xDB46_4E8F));
    assert_eq!(v0.movie.movie_path, "common/movie/yw-y1_evop_0010.usm");
    assert_eq!(v0.movie.bgm_name.to_hex(), "0xC1EEF2A7");
    assert_eq!(v0.movie.fede_in_time, 1.0);
    assert_eq!(v0.movie.fede_out_time, 1.0);
    assert_eq!(v0.movie.staffroll_data_name, "0xFFFFFFFF"); // sentinelle « aucun »
    assert!(
        v0.caption.is_none(),
        "captionId 0x00000000 → pas de légende"
    );
}

#[test]
fn movie_one_joins_caption() {
    let db = parse_video_waza(&root_fixture());
    let v1 = &db.videos[1];
    assert_eq!(v1.movie.event_id, HashId(0x147C_F41E));
    assert_eq!(v1.movie.staffroll_data_name, "op_01");
    // Jointure par captionId → la vraie légende.
    let cap = v1.caption.as_ref().expect("légende liée");
    assert_eq!(cap.caption_id, HashId(0x1A32_3042));
    assert_eq!(cap.start_frame, 220);
    assert_eq!(cap.end_frame, 453);
    assert_eq!(cap.caption_name, "telop_ev01_2501");
}

#[test]
fn raw_lists_preserved() {
    let db = parse_video_waza(&root_fixture());
    assert_eq!(db.captions.len(), 1);
    assert_eq!(
        db.captions[0],
        EventSongCaptionData {
            caption_id: HashId(0x1A32_3042),
            start_frame: 220,
            end_frame: 453,
            caption_name: "telop_ev01_2501".into(),
        }
    );
    assert_eq!(db.subtitles.len(), 1);
    assert_eq!(
        db.subtitles[0],
        EventSubtitleMenuData {
            subtitle_id: HashId(0xC8A5_975A),
            menu_crc: HashId(0x5185_97B1),
            layer_crc: HashId(0x5185_97B1),
            text_locate_crc: HashId(0xE40E_3B28),
            used_group_crc: HashId(0xF4DB_DF21),
        }
    );
}

#[test]
fn find_by_event_id_works() {
    let db = parse_video_waza(&root_fixture());
    assert!(db.find_by_event_id(HashId(0xDB46_4E8F)).is_some());
    assert!(db.find_by_event_id(HashId(0xDEAD_BEEF)).is_none());
}

#[test]
fn empty_config_is_empty() {
    let db = parse_video_waza(&json!({ "lists": [] }));
    assert!(db.videos.is_empty() && db.subtitles.is_empty() && db.captions.is_empty());
}
