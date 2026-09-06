//! Export industrialisé du **roster de personnages résolu** (pilier B′3) : lit le jeu réel
//! (`chara_base` + `chara_text` fr + `chara_description` fr) via le VFS, résout prénom/nom/bio par
//! les jointures de hash de `nie-data`, et écrit un JSON stable consommable (azalee/wiki).
//!
//! Usage : `cargo run -p nie-game --example export_characters -- [out.json]`
//! (défaut : `var/characters-resolved.json`).
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::vfs::Vfs;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

fn to_iecode(siblings: &[CfgEntry]) -> Vec<serde_json::Value> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<serde_json::Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    Value::String(s) => json!({ "type": "String", "value": s }),
                    Value::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    Value::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            json!({ "name": name, "variables": variables, "children": to_iecode(&e.children) })
        })
        .collect()
}

fn load(vfs: &Vfs, pred: impl Fn(&str) -> bool, what: &str) -> serde_json::Value {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| pred(p))
        .min()
        .unwrap_or_else(|| panic!("{what} introuvable"));
    eprintln!("  {path}");
    let file = cfgbin::parse_t2b(&vfs.read(&path).expect("read")).expect("parse");
    json!({ "entries": to_iecode(&file.entries) })
}

fn rdbn_value_to_json(v: &cfgbin::RdbnValue) -> serde_json::Value {
    use cfgbin::RdbnValue as R;
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
        _ => serde_json::Value::Null,
    }
}

fn load_rdbn(vfs: &Vfs, pred: impl Fn(&str) -> bool, what: &str) -> serde_json::Value {
    use serde_json::Map;
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| pred(p))
        .min()
        .unwrap_or_else(|| panic!("{what} introuvable"));
    eprintln!("  {path}");
    let bytes = vfs.read(&path).expect("read");
    let rdbn = cfgbin::parse(&bytes).expect("parse rdbn");
    let lists: Vec<serde_json::Value> = cfgbin::read_values(&rdbn, &bytes)
        .iter()
        .map(|l| {
            let values: Vec<serde_json::Value> = l
                .rows
                .iter()
                .map(|row| {
                    let mut m = Map::new();
                    for (n, v) in &row.fields {
                        m.insert(n.clone(), rdbn_value_to_json(v));
                    }
                    serde_json::Value::Object(m)
                })
                .collect();
            json!({ "name": l.name, "values": values })
        })
        .collect();
    json!({ "lists": lists })
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "var/characters-resolved.json".into());
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let base_root = load(
        &vfs,
        |p| {
            p.contains("/character/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("chara_base_1") && b.ends_with(".cfg.bin"))
        },
        "chara_base",
    );
    let bases = nie_data::chara_base::parse_all_chara_base(&base_root);
    let nouns = nie_data::chara_text::parse_all_nouns(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b == "chara_text.cfg.bin")
        },
        "chara_text fr",
    ));
    let descs = nie_data::chara_description::parse_chara_descriptions(&load(
        &vfs,
        |p| {
            p.contains("/fr/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b == "chara_description_text.cfg.bin")
        },
        "chara_description fr",
    ));

    // Équipes (belong_team RDBN) + noms d'équipe (team_text T2B) pour résoudre l'équipe de chaque perso.
    let teams = nie_data::belong_team::parse_belong_team_config(&load_rdbn(
        &vfs,
        |p| {
            p.contains("/gamedata/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("belong_team_config") && b.ends_with(".cfg.bin"))
        },
        "belong_team_config",
    ));
    let team_text = nie_data::text::parse_text_file(&load(
        &vfs,
        |p| p.contains("/fr/") && p.rsplit('/').next() == Some("team_text.cfg.bin"),
        "team_text fr",
    ));
    // Séries (chara_series RDBN) pour résoudre la série de chaque perso (origine franchise).
    let series = nie_data::chara_series::parse_chara_series_config(&load_rdbn(
        &vfs,
        |p| {
            p.contains("/character/")
                && p.rsplit('/').next().is_some_and(|b| {
                    b.starts_with("chara_series_config") && b.ends_with(".cfg.bin")
                })
        },
        "chara_series_config",
    ));

    use nie_data::chara_base::{resolve_description, resolve_first_name, resolve_last_name};
    let mut roster: Vec<serde_json::Value> = Vec::new();
    for b in &bases {
        let first = resolve_first_name(b, &nouns);
        // On n'exporte que les personnages avec un prénom localisé (roster réel, pas les stubs NPC).
        let Some(first) = first else { continue };
        // Nom d'équipe résolu via belong_team → team_text.
        let team_name = b
            .belong_team_id
            .and_then(|id| nie_data::belong_team::find_by_id(&teams, id))
            .and_then(|t| nie_data::belong_team::resolve_team_name(t, &team_text));
        roster.push(json!({
            "charaId": b.chara_id.to_hex(),
            "code": b.internal_code,
            "firstName": first,
            "lastName": resolve_last_name(b, &nouns),
            "gender": b.gender,
            "seriesId": b.series_id.map(|h| h.to_hex()),
            "series": nie_data::chara_base::resolve_series_name_fr(b, &series),
            "teamId": b.belong_team_id.map(|h| h.to_hex()),
            "team": team_name,
            "bio": resolve_description(b, &descs),
        }));
    }

    let doc = json!({
        "schema": "niers/characters-resolved/v3",
        "locale": "fr",
        "count": roster.len(),
        "characters": roster,
    });
    let txt = serde_json::to_string_pretty(&doc).expect("serialize");
    std::fs::create_dir_all(Path::new(&out).parent().unwrap_or(Path::new("."))).ok();
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {out} : {e}"));
    eprintln!(
        "✓ export-characters: {} personnages résolus → {out} ({} octets)",
        doc["count"],
        txt.len()
    );
}
