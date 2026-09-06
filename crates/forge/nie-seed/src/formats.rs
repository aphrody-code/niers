//! Catalogue des formats Level-5 / Criware documentés par iecode.
//!
//! Chaque format est inséré via [`nie_index::ingest::format`]. Les magic bytes
//! sont extraits depuis `IECODE.Core/Formats/FormatDetector.cs` et les parsers
//! individuels (G4pkParser, NavmParser, PxclParser, HcaDecoder, etc.).
//!
//! Sources consultées :
//! - `IECODE.Core/Formats/FormatDetector.cs` — magic bytes unifiés ;
//! - `IECODE.Core/Formats/Level5/*.cs` — parsers Level-5 ;
//! - `IECODE.Core/Formats/Criware/*.cs` — parsers Criware ;
//! - `IECODE.Core/Formats/Shader/DxbcReader.cs` — DXBC ;
//! - `IECODE.Core/Audio/HcaDecoder.cs` — HCA.

use anyhow::{Context, Result};
use nie_index::{Db, ingest};
use tracing::debug;

/// Un format documenté par iecode.
struct FormatDef {
    /// Nom court du format (clé primaire de la table `format`).
    name: &'static str,
    /// Magic bytes en représentation ASCII ou hexadécimale lisible.
    magic: Option<&'static str>,
    /// Chemin du parser C# (relatif à `iecode/src/IECODE.Core/`).
    doc: &'static str,
}

/// Catalogue complet des formats documentés par iecode.
///
/// Conventiont magic :
/// - Magic ASCII entre guillemets si le magic est lisible (ex. `"G4TX"`) ;
/// - Représentation hex LE `0xXXXXXXXX` sinon ;
/// - `None` pour les formats sans magic fixe (cfg.bin, G4MG, FXBIN/MEVBIN).
static FORMATS: &[FormatDef] = &[
    // ── Level-5 G4* ───────────────────────────────────────────────────────────
    FormatDef {
        name: "G4TX",
        magic: Some("\"G4TX\" (0x58543447 LE)"),
        doc: "Formats/Level5/G4txParser.cs",
    },
    FormatDef {
        name: "G4MD",
        magic: Some("\"G4MD\" (0x444D3447 LE)"),
        doc: "Formats/Level5/G4mdParser.cs",
    },
    FormatDef {
        // G4MG : headerless, pas de magic propre au format
        name: "G4MG",
        magic: None,
        doc: "Formats/Level5/G4mgParser.cs",
    },
    FormatDef {
        name: "G4MT",
        magic: Some("\"G4MT\" (0x544D3447 LE)"),
        doc: "Formats/Level5/G4mtParser.cs",
    },
    FormatDef {
        name: "G4SK",
        magic: Some("\"G4SK\" (0x4B533447 LE)"),
        doc: "Formats/Level5/G4skParser.cs",
    },
    FormatDef {
        name: "G4PK",
        magic: Some("\"G4PK\" (0x4B503447 LE)"),
        doc: "Formats/Level5/G4pkParser.cs",
    },
    FormatDef {
        // G4PKM : variante multi-platform du G4PK
        name: "G4PKM",
        magic: Some("\"G4PK\" (0x4B503447 LE)"),
        doc: "Formats/Level5/G4pkParser.cs",
    },
    FormatDef {
        name: "G4RA",
        magic: Some("\"G4RA\" (0x41523447 LE)"),
        doc: "Formats/Level5/G4raParser.cs",
    },
    FormatDef {
        // G4NV : extension .g4nv mais magic interne "NAVM"
        name: "G4NV",
        magic: Some("\"NAVM\" (0x4D56414E LE)"),
        doc: "Formats/Level5/NavmParser.cs",
    },
    // ── Level-5 divers ────────────────────────────────────────────────────────
    FormatDef {
        // cfg.bin : sans magic fixe (détection heuristique par entryCount+stringTableOffset)
        name: "cfg.bin",
        magic: Some("RDBN (0x4E424452 LE) ou heuristique"),
        doc: "Formats/Level5/CfgBin/CfgBin.cs",
    },
    FormatDef {
        // RDBN : magic explicite "RDBN" pour le nouveau format cfg.bin
        name: "RDBN",
        magic: Some("\"RDBN\" (0x4E424452 LE)"),
        doc: "Formats/Level5/CfgBin/Rdbn/RdbnReader.cs",
    },
    FormatDef {
        // PXCL : extension .col mais magic interne "PXCL"
        name: "PXCL",
        magic: Some("\"PXCL\" (0x5058434C LE)"),
        doc: "Formats/Level5/PxclParser.cs",
    },
    FormatDef {
        // P3LIP : lip sync, magic "lip\0"
        name: "P3LIP",
        magic: Some("\"lip\\0\" (0x00 0x70 0x69 0x6C)"),
        doc: "Formats/Level5/P3lipParser.cs",
    },
    FormatDef {
        // MEVBIN : conteneur cfg.bin "t2b" pour les motion events
        name: "MEVBIN",
        magic: Some("footer cfg.bin 't2b' (0x01 0x74 0x32 0x62 0xFE)"),
        doc: "Formats/Level5/MevbinDocument.cs",
    },
    FormatDef {
        // FXBIN : ShaderFX — aussi conteneur cfg.bin "t2b"
        name: "FXBIN",
        magic: Some("footer cfg.bin 't2b' (0x01 0x74 0x32 0x62 0xFE)"),
        doc: "Formats/Level5/FxbinParser.cs",
    },
    FormatDef {
        // NAVM : navigation mesh (aussi accédé via G4NV ci-dessus)
        name: "NAVM",
        magic: Some("\"NAVM\" (0x4D56414E LE)"),
        doc: "Formats/Level5/NavmParser.cs",
    },
    FormatDef {
        name: "AGI",
        magic: Some("\".IGA\" / \"AGI.\" (0x4147492E LE)"),
        doc: "Formats/Level5/AgiParser.cs",
    },
    FormatDef {
        name: "OBJB",
        magic: Some("\"objb\" (0x6F626A62 LE)"),
        doc: "Formats/FormatDetector.cs",
    },
    // ── Criware ───────────────────────────────────────────────────────────────
    FormatDef {
        name: "CPK",
        magic: Some("\"CPK \" (0x204B5043 LE)"),
        doc: "Formats/Criware/CriFs/CpkReader.cs",
    },
    FormatDef {
        // @UTF : Universal Table Format, utilisé dans CPK/ACB/ACF
        name: "@UTF",
        magic: Some("\"@UTF\" (0x40555446 LE)"),
        doc: "Formats/Criware/UtfParser.cs",
    },
    FormatDef {
        // ACB : Audio Container Bundle — embed une table @UTF
        name: "ACB",
        magic: Some("\"@UTF\" (0x40555446 LE) / magic ACB 0x46544140"),
        doc: "Formats/Criware/AcbReader.cs",
    },
    FormatDef {
        // AWB : Audio Wave Bank — magic "AFS2"
        name: "AWB",
        magic: Some("\"AFS2\" (0x32534641 LE)"),
        doc: "Formats/Criware/AwbReader.cs",
    },
    FormatDef {
        // HCA : High Compression Audio, big-endian, tags masqués 0x7F
        name: "HCA",
        magic: Some("\"HCA\\0\" (0x48434100 BE, bits 0x7F masqués)"),
        doc: "Audio/HcaDecoder.cs",
    },
    FormatDef {
        // ADX : format audio Criware simple
        name: "ADX",
        magic: Some("0x0000 (big-endian, signature propriétaire)"),
        doc: "Formats/FormatDetector.cs",
    },
    FormatDef {
        // USM : vidéo Criware (conteneur UTF + streams)
        name: "USM",
        magic: Some("\"CRID\" (Criware Resource ID, détection par UtfParser)"),
        doc: "Formats/Criware/UsmDemuxer.cs",
    },
    // ── Shader / GPU ──────────────────────────────────────────────────────────
    FormatDef {
        // DXBC : DirectX Byte Code (shaders compilés HLSL)
        name: "DXBC",
        magic: Some("\"DXBC\" (0x43425844 LE)"),
        doc: "Formats/Shader/DxbcReader.cs",
    },
];

/// Insère le catalogue complet des formats iecode dans la base.
/// Renvoie le nombre de formats insérés.
///
/// # Erreurs
///
/// Retourne une erreur si une insertion SQLite échoue.
pub fn ingest_formats(db: &mut Db) -> Result<usize> {
    let tx = db
        .conn_mut()
        .transaction()
        .context("ouverture de transaction formats")?;

    let mut count = 0usize;

    for fmt in FORMATS {
        let source = "iecode";
        let doc = Some(fmt.doc);
        ingest::format(&tx, fmt.name, fmt.magic, source, doc)
            .with_context(|| format!("format({})", fmt.name))?;
        debug!("format: {} magic={:?}", fmt.name, fmt.magic);
        count += 1;
    }

    tx.commit().context("commit formats")?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_catalogue_non_vide() {
        assert!(
            FORMATS.len() >= 24,
            "le catalogue doit contenir au moins 24 formats, trouvé {}",
            FORMATS.len()
        );
    }

    #[test]
    fn formats_noms_uniques() {
        let mut noms: Vec<&str> = FORMATS.iter().map(|f| f.name).collect();
        noms.sort_unstable();
        let avant = noms.len();
        noms.dedup();
        assert_eq!(avant, noms.len(), "des noms de formats sont dupliqués");
    }

    #[test]
    fn ingest_formats_in_memory() {
        let mut db = nie_index::Db::open_in_memory().unwrap();
        let count = ingest_formats(&mut db).unwrap();
        assert_eq!(count, FORMATS.len());

        // Vérifier quelques formats emblématiques.
        for name in &[
            "G4TX", "G4MD", "G4PK", "CPK", "@UTF", "ACB", "AWB", "HCA", "DXBC", "RDBN",
        ] {
            let found: i64 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM format WHERE name = ?1", [name], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(found, 1, "format {name} introuvable");
        }

        // Vérifier le total.
        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM format", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, FORMATS.len() as i64);
    }

    #[test]
    fn ingest_formats_idempotent() {
        let mut db = nie_index::Db::open_in_memory().unwrap();
        let c1 = ingest_formats(&mut db).unwrap();
        let c2 = ingest_formats(&mut db).unwrap();
        // La seconde passe ne doit pas créer de doublons.
        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM format", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, c1 as i64);
        assert_eq!(c1, c2);
    }
}
