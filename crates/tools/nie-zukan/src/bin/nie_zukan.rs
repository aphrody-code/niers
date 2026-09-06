//! Binaire CLI nie-zukan — ingesteur Zukan Inagle.
//!
//! # Usage
//!
//! ```text
//! # Pull échantillon validé (200 perso × 3 langues + skills + items)
//! nie-zukan pull --sample 200
//!
//! # Pull complet des 5454 perso (reprend depuis le cache)
//! nie-zukan pull --all
//!
//! # Pull une seule langue
//! nie-zukan pull --lang ja --all
//!
//! # Croisement avec inagle (après pull)
//! nie-zukan cross --mirror <azalee>/data/backups/mirror.sqlite
//!
//! # Tests de forge (round-trip + ancre Endou)
//! nie-zukan forge-test
//! ```

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use nie_zukan::{
    cross::{cross_with_inagle, load_zukan_charas_from_ndjson},
    forge,
    models::Lang,
    pull::{PullConfig, run_pull},
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nie-zukan",
    about = "Ingesteur Zukan Inagle (zukan.inazuma.jp)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Répertoire racine du cache et de la sortie. Défaut : `<racine du jeu>/var/zukan`.
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Pull les données du Zukan (chara_list, chara_param, skills, items)
    Pull {
        /// Langues à puller (ja, fr, en). Par défaut : toutes.
        #[arg(long, value_delimiter = ',')]
        lang: Option<Vec<String>>,
        /// Pull un échantillon de N perso (0 = tous)
        #[arg(long, default_value = "0")]
        sample: usize,
        /// Pull la totalité des perso (équivalent --sample 0)
        #[arg(long)]
        all: bool,
    },
    /// Croise les données zukan avec le miroir inagle
    Cross {
        /// Chemin vers le miroir SQLite inagle — vit dans le dépôt azalee, donc requis.
        #[arg(long)]
        mirror: PathBuf,
        /// Langue source des données zukan (défaut: ja)
        #[arg(long, default_value = "ja")]
        lang: String,
    },
    /// Teste le forge/décodage q (round-trip + ancre Endou)
    ForgeTest,
    /// Affiche des infos sur le cache existant
    Status,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nie_zukan=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    // Défaut relatif au répertoire courant : `var/` est le magasin d'artefacts
    // régénérables du dépôt, et il vit à côté du jeu sur toutes les plateformes.
    let root = cli
        .root
        .clone()
        .unwrap_or_else(|| PathBuf::from("var/zukan"));

    match cli.cmd {
        Cmd::ForgeTest => {
            run_forge_test()?;
        }
        Cmd::Pull { lang, sample, all } => {
            let langs = parse_langs(lang);
            let limit = if all { 0 } else { sample };
            let config = PullConfig {
                cache_root: root.clone(),
                output_root: root.clone(),
                langs,
                chara_param_limit: limit,
            };
            let stats = run_pull(&config)?;
            println!(
                "pull terminé: chara_ids={} chara_params={} skills={} items={} errors={}",
                stats.chara_ids_discovered,
                stats.chara_params_fetched,
                stats.skills_fetched,
                stats.items_fetched,
                stats.errors,
            );
        }
        Cmd::Cross { mirror, lang } => {
            let lang_enum = parse_lang(&lang);
            let ndjson_path = root.join(lang_enum.code()).join("chara_param.ndjson");
            let charas = load_zukan_charas_from_ndjson(&ndjson_path)?;
            if charas.is_empty() {
                eprintln!(
                    "WARN: aucun chara dans {} — lancer pull d'abord",
                    ndjson_path.display()
                );
                return Ok(());
            }
            let result = cross_with_inagle(&charas, &mirror)?;
            println!(
                "cross: zukan_total={} matched={} absent_from_inagle={} exemples_enrichissement={}",
                result.zukan_total,
                result.matched,
                result.absent_from_inagle.len(),
                result.enrichment_examples.len(),
            );
            if !result.absent_from_inagle.is_empty() {
                println!(
                    "absent_examples={}",
                    result.absent_from_inagle[..result.absent_from_inagle.len().min(5)].join(",")
                );
            }
            for ex in &result.enrichment_examples[..result.enrichment_examples.len().min(5)] {
                println!(
                    "  enrichissement id={} name={} field={} zukan={} inagle={:?}",
                    ex.game_id, ex.name_ja, ex.field, ex.zukan_value, ex.inagle_value,
                );
            }
            // Sauvegarder le résultat en JSON
            let out = root.join("cross_result.json");
            std::fs::write(&out, serde_json::to_string_pretty(&result)?)?;
            println!("cross_result={}", out.display());
        }
        Cmd::Status => {
            for lang in Lang::all() {
                let param_path = root.join(lang.code()).join("chara_param.ndjson");
                let skills_path = root.join(lang.code()).join("skills.ndjson");
                let items_path = root.join(lang.code()).join("items.ndjson");
                let param_lines = count_lines(&param_path);
                let skill_lines = count_lines(&skills_path);
                let item_lines = count_lines(&items_path);
                println!(
                    "lang={} chara_param={} skills={} items={}",
                    lang.code(),
                    param_lines,
                    skill_lines,
                    item_lines,
                );
            }
        }
    }
    Ok(())
}

fn run_forge_test() -> Result<()> {
    // Ancre Endou
    let json = r#"{"character_id":["c01000010"]}"#;
    let q = forge::encode_q(json)?;
    let expected = "hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC";
    assert_eq!(
        q, expected,
        "ancre Endou échouée: got {q} expected {expected}"
    );
    let decoded = forge::decode_q(&q)?;
    assert_eq!(decoded, json, "round-trip échoué");

    println!("forge_test=ok");
    println!("ancre_endou=ok q={q}");

    // Tests supplémentaires
    let test_cases = [
        (
            r#"{"filter_chara_id_str":["c01000010"]}"#,
            "filter_chara_id_str",
        ),
        (r#"{"category_filter":[30]}"#, "category_filter_30"),
        (r#"{"category_filter":[1]}"#, "category_filter_1"),
    ];
    for (json, label) in &test_cases {
        let q = forge::encode_q(json)?;
        let decoded = forge::decode_q(&q)?;
        assert_eq!(&decoded, json, "round-trip échoué pour {label}");
        println!("roundtrip_{label}=ok");
    }

    // Vérifier que le q URL-encodé live fonctionne
    let q_live = "hN2ZlpOLmo2gnJeejZ6glpugjIuN3cWk3ZzPzs_Pz8_Oz92igg%3D%3D";
    let decoded = forge::decode_q(q_live)?;
    assert_eq!(
        decoded, r#"{"filter_chara_id_str":["c01000010"]}"#,
        "décodage q live échoué"
    );
    println!("decode_live_q=ok");

    Ok(())
}

fn parse_langs(lang: Option<Vec<String>>) -> Vec<Lang> {
    match lang {
        None => Lang::all().to_vec(),
        Some(codes) => codes.iter().map(|s| parse_lang(s)).collect(),
    }
}

fn parse_lang(s: &str) -> Lang {
    match s.to_lowercase().as_str() {
        "ja" | "jp" => Lang::Ja,
        "fr" => Lang::Fr,
        "en" => Lang::En,
        other => {
            eprintln!("WARN: langue inconnue '{other}', utilisation de 'ja'");
            Lang::Ja
        }
    }
}

fn count_lines(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    std::fs::read_to_string(path)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}
