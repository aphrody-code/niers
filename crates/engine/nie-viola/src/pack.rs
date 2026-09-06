//! **Pack** — rend un dossier de fichiers édités chargeable par le jeu.
//!
//! Aucune archive n'est fabriquée. Le jeu accepte un fichier **loose** (sur le disque, hors CPK)
//! dès lors que son entrée dans `cpk_list.cfg.bin` porte un nom de pack **vide** — c'est ce que
//! constate déjà l'indexation du VFS (`nie_formats::vfs::Vfs::init`). Packer, c'est donc
//! réécrire `cpk_list.cfg.bin` : pour chaque fichier du mod, vider les deux champs de pack et
//! inscrire la taille réelle, en ajoutant l'entrée si le fichier est neuf.
//!
//! # Écarts avec l'amont
//!
//! * `src/viola/pack.cpp` ne connaît que l'enveloppe XorShift. Le `cpk_list.cfg.bin` d'IEVR est
//!   chiffré en **AES-256-CBC** (clé et IV reversés de `nie.exe`, cf.
//!   [`nie_formats::cpk::decrypt_cpk_list`]) : le pack amont produirait ici un fichier illisible.
//!   [`pack_mod`] relit et réécrit dans l'enveloppe **réellement trouvée**.
//! * Le `cpk_list.cfg.bin` produit est écrit par fichier temporaire puis renommé. Une coupure
//!   pendant l'écriture laisserait sinon une liste tronquée — c'est-à-dire un jeu qui ne démarre
//!   plus si l'utilisatrice l'a pointée sur son installation.
//! * La recopie des fichiers du mod est parallèle et saute ceux déjà présents à la bonne taille.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use nie_formats::cfgbin::{self, CfgBinFile, CfgEntry, Value};
use nie_formats::cpk;
use rayon::prelude::*;

use crate::enumerer;

/// Enveloppe de chiffrement d'un `cpk_list.cfg.bin`.
///
/// Deux variantes coexistent selon le build et un fichier donné n'en accepte qu'une : l'autre
/// rend du charabia. Le repli est déjà éprouvé dans `Vfs::init` ; on le reprend ici pour pouvoir
/// **réécrire dans l'enveloppe d'origine**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpkListCrypto {
    /// Builds Steam récents : AES-256-CBC, clé et IV reversés de `nie.exe`.
    Aes,
    /// Dumps plus anciens : enveloppe XOR à clé fixe Viola (`0x1717_E18E`).
    Viola,
    /// Fichier déjà en clair.
    Clair,
}

/// Décode un `cpk_list.cfg.bin` brut et dit **dans quelle enveloppe** il se trouvait.
///
/// L'ordre d'essai (AES, Viola, clair) est celui du VFS, et chaque tentative est validée par un
/// vrai parsing : les deux enveloppes n'ont aucun marqueur distinctif en tête de fichier, c'est
/// le seul moyen de trancher.
///
/// # Errors
/// Si aucune des trois lectures ne produit un `cfg.bin` analysable.
pub fn decode_cpk_list(brut: &[u8]) -> Result<(CfgBinFile, CpkListCrypto), String> {
    if let Ok(clair) = cpk::decrypt_cpk_list(brut)
        && let Ok(cfg) = cfgbin::cfgbin_parse(&clair)
    {
        return Ok((cfg, CpkListCrypto::Aes));
    }
    let mut viola = brut.to_vec();
    cpk::decrypt_block(&mut viola, 0, cpk::VIOLA_FIXED_KEY);
    if let Ok(cfg) = cfgbin::cfgbin_parse(&viola) {
        return Ok((cfg, CpkListCrypto::Viola));
    }
    if let Ok(cfg) = cfgbin::cfgbin_parse(brut) {
        return Ok((cfg, CpkListCrypto::Clair));
    }
    Err("cpk_list.cfg.bin illisible : ni AES-256-CBC, ni clé Viola, ni clair".to_string())
}

/// Réencode un `cpk_list.cfg.bin` dans l'enveloppe d'où il vient.
#[must_use]
pub fn encode_cpk_list(entries: &[CfgEntry], crypto: CpkListCrypto) -> Vec<u8> {
    let clair = cfgbin::encode_t2b(entries);
    match crypto {
        CpkListCrypto::Aes => cpk::encrypt_cpk_list(&clair),
        CpkListCrypto::Viola => {
            let mut buf = clair;
            // XOR involutif : la même passe qu'au déchiffrement rechiffre.
            cpk::decrypt_block(&mut buf, 0, cpk::VIOLA_FIXED_KEY);
            buf
        }
        CpkListCrypto::Clair => clair,
    }
}

/// Plateforme cible — elle ne change que l'emplacement du `cpk_list.cfg.bin` produit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Version PC (Steam) : `data/cpk_list.cfg.bin`.
    Pc,
    /// Version Nintendo Switch : `romfs/data/cpk_list.cfg.bin`.
    Switch,
}

impl Platform {
    /// Chemin relatif du `cpk_list.cfg.bin` dans l'arborescence de sortie.
    #[must_use]
    pub fn cpk_list_rel(self) -> &'static str {
        match self {
            Platform::Pc => "data/cpk_list.cfg.bin",
            Platform::Switch => "romfs/data/cpk_list.cfg.bin",
        }
    }
}

/// Bilan d'un [`pack_mod`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackReport {
    /// Fichiers du mod qui remplacent une entrée existante (pack vidé, taille corrigée).
    pub mis_a_jour: usize,
    /// Fichiers du mod absents du `cpk_list` d'origine, donc ajoutés.
    pub ajoutes: usize,
    /// Fichiers recopiés dans la sortie.
    pub copies: usize,
    /// Nombre d'entrées après mise à jour.
    pub total: usize,
    /// Enveloppe dans laquelle le fichier a été relu **et** réécrit.
    pub crypto: CpkListCrypto,
    /// Entrées déjà *loose* dans le `cpk_list` fourni, avant toute modification.
    ///
    /// Le jeu en a légitimement quelques-unes (vidéos d'introduction, configuration système). Un
    /// nombre élevé signale en revanche un `cpk_list` **déjà packé** : repartir de celui-là
    /// empilerait les entrées d'un mod précédent, et l'interface doit le dire.
    pub loose_avant: usize,
}

/// Réécrit `cpk_list.cfg.bin` pour que le jeu charge les fichiers de `mod_dir` depuis le disque,
/// et recopie ces fichiers dans `sortie`.
///
/// `cpk_list` doit être le fichier **vanilla** sauvegardé avant tout modding (cf.
/// [`PackReport::loose_avant`]).
///
/// # Errors
/// Si le `cpk_list` est illisible, si sa structure n'est pas celle attendue, ou si une écriture
/// échoue.
pub fn pack_mod(
    cpk_list: &Path,
    mod_dir: &Path,
    sortie: &Path,
    plateforme: Platform,
) -> Result<PackReport, String> {
    let brut = std::fs::read(cpk_list).map_err(|e| format!("{} : {e}", cpk_list.display()))?;
    let (mut cfg, crypto) = decode_cpk_list(&brut)?;

    let racine = cfg
        .entries
        .first_mut()
        .ok_or_else(|| "cpk_list.cfg.bin sans entrée racine".to_string())?;
    if racine.children.is_empty() {
        return Err("cpk_list.cfg.bin sans aucune entrée de fichier".to_string());
    }

    // Index des entrées existantes par chemin complet, construit une fois : le pack amont fait
    // pareil, c'est la partie déjà correcte de son algorithme.
    let mut existantes: HashMap<String, usize> = HashMap::with_capacity(racine.children.len());
    let mut loose_avant = 0usize;
    for (i, enfant) in racine.children.iter().enumerate() {
        if enfant.variables.len() < 5 {
            continue;
        }
        if let (Value::String(dir), Value::String(nom)) =
            (&enfant.variables[0], &enfant.variables[1])
        {
            existantes.insert(format!("{dir}{nom}"), i);
        }
        if matches!(&enfant.variables[3], Value::String(s) if s.is_empty()) {
            loose_avant += 1;
        }
    }

    // Le gabarit d'une entrée neuve est une entrée existante : elle porte le nom de nœud et les
    // types de variables attendus, qu'on ne saurait pas reconstruire de zéro.
    let gabarit = racine
        .children
        .last()
        .cloned()
        .ok_or_else(|| "cpk_list.cfg.bin sans entrée à cloner".to_string())?;

    let fichiers = enumerer(mod_dir)?;
    let mut rapport = PackReport {
        mis_a_jour: 0,
        ajoutes: 0,
        copies: 0,
        total: 0,
        crypto,
        loose_avant,
    };

    for (absolu, rel) in &fichiers {
        // Un `cpk_list.cfg.bin` traîné dans le dossier de mod se décrirait lui-même.
        if rel.ends_with("cpk_list.cfg.bin") {
            continue;
        }
        // Tout ce qui n'est pas sous `data/` est une métadonnée du mod — manifeste, README,
        // capture — et n'a rien à faire dans l'index du jeu. Sans ce filtre, `mod.json` était
        // copié à la racine de l'installation ET inscrit comme une entrée neuve : le compte
        // passait à 255 309 pour un mod de trois fichiers, ce qui se voit, mais un mod livré
        // avec dix documents aurait pollué l'index d'autant sans que rien ne l'annonce.
        if crate::manifeste::est_metadonnee(rel) {
            continue;
        }
        let taille = std::fs::metadata(absolu).map(|m| m.len()).unwrap_or(0);
        let taille = i32::try_from(taille).map_err(|_| {
            format!("{rel} : dépasse 2 Gio, taille non représentable dans cpk_list")
        })?;

        if let Some(&i) = existantes.get(rel) {
            let e = &mut racine.children[i];
            if e.variables.len() >= 5 {
                // Vider les deux champs de pack bascule l'entrée en fichier loose : c'est CE
                // geste qui fait charger le fichier modifié plutôt que celui de l'archive.
                e.variables[2] = Value::String(String::new());
                e.variables[3] = Value::String(String::new());
                e.variables[4] = Value::Int(taille);
                rapport.mis_a_jour += 1;
            }
        } else {
            let mut neuve = gabarit.clone();
            if neuve.variables.len() >= 5 {
                let (dir, nom) = match rel.rfind('/') {
                    Some(p) => (rel[..=p].to_string(), rel[p + 1..].to_string()),
                    None => (String::new(), rel.clone()),
                };
                neuve.variables[0] = Value::String(dir);
                neuve.variables[1] = Value::String(nom);
                neuve.variables[2] = Value::String(String::new());
                neuve.variables[3] = Value::String(String::new());
                neuve.variables[4] = Value::Int(taille);
                racine.children.push(neuve);
                rapport.ajoutes += 1;
            }
        }
    }

    rapport.total = racine.children.len();
    // L'entrée racine porte le compte des entrées : le laisser périmé rend la liste incohérente.
    if let Some(Value::Int(n)) = racine.variables.first_mut() {
        *n =
            i32::try_from(rapport.total).map_err(|_| "trop d'entrées pour cpk_list".to_string())?;
    }

    // ── Recopie des fichiers, en parallèle ───────────────────────────────────────────────────
    let copies = AtomicUsize::new(0);
    let echecs: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    fichiers.par_iter().for_each(|(absolu, rel)| {
        if rel.ends_with("cpk_list.cfg.bin") {
            return;
        }
        // Même filtre que l'indexation ci-dessus : les deux boucles parcourent la même liste,
        // et n'en écarter qu'une déposait le manifeste à la racine de l'installation du jeu
        // tout en laissant l'index propre — un demi-correctif est ici pire qu'aucun, parce
        // qu'il rend le compte rassurant alors que le dossier ne l'est pas.
        if crate::manifeste::est_metadonnee(rel) {
            return;
        }
        let dest = sortie.join(rel);
        let a_jour = match (std::fs::metadata(absolu), std::fs::metadata(&dest)) {
            (Ok(s), Ok(d)) => s.len() == d.len(),
            _ => false,
        };
        if a_jour {
            copies.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let r = dest
            .parent()
            .map(std::fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|()| std::fs::copy(absolu, &dest));
        match r {
            Ok(_) => {
                copies.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                if let Ok(mut v) = echecs.lock() {
                    v.push(format!("{rel} : {e}"));
                }
            }
        }
    });
    let echecs = echecs
        .into_inner()
        .map_err(|_| "verrou d'échecs empoisonné".to_string())?;
    if !echecs.is_empty() {
        // Un pack partiellement recopié produirait un `cpk_list` annonçant des fichiers absents,
        // donc un jeu qui cherche ce qui n'existe pas : mieux vaut échouer franchement.
        return Err(format!(
            "{} fichier(s) non copié(s) : {}",
            echecs.len(),
            echecs.join(" ; ")
        ));
    }
    rapport.copies = copies.into_inner();

    // ── Écriture atomique du cpk_list ────────────────────────────────────────────────────────
    let encode = encode_cpk_list(&cfg.entries, crypto);
    let dest = sortie.join(plateforme.cpk_list_rel());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    let tmp = dest.with_extension("bin.tmp");
    std::fs::write(&tmp, &encode).map_err(|e| format!("{} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("{} : {e}", dest.display()))?;

    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_emplacement_du_cpk_list_depend_de_la_plateforme() {
        assert_eq!(Platform::Pc.cpk_list_rel(), "data/cpk_list.cfg.bin");
        assert_eq!(
            Platform::Switch.cpk_list_rel(),
            "romfs/data/cpk_list.cfg.bin"
        );
    }

    #[test]
    fn l_enveloppe_aes_fait_un_aller_retour_exact() {
        // Si le rechiffrement n'était pas l'inverse strict du déchiffrement, tout pack produirait
        // un cpk_list que le jeu ne relit pas.
        let clair: Vec<u8> = (0..64u8).collect();
        let chiffre = cpk::encrypt_cpk_list(&clair);
        let retour = cpk::decrypt_cpk_list(&chiffre).expect("déchiffrement du bloc rechiffré");
        assert_eq!(&retour[..clair.len()], &clair[..]);
    }

    #[test]
    fn l_enveloppe_viola_est_involutive() {
        let clair: Vec<u8> = (0..48u8).collect();
        let mut buf = clair.clone();
        cpk::decrypt_block(&mut buf, 0, cpk::VIOLA_FIXED_KEY);
        assert_ne!(buf, clair);
        cpk::decrypt_block(&mut buf, 0, cpk::VIOLA_FIXED_KEY);
        assert_eq!(buf, clair);
    }
}
