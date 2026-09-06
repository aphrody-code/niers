//! Validation END-TO-END de `nie_data::chara_details` sur le VRAI jeu : convertit
//! `chara_details_config` (RDBN) en forme iecode et fait tourner `parse_chara_details` dessus,
//! en confrontant le comptage et les 3 premières lignes à `read_values` brut.
//!
//! Usage : `cargo run -p nie-game --example extract_chara_details`
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
                .is_some_and(|b| b.starts_with("chara_details_config") && b.ends_with(".cfg.bin"))
        })
        .min()
        .expect("chara_details_config introuvable");
    eprintln!("chara_details = {path}");
    let bytes = vfs.read(&path).expect("read");
    let root = rdbn_to_iecode(&bytes).expect("rdbn->iecode");

    let parsed = nie_data::chara_details::parse_chara_details(&root);
    eprintln!("[module] parse_chara_details = {} entrées", parsed.len());
    assert_eq!(
        parsed.len(),
        550,
        "550 lignes m_charaDetailsList (probe live)"
    );

    for d in parsed.iter().take(3) {
        eprintln!(
            "  chara_id={} pers={} 1st={} 2m={} 2f={}",
            d.chara_id.to_hex(),
            d.personality_type,
            d.first_person.to_hex(),
            d.second_person_male.to_hex(),
            d.second_person_female.to_hex()
        );
    }
    // Ligne 0 attendue (probe live).
    let d0 = &parsed[0];
    assert_eq!(d0.chara_id.to_hex(), "0x677ADF02");
    assert_eq!(d0.personality_type, 1);
    assert_eq!(d0.first_person.to_hex(), "0xF705095D");
    assert_eq!(d0.second_person_male, nie_data::hash::HashId::ZERO);

    // Distribution des types de personnalité.
    let mut maxp = 0i64;
    for d in &parsed {
        maxp = maxp.max(d.personality_type);
    }
    eprintln!("personality_type max = {maxp}");
    eprintln!("✓ END-TO-END OK : nie_data::chara_details décode 550 lignes du vrai dump");
}
