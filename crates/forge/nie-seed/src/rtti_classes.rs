//! Ingestion des classes RTTI `game::` / `lives::` depuis `nie-rtti-classes.txt`.
//!
//! Chaque ligne du fichier a la forme `ClassName [namespace]` où `namespace`
//! vaut `game` ou `lives`. Pour chaque classe on insère :
//! - une ligne dans `rtti_class` via [`nie_index::ingest::rtti_class`] ;
//! - une ancre de kind `rtti`, vaddr `0` (adresse inconnue à ce stade),
//!   via [`nie_index::ingest::anchor`].

use std::path::Path;

use anyhow::{Context, Result};
use nie_index::{Db, ingest};
use tracing::debug;

/// Parse `refs/iecode-re/research/nie-rtti-classes.txt` et insère toutes les
/// classes RTTI dans la base. Renvoie le nombre de classes insérées.
///
/// # Erreurs
///
/// Retourne une erreur si le fichier est illisible ou si une insertion SQLite
/// échoue.
pub fn ingest_rtti_classes(db: &mut Db, binary_id: i64, path: &Path) -> Result<usize> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;

    let tx = db
        .conn_mut()
        .transaction()
        .context("ouverture de transaction rtti_classes")?;

    let mut count = 0usize;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (class_name, namespace) =
            parse_rtti_line(line).with_context(|| format!("format inattendu : {:?}", line))?;

        // Namespace complet : "game::<ClassName>" ou "lives::<ClassName>"
        let full_ns = format!("{namespace}::{class_name}");

        let _id = ingest::rtti_class(&tx, binary_id, class_name, Some(namespace), None)
            .with_context(|| format!("rtti_class({class_name})"))?;

        // Ancre de vérité : vaddr=0 car l'adresse réelle n'est pas connue ici.
        // La boucle RE la mettra à jour ultérieurement.
        ingest::anchor(
            &tx,
            binary_id,
            0,
            &full_ns,
            "rtti",
            "nie-rtti-classes.txt",
            1.0,
        )
        .with_context(|| format!("anchor({full_ns})"))?;

        count += 1;
        debug!("rtti: {full_ns}");
    }

    tx.commit().context("commit rtti_classes")?;
    Ok(count)
}

/// Analyse une ligne `ClassName [namespace]`.
/// Renvoie `(&str classe, &str namespace)` ou une erreur si le format est invalide.
fn parse_rtti_line(line: &str) -> Result<(&str, &str)> {
    let bracket_start = line
        .rfind('[')
        .with_context(|| "crochet ouvrant '[' absent")?;
    let bracket_end = line
        .rfind(']')
        .with_context(|| "crochet fermant ']' absent")?;

    anyhow::ensure!(
        bracket_end > bracket_start,
        "ordre des crochets invalide dans {:?}",
        line
    );

    let namespace = &line[bracket_start + 1..bracket_end];
    let class_name = line[..bracket_start].trim();

    anyhow::ensure!(!class_name.is_empty(), "nom de classe vide dans {:?}", line);
    anyhow::ensure!(
        namespace == "game" || namespace == "lives",
        "namespace inconnu {:?} dans {:?}",
        namespace,
        line
    );

    Ok((class_name, namespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_game_class() {
        let (name, ns) = parse_rtti_line("BallComponent [game]").unwrap();
        assert_eq!(name, "BallComponent");
        assert_eq!(ns, "game");
    }

    #[test]
    fn parse_lives_class() {
        let (name, ns) = parse_rtti_line("CAddContentControllerSteam [lives]").unwrap();
        assert_eq!(name, "CAddContentControllerSteam");
        assert_eq!(ns, "lives");
    }

    #[test]
    fn parse_class_with_prefix_spaces() {
        let (name, ns) = parse_rtti_line("  AbilityListMenu [game]  ").unwrap();
        assert_eq!(name, "AbilityListMenu");
        assert_eq!(ns, "game");
    }

    #[test]
    fn parse_invalid_line_no_bracket() {
        assert!(parse_rtti_line("BallComponent").is_err());
    }

    #[test]
    fn parse_invalid_namespace() {
        assert!(parse_rtti_line("Foo [unknown]").is_err());
    }

    #[test]
    fn ingest_sample_classes_in_memory() {
        // Échantillon réel de 6 lignes du fichier nie-rtti-classes.txt.
        let sample = "AbilityLearningBoardMenu [game]\n\
                      BallComponent [game]\n\
                      CAddContentControllerSteam [lives]\n\
                      CBlendShapeProp [lives]\n\
                      gmdCAnimation [lives]\n\
                      SoccerValidConditionManager [game]\n";

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), sample).unwrap();

        let mut db = nie_index::Db::open_in_memory().unwrap();
        let bin = db
            .upsert_binary("nie.exe", "abc", "x86_64", 64, 0, 0, None, None)
            .unwrap();

        let count = ingest_rtti_classes(&mut db, bin, tmp.path()).unwrap();
        assert_eq!(count, 6);

        // Vérifier que les classes sont bien dans la table rtti_class.
        let db_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM rtti_class WHERE binary_id = ?1",
                [bin],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(db_count, 6);

        // Vérifier les ancres.
        let anchor_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM anchor WHERE binary_id = ?1 AND kind = 'rtti'",
                [bin],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(anchor_count, 6);

        // Vérifier les namespaces.
        let ns_game: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM rtti_class WHERE binary_id = ?1 AND namespace = 'game'",
                [bin],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ns_game, 3); // BallComponent, AbilityLearningBoardMenu, SoccerValidConditionManager

        let ns_lives: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM rtti_class WHERE binary_id = ?1 AND namespace = 'lives'",
                [bin],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ns_lives, 3);
    }
}
