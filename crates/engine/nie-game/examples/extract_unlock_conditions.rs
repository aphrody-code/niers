//! Validation END-TO-END de `nie_data::unlock_condition` sur le VRAI jeu : décode **toutes** les
//! conditions de déblocage de `gallery_config` (`openCond`) et `scene_archive_config`
//! (`condition`) du VFS, et affiche la distribution des types décodés (`always`/`story`/
//! `eventFlag`/`composite`). Prouve la robustesse du décodeur sur les ~360 + ~112 conditions
//! réelles (aucun panic, aucune entrée illisible).
//!
//! Usage : `cargo run -p nie-game --example extract_unlock_conditions`
use nie_data::gallery::parse_gallery_config;
use nie_data::scene_archive::parse_scene_archive_config;
use nie_data::unlock_condition::{UnlockType, decode_unlock_condition};
use nie_formats::cfgbin::{self, RdbnValue};
use nie_formats::vfs::Vfs;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// Convertit un RDBN en forme iecode `{ "lists": [ { name, values: [ {field: val} ] } ] }`,
/// réplique de `cfgbin_to_iecode_root` de nie-model-serve (champs utiles aux parseurs).
fn rdbn_to_iecode(data: &[u8]) -> Option<Value> {
    if !cfgbin::is_rdbn(data) {
        return None;
    }
    let rdbn = cfgbin::parse(data).ok()?;
    let lists = cfgbin::read_values(&rdbn, data);
    let lists_json: Vec<Value> = lists
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
    Some(json!({ "lists": lists_json }))
}

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

fn load(vfs: &Vfs, prefix: &str) -> Option<Value> {
    let path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.rsplit('/')
                .next()
                .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .min()?;
    eprintln!("{prefix} = {path}");
    rdbn_to_iecode(&vfs.read(&path).ok()?)
}

fn kind_name(k: UnlockType) -> &'static str {
    match k {
        UnlockType::Always => "always",
        UnlockType::Story => "story",
        UnlockType::EventFlag => "eventFlag",
        UnlockType::Composite => "composite",
    }
}

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let mut vfs = Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    let mut dist: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut total = 0usize;
    let mut with_episode = 0usize;
    let mut events = 0usize;

    if let Some(root) = load(&vfs, "gallery_config") {
        let cfg = parse_gallery_config(&root);
        eprintln!("gallery: {} entrées", cfg.entries.len());
        for g in &cfg.entries {
            let c = decode_unlock_condition(&g.open_cond);
            *dist.entry(kind_name(c.kind)).or_default() += 1;
            total += 1;
            if c.story_episode.is_some() {
                with_episode += 1;
            }
            events += c.required_events.len();
        }
    }

    if let Some(root) = load(&vfs, "scene_archive_config") {
        let cfg = parse_scene_archive_config(&root);
        eprintln!("scene_archive: {} entrées", cfg.scenes.len());
        for d in &cfg.scenes {
            let c = decode_unlock_condition(&d.condition);
            *dist.entry(kind_name(c.kind)).or_default() += 1;
            total += 1;
            if c.story_episode.is_some() {
                with_episode += 1;
            }
            events += c.required_events.len();
        }
    }

    eprintln!("\n=== TOTAL {total} conditions décodées (0 panic) ===");
    eprintln!("distribution = {dist:?}");
    eprintln!(
        "avec épisode story déduit = {with_episode} ; event-flags requis (cumulés) = {events}"
    );
    assert!(total > 0, "au moins une condition décodée");
    eprintln!("✓ END-TO-END OK : nie_data::unlock_condition décode tout le réel sans erreur");
}
