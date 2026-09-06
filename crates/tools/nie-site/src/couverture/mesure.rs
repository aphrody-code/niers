//! La **mesure** — l'énumération des capacités depuis les sources réelles du dépôt.
//!
//! Ce module ne classe rien et ne juge rien : il compte. Chaque fonction porte la commande
//! qu'elle reproduit (cf. [`Source::commande`]), de sorte qu'un compte de la matrice se rejoue
//! en une ligne de shell — la règle du dépôt : *un compte cité porte sa commande et sa date*.
//!
//! Deux partis pris, tirés de défauts déjà payés ici :
//!
//! - **on lit les sources, on ne devine pas**. Les commandes d'Inacord sont extraites du
//!   `collect_commands!` qui les enregistre — la seule liste que le compilateur tient à jour —
//!   et non d'un décompte de `#[tauri::command]` qui inclurait celles qu'on a oublié d'ajouter ;
//! - **une source absente est une erreur, pas un zéro**. Une mesure qui rend 0 parce qu'un
//!   fichier manque produit une matrice verte sur un dépôt vide.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Source;

/// Une capacité énumérée, avant tout classement.
#[derive(Debug, Clone)]
pub struct Entree {
    /// D'où elle vient.
    pub source: Source,
    /// Son nom, tel qu'il est écrit dans la source.
    pub nom: String,
    /// Son poids : 1, sauf pour le VFS où c'est un nombre de fichiers.
    pub poids: u64,
}

/// Tout ce qui a été énuméré, toutes sources confondues.
#[derive(Debug, Clone, Default)]
pub struct Inventaire {
    /// Les entrées, dans l'ordre des sources.
    pub entrees: Vec<Entree>,
}

impl Inventaire {
    fn pousser(&mut self, source: Source, nom: impl Into<String>, poids: u64) {
        self.entrees.push(Entree {
            source,
            nom: nom.into(),
            poids,
        });
    }

    /// Combien d'entrées viennent de cette source.
    #[must_use]
    pub fn compte(&self, source: Source) -> usize {
        self.entrees.iter().filter(|e| e.source == source).count()
    }
}

/// Mesure les neuf sources depuis la racine du dépôt.
///
/// # Erreurs
///
/// Rend une erreur dès qu'une source est illisible : une matrice construite sur une source
/// muette annoncerait une couverture qu'elle n'a pas mesurée.
pub fn mesurer(racine: &Path) -> anyhow::Result<Inventaire> {
    let mut inv = Inventaire::default();
    niers(racine, &mut inv)?;
    inacord(racine, &mut inv)?;
    azalee(racine, &mut inv)?;
    modules(
        racine,
        Source::NieData,
        "crates/engine/nie-data/src/lib.rs",
        &mut inv,
    )?;
    modules(
        racine,
        Source::NieFormats,
        "crates/engine/nie-formats/src/lib.rs",
        &mut inv,
    )?;
    nie_lua(racine, &mut inv)?;
    iecode(racine, &mut inv)?;
    vfs(racine, &mut inv)?;
    Ok(inv)
}

/// `niers --help` — les sous-commandes de la CLI unique, `help` exclue (elle n'est pas une
/// capacité du dépôt, c'est clap qui la pose).
fn niers(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let binaire = {
        let local = racine.join("target/release/niers");
        if local.is_file() {
            local
        } else {
            PathBuf::from("niers")
        }
    };
    let sortie = std::process::Command::new(&binaire)
        .arg("--help")
        .output()
        .map_err(|e| {
            anyhow::anyhow!(
                "`{} --help` injouable ({e}) — construire `cargo build --release -p nie-cli`",
                binaire.display()
            )
        })?;
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let mut dans_les_commandes = false;
    let mut n = 0;
    for ligne in texte.lines() {
        if ligne.starts_with("Commands:") {
            dans_les_commandes = true;
            continue;
        }
        if dans_les_commandes {
            if ligne.starts_with("Options:") || ligne.trim().is_empty() {
                if ligne.starts_with("Options:") {
                    break;
                }
                continue;
            }
            // Une sous-commande est indentée de deux espaces et n'est pas la suite d'une
            // description (celles-ci sont indentées bien davantage).
            let Some(reste) = ligne.strip_prefix("  ") else {
                continue;
            };
            if reste.starts_with(' ') {
                continue;
            }
            let Some(nom) = reste.split_whitespace().next() else {
                continue;
            };
            if nom == "help" {
                continue;
            }
            inv.pousser(Source::Niers, nom, 1);
            n += 1;
        }
    }
    anyhow::ensure!(n > 0, "`niers --help` n'a listé aucune sous-commande");
    Ok(())
}

/// Le `collect_commands!` de `apps/inacord/src-tauri/src/lib.rs` — la seule liste que le
/// compilateur tient à jour, puisqu'elle sert **à la fois** l'`invoke_handler` et l'export des
/// bindings TypeScript.
fn inacord(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let chemin = racine.join("apps/inacord/src-tauri/src/lib.rs");
    let source = fs::read_to_string(&chemin)
        .map_err(|e| anyhow::anyhow!("{} illisible: {e}", chemin.display()))?;
    let Some(debut) = source.find("collect_commands![") else {
        anyhow::bail!("`collect_commands![` introuvable dans {}", chemin.display());
    };
    let reste = &source[debut + "collect_commands![".len()..];
    let Some(fin) = reste.find(']') else {
        anyhow::bail!("`collect_commands![` non refermé dans {}", chemin.display());
    };
    let mut n = 0;
    for brut in reste[..fin].split(',') {
        let nom = brut.trim();
        if nom.is_empty() || nom.starts_with("//") {
            continue;
        }
        inv.pousser(Source::Inacord, nom, 1);
        n += 1;
    }
    anyhow::ensure!(n > 0, "`collect_commands!` vide");
    Ok(())
}

/// Les pages et les routes d'API d'Azalée.
///
/// Le nom d'une page est sa **route**, pas son chemin de fichier : les segments de groupe
/// (`(liste)`) n'apparaissent pas dans l'URL et sont retirés, faute de quoi `/aura/(liste)` et
/// `/aura` compteraient pour deux pages là où le visiteur n'en voit qu'une.
fn azalee(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let base = racine.join("apps/azalee/app");
    anyhow::ensure!(base.is_dir(), "{} absent", base.display());
    let mut pages = Vec::new();
    let mut apis = Vec::new();
    parcourir(&base, &mut |fichier: &Path| {
        let nom_fichier = fichier.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let est_page = nom_fichier == "page.tsx";
        let est_route = nom_fichier == "route.ts";
        if !est_page && !est_route {
            return;
        }
        let Ok(relatif) = fichier.strip_prefix(&base) else {
            return;
        };
        let mut segments: Vec<&str> = relatif
            .parent()
            .map(|p| p.components().filter_map(|c| c.as_os_str().to_str()).collect())
            .unwrap_or_default();
        segments.retain(|s| !(s.starts_with('(') && s.ends_with(')')));
        let route = if segments.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", segments.join("/"))
        };
        if est_page {
            pages.push(route);
        } else {
            apis.push(route);
        }
    })?;
    pages.sort_unstable();
    pages.dedup();
    apis.sort_unstable();
    apis.dedup();
    anyhow::ensure!(!pages.is_empty(), "aucune page trouvée sous {}", base.display());
    for p in pages {
        inv.pousser(Source::Azalee, p, 1);
    }
    for a in apis {
        inv.pousser(Source::AzaleeApi, a, 1);
    }
    Ok(())
}

/// Les `pub mod` de premier niveau d'un `lib.rs` — une famille de données, un module.
fn modules(
    racine: &Path,
    source: Source,
    relatif: &str,
    inv: &mut Inventaire,
) -> anyhow::Result<()> {
    let chemin = racine.join(relatif);
    let texte = fs::read_to_string(&chemin)
        .map_err(|e| anyhow::anyhow!("{} illisible: {e}", chemin.display()))?;
    let mut n = 0;
    for ligne in texte.lines() {
        let Some(reste) = ligne.strip_prefix("pub mod ") else {
            continue;
        };
        let nom = reste.trim_end_matches(';').trim();
        if nom.is_empty() {
            continue;
        }
        inv.pousser(source, nom, 1);
        n += 1;
    }
    anyhow::ensure!(n > 0, "aucun `pub mod` dans {}", chemin.display());
    Ok(())
}

/// Les `pub fn` de premier niveau de `nie-lua` — 34, mesurées le 2026-09-06.
///
/// **Non indentées, à dessein** : les `pub fn` d'un bloc `impl` sont des méthodes, pas des
/// capacités, et les compter donnait 99 — le chiffre que ce dépôt a longtemps cité et qui ne se
/// rejouait pas.
fn nie_lua(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let base = racine.join("crates/engine/nie-lua/src");
    anyhow::ensure!(base.is_dir(), "{} absent", base.display());
    let mut noms = Vec::new();
    parcourir(&base, &mut |fichier: &Path| {
        if fichier.extension().and_then(|e| e.to_str()) != Some("rs") {
            return;
        }
        let Ok(texte) = fs::read_to_string(fichier) else {
            return;
        };
        for ligne in texte.lines() {
            let Some(reste) = ligne.strip_prefix("pub fn ") else {
                continue;
            };
            let nom: String = reste
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !nom.is_empty() {
                noms.push(nom);
            }
        }
    })?;
    noms.sort_unstable();
    noms.dedup();
    anyhow::ensure!(!noms.is_empty(), "aucune `pub fn` sous {}", base.display());
    for n in noms {
        inv.pousser(Source::NieLua, n, 1);
    }
    Ok(())
}

/// Les sous-commandes du toolkit C++ — un fichier par commande sous `src/cli/commands/`.
fn iecode(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let base = racine.join("src/cli/commands");
    anyhow::ensure!(base.is_dir(), "{} absent", base.display());
    let mut noms = Vec::new();
    parcourir(&base, &mut |fichier: &Path| {
        if fichier.extension().and_then(|e| e.to_str()) != Some("cpp") {
            return;
        }
        if let Some(stem) = fichier.file_stem().and_then(|s| s.to_str()) {
            noms.push(stem.to_string());
        }
    })?;
    noms.sort_unstable();
    noms.dedup();
    anyhow::ensure!(!noms.is_empty(), "aucune commande sous {}", base.display());
    for n in noms {
        inv.pousser(Source::Iecode, n, 1);
    }
    Ok(())
}

/// L'inventaire figé du VFS, agrégé **par extension**.
///
/// Deux pièges payés le 2026-09-06 et corrigés ici :
///
/// - **des chemins du VFS contiennent un espace** (`…/u021801/u021802 .g4md`). Découper la
///   ligne par espaces croissants en fait de faux « fichiers sans extension » : le chemin se
///   lit en retirant les DEUX derniers champs (taille, cpk), jamais en prenant le premier ;
/// - `.bin` seul confond trois corpus. `.cfg.bin` (71 101) et `.lua.bin` (1 197) sont servis
///   par deux routes différentes ; les compter ensemble en cacherait une.
fn vfs(racine: &Path, inv: &mut Inventaire) -> anyhow::Result<()> {
    let chemin = racine.join("var/vfs/inventaire.txt");
    let texte = fs::read_to_string(&chemin).map_err(|e| {
        anyhow::anyhow!(
            "{} illisible ({e}) — le regénérer par `niers vfs find 'data/' -n 300000`",
            chemin.display()
        )
    })?;
    let mut comptes: BTreeMap<String, u64> = BTreeMap::new();
    let mut total = 0u64;
    for ligne in texte.lines() {
        if ligne.trim().is_empty() {
            continue;
        }
        let chemin_fichier = chemin_du_vfs(ligne);
        *comptes.entry(extension(chemin_fichier)).or_default() += 1;
        total += 1;
    }
    anyhow::ensure!(total > 0, "inventaire du VFS vide");
    for (ext, n) in comptes {
        inv.pousser(Source::Vfs, ext, n);
    }
    Ok(())
}

/// Retire de la ligne d'inventaire les deux derniers champs (`taille` et `[cpk]`).
fn chemin_du_vfs(ligne: &str) -> &str {
    let mut fin = ligne.trim_end();
    for _ in 0..2 {
        match fin.rsplit_once(' ') {
            Some((gauche, _)) => fin = gauche.trim_end(),
            None => return fin,
        }
    }
    fin
}

/// L'extension d'un chemin, avec les doubles suffixes que le jeu utilise réellement.
fn extension(chemin: &str) -> String {
    let nom = chemin.rsplit('/').next().unwrap_or(chemin);
    for double in [".cfg.bin", ".lua.bin"] {
        if nom.ends_with(double) {
            return double.to_string();
        }
    }
    match nom.rfind('.') {
        Some(i) if i + 1 < nom.len() => nom[i..].to_string(),
        _ => "(sans extension)".to_string(),
    }
}

/// Parcourt récursivement un répertoire, en appelant `visite` sur chaque fichier.
///
/// `node_modules` et `.next` sont sautés : ils portent des dizaines de milliers de fichiers qui
/// n'appartiennent pas au dépôt, et c'est ce qui fait qu'un `grep` à la racine met 60 s.
fn parcourir(base: &Path, visite: &mut dyn FnMut(&Path)) -> anyhow::Result<()> {
    let mut piles = vec![base.to_path_buf()];
    while let Some(dossier) = piles.pop() {
        let entrees = fs::read_dir(&dossier)
            .map_err(|e| anyhow::anyhow!("{} illisible: {e}", dossier.display()))?;
        for entree in entrees {
            let entree = entree?;
            let chemin = entree.path();
            let nom = entree.file_name();
            let nom = nom.to_string_lossy();
            if chemin.is_dir() {
                if nom == "node_modules" || nom == ".next" || nom.starts_with('.') {
                    continue;
                }
                piles.push(chemin);
            } else {
                visite(&chemin);
            }
        }
    }
    Ok(())
}

/// L'horodatage de génération, en ISO 8601 UTC, sans dépendance de date.
///
/// L'algorithme est celui de Howard Hinnant (`civil_from_days`) : il n'a pas de table, pas de
/// cas particulier d'année bissextile, et il est exact sur toute la plage utile.
#[must_use]
pub fn horodatage() -> String {
    let secondes = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let jours = i64::try_from(secondes / 86_400).unwrap_or(0);
    let reste = secondes % 86_400;
    let (a, m, j) = civil_depuis_jours(jours);
    format!(
        "{a:04}-{m:02}-{j:02}T{:02}:{:02}:{:02}Z",
        reste / 3600,
        (reste % 3600) / 60,
        reste % 60
    )
}

/// Convertit un nombre de jours depuis l'époque Unix en date civile.
fn civil_depuis_jours(jours: i64) -> (i64, u32, u32) {
    let z = jours + 719_468;
    let ere = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + ere * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 { y + 1 } else { y },
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_chemin_a_espace_garde_son_extension() {
        // Le VFS porte de vrais noms de fichier avec un espace : deux des « 48 fichiers sans
        // extension » du 2026-09-06 n'étaient qu'un artefact de découpage.
        let ligne = "data/common/chara/u021801/u021802 .g4md 4096 [chara.cpk]";
        assert_eq!(
            chemin_du_vfs(ligne),
            "data/common/chara/u021801/u021802 .g4md"
        );
        assert_eq!(extension(chemin_du_vfs(ligne)), ".g4md");
    }

    #[test]
    fn les_doubles_suffixes_ne_se_confondent_pas() {
        assert_eq!(extension("data/x/chara_base.cfg.bin"), ".cfg.bin");
        assert_eq!(extension("data/x/menu.lua.bin"), ".lua.bin");
        assert_eq!(extension("data/x/ev99.cfg_test.bin"), ".bin");
        assert_eq!(extension("data/x/sans_point"), "(sans extension)");
    }

    #[test]
    fn horodatage_est_une_date_iso() {
        let h = horodatage();
        assert_eq!(h.len(), 20, "{h}");
        assert!(h.ends_with('Z'), "{h}");
        assert_eq!(&h[4..5], "-");
        assert_eq!(&h[10..11], "T");
        // 2026-01-01 = 20 454 jours après l'époque : la conversion est vérifiée sur une date
        // connue, pas seulement sur sa forme.
        assert_eq!(civil_depuis_jours(20_454), (2026, 1, 1));
        assert_eq!(civil_depuis_jours(0), (1970, 1, 1));
        assert_eq!(civil_depuis_jours(19_723), (2024, 1, 1));
    }
}
