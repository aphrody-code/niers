//! Ingestion de `research/nie-index.json` (export Ghidra) dans la base de connaissance.
//!
//! Schéma source (clés courtes) : top-level `functions` (map `FUN_<hex>` → objet), avec
//! par fonction `rt` (type retour), `p` (params), `sl`/`el` (lignes nie.c), `ce` (callees),
//! `st` (strings référencées), `hx` (constantes hex), `dr` (data-refs `DAT_<hex>`),
//! `cx` (complexité), `ns` (sous-système), `ro` (rôle). Plus `pagerank_scores` (map clé→score).

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use nie_index::{Db, ingest};
use serde::Deserialize;

use crate::Stats;

#[derive(Debug, Deserialize)]
pub struct NieIndex {
    #[serde(default)]
    pub total_functions: u64,
    pub functions: HashMap<String, RawFunction>,
    #[serde(default)]
    pub pagerank_scores: HashMap<String, f64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawFunction {
    #[serde(default)]
    pub rt: Option<String>,
    #[serde(default)]
    pub p: Option<String>,
    #[serde(default)]
    pub sl: i64,
    #[serde(default)]
    pub el: i64,
    #[serde(default)]
    pub ce: Vec<String>,
    #[serde(default)]
    pub st: Vec<String>,
    #[serde(default)]
    pub hx: Vec<String>,
    #[serde(default)]
    pub dr: Vec<String>,
    #[serde(default)]
    pub cx: i64,
    #[serde(default)]
    pub ns: Option<String>,
    #[serde(default)]
    pub ro: Option<String>,
}

/// Adresse de base des fonctions synthétiques (clés sans `FUN_<hex>` — les ~192 nommées
/// dont l'index Ghidra ne porte pas l'adresse dans la clé). Hors plage réelle 0x140000000.
const SYNTH_BASE: i64 = 0x7000_0000_0000_0000;

fn parse_fun_vaddr(key: &str) -> Option<i64> {
    let hex = key.strip_prefix("FUN_")?;
    u64::from_str_radix(hex, 16).ok().map(|v| v as i64)
}

fn parse_dat_vaddr(key: &str) -> Option<i64> {
    let hex = key.strip_prefix("DAT_")?;
    u64::from_str_radix(hex, 16).ok().map(|v| v as i64)
}

fn parse_hex_const(s: &str) -> Option<i64> {
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(hex, 16).ok().map(|v| v as i64)
}

/// Charge tout `nie-index.json` et le déverse dans la base sous une seule transaction.
/// Renvoie les compteurs d'ingestion.
pub fn ingest_file(db: &mut Db, binary_id: i64, path: &Path) -> anyhow::Result<Stats> {
    let file =
        File::open(path).map_err(|e| anyhow::anyhow!("ouverture {}: {e}", path.display()))?;
    let idx: NieIndex = serde_json::from_reader(BufReader::new(file))?;
    tracing::info!(
        functions = idx.functions.len(),
        declared = idx.total_functions,
        "nie-index.json chargé"
    );

    // 1re passe (en mémoire) : clé de fonction → adresse virtuelle stable.
    let mut keymap: HashMap<&str, i64> = HashMap::with_capacity(idx.functions.len());
    let mut synth = SYNTH_BASE;
    for key in idx.functions.keys() {
        let vaddr = parse_fun_vaddr(key).unwrap_or_else(|| {
            let v = synth;
            synth += 1;
            v
        });
        keymap.insert(key.as_str(), vaddr);
    }

    let mut stats = Stats::default();
    let tx = db.conn_mut().transaction()?;

    // 2e passe : fonctions + métriques + strings + consts + globals + ancres.
    for (key, f) in &idx.functions {
        let vaddr = keymap[key.as_str()];
        let named = !key.starts_with("FUN_");
        let name = named.then_some(key.as_str());
        let ns = f.ns.as_deref().unwrap_or("standalone");
        let role = f.ro.as_deref().unwrap_or("");
        let confidence = if named { 1.0 } else { 0.0 };

        let fid = ingest::function(
            &tx,
            binary_id,
            vaddr,
            name,
            Some("ghidra"),
            ns,
            role,
            confidence,
        )?;
        let pagerank = idx.pagerank_scores.get(key).copied().unwrap_or(0.0);
        let n_lines = (f.el - f.sl).max(0);
        ingest::function_meta(
            &tx,
            fid,
            f.rt.as_deref(),
            f.p.as_deref(),
            f.cx,
            n_lines,
            pagerank,
        )?;
        stats.functions += 1;

        for s in &f.st {
            ingest::str_ref(&tx, binary_id, fid, s)?;
            stats.str_refs += 1;
        }
        for h in &f.hx {
            if let Some(v) = parse_hex_const(h) {
                ingest::const_value(&tx, fid, v)?;
                stats.consts += 1;
            }
        }
        for d in &f.dr {
            if let Some(g) = parse_dat_vaddr(d) {
                ingest::glob(&tx, binary_id, g, Some(d))?;
                ingest::xref(&tx, binary_id, vaddr, g, "data")?;
                stats.globals += 1;
            }
        }
        // Une fonction nommée par Ghidra est une ancre de vérité pour la propagation.
        if let Some(nm) = name {
            ingest::anchor(&tx, binary_id, vaddr, nm, "func", "ghidra", 1.0)?;
            stats.anchors += 1;
        }
    }

    // 3e passe : arêtes d'appel (call-graph).
    for (key, f) in &idx.functions {
        let from = keymap[key.as_str()];
        for callee in &f.ce {
            let to = keymap
                .get(callee.as_str())
                .copied()
                .or_else(|| parse_fun_vaddr(callee));
            if let Some(to) = to {
                ingest::xref(&tx, binary_id, from, to, "call")?;
                stats.xrefs += 1;
            }
        }
    }

    tx.commit()?;
    tracing::info!(?stats, "ingestion nie-index.json terminée");
    Ok(stats)
}
