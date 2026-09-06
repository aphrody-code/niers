//! Ingestion des noms trouvés par Ghidra sur **le bon binaire**.
//!
//! L'index Ghidra historique de la base (`binary_id=1`) vient d'un autre build
//! et de surcroît désaligné : 3,7 % seulement de ses adresses coïncident avec
//! un début de fonction réel (cf. `docs/RE.md`). Ce module ingère à la place la
//! sortie d'une analyse `analyzeHeadless` rejouée sur le binaire courant, et
//! n'accepte que les correspondances **exactes** d'adresse.
//!
//! Ce qui a de la valeur là-dedans n'est pas la détection de fonctions —
//! `.pdata` et `nie_re::recover` font mieux — mais les noms que Ghidra sait
//! trouver et que rien d'autre ici ne donne :
//!
//! - **FID** (*Function ID*) : reconnaissance par signature des fonctions de
//!   bibliothèques statiques. C'est la seule source de noms **réels** pour la
//!   CRT MSVC et les bibliothèques liées en dur ;
//! - symboles importés et démanglés ;
//! - points d'entrée et fonctions de démarrage.
//!
//! ## Priorité des noms
//!
//! Un nom Ghidra non-`DEFAULT` prime sur un nom **structurel** du dépôt
//! (`vtbl_…::slot_N`, `get_const_…`, `thunk_to_…`) : le structurel désigne, le
//! nom FID *identifie*. Il ne prime en revanche pas sur un nom `strref`, qui
//! vient d'une chaîne du binaire lui-même. Les noms `DEFAULT` de Ghidra
//! (`FUN_<hex>`) sont rejetés — ils n'apprennent rien.

use anyhow::{Context, Result};
use nie_index::{Db, rusqlite};
use tracing::info;

/// Statistiques de l'ingestion.
#[derive(Debug, Clone, Copy, Default)]
pub struct GhidraImportStats {
    /// Lignes lues dans le CSV.
    pub rows: usize,
    /// Lignes dont le nom est un `FUN_<hex>` par défaut — écartées.
    pub default_names: usize,
    /// Lignes dont l'adresse ne correspond à aucune fonction connue.
    pub unmatched: usize,
    /// Noms écrits.
    pub named: usize,
    /// Parmi eux, ceux qui ont remplacé un nom structurel.
    pub replaced_struct: usize,
}

/// Sources de nom de Ghidra qui portent une information.
///
/// `DEFAULT` est le nom auto-généré (`FUN_<hex>`) : sans valeur.
fn is_informative(source: &str) -> bool {
    matches!(source, "ANALYSIS" | "IMPORTED" | "USER_DEFINED")
}

/// Noms structurels produits par ce dépôt, qu'un nom Ghidra peut remplacer.
fn is_structural(name_source: &str) -> bool {
    matches!(
        name_source,
        "leaf-shape" | "vtable-anon-struct" | "vtable-struct" | "iat-thunk" | "pdata" | "vtable"
    )
}

/// Ingère le CSV produit par `scripts/ghidra_export_functions.py`.
///
/// Format attendu : `vaddr,name,source,size,params,cc`, une ligne d'en-tête.
///
/// # Errors
///
/// Échoue si le fichier est illisible ou sur toute erreur SQLite.
pub fn ingest_ghidra_csv(
    db: &mut Db,
    bin: i64,
    csv: &std::path::Path,
) -> Result<GhidraImportStats> {
    let text =
        std::fs::read_to_string(csv).with_context(|| format!("lecture {}", csv.display()))?;
    let mut stats = GhidraImportStats::default();

    let tx = db.conn_mut().transaction()?;
    {
        let mut get = tx.prepare(
            "SELECT name, COALESCE(name_source,'') FROM function WHERE binary_id=?1 AND vaddr=?2",
        )?;
        let mut upd = tx.prepare(
            "UPDATE function SET name=?3, name_source='ghidra-fid' WHERE binary_id=?1 AND vaddr=?2",
        )?;
        for line in text.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let mut it = line.split(',');
            let (Some(va), Some(name), Some(source)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            stats.rows += 1;
            if !is_informative(source) {
                stats.default_names += 1;
                continue;
            }
            let Ok(va) = u64::from_str_radix(va.trim_start_matches("0x"), 16) else {
                continue;
            };
            // Correspondance **exacte** d'adresse uniquement : un début de
            // fonction Ghidra qui ne coïncide avec rien de connu est plus
            // probablement un désalignement qu'une découverte.
            let existing: Option<(Option<String>, String)> = get
                .query_row(rusqlite::params![bin, va as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .ok();
            let Some((cur_name, cur_src)) = existing else {
                stats.unmatched += 1;
                continue;
            };
            match cur_name {
                // Sans nom : on écrit.
                None => {}
                // Nom structurel : le nom Ghidra identifie mieux.
                Some(_) if is_structural(&cur_src) => stats.replaced_struct += 1,
                // `strref`, `funclua`, ou un nom déjà issu de Ghidra : on garde.
                Some(_) => continue,
            }
            stats.named += upd.execute(rusqlite::params![bin, va as i64, name])?;
        }
    }
    tx.commit()?;

    info!(
        rows = stats.rows,
        named = stats.named,
        replaced = stats.replaced_struct,
        "ghidra: noms ingérés"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_sources_informatives_sont_retenues() {
        assert!(is_informative("ANALYSIS"));
        assert!(is_informative("IMPORTED"));
        assert!(is_informative("USER_DEFINED"));
        // `FUN_140001000` : le nom auto-généré n'apprend rien.
        assert!(!is_informative("DEFAULT"));
        assert!(!is_informative("UNKNOWN"));
    }

    #[test]
    fn les_noms_structurels_sont_remplacables_pas_les_semantiques() {
        for s in [
            "leaf-shape",
            "vtable-anon-struct",
            "vtable-struct",
            "iat-thunk",
        ] {
            assert!(is_structural(s), "{s} désigne sans identifier");
        }
        // Ceux-là viennent du binaire lui-même : Ghidra ne fait pas mieux.
        assert!(!is_structural("strref"));
        assert!(!is_structural("funclua"));
        assert!(!is_structural("ghidra-fid"));
    }

    #[test]
    fn le_csv_est_ingere_et_les_noms_par_defaut_ecartes() {
        let mut db = Db::open_in_memory().unwrap();
        let bin = db
            .upsert_binary("t.exe", "sha", "x86_64", 64, 0x1_4000_0000, 0, None, None)
            .unwrap();
        {
            let tx = db.conn_mut().transaction().unwrap();
            // sans nom / nom structurel / nom sémantique
            nie_index::ingest::function(&tx, bin, 0x1000, None, None, "menu", "", 0.0).unwrap();
            nie_index::ingest::function(
                &tx,
                bin,
                0x2000,
                Some("get_const_00ff"),
                Some("leaf-shape"),
                "menu",
                "",
                0.0,
            )
            .unwrap();
            nie_index::ingest::function(
                &tx,
                bin,
                0x3000,
                Some("fn_Truc"),
                Some("strref"),
                "menu",
                "",
                0.0,
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let dir = std::env::temp_dir().join("nie_ghidra_import_test");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("f.csv");
        std::fs::write(
            &csv,
            "vaddr,name,source,size,params,cc\n\
             0x1000,memcpy,ANALYSIS,10,3,__fastcall\n\
             0x2000,_stricmp,ANALYSIS,10,2,__fastcall\n\
             0x3000,autre,ANALYSIS,10,0,\n\
             0x4000,FUN_140004000,DEFAULT,10,0,\n\
             0x9999,orpheline,ANALYSIS,10,0,\n",
        )
        .unwrap();

        let st = ingest_ghidra_csv(&mut db, bin, &csv).unwrap();
        assert_eq!(st.rows, 5);
        assert_eq!(st.default_names, 1, "le FUN_ par défaut est écarté");
        assert_eq!(st.unmatched, 1, "0x9999 ne correspond à aucune fonction");
        assert_eq!(st.named, 2, "0x1000 (sans nom) et 0x2000 (structurel)");
        assert_eq!(st.replaced_struct, 1);

        let nom = |va: i64| -> String {
            db.conn()
                .query_row(
                    "SELECT name FROM function WHERE binary_id=?1 AND vaddr=?2",
                    rusqlite::params![bin, va],
                    |r| r.get::<_, String>(0),
                )
                .unwrap()
        };
        assert_eq!(nom(0x1000), "memcpy");
        assert_eq!(nom(0x2000), "_stricmp");
        assert_eq!(nom(0x3000), "fn_Truc", "un nom sémantique n'est pas écrasé");
    }
}
