//! Le **manifeste** d'un mod — ce qui fait d'un dossier de fichiers un mod partageable.
//!
//! # Pourquoi ce module existe
//!
//! Avant lui, un mod de ce dépôt n'existait que sous deux formes locales, aucune portable :
//! des lignes dans un SQLite d'`AppData` (`id, name, description, enabled, priority`) et des
//! fichiers aux noms **aplatis** (`/` → `_`) dans un dossier de travail. Pas d'auteur, pas de
//! version, pas de dépendances, pas de jeu cible — rien qui puisse s'échanger, et deux champs
//! (`enabled`, `priority`) qui n'étaient lus par personne.
//!
//! Pire, le nom aplati rendait ce dossier **inutilisable par [`crate::pack_mod`]**, qui attend
//! une arborescence relative au VFS. Les deux moitiés de la chaîne ne se parlaient pas.
//!
//! # Le format
//!
//! Un mod est un dossier contenant [`NOM_MANIFESTE`] à sa racine et une arborescence
//! **exactement celle du VFS** :
//!
//! ```text
//! mods/aphrody/
//!   mod.json
//!   data/common/gamedata/character/chara_param_1.03.66.00.cfg.bin
//!   data/common/text/fr/chara_text.cfg.bin
//! ```
//!
//! C'est déjà ce que `pack_mod` consomme : l'index du `cpk_list` est bâti sur `dir + nom`, où
//! `dir` commence par `data/`. Un fichier rangé ailleurs ne correspondra à aucune entrée, ne
//! sera jamais chargé, et — c'est le piège — **sans la moindre erreur**. [`valider_arborescence`]
//! est là pour que ce silence ne se produise pas.
//!
//! # JSON et non TOML
//!
//! Le plan disait TOML ; c'est JSON. Deux raisons : `serde_json` est déjà tiré par ce crate (le
//! manifeste de reprise du dump), là où `toml` serait une dépendance de plus pour un fichier de
//! dix lignes ; et l'outillage C++ amont écrit déjà un `mod_data.json`, ce qui rend les deux
//! mondes lisibles l'un par l'autre.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Nom du manifeste, à la racine du dossier de mod.
pub const NOM_MANIFESTE: &str = "mod.json";

/// Préfixe obligatoire des chemins d'un mod — celui du VFS.
const PREFIXE_VFS: &str = "data/";

/// Un numéro de version en trois nombres.
///
/// Écrit à la main plutôt que tiré d'un crate SemVer : seule la comparaison de trois entiers
/// sert ici, et le dépôt applique déjà cette économie ailleurs (cf. [`crate::glob_match`], qui
/// ne reconnaît que `*` faute d'appelant pour le reste).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Incompatibilité annoncée.
    pub majeure: u32,
    /// Ajout compatible.
    pub mineure: u32,
    /// Correction.
    pub corrective: u32,
}

impl Version {
    /// Analyse `majeure.mineure.corrective`.
    ///
    /// # Errors
    /// Si la chaîne n'a pas trois composantes entières.
    pub fn analyser(s: &str) -> Result<Self, String> {
        let mut it = s.trim().split('.');
        let mut suivant = |quoi: &str| -> Result<u32, String> {
            it.next()
                .ok_or_else(|| format!("version « {s} » : composante {quoi} absente"))?
                .parse::<u32>()
                .map_err(|_| format!("version « {s} » : composante {quoi} non entière"))
        };
        let majeure = suivant("majeure")?;
        let mineure = suivant("mineure")?;
        let corrective = suivant("corrective")?;
        if it.next().is_some() {
            return Err(format!("version « {s} » : plus de trois composantes"));
        }
        Ok(Self {
            majeure,
            mineure,
            corrective,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.majeure, self.mineure, self.corrective)
    }
}

/// Un mod dont celui-ci dépend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependance {
    /// Nom du mod requis, tel qu'il figure dans *son* manifeste.
    pub nom: String,
    /// Version minimale acceptée, `None` si n'importe laquelle convient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_min: Option<String>,
}

/// Le manifeste lui-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifeste {
    /// Nom lisible, et identifiant pour les dépendances.
    pub nom: String,
    /// Auteur ou équipe.
    pub auteur: String,
    /// Version du mod, en trois nombres.
    pub version: String,
    /// À quoi sert ce mod.
    #[serde(default)]
    pub description: String,
    /// Jeu visé — un seul aujourd'hui, mais l'écrire évite qu'un mod d'un autre titre s'installe.
    #[serde(default = "jeu_par_defaut")]
    pub jeu: String,
    /// Version du jeu sur laquelle le mod a été construit, si connue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_jeu: Option<String>,
    /// Mods requis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependances: Vec<Dependance>,
    /// Priorité de fusion : **le plus grand gagne** en cas de désaccord sur un même champ.
    #[serde(default)]
    pub priorite: i32,
}

/// Identifiant du jeu visé par défaut.
fn jeu_par_defaut() -> String {
    "IEVR".to_string()
}

impl Manifeste {
    /// Manifeste neuf, prêt à être complété.
    #[must_use]
    pub fn gabarit(nom: &str, auteur: &str) -> Self {
        Self {
            nom: nom.to_string(),
            auteur: auteur.to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            jeu: jeu_par_defaut(),
            version_jeu: None,
            dependances: Vec::new(),
            priorite: 0,
        }
    }

    /// Chemin du manifeste dans un dossier de mod.
    #[must_use]
    pub fn chemin(dossier: &Path) -> PathBuf {
        dossier.join(NOM_MANIFESTE)
    }

    /// Lit le manifeste d'un dossier de mod.
    ///
    /// # Errors
    /// Si le fichier est absent ou mal formé.
    pub fn lire(dossier: &Path) -> Result<Self, String> {
        let p = Self::chemin(dossier);
        let texte = std::fs::read_to_string(&p)
            .map_err(|e| format!("{} : {e} — ce dossier n'est pas un mod", p.display()))?;
        serde_json::from_str(&texte).map_err(|e| format!("{} : {e}", p.display()))
    }

    /// Écrit le manifeste dans un dossier de mod.
    ///
    /// # Errors
    /// Si le dossier ou le fichier ne peut pas être écrit.
    pub fn ecrire(&self, dossier: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let p = Self::chemin(dossier);
        std::fs::write(&p, json).map_err(|e| format!("{} : {e}", p.display()))
    }

    /// Version analysée.
    ///
    /// # Errors
    /// Si le champ `version` n'est pas en trois nombres.
    pub fn version(&self) -> Result<Version, String> {
        Version::analyser(&self.version)
    }

    /// Reproches faits au manifeste — vide si tout va bien.
    ///
    /// Rend une **liste** plutôt qu'un premier échec : corriger un manifeste à coups d'erreurs
    /// successives est pénible, et rien n'oblige à s'arrêter au premier problème.
    #[must_use]
    pub fn reproches(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.nom.trim().is_empty() {
            v.push("« nom » est vide".to_string());
        }
        if self.auteur.trim().is_empty() {
            v.push("« auteur » est vide".to_string());
        }
        if let Err(e) = self.version() {
            v.push(e);
        }
        for d in &self.dependances {
            if d.nom.trim().is_empty() {
                v.push("une dépendance sans nom".to_string());
            }
            if let Some(m) = &d.version_min
                && let Err(e) = Version::analyser(m)
            {
                v.push(format!("dépendance « {} » : {e}", d.nom));
            }
        }
        v
    }
}

/// Un fichier du mod à installer : son chemin sur le disque et son chemin VFS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierMod {
    /// Emplacement réel.
    pub absolu: PathBuf,
    /// Chemin VFS correspondant (`data/...`).
    pub vfs: String,
}

/// Ce fichier est-il une métadonnée du mod plutôt qu'un asset à installer ?
///
/// Tout ce qui n'est pas sous `data/` en est une : le manifeste, un README, une capture. Le
/// critère est le même que celui du VFS, pas une liste de noms — une liste finirait toujours
/// par rater le fichier qu'on n'avait pas prévu.
#[must_use]
pub fn est_metadonnee(chemin_relatif: &str) -> bool {
    !chemin_relatif.starts_with(PREFIXE_VFS)
}

/// Les assets d'un mod, en chemins VFS, triés.
///
/// # Errors
/// Si le dossier est illisible.
pub fn fichiers(dossier: &Path) -> Result<Vec<FichierMod>, String> {
    Ok(crate::enumerer(dossier)?
        .into_iter()
        .filter(|(_, rel)| !est_metadonnee(rel))
        .map(|(absolu, vfs)| FichierMod { absolu, vfs })
        .collect())
}

/// Fichiers rangés hors de `data/`, donc **jamais chargés par le jeu**.
///
/// Le manifeste et les documents d'accompagnement sont exclus : ce sont des métadonnées
/// légitimes. Ne restent que les fichiers qui *ressemblent* à des assets sans en être — un mod
/// construit en aplatissant les chemins (`data_common_...`) tombe entièrement ici, ce qui est
/// exactement le cas qu'on veut voir signalé plutôt qu'installé en silence.
///
/// # Errors
/// Si le dossier est illisible.
pub fn valider_arborescence(dossier: &Path) -> Result<Vec<String>, String> {
    const TOLERES: [&str; 4] = [NOM_MANIFESTE, "README.md", "LICENSE", ".gitignore"];
    Ok(crate::enumerer(dossier)?
        .into_iter()
        .map(|(_, rel)| rel)
        .filter(|rel| est_metadonnee(rel) && !TOLERES.contains(&rel.as_str()))
        .collect())
}

/// Ordonne des mods pour la fusion : dépendances d'abord, puis priorité **croissante**.
///
/// [`crate::merge_dirs`] prend ses sources en priorité **décroissante** (le premier gagne) ;
/// cette fonction rend l'ordre d'application, du moins prioritaire au plus prioritaire. C'est
/// à l'appelant d'inverser s'il passe le résultat à `merge_dirs` — l'inversion est explicite
/// pour qu'on ne puisse pas se tromper de sens sans le voir.
///
/// Un mod placé après un autre l'emporte donc sur lui. Une dépendance est appliquée avant son
/// dépendant, ce qui laisse ce dernier écraser ce qu'il veut de sa base.
///
/// # Errors
/// Si une dépendance est manquante, si sa version est trop ancienne, ou si les dépendances
/// forment un cycle.
pub fn ordonner(mods: &[Manifeste]) -> Result<Vec<usize>, String> {
    let mut par_nom: HashMap<&str, usize> = HashMap::with_capacity(mods.len());
    for (i, m) in mods.iter().enumerate() {
        if let Some(precedent) = par_nom.insert(m.nom.as_str(), i) {
            return Err(format!(
                "deux mods portent le nom « {} » (positions {precedent} et {i}) — les dépendances deviendraient ambiguës",
                m.nom
            ));
        }
    }

    // Vérifier les versions avant le tri : une dépendance présente mais trop ancienne est une
    // erreur de compatibilité, pas un problème d'ordre, et le dire ainsi est plus clair.
    for m in mods {
        for d in &m.dependances {
            let Some(&j) = par_nom.get(d.nom.as_str()) else {
                return Err(format!(
                    "« {} » dépend de « {} », absent de la sélection",
                    m.nom, d.nom
                ));
            };
            if let Some(min) = &d.version_min {
                let min = Version::analyser(min)?;
                let trouvee = mods[j].version()?;
                if trouvee < min {
                    return Err(format!(
                        "« {} » exige « {} » >= {min}, or la version fournie est {trouvee}",
                        m.nom, d.nom
                    ));
                }
            }
        }
    }

    // Tri topologique en profondeur. Les racines sont visitées par priorité croissante, ce qui
    // fait de la priorité le départage naturel entre mods indépendants.
    let mut ordre_depart: Vec<usize> = (0..mods.len()).collect();
    ordre_depart.sort_by(|&a, &b| {
        mods[a]
            .priorite
            .cmp(&mods[b].priorite)
            .then_with(|| mods[a].nom.cmp(&mods[b].nom))
    });

    // `0` = jamais vu, `1` = en cours de visite (donc un retour ici est un cycle), `2` = fini.
    let mut etat = vec![0u8; mods.len()];
    let mut sortie = Vec::with_capacity(mods.len());
    let mut pile: Vec<(usize, usize)> = Vec::new();

    for &depart in &ordre_depart {
        if etat[depart] != 0 {
            continue;
        }
        pile.push((depart, 0));
        etat[depart] = 1;
        while let Some((i, k)) = pile.pop() {
            match mods[i].dependances.get(k) {
                Some(d) => {
                    pile.push((i, k + 1));
                    let j = par_nom[d.nom.as_str()];
                    match etat[j] {
                        0 => {
                            etat[j] = 1;
                            pile.push((j, 0));
                        }
                        1 => {
                            return Err(format!(
                                "cycle de dépendances : « {} » et « {} » s'attendent mutuellement",
                                mods[i].nom, mods[j].nom
                            ));
                        }
                        _ => {}
                    }
                }
                None => {
                    etat[i] = 2;
                    sortie.push(i);
                }
            }
        }
    }
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(nom: &str, priorite: i32, deps: &[&str]) -> Manifeste {
        Manifeste {
            dependances: deps
                .iter()
                .map(|d| Dependance {
                    nom: (*d).to_string(),
                    version_min: None,
                })
                .collect(),
            priorite,
            ..Manifeste::gabarit(nom, "aphrody")
        }
    }

    #[test]
    fn une_version_se_lit_et_se_compare() {
        assert_eq!(
            Version::analyser("1.3.66").expect("valide"),
            Version {
                majeure: 1,
                mineure: 3,
                corrective: 66
            }
        );
        assert!(Version::analyser("1.3.66").unwrap() > Version::analyser("1.3.9").unwrap());
        assert!(Version::analyser("2.0.0").unwrap() > Version::analyser("1.99.99").unwrap());
        // Une comparaison textuelle rendrait « 1.3.9 » > « 1.3.66 » : c'est tout l'intérêt.
        assert!("1.3.9" > "1.3.66");
        assert!(
            Version::analyser("1.3").is_err(),
            "trois composantes exigées"
        );
        assert!(Version::analyser("1.3.a").is_err());
        assert!(Version::analyser("1.3.4.5").is_err());
    }

    #[test]
    fn le_manifeste_fait_un_aller_retour() {
        let dir = std::env::temp_dir().join(format!("nie-manif-ar-{}", std::process::id()));
        let mut attendu = Manifeste::gabarit("aphrody", "Yohan");
        attendu.description = "Byron Love, inarrêtable".to_string();
        attendu.priorite = 10;
        attendu.ecrire(&dir).expect("écriture");
        assert_eq!(Manifeste::lire(&dir).expect("relecture"), attendu);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn un_dossier_sans_manifeste_n_est_pas_un_mod() {
        let dir = std::env::temp_dir().join(format!("nie-manif-vide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier");
        assert!(Manifeste::lire(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn les_reproches_sont_cumules() {
        let mauvais = Manifeste {
            nom: String::new(),
            auteur: "  ".to_string(),
            version: "1.3".to_string(),
            ..Manifeste::gabarit("x", "y")
        };
        let r = mauvais.reproches();
        assert_eq!(r.len(), 3, "trois défauts, trois reproches : {r:?}");
        assert!(Manifeste::gabarit("a", "b").reproches().is_empty());
    }

    #[test]
    fn un_chemin_hors_data_est_signale_car_il_ne_serait_jamais_charge() {
        let dir = std::env::temp_dir().join(format!("nie-manif-arbo-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("data/common/gamedata")).expect("arborescence");
        std::fs::write(dir.join("data/common/gamedata/a.cfg.bin"), b"a").expect("asset");
        std::fs::write(dir.join(NOM_MANIFESTE), b"{}").expect("manifeste");
        std::fs::write(dir.join("README.md"), b"doc").expect("readme");
        // Le piège réel : un dossier de travail à noms aplatis. Rien ne le distingue d'un mod
        // valide à l'œil, et `pack_mod` n'y verrait aucune entrée à basculer.
        std::fs::write(dir.join("data_common_gamedata_b.cfg.bin"), b"b").expect("aplati");

        let fautifs = valider_arborescence(&dir).expect("validation");
        assert_eq!(
            fautifs,
            vec!["data_common_gamedata_b.cfg.bin"],
            "manifeste et README tolérés"
        );

        let assets = fichiers(&dir).expect("fichiers");
        assert_eq!(assets.len(), 1, "seuls les chemins VFS sont des assets");
        assert_eq!(assets[0].vfs, "data/common/gamedata/a.cfg.bin");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn les_dependances_passent_avant_leurs_dependants() {
        // `haut` dépend de `bas` : `bas` doit s'appliquer d'abord pour que `haut` l'écrase.
        let mods = vec![m("haut", 0, &["bas"]), m("bas", 0, &[])];
        let ordre = ordonner(&mods).expect("ordonnancement");
        let noms: Vec<&str> = ordre.iter().map(|&i| mods[i].nom.as_str()).collect();
        assert_eq!(noms, vec!["bas", "haut"]);
    }

    #[test]
    fn a_egalite_de_dependances_la_priorite_departage() {
        let mods = vec![
            m("fort", 10, &[]),
            m("faible", -5, &[]),
            m("neutre", 0, &[]),
        ];
        let ordre = ordonner(&mods).expect("ordonnancement");
        let noms: Vec<&str> = ordre.iter().map(|&i| mods[i].nom.as_str()).collect();
        // Ordre d'application croissant : le plus prioritaire s'applique en dernier, donc gagne.
        assert_eq!(noms, vec!["faible", "neutre", "fort"]);
    }

    #[test]
    fn un_cycle_est_refuse_plutot_que_de_boucler() {
        let mods = vec![m("a", 0, &["b"]), m("b", 0, &["a"])];
        let e = ordonner(&mods).expect_err("cycle");
        assert!(e.contains("cycle"), "{e}");
    }

    #[test]
    fn une_dependance_absente_ou_trop_ancienne_est_nommee() {
        let mods = vec![m("seul", 0, &["fantome"])];
        assert!(ordonner(&mods).expect_err("absente").contains("fantome"));

        let mut base = m("base", 0, &[]);
        base.version = "1.0.0".to_string();
        let mut haut = m("haut", 0, &[]);
        haut.dependances = vec![Dependance {
            nom: "base".to_string(),
            version_min: Some("2.0.0".to_string()),
        }];
        let e = ordonner(&[haut, base]).expect_err("trop ancienne");
        assert!(e.contains(">= 2.0.0"), "{e}");
    }

    #[test]
    fn deux_mods_de_meme_nom_sont_refuses() {
        let mods = vec![m("doublon", 0, &[]), m("doublon", 1, &[])];
        assert!(ordonner(&mods).expect_err("doublon").contains("doublon"));
    }
}
