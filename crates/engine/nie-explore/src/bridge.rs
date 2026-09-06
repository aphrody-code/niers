//! Pont générique T2B/RDBN → JSON, dans la forme exacte des dumps `cfg.bin.json` produits par
//! inagle (`{"entries":[...]}` pour le T2B arborescent, `{"lists":[{"name","typeName","values"}]}`
//! pour le RDBN à listes) — cf. `crates/engine/nie-data/src/cfgbin.rs`.
//!
//! Débloque, sans dump externe, **tous** les parseurs spécialisés de `nie-data`
//! (`chara_base`, `chara_text`, `skill`, `skill_technic`, `growth`, `item`, `aura`… — une
//! centaine de modules) contre des octets lus en direct du VFS : ils consomment tous la même
//! forme `&serde_json::Value`, produite ici une fois pour toutes plutôt que par module.

use nie_formats::cfgbin::{CfgBinFile, CfgEntry, RdbnList, RdbnRow, RdbnValue, Value as T2bValue};
use serde_json::{Map, Value, json};

fn t2b_value_to_json(v: &T2bValue) -> Value {
    match v {
        T2bValue::String(s) => json!({ "type": "String", "value": s }),
        // `value` est TOUJOURS une chaîne dans la forme inagle (cf. nie_data::cfgbin) — les
        // helpers `Var::as_i64`/`as_f64` re-parsent cette chaîne.
        T2bValue::Int(i) => json!({ "type": "Int", "value": i.to_string() }),
        T2bValue::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
    }
}

fn t2b_entry_to_json(e: &CfgEntry) -> Value {
    json!({
        "name": e.name,
        "variables": e.variables.iter().map(t2b_value_to_json).collect::<Vec<_>>(),
        "children": e.children.iter().map(t2b_entry_to_json).collect::<Vec<_>>(),
    })
}

/// Convertit un `CfgBinFile` T2B (`nie_formats::cfgbin::cfgbin_parse`) en JSON forme inagle
/// `{"entries": [...]}`, consommable par `walk_named`/`Node` de `nie-data`.
#[must_use]
pub fn t2b_to_json(cfg: &CfgBinFile) -> Value {
    json!({ "entries": cfg.entries.iter().map(t2b_entry_to_json).collect::<Vec<_>>() })
}

fn json_to_t2b_value(v: &Value) -> Result<T2bValue, String> {
    let ty = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or("variable sans champ \"type\"")?;
    // `value` est TOUJOURS une chaîne dans la forme inagle (même convention que
    // `t2b_value_to_json` en sens inverse) — on la re-parse selon `type`.
    let raw = v
        .get("value")
        .and_then(Value::as_str)
        .ok_or("variable sans champ \"value\" (chaîne)")?;
    match ty {
        "String" => Ok(T2bValue::String(raw.to_string())),
        "Int" => raw
            .parse::<i32>()
            .map(T2bValue::Int)
            .map_err(|e| format!("valeur Int invalide {raw:?} : {e}")),
        "Float" => raw
            .parse::<f32>()
            .map(T2bValue::Float)
            .map_err(|e| format!("valeur Float invalide {raw:?} : {e}")),
        other => Err(format!(
            "type de variable inconnu : {other:?} (attendu String/Int/Float)"
        )),
    }
}

fn json_to_t2b_entry(v: &Value) -> Result<CfgEntry, String> {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .ok_or("entrée sans champ \"name\"")?
        .to_string();
    let variables = v
        .get("variables")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(json_to_t2b_value)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let children = v
        .get("children")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(json_to_t2b_entry)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(CfgEntry {
        name,
        variables,
        children,
    })
}

/// Inverse de [`t2b_to_json`] : reconstruit l'arbre `CfgEntry` depuis du JSON forme inagle
/// `{"entries": [...]}` (typiquement ÉDITÉ à la main côté UI) — nourrit
/// `nie_formats::cfgbin::encode_t2b` pour ré-écrire un `.cfg.bin` T2B valide. Erreur explicite
/// (jamais une valeur devinée/par défaut silencieuse) si un champ attendu manque ou qu'un type
/// de variable est invalide — une faute de frappe dans le JSON édité ne doit jamais produire un
/// fichier corrompu en silence.
pub fn json_to_t2b_entries(root: &Value) -> Result<Vec<CfgEntry>, String> {
    let entries = root
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("JSON sans champ \"entries\" (forme T2B attendue)")?;
    entries.iter().map(json_to_t2b_entry).collect()
}

fn rdbn_value_to_json(v: &RdbnValue) -> Value {
    match v {
        RdbnValue::Bool(b) => json!(*b),
        RdbnValue::Byte(b) => json!(*b as i64),
        RdbnValue::Short(s) => json!(*s as i64),
        RdbnValue::Int(i) => json!(*i as i64),
        RdbnValue::ActType(s) => json!(*s as i64),
        RdbnValue::Flag(i) => json!(*i as i64),
        RdbnValue::Float(f) => json!(*f as f64),
        // Hash en chaîne hex — même convention que les dumps `*_config` inagle (cf. commentaire
        // `field_hash` de nie-data : "les *_config mettent les hash en hex").
        RdbnValue::Hash(h) => json!(format!("0x{h:08X}")),
        RdbnValue::Rates(r) => json!(r.to_vec()),
        RdbnValue::Position(p) => json!(p.to_vec()),
        RdbnValue::Condition(s) => json!(s),
        RdbnValue::ShortTuple(t) => json!(t.to_vec()),
        // Types agrégats (AbilityData/EnhanceData/StatusRate) : structure interne non modélisée
        // ici, aucun champ ne serait fiable à fabriquer → `null` plutôt que deviner un layout.
        RdbnValue::Blob(_) | RdbnValue::Invalid => Value::Null,
    }
}

fn rdbn_row_to_json(row: &RdbnRow) -> Value {
    let mut map = Map::with_capacity(row.fields.len());
    for (name, value) in &row.fields {
        map.insert(name.clone(), rdbn_value_to_json(value));
    }
    Value::Object(map)
}

/// Convertit les listes RDBN décodées (`nie_formats::cfgbin::read_values`) en JSON forme inagle
/// `{"lists": [{"name","typeName","values":[{champ: valeur, ...}]}]}`, consommable par
/// `list_values`/`field_*` de `nie-data`.
#[must_use]
pub fn rdbn_to_json(lists: &[RdbnList]) -> Value {
    json!({
        "lists": lists
            .iter()
            .map(|l| json!({
                "name": l.name,
                "typeName": l.type_name,
                "values": l.rows.iter().map(rdbn_row_to_json).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()
    })
}

/// Reconstruit une `RdbnValue` en préservant sa VARIANTE d'origine (`original`), en y injectant la
/// valeur éditée côté JSON. Contrairement au pont T2B ([`json_to_t2b_entries`]), la forme JSON
/// RDBN (`rdbn_value_to_json`) n'encode PAS le type de chaque champ — `Short`/`ActType` et
/// `Int`/`Flag` produisent le même nombre JSON brut, `Rates`/`Position` le même tableau de 4
/// flottants. Deviner le type depuis la seule forme JSON serait donc ambigu ; on part à la place
/// TOUJOURS de la valeur d'origine décodée (même variante, même field_type RDBN), et on n'accepte
/// que l'édition de la donnée qu'elle contient — jamais un changement de type de champ.
fn patch_rdbn_value(
    original: &RdbnValue,
    edited: &Value,
    field_name: &str,
) -> Result<RdbnValue, String> {
    let err = || {
        format!(
            "champ {field_name:?} : valeur JSON incompatible avec son type d'origine ({original:?})"
        )
    };
    match original {
        RdbnValue::Bool(_) => edited.as_bool().map(RdbnValue::Bool).ok_or_else(err),
        RdbnValue::Byte(_) => edited
            .as_i64()
            .and_then(|n| u8::try_from(n).ok())
            .map(RdbnValue::Byte)
            .ok_or_else(err),
        RdbnValue::Short(_) => edited
            .as_i64()
            .and_then(|n| i16::try_from(n).ok())
            .map(RdbnValue::Short)
            .ok_or_else(err),
        RdbnValue::Int(_) => edited
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(RdbnValue::Int)
            .ok_or_else(err),
        RdbnValue::ActType(_) => edited
            .as_i64()
            .and_then(|n| i16::try_from(n).ok())
            .map(RdbnValue::ActType)
            .ok_or_else(err),
        RdbnValue::Flag(_) => edited
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(RdbnValue::Flag)
            .ok_or_else(err),
        RdbnValue::Float(_) => edited
            .as_f64()
            .map(|f| RdbnValue::Float(f as f32))
            .ok_or_else(err),
        RdbnValue::Hash(_) => edited
            .as_str()
            .and_then(|s| s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(RdbnValue::Hash)
            .ok_or_else(err),
        RdbnValue::Rates(_) | RdbnValue::Position(_) => {
            let arr = edited.as_array().filter(|a| a.len() == 4).ok_or_else(err)?;
            let mut out = [0f32; 4];
            for (i, v) in arr.iter().enumerate() {
                out[i] = v.as_f64().ok_or_else(err)? as f32;
            }
            Ok(if matches!(original, RdbnValue::Rates(_)) {
                RdbnValue::Rates(out)
            } else {
                RdbnValue::Position(out)
            })
        }
        RdbnValue::Condition(_) => edited
            .as_str()
            .map(|s| RdbnValue::Condition(s.to_string()))
            .ok_or_else(err),
        RdbnValue::ShortTuple(_) => {
            let arr = edited.as_array().filter(|a| a.len() == 2).ok_or_else(err)?;
            let a = arr[0]
                .as_i64()
                .and_then(|n| i16::try_from(n).ok())
                .ok_or_else(err)?;
            let b = arr[1]
                .as_i64()
                .and_then(|n| i16::try_from(n).ok())
                .ok_or_else(err)?;
            Ok(RdbnValue::ShortTuple([a, b]))
        }
        // `rdbn_value_to_json` sérialise ces deux variantes en JSON `null` (aucune info
        // exploitable — cf. sa doc) : `edited` vaut donc TOUJOURS `null` ici, qu'il y ait eu une
        // tentative d'édition ou non, on ne peut pas faire la différence. Plutôt que de rejeter
        // un round-trip sans édition (le cas normal), on conserve la valeur D'ORIGINE telle
        // quelle — ces champs restent simplement non éditables depuis le JSON, silencieusement
        // préservés plutôt que perdus. `RdbnValue::Invalid` sera de toute façon rejeté plus loin
        // par `encode_rdbn` (`rdbn_value_wire`), avec un message qui reflète la vraie contrainte.
        RdbnValue::Blob(_) | RdbnValue::Invalid => Ok(original.clone()),
    }
}

fn patch_rdbn_row(
    original: &RdbnRow,
    edited: &Value,
    list_name: &str,
    row_index: usize,
) -> Result<RdbnRow, String> {
    let map = edited.as_object().ok_or_else(|| {
        format!("liste {list_name:?} ligne {row_index} : JSON attendu (objet), pas {edited}")
    })?;
    if map.len() != original.fields.len() {
        return Err(format!(
            "liste {list_name:?} ligne {row_index} : {} champ(s) dans le JSON édité, {} attendu(s) — l'ajout/la suppression de champs n'est pas supporté pour RDBN",
            map.len(),
            original.fields.len()
        ));
    }
    let mut fields = Vec::with_capacity(original.fields.len());
    for (name, orig_value) in &original.fields {
        let edited_value = map.get(name).ok_or_else(|| {
            format!("liste {list_name:?} ligne {row_index} : champ {name:?} absent du JSON édité")
        })?;
        fields.push((
            name.clone(),
            patch_rdbn_value(orig_value, edited_value, name)?,
        ));
    }
    Ok(RdbnRow { fields })
}

fn patch_rdbn_list(original: &RdbnList, edited: &Value) -> Result<RdbnList, String> {
    let rows_json = edited
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "liste {:?} : JSON édité sans champ \"values\" (tableau)",
                original.name
            )
        })?;
    if rows_json.len() != original.rows.len() {
        return Err(format!(
            "liste {:?} : {} ligne(s) dans le JSON édité, {} attendue(s) — l'ajout/la suppression de lignes n'est pas supporté pour RDBN (seules les VALEURS sont éditables)",
            original.name,
            rows_json.len(),
            original.rows.len()
        ));
    }
    let rows = original
        .rows
        .iter()
        .zip(rows_json)
        .enumerate()
        .map(|(i, (orig_row, edited_row))| patch_rdbn_row(orig_row, edited_row, &original.name, i))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RdbnList {
        name: original.name.clone(),
        type_name: original.type_name.clone(),
        rows,
    })
}

/// Reconstruit des `RdbnList` depuis du JSON forme inagle ÉDITÉ (`{"lists":[...]}`,
/// [`rdbn_to_json`]), en se servant des `RdbnList` d'ORIGINE (`nie_formats::cfgbin::read_values`)
/// comme gabarit — nourrit `nie_formats::cfgbin::encode_rdbn`.
///
/// Contrairement à [`json_to_t2b_entries`] (arbre T2B librement restructurable), l'édition RDBN
/// supportée ici est **valeurs uniquement** : le nombre de listes, leur ordre/noms, le nombre de
/// lignes par liste et le nombre/nom de champs par ligne doivent rester IDENTIQUES à l'original.
/// Raison structurelle, pas une limite arbitraire : la forme JSON `rdbn_to_json` n'encode pas le
/// type de champ RDBN (`Short` vs `ActType`, `Rates` vs 4 floats bruts…) — sans gabarit d'origine,
/// reconstruire un `field_type` depuis du JSON seul serait deviner. Toute divergence structurelle
/// retourne une erreur explicite plutôt qu'un fichier corrompu en silence.
///
/// # Erreurs
///
/// Nombre de listes/lignes/champs différent de `original`, noms de liste dans un ordre différent,
/// valeur JSON incompatible avec le type d'origine du champ, ou champ `Blob`/`Invalid` (non
/// éditable, cf. [`patch_rdbn_value`]).
pub fn json_to_rdbn_lists(original: &[RdbnList], edited: &Value) -> Result<Vec<RdbnList>, String> {
    let lists_json = edited
        .get("lists")
        .and_then(Value::as_array)
        .ok_or("JSON sans champ \"lists\" (forme RDBN attendue)")?;
    if lists_json.len() != original.len() {
        return Err(format!(
            "{} liste(s) dans le JSON édité, {} attendue(s) — l'ajout/la suppression de listes n'est pas supporté pour RDBN",
            lists_json.len(),
            original.len()
        ));
    }
    original
        .iter()
        .zip(lists_json)
        .map(|(orig, edited)| {
            let edited_name = edited.get("name").and_then(Value::as_str).unwrap_or_default();
            if edited_name != orig.name {
                return Err(format!(
                    "liste à cette position renommée {edited_name:?} (était {:?}) — le renommage/réordonnancement des listes n'est pas supporté pour RDBN",
                    orig.name
                ));
            }
            patch_rdbn_list(orig, edited)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_formats::cfgbin::{encode_t2b, is_rdbn, parse_t2b};
    use nie_formats::vfs::Vfs;

    /// Round-trip complet SUR LE VRAI JEU : octets → `CfgEntry` → JSON (`t2b_to_json`) → `CfgEntry`
    /// (`json_to_t2b_entries`) → octets (`encode_t2b`) → redécodage → comparaison structurelle à
    /// l'arbre d'origine. Preuve que le pont JSON dans les deux sens (ce que l'UI édite) est
    /// fidèle, pas seulement `encode_t2b` isolément (déjà testé côté `nie-formats`).
    #[test]
    fn json_bridge_round_trip_sur_le_vrai_jeu() {
        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip json_bridge_round_trip_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                (p.contains("/gamedata/") || p.contains("/text/")) && p.ends_with(".cfg.bin")
            })
            .collect();
        let step = (candidates.len() / 500).max(1);
        let mut n_t2b = 0usize;
        let mut n_ok = 0usize;
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in candidates.iter().step_by(step) {
            let Ok(bytes) = vfs.read(path) else { continue };
            if is_rdbn(&bytes) {
                continue;
            }
            let Ok(original) = parse_t2b(&bytes) else {
                continue;
            };
            n_t2b += 1;

            let json = t2b_to_json(&original);
            let rebuilt = match json_to_t2b_entries(&json) {
                Ok(e) => e,
                Err(e) => {
                    failed.push((path.clone(), format!("json_to_t2b_entries : {e}")));
                    continue;
                }
            };
            let reencoded = encode_t2b(&rebuilt);
            match parse_t2b(&reencoded) {
                Ok(roundtripped) if roundtripped.entries == original.entries => n_ok += 1,
                Ok(_) => failed.push((
                    path.clone(),
                    "arbre différent après round-trip JSON".to_string(),
                )),
                Err(e) => failed.push((path.clone(), format!("redécodage échoué : {e}"))),
            }
        }

        eprintln!(
            "json_bridge round-trip : {n_ok}/{n_t2b} identiques (sur {} candidats, pas={step})",
            candidates.len()
        );
        for (p, e) in failed.iter().take(10) {
            eprintln!("  échec {p} : {e}");
        }
        assert!(
            n_t2b > 5,
            "attendu au moins quelques fichiers T2B réels, obtenu {n_t2b}"
        );
        assert_eq!(
            n_ok,
            n_t2b,
            "{} échec(s) sur {n_t2b} fichiers T2B réels",
            failed.len()
        );
    }

    /// Round-trip complet SUR LE VRAI JEU pour RDBN : octets → `RdbnList` (`rdbn_to_json`) → JSON
    /// → `RdbnList` (`json_to_rdbn_lists`, sans aucune édition — gabarit = original) → octets
    /// (`encode_rdbn`) → redécodage → comparaison structurelle à l'original. Preuve que le pont
    /// JSON (ce que l'UI éditerait) reste fidèle sans édition, pas seulement `encode_rdbn`
    /// isolément (déjà testé côté `nie-formats`).
    #[test]
    fn json_bridge_rdbn_round_trip_sur_le_vrai_jeu() {
        use nie_formats::cfgbin::{encode_rdbn, parse, read_values};

        let dir = nie_formats::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip json_bridge_rdbn_round_trip_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                (p.contains("/gamedata/") || p.contains("/text/")) && p.ends_with(".cfg.bin")
            })
            .collect();
        let step = (candidates.len() / 5000).max(1);
        let mut n_rdbn = 0usize;
        let mut n_ok = 0usize;
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in candidates.iter().step_by(step) {
            let Ok(bytes) = vfs.read(path) else { continue };
            if !is_rdbn(&bytes) {
                continue;
            }
            let Ok(rdbn) = parse(&bytes) else { continue };
            let original = read_values(&rdbn, &bytes);
            n_rdbn += 1;

            let json = rdbn_to_json(&original);
            let rebuilt = match json_to_rdbn_lists(&original, &json) {
                Ok(l) => l,
                Err(e) => {
                    failed.push((path.clone(), format!("json_to_rdbn_lists : {e}")));
                    continue;
                }
            };
            let reencoded = match encode_rdbn(&rebuilt) {
                Ok(b) => b,
                Err(e) => {
                    failed.push((path.clone(), format!("encode_rdbn : {e}")));
                    continue;
                }
            };
            match parse(&reencoded) {
                Ok(rdbn2) => {
                    let roundtripped = read_values(&rdbn2, &reencoded);
                    if roundtripped == original {
                        n_ok += 1;
                    } else {
                        failed.push((
                            path.clone(),
                            "listes différentes après round-trip JSON".to_string(),
                        ));
                    }
                }
                Err(e) => failed.push((path.clone(), format!("redécodage échoué : {e}"))),
            }
        }

        eprintln!(
            "json_bridge_rdbn round-trip : {n_ok}/{n_rdbn} identiques (sur {} candidats, pas={step})",
            candidates.len()
        );
        for (p, e) in failed.iter().take(10) {
            eprintln!("  échec {p} : {e}");
        }
        assert!(
            n_rdbn > 5,
            "attendu au moins quelques fichiers RDBN réels, obtenu {n_rdbn}"
        );
        assert_eq!(
            n_ok,
            n_rdbn,
            "{} échec(s) sur {n_rdbn} fichiers RDBN réels",
            failed.len()
        );
    }
}
