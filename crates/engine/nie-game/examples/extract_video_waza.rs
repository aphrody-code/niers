//! Validation END-TO-END de `nie_data::video_waza` sur le VRAI jeu : convertit
//! `event_movie_config` (RDBN, 3 listes) en forme iecode et fait tourner `parse_video_waza`,
//! puis affiche le nombre de movies/subtitles/captions + combien de movies ont une légende liée.
//!
//! Usage : `cargo run -p nie-game --example extract_video_waza`
use nie_formats::cfgbin::{self, RdbnValue};
use nie_formats::vfs::Vfs;
use serde_json::{Map, Value, json};
use std::path::Path;

fn rdbn_value_to_json(v: &RdbnValue) -> Value {
    use RdbnValue as R;
    match v {
        R::Bool(b) => json!(b),
        R::Byte(n) => json!(n),
        R::Short(n) | R::ActType(n) => json!(n),
        R::Int(n) | R::Flag(n) => json!(n),
        R::Float(f) => json!(f),
        R::Hash(h) => json!(format!("0x{h:08X}")),
        R::Rates(a) | R::Position(a) => json!(a),
        R::Condition(s) => json!(s),
        R::ShortTuple(t) => json!(t),
        _ => Value::Null,
    }
}

fn rdbn_to_iecode(data: &[u8]) -> Option<Value> {
    if !cfgbin::is_rdbn(data) {
        return None;
    }
    let rdbn = cfgbin::parse(data).ok()?;
    let lists: Vec<Value> = cfgbin::read_values(&rdbn, data)
        .iter()
        .map(|l| {
            let values: Vec<Value> = l
                .rows
                .iter()
                .map(|row| {
                    let mut m = Map::new();
                    for (name, val) in &row.fields {
                        m.insert(name.clone(), rdbn_value_to_json(val));
                    }
                    Value::Object(m)
                })
                .collect();
            json!({ "name": l.name, "typeName": l.type_name, "values": values })
        })
        .collect();
    Some(json!({ "lists": lists }))
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.rsplit('/')
                .next()
                .is_some_and(|b| b.starts_with("event_movie_config") && b.ends_with(".cfg.bin"))
        })
        .min()
        .expect("event_movie_config introuvable");
    eprintln!("event_movie = {path}");
    let bytes = vfs.read(&path).expect("read");
    let root = rdbn_to_iecode(&bytes).expect("rdbn->iecode");

    let db = nie_data::video_waza::parse_video_waza(&root);
    eprintln!(
        "[module] videos={} subtitles={} captions={}",
        db.videos.len(),
        db.subtitles.len(),
        db.captions.len()
    );
    assert!(!db.videos.is_empty(), "au moins une vidéo");

    let with_caption = db.videos.iter().filter(|v| v.caption.is_some()).count();
    let with_staffroll = db
        .videos
        .iter()
        .filter(|v| {
            !v.movie.staffroll_data_name.is_empty() && v.movie.staffroll_data_name != "0xFFFFFFFF"
        })
        .count();
    eprintln!(
        "movies avec légende liée = {with_caption} ; avec staffroll nommé = {with_staffroll}"
    );

    for v in db.videos.iter().take(3) {
        eprintln!(
            "  event={} path={:?} bgm={} fadeIn={} caption={:?}",
            v.movie.event_id.to_hex(),
            v.movie.movie_path,
            v.movie.bgm_name.to_hex(),
            v.movie.fede_in_time,
            v.caption.as_ref().map(|c| c.caption_name.as_str())
        );
    }

    // Ligne 0 attendue (probe live).
    let m0 = &db.videos[0].movie;
    assert_eq!(m0.event_id.to_hex(), "0xDB464E8F");
    assert_eq!(m0.movie_path, "common/movie/yw-y1_evop_0010.usm");
    assert_eq!(m0.bgm_name.to_hex(), "0xC1EEF2A7");
    assert_eq!(m0.fede_in_time, 1.0);
    assert!(
        db.videos[0].caption.is_none(),
        "captionId 0 → pas de légende"
    );
    eprintln!("✓ END-TO-END OK : nie_data::video_waza décode le vrai event_movie_config");
}
