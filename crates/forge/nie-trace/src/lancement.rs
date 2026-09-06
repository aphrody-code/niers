//! Lancement de la chaîne de live modding : éditeur de sauvegarde, jeu **sans EAC**, puis
//! application automatique d'une recette.
//!
//! # Pourquoi « sans EAC »
//!
//! La chaîne officielle est `EACLauncher.exe` → `GameBootstrapper.exe` → `nie.exe`, avec le
//! driver anti-triche chargé. Lancer `nie.exe` **directement** évite ce chargement : c'est ce qui
//! rend la mémoire du process lisible et modifiable depuis un outil tiers. Aucun contournement
//! d'EAC n'est fait ici — on ne le démarre simplement pas.
//!
//! # L'attente
//!
//! Le jeu met plusieurs secondes à ouvrir sa fenêtre et bien plus à charger ses données.
//! [`attendre_process`] sonde par nom jusqu'à ce que le process réponde à une lecture mémoire —
//! pas seulement qu'il existe. Un process qui vient de démarrer et dont le tas n'est pas encore
//! peuplé ferait échouer toutes les règles d'une recette.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::{find_module_base, find_pid_by_name, read_exact};

/// Ce qu'un lancement a démarré.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lancement {
    /// Exécutables réellement démarrés.
    pub demarres: Vec<PathBuf>,
    /// Exécutables demandés mais absents du disque.
    pub absents: Vec<PathBuf>,
    /// Exécutables présents dont le démarrage a échoué, y compris après passage par le shell.
    pub echecs: Vec<PathBuf>,
    /// PID du jeu, une fois qu'il répond.
    pub pid: Option<i32>,
}

/// Démarre un exécutable dans son propre répertoire, sans attendre sa fin.
///
/// Deux tentatives : `CreateProcess` direct, puis — sur Windows uniquement — un passage par le
/// shell (`cmd /c start`). Le second sert aux exécutables qui **exigent une élévation** :
/// Windows refuse de les lancer directement avec « os error 740 », alors que le shell, lui,
/// déclenche l'invite UAC. C'est le cas de l'éditeur de sauvegarde livré avec le dépôt, dont le
/// nom déclenche l'« installer detection » de Windows.
///
/// Un échec est **rapporté, pas propagé** : une chaîne partielle (le jeu sans l'éditeur) reste
/// utile, et faire échouer tout le lancement parce qu'un outil annexe refuse de démarrer serait
/// le pire des comportements.
fn demarrer(exe: &Path, l: &mut Lancement) {
    if !exe.is_file() {
        l.absents.push(exe.to_path_buf());
        return;
    }
    let dossier = exe.parent().unwrap_or(Path::new("."));
    if Command::new(exe).current_dir(dossier).spawn().is_ok() {
        l.demarres.push(exe.to_path_buf());
        return;
    }

    #[cfg(windows)]
    {
        // `start ""` : le premier argument entre guillemets est le TITRE de la fenêtre, pas le
        // programme — l'omettre ferait prendre le chemin pour un titre et ne lancerait rien.
        if Command::new("cmd")
            .args(["/c", "start", "", exe.to_string_lossy().as_ref()])
            .current_dir(dossier)
            .spawn()
            .is_ok()
        {
            l.demarres.push(exe.to_path_buf());
            return;
        }
    }
    l.echecs.push(exe.to_path_buf());
}

/// Attend qu'un process du nom donné existe **et réponde à une lecture mémoire**.
///
/// Renvoie son PID, ou `None` au bout de `delai`. Sonder la seule existence du process ne suffit
/// pas : entre `CreateProcess` et le moment où son module principal est mappé, toute lecture
/// échoue.
#[must_use]
pub fn attendre_process(nom: &str, delai: Duration) -> Option<i32> {
    let debut = Instant::now();
    while debut.elapsed() < delai {
        if let Some(pid) = find_pid_by_name(nom)
            && let Some(base) = find_module_base(pid, nom)
            && read_exact(pid, base, 2).is_ok()
        {
            return Some(pid);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    None
}

/// Lance la chaîne complète : éditeur de sauvegarde d'abord, jeu ensuite, puis attend que le jeu
/// réponde.
///
/// L'ordre compte : l'éditeur en premier permet de préparer la sauvegarde **avant** que le jeu ne
/// la charge — une fois le jeu démarré, il a la sienne en mémoire et l'écrasera en quittant.
///
/// `save_editor` et `jeu` sont facultatifs : passer `None` saute l'étape.
///
/// Aucun échec n'est fatal : `demarres`, `absents` et `echecs` disent exactement ce qui s'est
/// passé, et `pid` reste `None` si le jeu n'a jamais répondu.
#[must_use]
pub fn lancer_chaine(
    save_editor: Option<&Path>,
    jeu: Option<&Path>,
    attente: Duration,
) -> Lancement {
    let mut l = Lancement::default();

    if let Some(e) = save_editor {
        demarrer(e, &mut l);
    }
    if let Some(j) = jeu {
        demarrer(j, &mut l);
        let nom = j.file_name().and_then(|n| n.to_str()).unwrap_or("nie.exe");
        l.pid = attendre_process(nom, attente);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_executable_absent_est_rapporte_pas_une_erreur() {
        let l = lancer_chaine(
            Some(Path::new("./ce-fichier-n-existe-pas-editeur.exe")),
            None,
            Duration::from_millis(1),
        );
        assert!(l.demarres.is_empty());
        assert_eq!(l.absents.len(), 1);
        assert!(l.pid.is_none());
    }

    #[test]
    fn sans_rien_a_lancer_le_bilan_est_vide() {
        let l = lancer_chaine(None, None, Duration::from_millis(1));
        assert_eq!(l, Lancement::default());
    }

    #[test]
    fn attendre_un_process_inexistant_rend_none_sans_bloquer() {
        let debut = Instant::now();
        assert!(
            attendre_process("ce-process-n-existe-pas.exe", Duration::from_millis(600)).is_none()
        );
        // La sonde dort par tranches de 500 ms : on vérifie qu'elle rend la main vite, sans
        // partir en boucle infinie.
        assert!(debut.elapsed() < Duration::from_secs(5));
    }
}
