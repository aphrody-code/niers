//! Génère un **manifeste auto-descriptif** des bases de jeu résolues (`var/*-resolved.json`) :
//! catalogue (schéma, nombre d'entrées, fichier) + les **clés de référence croisée** entre bases.
//! Rend l'ensemble des 7 bases consommable comme un tout (azalee/wiki).
//!
//! Usage : `cargo run -p nie-game --example export_manifest -- [dir]`  (défaut : `var`)
use serde_json::json;
use std::path::Path;

/// (fichier, description, clés de jointure sortantes vers d'autres bases).
const DATABASES: &[(&str, &str, &[&str])] = &[
    (
        "characters-resolved.json",
        "Personnages (nom, équipe, série, genre, bio)",
        &["teamId→teams", "seriesId→(série)"],
    ),
    ("teams-resolved.json", "Équipes (nom localisé)", &[]),
    (
        "auras-resolved.json",
        "Avatars / Keshin / hissatsu (nom, description, élément)",
        &[],
    ),
    (
        "items-resolved.json",
        "Objets (catégorie, nom, description, prix, stats)",
        &[],
    ),
    (
        "trophies-resolved.json",
        "Succès (nom, description, condition de déblocage)",
        &[],
    ),
    (
        "quests-resolved.json",
        "Quêtes (titre, phase, type, image)",
        &[],
    ),
    (
        "shops-resolved.json",
        "Boutiques (nom, inventaire)",
        &["items[]→items"],
    ),
];

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "var".into());
    let mut entries = Vec::new();
    let mut total = 0u64;
    for (file, desc, refs) in DATABASES {
        let path = Path::new(&dir).join(file);
        let (schema, count) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .map(|v| {
                (
                    v.get("schema")
                        .and_then(|s| s.as_str())
                        .unwrap_or("?")
                        .to_string(),
                    v.get("count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0),
                )
            })
            .unwrap_or_else(|| ("(absent — lancer export_*)".into(), 0));
        total += count;
        entries.push(json!({
            "file": file,
            "schema": schema,
            "count": count,
            "description": desc,
            "crossRefs": refs,
        }));
    }

    let manifest = json!({
        "schema": "niers/databases-manifest/v1",
        "locale": "fr",
        "source": "VFS live IEVR (résolution texte nie-data)",
        "databaseCount": entries.len(),
        "totalEntries": total,
        "databases": entries,
    });
    let out = Path::new(&dir).join("databases-manifest.json");
    let txt = serde_json::to_string_pretty(&manifest).expect("serialize");
    std::fs::write(&out, &txt).unwrap_or_else(|e| panic!("écriture {} : {e}", out.display()));
    eprintln!(
        "✓ export-manifest: {} bases, {total} entrées → {} ({} octets)",
        manifest["databaseCount"],
        out.display(),
        txt.len()
    );
}
