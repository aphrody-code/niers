//! `nie-headless` — runner CLI headless pour les formats IEVR et la simulation de match.
//!
//! ## Sous-commandes
//!
//! ```text
//! nie-headless detect <FICHIER> [--indent N]
//! nie-headless match [--home NOM] [--away NOM] [--seed N] [--home-stats Kc:Cr:Tc:Pr:Ps:Ag:It] [--away-stats ...]
//! ```
//!
//! ### `detect`
//!
//! Prend un fichier extrait du jeu, détecte son format et affiche un résumé JSON
//! sur stdout.
//!
//! ### `match`
//!
//! Simule un match de football complet (kickoff → 90 minutes → résultat) de façon
//! déterministe et affiche le score en 1 ligne terse (`clé=val`).
//!
//! Les stats par défaut sont les stats FW UR lv99 confirmées depuis les tables de
//! croissance embarquées de nie-core (golden test `growth::golden_fw_ur`) :
//! `Kc=207 Cr=216 Tc=218 Pr=235 Ps=242 Ag=210 It=261`.

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

use nie_core::{
    match_sim::{TeamSetup, simulate_match},
    stats::StatBlock,
};
use nie_formats::{FileFormat, cfgbin, cpk, crilayla, detect};

// ---------------------------------------------------------------------------
// Détection étendue
// ---------------------------------------------------------------------------

/// Détecte le format d'un tampon, avec prise en charge du magic RDBN (cfg.bin)
/// en complément de [`nie_formats::detect`] (qui ne couvre pas encore RDBN).
fn detect_etendu(donnees: &[u8]) -> FileFormat {
    let format = detect(donnees);
    if format == FileFormat::Unknown && cfgbin::is_rdbn(donnees) {
        FileFormat::CfgBin
    } else {
        format
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Runner headless IEVR — détecte un format ou simule un match de football.
#[derive(Debug, Parser)]
#[command(name = "nie-headless", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    commande: Commande,
}

/// Sous-commandes disponibles.
#[derive(Debug, Subcommand)]
enum Commande {
    /// Détecte le format d'un fichier et affiche un résumé JSON.
    ///
    /// Formats reconnus : CRILAYLA, @UTF, CPK, cfg.bin (RDBN).
    Detect {
        /// Chemin vers le fichier à analyser.
        fichier: PathBuf,
        /// Indentation JSON (0 = compact, N = lisible avec N espaces de base).
        #[arg(long, default_value = "2")]
        indent: usize,
    },
    /// Simule un match de football complet (headless, déterministe).
    ///
    /// Affiche sur stdout : `home=<NOM> away=<NOM> résultat=<H>-<A> horloge=<CLOCK>`
    /// (1 ligne terse). Les détails sont accessibles via `RUST_LOG=debug`.
    ///
    /// Les stats par défaut sont FW UR lv99 — confirmées sur les vraies tables
    /// de croissance IEVR embarquées dans nie-core (voir `growth::golden_fw_ur`).
    #[command(name = "match")]
    SimulerMatch {
        /// Nom de l'équipe domicile.
        #[arg(long, default_value = "Equipe-A")]
        home: String,
        /// Nom de l'équipe visiteuse.
        #[arg(long, default_value = "Equipe-B")]
        away: String,
        /// Graine PRNG pour le déterminisme (même graine → même résultat).
        #[arg(long, default_value = "42")]
        seed: u64,
        /// Stats domicile au format `Kc:Cr:Tc:Pr:Ps:Ag:It` (7 entiers séparés par `:`).
        ///
        /// Si absent, utilise les stats FW UR lv99 des tables réelles IEVR.
        #[arg(long)]
        home_stats: Option<String>,
        /// Stats visiteur au format `Kc:Cr:Tc:Pr:Ps:Ag:It`.
        ///
        /// Si absent, utilise les stats FW UR lv99 des tables réelles IEVR.
        #[arg(long)]
        away_stats: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.commande {
        Commande::Detect { fichier, indent } => cmd_detect(fichier, indent),
        Commande::SimulerMatch {
            home,
            away,
            seed,
            home_stats,
            away_stats,
        } => cmd_match(home, away, seed, home_stats, away_stats),
    }
}

// ---------------------------------------------------------------------------
// Sous-commande `detect`
// ---------------------------------------------------------------------------

/// Résumé d'analyse d'un fichier.
#[derive(Debug, Serialize)]
struct Resume {
    /// Chemin du fichier tel que passé en argument.
    chemin: String,
    /// Taille en octets.
    taille_octets: u64,
    /// Nom court du format détecté (ex. `"CPK"`, `"CRILAYLA"`, `"?"`).
    format: &'static str,
    /// Détails supplémentaires, spécifiques au format.
    detail: Value,
}

fn cmd_detect(fichier: PathBuf, indent: usize) -> Result<()> {
    let chemin_str = fichier.display().to_string();
    let donnees =
        std::fs::read(&fichier).with_context(|| format!("impossible de lire '{chemin_str}'"))?;

    let taille_octets = donnees.len() as u64;
    let format = detect_etendu(&donnees);
    let detail = construire_detail(format, &donnees)?;

    let resume = Resume {
        chemin: chemin_str,
        taille_octets,
        format: format.name(),
        detail,
    };

    let json = if indent == 0 {
        serde_json::to_string(&resume).context("sérialisation JSON")?
    } else {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        resume.serialize(&mut ser).context("sérialisation JSON")?;
        String::from_utf8(buf).context("UTF-8 JSON")?
    };

    println!("{json}");
    Ok(())
}

/// Construit le champ `detail` JSON en fonction du format détecté.
fn construire_detail(format: FileFormat, donnees: &[u8]) -> Result<Value> {
    match format {
        FileFormat::CriLayla => detail_crilayla(donnees),
        FileFormat::Utf => detail_utf(donnees),
        FileFormat::Cpk => detail_cpk(donnees),
        FileFormat::CfgBin => detail_cfgbin(donnees),
        FileFormat::Hca
        | FileFormat::Acb
        | FileFormat::Awb
        | FileFormat::Usm
        | FileFormat::G4mg
        | FileFormat::G4md
        | FileFormat::G4tx
        | FileFormat::G4sk
        | FileFormat::G4pk
        | FileFormat::G4nv
        | FileFormat::Unknown => Ok(serde_json::json!({})),
    }
}

fn detail_crilayla(donnees: &[u8]) -> Result<Value> {
    let decompresse =
        crilayla::decompress(donnees).map_err(|e| anyhow::anyhow!("CRILAYLA : {e}"))?;
    Ok(serde_json::json!({ "taille_decompresse": decompresse.len() }))
}

fn detail_utf(donnees: &[u8]) -> Result<Value> {
    let table = cpk::parse_utf(donnees).map_err(|e| anyhow::anyhow!("@UTF : {e}"))?;
    Ok(serde_json::json!({
        "nom_table": table.name,
        "colonnes":  table.column_count(),
        "lignes":    table.row_count(),
    }))
}

fn detail_cpk(donnees: &[u8]) -> Result<Value> {
    let header = cpk::parse_cpk(donnees).map_err(|e| anyhow::anyhow!("CPK : {e}"))?;
    Ok(serde_json::json!({
        "nom_table": header.utf.name,
        "colonnes":  header.utf.column_count(),
        "lignes":    header.utf.row_count(),
    }))
}

fn detail_cfgbin(donnees: &[u8]) -> Result<Value> {
    let rdbn = cfgbin::parse(donnees).map_err(|e| anyhow::anyhow!("RDBN/cfg.bin : {e}"))?;
    Ok(serde_json::json!({
        "version": rdbn.header.version,
        "types":   rdbn.types.len(),
        "champs":  rdbn.fields.len(),
        "racines": rdbn.roots.len(),
    }))
}

// ---------------------------------------------------------------------------
// Sous-commande `match`
// ---------------------------------------------------------------------------

/// Stats FW UR lv99 par défaut — confirmées par `growth::tests::golden_fw_ur`
/// sur les tables de croissance réelles IEVR embarquées dans nie-core.
///
/// `[Kc=207, Cr=216, Tc=218, Pr=235, Ps=242, Ag=210, It=261]`
fn stats_par_defaut() -> StatBlock {
    StatBlock::from_array([207, 216, 218, 235, 242, 210, 261])
}

/// Parse une chaîne `Kc:Cr:Tc:Pr:Ps:Ag:It` (7 entiers séparés par `:`) en `StatBlock`.
fn parse_stats(s: &str) -> Result<StatBlock> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 7 {
        anyhow::bail!(
            "format stats invalide : '{s}' — attendu 7 valeurs séparées par ':' (ex. 207:216:218:235:242:210:261)"
        );
    }
    let mut arr = [0u16; 7];
    for (i, part) in parts.iter().enumerate() {
        arr[i] = part
            .trim()
            .parse::<u16>()
            .with_context(|| format!("stat#{i} invalide : '{part}'"))?;
    }
    Ok(StatBlock::from_array(arr))
}

fn cmd_match(
    home_name: String,
    away_name: String,
    seed: u64,
    home_stats_str: Option<String>,
    away_stats_str: Option<String>,
) -> Result<()> {
    let home_stats = match home_stats_str {
        Some(s) => parse_stats(&s)?,
        None => stats_par_defaut(),
    };
    let away_stats = match away_stats_str {
        Some(s) => parse_stats(&s)?,
        None => stats_par_defaut(),
    };

    let home = TeamSetup::new(&home_name, home_stats);
    let away = TeamSetup::new(&away_name, away_stats);

    let res = simulate_match(home, away, seed);

    // Sortie terse (convention CLAUDE.md : 1 ligne clé=val)
    println!(
        "home={home_name} away={away_name} résultat={}-{} horloge={}",
        res.home_score, res.away_score, res.final_clock
    );
    Ok(())
}
