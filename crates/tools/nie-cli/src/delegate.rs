//! Délégation aux CLI des autres arbres du dépôt.
//!
//! Doctrine (`docs/ARCHITECTURE.md`) : **`niers` est la seule CLI utilisateur**.
//! Le toolkit C++ (`iecode`, ~40 commandes) et l'outillage .NET (`IECODE.CLI`, ~37 commandes)
//! gardent des fonctions que le Rust n'a pas encore ; les supprimer d'un trait perdrait des
//! dizaines de features. `niers` les expose donc **sous son propre nom** en attendant que
//! chaque fonction soit portée — chaque portage retire une délégation, l'écart se mesure.
//!
//! Aucune magie : les arguments sont passés tels quels, le code de sortie est propagé, et
//! l'absence de binaire donne un message qui dit quoi construire.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, bail};

/// Emplacements possibles du binaire C++ `iecode`, dans l'ordre d'essai.
///
/// CMake dépose l'exécutable dans `build/<preset>/src/cli/` ; le générateur Visual Studio
/// ajoute un niveau de configuration (`Debug/`, `Release/`).
fn iecode_candidates() -> Vec<PathBuf> {
    let exe = if cfg!(windows) {
        "iecode.exe"
    } else {
        "iecode"
    };
    let mut out = Vec::new();
    for preset in ["msvc", "debug", "release", "relwithdebinfo"] {
        for conf in ["", "Debug", "Release"] {
            let mut p = PathBuf::from("build");
            p.push(preset);
            p.push("src");
            p.push("cli");
            if !conf.is_empty() {
                p.push(conf);
            }
            p.push(exe);
            out.push(p);
        }
    }
    out
}

/// Emplacements possibles de l'assembly .NET `iecode.dll` (`IECODE.CLI`).
fn iecode_cli_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for conf in ["Release", "Debug"] {
        let mut p = PathBuf::from("csharp");
        p.push("IECODE.CLI");
        p.push("bin");
        p.push(conf);
        p.push("net10.0");
        p.push("iecode.dll");
        out.push(p);
    }
    out
}

/// Premier chemin existant parmi les candidats.
fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}

/// Chemin du binaire C++ (`$NIE_IECODE_EXE` sinon découverte).
pub fn iecode_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NIE_IECODE_EXE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    first_existing(&iecode_candidates())
}

/// Chemin de l'assembly .NET (`$NIE_IECODE_DLL` sinon découverte).
pub fn iecode_dll() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NIE_IECODE_DLL") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    first_existing(&iecode_cli_candidates())
}

/// Exécute une commande et propage son code de sortie tel quel.
fn run(mut cmd: Command, quoi: &str) -> anyhow::Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("lancement de {quoi} impossible"))?;
    if status.success() {
        return Ok(());
    }
    // Le code de sortie du délégué EST le résultat : on ne le traduit pas en erreur Rust.
    std::process::exit(status.code().unwrap_or(1));
}

/// Délègue au toolkit C++ `iecode`.
///
/// # Erreurs
/// Si le binaire n'est pas construit (message indiquant `just cpp-build`).
pub fn cpp(args: &[String]) -> anyhow::Result<()> {
    let Some(exe) = iecode_exe() else {
        bail!(
            "binaire C++ `iecode` introuvable — lance `just cpp-build` \
             (ou pointe NIE_IECODE_EXE dessus)"
        );
    };
    let mut cmd = Command::new(&exe);
    cmd.args(args);
    run(cmd, "iecode")
}

/// Délègue à l'outillage .NET `IECODE.CLI`.
///
/// # Erreurs
/// Si l'assembly n'est pas construite (message indiquant `just cs-build`).
pub fn cs(args: &[String]) -> anyhow::Result<()> {
    let Some(dll) = iecode_dll() else {
        bail!(
            "assembly .NET `iecode.dll` introuvable — lance `just cs-build` \
             (ou pointe NIE_IECODE_DLL dessus)"
        );
    };
    let mut cmd = Command::new("dotnet");
    cmd.arg(&dll).args(args);
    run(cmd, "dotnet iecode.dll")
}

/// Une ligne `clé=valeur` par back-end : présent ou non, et où.
/// Commandes de premier niveau enregistrées par `IECODE.CLI` (le binaire .NET délégué).
///
/// Comptées à la source : `grep -c 'rootCommand.AddCommand' csharp/IECODE.CLI/Program.cs`. C'est
/// le dénominateur de l'absorption — sans méthode écrite, le chiffre dérive à la première
/// relecture.
///
/// **Pas `grep -oE 'new Command\("[a-z][a-z0-9-]*"'`** : cette ancienne méthode (jusqu'au
/// 2026-09-06) ne voit que les commandes littérales dans `Program.cs` et rate toutes celles
/// enregistrées via une factory `XxxCommand.Create()` définie dans `Commands/*.cs`
/// (`BenchmarkCommand`, `CdnCommand`, `MemCommand`, `G4txCommand`, `LuaCommand`, `ShaderCommand`,
/// etc.) — elle donnait 27, mesuré à nouveau le 2026-09-07 : 38 `rootCommand.AddCommand(...)`,
/// tous distincts, vérifiés un par un.
pub const COMMANDES_IECODE_CLI: usize = 38;

/// Affiche l'état des back-ends et l'écart d'absorption.
///
/// `niers_commandes` est le nombre de sous-commandes que `niers` expose ; l'appelant le tient de
/// clap plutôt que d'une constante, pour qu'il suive le binaire au lieu d'une note.
pub fn status(niers_commandes: usize) {
    let cpp = iecode_exe();
    let cs = iecode_dll();
    println!(
        "cpp={} path={}",
        if cpp.is_some() { "present" } else { "absent" },
        cpp.map_or_else(|| "-".to_string(), |p| p.display().to_string())
    );
    println!(
        "cs={} path={}",
        if cs.is_some() { "present" } else { "absent" },
        cs.map_or_else(|| "-".to_string(), |p| p.display().to_string())
    );
    println!("rust=present path=<ce binaire>");
    // La mesure de l'absorption : chaque portage retire une delegation, l'ecart se lit ici.
    let ecart = COMMANDES_IECODE_CLI.saturating_sub(niers_commandes);
    println!("niers={niers_commandes} commandes natives");
    println!("iecode-cli={COMMANDES_IECODE_CLI} commandes deleguees");
    println!("ecart={ecart}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fige le denominateur de l'absorption. S'il bouge, c'est que `IECODE.CLI` a change : il faut
    /// recompter a la source (cf. doc de la constante), pas ajuster le chiffre a vue.
    #[test]
    fn le_denominateur_de_l_absorption_est_ancre() {
        assert_eq!(COMMANDES_IECODE_CLI, 38);
    }

    #[test]
    fn les_candidats_couvrent_les_dispositions_cmake() {
        let c = iecode_candidates();
        // 4 presets × 3 dispositions (plat, Debug/, Release/).
        assert_eq!(c.len(), 12);
        assert!(c.iter().any(|p| p.to_string_lossy().contains("msvc")));
        assert!(c.iter().all(|p| p.to_string_lossy().contains("src")));
    }

    #[test]
    fn les_candidats_dotnet_visent_net10() {
        let c = iecode_cli_candidates();
        assert_eq!(c.len(), 2);
        assert!(c.iter().all(|p| p.to_string_lossy().contains("net10.0")));
        // Release d'abord : c'est ce que produit `scripts/sync-gamedata.ts`.
        assert!(c[0].to_string_lossy().contains("Release"));
    }

    #[test]
    fn la_decouverte_ne_fabrique_pas_de_chemin() {
        // `first_existing` ne rend un chemin que s'il existe : c'est ce qui permet à
        // `cpp()`/`cs()` de donner un message utile au lieu d'un « fichier introuvable ».
        assert!(first_existing(&[PathBuf::from("build/inexistant/iecode.exe")]).is_none());
    }
}
