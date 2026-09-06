//! **Vérification** d'un dump : ce qui manque, ce qui est tronqué, ce qui a été écrasé.
//!
//! # Pourquoi cette passe est séparée du dump
//!
//! [`crate::dump`] rapporte ce qu'il a *fait* — extraits, sautés, échoués. C'est un journal
//! d'exécution, pas une preuve d'état : il ne dit rien d'un dump repris trois fois, d'un
//! dossier partiellement effacé depuis, d'un fichier écrasé par une collision de casse NTFS,
//! ni d'un dump produit par un autre outil. La question « ce dossier contient-il vraiment le
//! jeu ? » ne se répond qu'en **relisant le disque** et en le confrontant à l'index du VFS.
//!
//! C'est aussi la seule façon de valider le dump d'un tiers (Viola, `ievr_toolbox`) : aucun
//! d'eux ne produit de rapport exploitable, et tous s'arrêtent à « terminé ».
//!
//! # Deux niveaux
//!
//! * **Structure** (toujours) — présence et taille de chaque chemin attendu. Bon marché : un
//!   `stat` par fichier, aucune lecture. Détecte les manques, les troncatures et les
//!   écrasements par collision de casse.
//! * **Contenu** (échantillonné) — relecture du fichier et comparaison **octet à octet** avec
//!   ce que rend le VFS. Une taille juste ne prouve pas un contenu juste : un déchiffrement
//!   erroné rend exactement le bon nombre d'octets. L'échantillonnage est déterministe (un
//!   fichier sur `n`, dans l'ordre trié des chemins), donc deux exécutions vérifient le même
//!   sous-ensemble et sont comparables.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use nie_formats::vfs::Vfs;
use rayon::prelude::*;

use crate::filtre::Filtre;

/// Réglages d'une vérification.
#[derive(Debug, Clone)]
pub struct VerifOptions {
    /// Ne vérifier que les chemins retenus (cf. [`Filtre`]).
    pub filtre: Option<String>,
    /// Inclure les packs absents de `cpk_list.cfg.bin`, comme le fait le dump.
    pub inclure_extra: bool,
    /// Compare le contenu d'un fichier sur `n` avec le VFS. `0` = aucune comparaison.
    ///
    /// `1` compare tout : correct, mais relit l'intégralité du jeu deux fois.
    pub echantillon: usize,
    /// Nombre de travailleurs ; `None` = tous les cœurs.
    pub threads: Option<usize>,
}

impl Default for VerifOptions {
    fn default() -> Self {
        // Un fichier sur 500 : ~500 lectures sur 255 000 chemins, assez pour qu'un
        // déchiffrement cassé se voie, assez peu pour tenir en quelques secondes.
        Self {
            filtre: None,
            inclure_extra: true,
            echantillon: 500,
            threads: None,
        }
    }
}

/// Ce qui cloche sur un fichier donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Anomalie {
    /// Le chemin attendu est absent du dossier vérifié.
    Manquant,
    /// Le fichier existe mais sa taille diffère de celle annoncée par l'index du VFS.
    TailleDivergente,
    /// Le fichier existe à la bonne taille mais son contenu diffère de celui du VFS.
    ContenuDivergent,
    /// Le fichier existe mais n'a pas pu être relu.
    Illisible,
}

impl Anomalie {
    /// Identifiant stable, pour l'affichage et le rapport.
    #[must_use]
    pub const fn nom(self) -> &'static str {
        match self {
            Self::Manquant => "manquant",
            Self::TailleDivergente => "taille_divergente",
            Self::ContenuDivergent => "contenu_divergent",
            Self::Illisible => "illisible",
        }
    }
}

/// Une anomalie, telle qu'elle est rapportée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constat {
    /// Chemin VFS concerné.
    pub chemin: String,
    /// Nature du problème.
    pub anomalie: Anomalie,
    /// Taille annoncée par l'index du VFS.
    pub attendu: u64,
    /// Taille trouvée sur le disque (0 si absent).
    pub trouve: u64,
}

/// Bilan d'une vérification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifReport {
    /// Chemins attendus, filtre appliqué.
    pub attendus: usize,
    /// Chemins présents à la taille annoncée.
    pub conformes: usize,
    /// Chemins absents du dossier.
    pub manquants: usize,
    /// Chemins présents mais d'une autre taille.
    pub tailles_divergentes: usize,
    /// Fichiers dont le contenu a été comparé au VFS.
    pub compares: usize,
    /// Fichiers dont le contenu diffère du VFS.
    pub contenus_divergents: usize,
    /// Fichiers présents mais illisibles.
    pub illisibles: usize,
    /// Octets totalisés sur les fichiers présents.
    pub octets: u64,
    /// Détail, borné par [`MAX_CONSTATS`].
    pub constats: Vec<Constat>,
}

impl VerifReport {
    /// Part des chemins attendus réellement présents à la bonne taille, en pourcentage.
    ///
    /// C'est le seul chiffre qui répond à « le dump est-il complet ? ». Vaut 100 pour un
    /// périmètre vide : rien à trouver n'est pas un manque.
    #[must_use]
    pub fn couverture(&self) -> f64 {
        if self.attendus == 0 {
            return 100.0;
        }
        self.conformes as f64 * 100.0 / self.attendus as f64
    }

    /// `true` si rien ne cloche : tout est présent, à la bonne taille, et les contenus
    /// échantillonnés correspondent.
    #[must_use]
    pub fn conforme(&self) -> bool {
        self.manquants == 0
            && self.tailles_divergentes == 0
            && self.contenus_divergents == 0
            && self.illisibles == 0
    }
}

/// Au-delà, seuls les compteurs restent — un dossier vide produirait sinon 255 308 constats
/// identiques, qui ne s'affichent ni ne se lisent.
pub const MAX_CONSTATS: usize = 5_000;

/// Confronte le contenu de `dump` à l'index du VFS.
///
/// La taille de référence est celle de l'index (`cpk_list.cfg.bin`). Les entrées dont l'index
/// n'annonce pas de taille (celles de l'index supplémentaire, dont la taille vit dans le
/// sommaire du pack) ne sont vérifiées qu'en présence — les inventer serait pire que de
/// l'admettre.
///
/// # Errors
/// Si le pool de threads demandé ne peut pas être construit.
pub fn verifier(vfs: &Vfs, dump: &Path, options: &VerifOptions) -> Result<VerifReport, String> {
    let filtre = options
        .filtre
        .as_deref()
        .map_or_else(Filtre::default, Filtre::parse);

    // Trié : l'échantillonnage « un sur n » doit tomber sur les mêmes fichiers d'une exécution
    // à l'autre, sans quoi deux vérifications ne se comparent pas. L'ordre d'un HashMap ne le
    // garantit pas.
    let mut attendus: Vec<(&str, u64)> = vfs
        .iter()
        .filter(|(c, _)| filtre.accepte(c))
        .map(|(c, e)| {
            // Pour un fichier « loose », la taille de l'index n'est pas une référence : sur
            // l'installation Steam, les deux vidéos d'introduction y sont annoncées à la taille
            // d'une autre variante que celle réellement livrée sous `dx11/`. La référence est
            // alors le fichier source lui-même — sinon la vérification déclare un écart là où
            // la copie est exacte au sha256 près, et un `verify` qui crie au loup ne sert plus.
            if e.cpk_filename.is_empty() {
                let reelle = vfs
                    .resolve_loose_path(c)
                    .and_then(|p| std::fs::metadata(p).ok())
                    .map(|m| m.len());
                return (c, reelle.unwrap_or_else(|| u64::from(e.file_size)));
            }
            (c, u64::from(e.file_size))
        })
        .collect();
    if options.inclure_extra {
        attendus.extend(
            vfs.iter_extra()
                .filter(|(c, _)| filtre.accepte(c))
                .map(|(c, _)| (c, 0)),
        );
    }
    attendus.sort_unstable();

    let conformes = AtomicUsize::new(0);
    let manquants = AtomicUsize::new(0);
    let tailles = AtomicUsize::new(0);
    let compares = AtomicUsize::new(0);
    let contenus = AtomicUsize::new(0);
    let illisibles = AtomicUsize::new(0);
    let octets = AtomicU64::new(0);
    let constats: std::sync::Mutex<Vec<Constat>> = std::sync::Mutex::new(Vec::new());

    let noter = |chemin: &str, anomalie: Anomalie, attendu: u64, trouve: u64| {
        if let Ok(mut c) = constats.lock()
            && c.len() < MAX_CONSTATS
        {
            c.push(Constat {
                chemin: chemin.to_string(),
                anomalie,
                attendu,
                trouve,
            });
        }
    };

    let verifier_un = |(i, (chemin, taille)): (usize, &(&str, u64))| {
        let chemin = *chemin;
        let taille = *taille;
        let dest = dump.join(chemin.trim_start_matches('/'));
        let Ok(meta) = std::fs::metadata(&dest) else {
            manquants.fetch_add(1, Ordering::Relaxed);
            noter(chemin, Anomalie::Manquant, taille, 0);
            return;
        };
        octets.fetch_add(meta.len(), Ordering::Relaxed);
        // Taille 0 dans l'index = taille inconnue (index supplémentaire) : la présence suffit.
        if taille > 0 && meta.len() != taille {
            tailles.fetch_add(1, Ordering::Relaxed);
            noter(chemin, Anomalie::TailleDivergente, taille, meta.len());
            return;
        }
        conformes.fetch_add(1, Ordering::Relaxed);

        // Comparaison de contenu, sur l'échantillon. Une taille juste ne prouve rien : un
        // déchiffrement à mauvaise clé rend exactement le bon nombre d'octets.
        if options.echantillon > 0 && i.is_multiple_of(options.echantillon) {
            let Ok(sur_disque) = std::fs::read(&dest) else {
                illisibles.fetch_add(1, Ordering::Relaxed);
                noter(chemin, Anomalie::Illisible, taille, meta.len());
                return;
            };
            // Le VFS est la référence : s'il ne sait pas lire le fichier, il n'y a rien à
            // comparer et compter une divergence serait mentir.
            if let Ok(attendu) = vfs.read(chemin) {
                compares.fetch_add(1, Ordering::Relaxed);
                if sur_disque != attendu {
                    contenus.fetch_add(1, Ordering::Relaxed);
                    noter(
                        chemin,
                        Anomalie::ContenuDivergent,
                        attendu.len() as u64,
                        sur_disque.len() as u64,
                    );
                }
            }
        }
    };

    let executer = || match options.threads {
        Some(n) if n > 0 => rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build()
            .map_err(|e| e.to_string())
            .map(|pool| pool.install(|| attendus.par_iter().enumerate().for_each(verifier_un))),
        _ => {
            attendus.par_iter().enumerate().for_each(verifier_un);
            Ok(())
        }
    };
    executer()?;

    let mut liste = constats.into_inner().unwrap_or_default();
    liste.sort_unstable_by(|a, b| a.chemin.cmp(&b.chemin));

    Ok(VerifReport {
        attendus: attendus.len(),
        conformes: conformes.into_inner(),
        manquants: manquants.into_inner(),
        tailles_divergentes: tailles.into_inner(),
        compares: compares.into_inner(),
        contenus_divergents: contenus.into_inner(),
        illisibles: illisibles.into_inner(),
        octets: octets.into_inner(),
        constats: liste,
    })
}

/// Chemins présents dans `dump` mais **absents** de l'index du VFS.
///
/// Un dump n'est pas seulement ce qui manque : des fichiers en trop signalent un dossier de
/// sortie réutilisé d'une autre version du jeu, ou un mod resté en place. Les artefacts posés
/// par le dump lui-même (manifeste, journal, index) sont exclus.
///
/// # Errors
/// Si `dump` n'est pas lisible.
pub fn intrus(vfs: &Vfs, dump: &Path) -> Result<Vec<String>, String> {
    let connus: std::collections::HashSet<&str> = vfs
        .iter()
        .map(|(c, _)| c)
        .chain(vfs.iter_extra().map(|(c, _)| c))
        .collect();
    let mut hors_index: Vec<String> = crate::enumerer(dump)?
        .into_iter()
        .map(|(_, rel)| rel)
        .filter(|rel| !rel.starts_with(".nie-dump-") && !connus.contains(rel.as_str()))
        .collect();
    hors_index.sort_unstable();
    Ok(hors_index)
}

/// Chemin conventionnel du rapport de vérification.
#[must_use]
pub fn chemin_rapport(dump: &Path) -> PathBuf {
    dump.join(".nie-dump-verif.json")
}

/// Dépose le rapport en JSON, pour que l'explorateur et les tests le relisent.
///
/// # Errors
/// Si l'écriture échoue.
pub fn ecrire_rapport(dump: &Path, r: &VerifReport) -> std::io::Result<()> {
    let constats: Vec<serde_json::Value> = r
        .constats
        .iter()
        .map(|c| {
            serde_json::json!({
                "chemin": c.chemin,
                "anomalie": c.anomalie.nom(),
                "attendu": c.attendu,
                "trouve": c.trouve,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "attendus": r.attendus,
        "conformes": r.conformes,
        "couverture_pct": r.couverture(),
        "manquants": r.manquants,
        "tailles_divergentes": r.tailles_divergentes,
        "compares": r.compares,
        "contenus_divergents": r.contenus_divergents,
        "illisibles": r.illisibles,
        "octets": r.octets,
        "conforme": r.conforme(),
        "constats_tronques": r.constats.len() >= MAX_CONSTATS,
        "constats": constats,
    });
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(chemin_rapport(dump), json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_couverture_vaut_cent_sur_un_perimetre_vide() {
        let r = VerifReport::default();
        assert!(
            (r.couverture() - 100.0).abs() < f64::EPSILON,
            "rien à trouver n'est pas un manque"
        );
        assert!(r.conforme());
    }

    #[test]
    fn la_couverture_ne_compte_que_les_conformes() {
        let r = VerifReport {
            attendus: 200,
            conformes: 150,
            manquants: 50,
            ..VerifReport::default()
        };
        assert!((r.couverture() - 75.0).abs() < 1e-9);
        assert!(!r.conforme(), "50 manquants : non conforme");
    }

    #[test]
    fn un_contenu_divergent_suffit_a_declasser_un_dump_complet() {
        // Piège réel : tout est présent, à la bonne taille, et pourtant faux — c'est ce qu'un
        // déchiffrement à mauvaise clé produit.
        let r = VerifReport {
            attendus: 10,
            conformes: 10,
            compares: 3,
            contenus_divergents: 1,
            ..VerifReport::default()
        };
        assert!((r.couverture() - 100.0).abs() < f64::EPSILON);
        assert!(!r.conforme(), "100 % de couverture ne vaut pas conformité");
    }

    #[test]
    fn le_rapport_est_relisible_et_dit_sa_troncature() {
        let dir = std::env::temp_dir().join(format!("nie-viola-verif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier de test");
        let r = VerifReport {
            attendus: 4,
            conformes: 3,
            manquants: 1,
            constats: vec![Constat {
                chemin: "data/x.bin".into(),
                anomalie: Anomalie::Manquant,
                attendu: 12,
                trouve: 0,
            }],
            ..VerifReport::default()
        };
        ecrire_rapport(&dir, &r).expect("écriture");
        let lu = std::fs::read_to_string(chemin_rapport(&dir)).expect("relecture");
        let v: serde_json::Value = serde_json::from_str(&lu).expect("json");
        assert_eq!(v["manquants"], 1);
        assert_eq!(v["conforme"], false);
        assert_eq!(v["constats"][0]["anomalie"], "manquant");
        assert_eq!(v["constats_tronques"], false);
        std::fs::remove_dir_all(&dir).ok();
    }
}
