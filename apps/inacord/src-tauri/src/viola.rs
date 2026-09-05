//! Onglet **Viola** — les opérations de modding LEVEL-5 (*dump*, *pack*, *merge*, chiffrement
//! Criware), au-dessus du crate [`nie_viola`].
//!
//! Tout se fait **en process**. Une première version de ce module lançait le toolkit C++
//! (`iecode.exe`) en sous-processus ; c'était le mauvais choix : le calcul existe en Rust, et
//! dépendre d'un binaire frère est exactement ce qui avait cassé l'aperçu 3D en build distribué.
//!
//! Le *dump* construit son **propre** VFS plutôt que d'emprunter celui de l'application : il dure
//! plusieurs minutes, et tenir le verrou du VFS partagé pendant ce temps figerait l'explorateur.
//!
//! Convention `specta` respectée partout : aucun entier 64 bits dans les DTO (l'export des
//! bindings refuse les « BigInt » et la panique survient au démarrage de l'application).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Exécutions en cours, pour pouvoir les annuler. Une entrée disparaît dès la fin de l'opération.
#[derive(Default)]
pub struct ViolaState {
    runs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

/// Plateforme cible d'un *pack* ou d'un *merge*.
#[derive(Deserialize, specta::Type, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ViolaPlatform {
    /// Version PC (Steam).
    Pc,
    /// Version Nintendo Switch.
    Switch,
}

impl From<ViolaPlatform> for nie_viola::Platform {
    fn from(p: ViolaPlatform) -> Self {
        match p {
            ViolaPlatform::Pc => nie_viola::Platform::Pc,
            ViolaPlatform::Switch => nie_viola::Platform::Switch,
        }
    }
}

/// Avancement d'un dump (événement `viola-dump-progress`).
#[derive(Serialize, Clone, specta::Type)]
pub struct ViolaProgressDto {
    /// Identifiant de l'exécution.
    pub run_id: String,
    /// Fichiers traités.
    pub faits: f64,
    /// Fichiers à traiter.
    pub total: f64,
    /// Octets écrits.
    pub octets: f64,
}

/// Fin d'un dump (événement `viola-dump-done`).
#[derive(Serialize, Clone, specta::Type)]
pub struct ViolaDumpDoneDto {
    /// Identifiant de l'exécution.
    pub run_id: String,
    /// Fichiers écrits.
    pub extraits: f64,
    /// Fichiers déjà présents à la bonne taille, donc non réécrits.
    pub sautes: f64,
    /// Fichiers en échec — comptés, jamais fatals.
    pub echecs: f64,
    /// Octets écrits.
    pub octets: f64,
    /// Packs entièrement sautés grâce au manifeste de reprise.
    pub packs_repris: f64,
    /// `true` si l'utilisatrice a interrompu l'opération.
    pub annule: bool,
    /// Message d'erreur si le dump n'a pas pu démarrer.
    pub erreur: Option<String>,
}

/// Bilan d'un *pack*.
#[derive(Serialize, specta::Type)]
pub struct ViolaPackDto {
    /// Fichiers du mod qui remplacent une entrée existante.
    pub mis_a_jour: f64,
    /// Fichiers ajoutés au `cpk_list`.
    pub ajoutes: f64,
    /// Fichiers recopiés dans la sortie.
    pub copies: f64,
    /// Nombre d'entrées après mise à jour.
    pub total: f64,
    /// Enveloppe de chiffrement relue et réécrite (`Aes`, `Viola` ou `Clair`).
    pub enveloppe: String,
    /// Entrées déjà *loose* AVANT modification — un nombre élevé trahit un `cpk_list` déjà packé.
    pub loose_avant: f64,
}

/// Un chemin disputé par plusieurs mods lors d'un *merge*.
#[derive(Serialize, specta::Type)]
pub struct ViolaConflitDto {
    /// Chemin relatif concerné.
    pub chemin: String,
    /// Rangs des mods qui le fournissent (0 = le plus prioritaire).
    pub rangs: Vec<f64>,
    /// Valeurs fusionnées sans désaccord.
    pub champs_fusionnes: f64,
    /// Valeurs que deux mods changent différemment, tranchées par la priorité.
    pub champs_en_desaccord: f64,
    /// Pourquoi la fusion au champ n'a pas pu s'appliquer, le cas échéant.
    pub repli: Option<String>,
}

/// Bilan d'un *merge*.
#[derive(Serialize, specta::Type)]
pub struct ViolaMergeDto {
    /// Fichiers écrits tels quels.
    pub copies: f64,
    /// Fichiers reconstruits par fusion au champ.
    pub fusionnes: f64,
    /// Chemins fournis par plusieurs mods.
    pub conflits: Vec<ViolaConflitDto>,
}

/// Démarre un *dump* en tâche de fond et rend immédiatement son identifiant.
///
/// L'avancement arrive par l'événement `viola-dump-progress`, la fin par `viola-dump-done`.
#[tauri::command]
#[specta::specta]
pub fn viola_dump_start(
    game_dir: Option<String>,
    sortie: String,
    filtre: Option<String>,
    reprise: bool,
    sauter_identiques: bool,
    threads: Option<u32>,
    app: tauri::AppHandle,
    state: tauri::State<ViolaState>,
) -> Result<String, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let annuler = Arc::new(AtomicBool::new(false));
    state
        .runs
        .lock()
        .map_err(|_| "verrou des exécutions Viola empoisonné".to_string())?
        .insert(run_id.clone(), Arc::clone(&annuler));

    let runs = Arc::clone(&state.runs);
    let id = run_id.clone();
    std::thread::spawn(move || {
        let fin = |dto: ViolaDumpDoneDto| {
            if let Ok(mut r) = runs.lock() {
                r.remove(&dto.run_id);
            }
            let _ = app.emit("viola-dump-done", dto);
        };
        let vide = |erreur: Option<String>| ViolaDumpDoneDto {
            run_id: id.clone(),
            extraits: 0.0,
            sautes: 0.0,
            echecs: 0.0,
            octets: 0.0,
            packs_repris: 0.0,
            annule: false,
            erreur,
        };

        // VFS privé : un dump dure des minutes, emprunter celui de l'application le figerait.
        let racine = match game_dir {
            Some(d) => PathBuf::from(d),
            None => nie_formats::vfs::resolve_game_dir(),
        };
        let mut vfs = nie_formats::vfs::Vfs::new();
        if let Err(e) = vfs.init(racine.join("data")) {
            fin(vide(Some(format!("VFS : {e}"))));
            return;
        }

        let options = nie_viola::DumpOptions {
            filtre,
            reprise,
            sauter_identiques,
            threads: threads.map(|t| t as usize),
            ..nie_viola::DumpOptions::default()
        };
        let app_progres = app.clone();
        let id_progres = id.clone();
        let rapporteur = move |p: nie_viola::DumpProgress| {
            let _ = app_progres.emit(
                "viola-dump-progress",
                ViolaProgressDto {
                    run_id: id_progres.clone(),
                    faits: p.faits as f64,
                    total: p.total as f64,
                    octets: p.octets as f64,
                },
            );
        };

        match nie_viola::dump_all(&vfs, &PathBuf::from(sortie), &options, &annuler, &rapporteur) {
            Ok(r) => fin(ViolaDumpDoneDto {
                run_id: id.clone(),
                extraits: r.extraits as f64,
                sautes: r.sautes as f64,
                echecs: r.echecs as f64,
                octets: r.octets as f64,
                packs_repris: r.packs_repris as f64,
                annule: r.annule,
                erreur: None,
            }),
            Err(e) => fin(vide(Some(e))),
        }
    });

    Ok(run_id)
}

/// Interrompt une exécution. Sans effet si elle est déjà terminée — l'événement de fin reste la
/// seule source de vérité.
#[tauri::command]
#[specta::specta]
pub fn viola_cancel(run_id: String, state: tauri::State<ViolaState>) -> Result<(), String> {
    if let Some(a) = state
        .runs
        .lock()
        .map_err(|_| "verrou des exécutions Viola empoisonné".to_string())?
        .get(&run_id)
    {
        a.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// **Pack** — réécrit `cpk_list.cfg.bin` pour que le jeu charge les fichiers du mod depuis le
/// disque, et recopie ces fichiers dans la sortie.
#[tauri::command]
#[specta::specta]
pub fn viola_pack(
    cpk_list: String,
    mod_dir: String,
    sortie: String,
    plateforme: ViolaPlatform,
) -> Result<ViolaPackDto, String> {
    let r = nie_viola::pack_mod(
        &PathBuf::from(cpk_list),
        &PathBuf::from(mod_dir),
        &PathBuf::from(sortie),
        plateforme.into(),
    )?;
    Ok(ViolaPackDto {
        mis_a_jour: r.mis_a_jour as f64,
        ajoutes: r.ajoutes as f64,
        copies: r.copies as f64,
        total: r.total as f64,
        enveloppe: format!("{:?}", r.crypto),
        loose_avant: r.loose_avant as f64,
    })
}

/// **Merge** — fusionne plusieurs mods, `sources` étant en priorité **décroissante**.
///
/// `semantique` active la fusion au champ des `.cfg.bin` : deux mods qui touchent des valeurs
/// différentes du même fichier sont alors compatibles, ce qu'une fusion au fichier ne permet pas.
/// Elle a besoin du jeu comme base de comparaison — sans lui, on retombe sur la fusion au fichier
/// et chaque repli est rapporté.
#[tauri::command]
#[specta::specta]
pub fn viola_merge(
    game_dir: Option<String>,
    sources: Vec<String>,
    sortie: String,
    semantique: bool,
) -> Result<ViolaMergeDto, String> {
    if sources.is_empty() {
        return Err("aucun dossier source à fusionner".to_string());
    }
    let chemins: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
    let sortie = PathBuf::from(sortie);

    let rapport = if semantique {
        let racine = match game_dir {
            Some(d) => PathBuf::from(d),
            None => nie_formats::vfs::resolve_game_dir(),
        };
        let mut vfs = nie_formats::vfs::Vfs::new();
        vfs.init(racine.join("data")).map_err(|e| format!("VFS : {e}"))?;
        let resolveur = |chemin: &str| vfs.read(chemin).ok();
        nie_viola::merge_dirs(&chemins, &sortie, &nie_viola::MergeStrategy::Semantique(&resolveur))?
    } else {
        nie_viola::merge_dirs(&chemins, &sortie, &nie_viola::MergeStrategy::Fichier)?
    };

    Ok(ViolaMergeDto {
        copies: rapport.copies as f64,
        fusionnes: rapport.fusionnes as f64,
        conflits: rapport
            .conflits
            .into_iter()
            .map(|c| ViolaConflitDto {
                chemin: c.chemin,
                rangs: c.rangs.into_iter().map(|r| r as f64).collect(),
                champs_fusionnes: c.champs_fusionnes as f64,
                champs_en_desaccord: c.champs_en_desaccord as f64,
                repli: c.repli,
            })
            .collect(),
    })
}

/// **Chiffrer / déchiffrer Criware** — le XOR est involutif, la même commande sert aux deux sens.
///
/// `cle` accepte l'hexadécimal (`1717E18E`) ; laissée vide, la clé est dérivée du nom du fichier
/// (CRC32), ce qui est la règle des packs CPK.
#[tauri::command]
#[specta::specta]
pub fn viola_crypto(
    entree: String,
    sortie: String,
    cle: Option<String>,
) -> Result<f64, String> {
    let source = PathBuf::from(entree);
    let cle = match cle.filter(|c| !c.trim().is_empty()) {
        Some(hex) => nie_viola::CriwareKey::depuis_hex(&hex)?,
        None => nie_viola::CriwareKey::DuNom(
            source.file_name().unwrap_or_default().to_string_lossy().to_string(),
        ),
    };
    nie_viola::crypt_file(&source, &PathBuf::from(sortie), &cle).map(|n| n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_plateforme_se_convertit_sans_perte() {
        assert_eq!(nie_viola::Platform::from(ViolaPlatform::Pc).cpk_list_rel(), "data/cpk_list.cfg.bin");
        assert_eq!(
            nie_viola::Platform::from(ViolaPlatform::Switch).cpk_list_rel(),
            "romfs/data/cpk_list.cfg.bin"
        );
    }
}
