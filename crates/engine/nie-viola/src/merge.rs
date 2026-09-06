//! **Merge** — fusionne plusieurs mods en un seul.
//!
//! # Le vrai manque de l'amont
//!
//! Viola (C#) et son port C++ fusionnent **au fichier** : à chemin égal, un mod gagne et l'autre
//! est jeté en entier. Or les données du jeu tiennent dans un petit nombre de très gros
//! `.cfg.bin` — un mod qui retouche un personnage et un mod qui retouche une technique éditent
//! le *même* `chara_base.cfg.bin`. Au fichier, ces deux mods sont **incompatibles**, alors qu'ils
//! ne se marchent pas dessus.
//!
//! Ce module ajoute une fusion **au champ**, possible uniquement parce que le format est compris
//! (`nie_formats::cfgbin`, T2B et RDBN, issus du reverse de `nie.exe`) : on compare chaque valeur
//! au **vanilla**, on ne retient que ce que chaque mod a réellement changé, et on ne déclare
//! conflit que si deux mods changent la *même* valeur en des choses différentes.
//!
//! C'est une fusion à trois points, exactement comme un `git merge` : sans la base commune, on ne
//! peut pas distinguer « ce mod a changé cette valeur » de « ce mod a recopié la valeur d'origine ».
//! Sans vanilla disponible, on retombe donc honnêtement sur la fusion au fichier.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nie_formats::cfgbin::{self, CfgEntry, RdbnList, Value};

use crate::enumerer;

/// Stratégie de résolution des chemins présents dans plusieurs mods.
pub enum MergeStrategy<'a> {
    /// Comportement de l'amont : le mod le plus prioritaire emporte le fichier entier.
    Fichier,
    /// Fusion au champ pour les `.cfg.bin`, avec repli au fichier si la base vanilla manque ou
    /// si les structures ne se correspondent pas.
    ///
    /// Le résolveur reçoit un chemin relatif normalisé (`data/…`) et rend les octets **vanilla**
    /// correspondants. Le brancher sur le VFS du jeu évite d'exiger un dump complet.
    Semantique(&'a (dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync)),
}

/// Un chemin que plusieurs mods se disputent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflit {
    /// Chemin relatif concerné.
    pub chemin: String,
    /// Rangs des mods qui fournissent ce chemin (0 = le plus prioritaire).
    pub rangs: Vec<usize>,
    /// Valeurs réellement fusionnées sans désaccord.
    pub champs_fusionnes: usize,
    /// Valeurs que deux mods changent différemment — tranchées par la priorité.
    pub champs_en_desaccord: usize,
    /// Pourquoi la fusion au champ n'a pas pu s'appliquer, quand c'est le cas.
    pub repli: Option<String>,
}

/// Bilan d'un [`merge_dirs`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MergeReport {
    /// Fichiers écrits dans la sortie.
    pub copies: usize,
    /// Fichiers reconstruits par fusion au champ.
    pub fusionnes: usize,
    /// Chemins fournis par plusieurs mods.
    pub conflits: Vec<Conflit>,
}

/// Compare deux valeurs T2B et RDBN sans se soucier du type exact.
fn t2b_egal(a: &Value, b: &Value) -> bool {
    a == b
}

/// Fusion à trois points d'un arbre T2B, en pas à pas sur la structure.
///
/// Renvoie `None` si les structures ne se correspondent pas — un mod qui ajoute ou retire des
/// entrées sort du cadre d'une fusion par valeur, et deviner y ferait plus de mal que de bien.
fn fusion_t2b(
    base: &[CfgEntry],
    versions: &[&[CfgEntry]],
    fusionnes: &mut usize,
    desaccords: &mut usize,
) -> Option<Vec<CfgEntry>> {
    let mut out = Vec::with_capacity(base.len());
    for (i, entree) in base.iter().enumerate() {
        let mut fusionnee = entree.clone();

        // Chaque version doit présenter la même entrée, avec le même nombre de variables.
        let mut vues: Vec<&CfgEntry> = Vec::with_capacity(versions.len());
        for v in versions {
            let e = v.get(i)?;
            if e.name != entree.name
                || e.variables.len() != entree.variables.len()
                || e.children.len() != entree.children.len()
            {
                return None;
            }
            vues.push(e);
        }

        for (k, valeur_base) in entree.variables.iter().enumerate() {
            // Ne retenir que les versions qui s'écartent réellement du vanilla.
            let modifs: Vec<&Value> = vues
                .iter()
                .map(|e| &e.variables[k])
                .filter(|v| !t2b_egal(v, valeur_base))
                .collect();
            match modifs.len() {
                0 => {}
                _ => {
                    // Les versions sont ordonnées par priorité décroissante : la première gagne.
                    let gagnante = modifs[0];
                    if modifs.iter().all(|m| t2b_egal(m, gagnante)) {
                        *fusionnes += 1;
                    } else {
                        *desaccords += 1;
                    }
                    fusionnee.variables[k] = gagnante.clone();
                }
            }
        }

        let enfants: Vec<&[CfgEntry]> = vues.iter().map(|e| e.children.as_slice()).collect();
        fusionnee.children = fusion_t2b(&entree.children, &enfants, fusionnes, desaccords)?;
        out.push(fusionnee);
    }
    Some(out)
}

/// Fusion à trois points de listes RDBN, ligne à ligne et champ à champ.
fn fusion_rdbn(
    base: &[RdbnList],
    versions: &[&[RdbnList]],
    fusionnes: &mut usize,
    desaccords: &mut usize,
) -> Option<Vec<RdbnList>> {
    let mut out = base.to_vec();
    for (l, liste) in base.iter().enumerate() {
        for v in versions {
            let vl = v.get(l)?;
            if vl.name != liste.name || vl.rows.len() != liste.rows.len() {
                return None;
            }
        }
        for (r, ligne) in liste.rows.iter().enumerate() {
            for (c, (nom, valeur_base)) in ligne.fields.iter().enumerate() {
                let mut gagnante = None;
                let mut accord = true;
                for v in versions {
                    let champ = v[l].rows[r].fields.get(c)?;
                    if &champ.0 != nom {
                        return None;
                    }
                    if &champ.1 != valeur_base {
                        match &gagnante {
                            None => gagnante = Some(champ.1.clone()),
                            Some(g) if g != &champ.1 => accord = false,
                            Some(_) => {}
                        }
                    }
                }
                if let Some(g) = gagnante {
                    if accord {
                        *fusionnes += 1;
                    } else {
                        *desaccords += 1;
                    }
                    out[l].rows[r].fields[c].1 = g;
                }
            }
        }
    }
    Some(out)
}

/// Tente la fusion au champ d'un `.cfg.bin`, les versions étant données par priorité décroissante.
///
/// Renvoie les octets fusionnés et le décompte, ou l'explication du repli.
fn fusionner_cfgbin(
    vanilla: &[u8],
    versions: &[Vec<u8>],
) -> Result<(Vec<u8>, usize, usize), String> {
    if cfgbin::is_rdbn(vanilla) {
        let base_rdbn =
            cfgbin::parse(vanilla).map_err(|e| format!("vanilla RDBN illisible : {e}"))?;
        let base = cfgbin::read_values(&base_rdbn, vanilla);
        let mut decodees = Vec::with_capacity(versions.len());
        for v in versions {
            let d = cfgbin::parse(v).map_err(|e| format!("version RDBN illisible : {e}"))?;
            decodees.push(cfgbin::read_values(&d, v));
        }
        let refs: Vec<&[RdbnList]> = decodees.iter().map(Vec::as_slice).collect();
        let (mut f, mut d) = (0, 0);
        let fusion = fusion_rdbn(&base, &refs, &mut f, &mut d)
            .ok_or_else(|| "structures RDBN divergentes (lignes ou champs ajoutés)".to_string())?;
        let octets = cfgbin::encode_rdbn(&fusion)?;
        Ok((octets, f, d))
    } else {
        let base =
            cfgbin::cfgbin_parse(vanilla).map_err(|e| format!("vanilla T2B illisible : {e}"))?;
        let mut decodees = Vec::with_capacity(versions.len());
        for v in versions {
            decodees
                .push(cfgbin::cfgbin_parse(v).map_err(|e| format!("version T2B illisible : {e}"))?);
        }
        let refs: Vec<&[CfgEntry]> = decodees.iter().map(|d| d.entries.as_slice()).collect();
        let (mut f, mut d) = (0, 0);
        let fusion = fusion_t2b(&base.entries, &refs, &mut f, &mut d).ok_or_else(|| {
            "structures T2B divergentes (entrées ajoutées ou retirées)".to_string()
        })?;
        Ok((cfgbin::encode_t2b(&fusion), f, d))
    }
}

/// Fusionne `sources` (en **priorité décroissante** : le premier gagne) vers `sortie`.
///
/// # Errors
/// Si une source est illisible ou si une écriture échoue.
pub fn merge_dirs(
    sources: &[PathBuf],
    sortie: &Path,
    strategie: &MergeStrategy<'_>,
) -> Result<MergeReport, String> {
    // Chemin relatif → (rang du mod, chemin absolu), rangs croissants = priorité décroissante.
    let mut par_chemin: BTreeMap<String, Vec<(usize, PathBuf)>> = BTreeMap::new();
    for (rang, source) in sources.iter().enumerate() {
        for (absolu, rel) in enumerer(source)? {
            par_chemin.entry(rel).or_default().push((rang, absolu));
        }
    }

    let mut rapport = MergeReport::default();
    for (rel, mut versions) in par_chemin {
        versions.sort_by_key(|(rang, _)| *rang);
        let dest = sortie.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
        }

        if versions.len() == 1 {
            std::fs::copy(&versions[0].1, &dest)
                .map_err(|e| format!("{} : {e}", dest.display()))?;
            rapport.copies += 1;
            continue;
        }

        let rangs: Vec<usize> = versions.iter().map(|(r, _)| *r).collect();
        let mut conflit = Conflit {
            chemin: rel.clone(),
            rangs,
            champs_fusionnes: 0,
            champs_en_desaccord: 0,
            repli: None,
        };

        // La fusion au champ ne vaut que pour les `.cfg.bin` et seulement avec une base vanilla.
        let tentee = match strategie {
            MergeStrategy::Semantique(resolveur) if rel.ends_with(".cfg.bin") => {
                match resolveur(&rel) {
                    Some(vanilla) => {
                        let mut octets = Vec::with_capacity(versions.len());
                        let mut erreur = None;
                        for (_, p) in &versions {
                            match std::fs::read(p) {
                                Ok(o) => octets.push(o),
                                Err(e) => {
                                    erreur = Some(format!("{} : {e}", p.display()));
                                    break;
                                }
                            }
                        }
                        match erreur {
                            Some(e) => Err(e),
                            None => fusionner_cfgbin(&vanilla, &octets),
                        }
                    }
                    None => Err("aucune version vanilla de ce fichier".to_string()),
                }
            }
            MergeStrategy::Semantique(_) => Err("format non fusionnable au champ".to_string()),
            MergeStrategy::Fichier => Err("fusion au fichier demandée".to_string()),
        };

        match tentee {
            Ok((octets, f, d)) => {
                std::fs::write(&dest, &octets).map_err(|e| format!("{} : {e}", dest.display()))?;
                conflit.champs_fusionnes = f;
                conflit.champs_en_desaccord = d;
                rapport.fusionnes += 1;
            }
            Err(raison) => {
                // Repli explicite : le mod prioritaire emporte le fichier, et on dit pourquoi.
                std::fs::copy(&versions[0].1, &dest)
                    .map_err(|e| format!("{} : {e}", dest.display()))?;
                conflit.repli = Some(raison);
                rapport.copies += 1;
            }
        }
        rapport.conflits.push(conflit);
    }
    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entree(nom: &str, vars: &[i32]) -> CfgEntry {
        CfgEntry {
            name: nom.to_string(),
            variables: vars.iter().map(|v| Value::Int(*v)).collect(),
            children: Vec::new(),
        }
    }

    #[test]
    fn deux_mods_touchant_des_champs_differents_fusionnent() {
        // Le cas que l'amont ne sait pas traiter : mod A change la 1re valeur, mod B la 3e.
        let base = vec![entree("PERSO", &[1, 2, 3])];
        let a = vec![entree("PERSO", &[99, 2, 3])];
        let b = vec![entree("PERSO", &[1, 2, 77])];
        let (mut f, mut d) = (0, 0);
        let out = fusion_t2b(&base, &[&a, &b], &mut f, &mut d).expect("structures identiques");
        assert_eq!(
            out[0].variables,
            vec![Value::Int(99), Value::Int(2), Value::Int(77)]
        );
        assert_eq!((f, d), (2, 0), "deux changements retenus, aucun désaccord");
    }

    #[test]
    fn un_desaccord_reel_est_tranche_par_la_priorite_et_compte() {
        let base = vec![entree("PERSO", &[1])];
        let a = vec![entree("PERSO", &[10])];
        let b = vec![entree("PERSO", &[20])];
        let (mut f, mut d) = (0, 0);
        let out = fusion_t2b(&base, &[&a, &b], &mut f, &mut d).expect("structures identiques");
        assert_eq!(
            out[0].variables,
            vec![Value::Int(10)],
            "le plus prioritaire gagne"
        );
        assert_eq!((f, d), (0, 1), "et le désaccord est signalé, pas masqué");
    }

    #[test]
    fn une_valeur_recopiee_du_vanilla_n_ecrase_pas_le_changement_de_l_autre() {
        // Sans base à trois points, le mod B (identique au vanilla) écraserait le mod A.
        let base = vec![entree("PERSO", &[1])];
        let a = vec![entree("PERSO", &[42])];
        let b = vec![entree("PERSO", &[1])];
        let (mut f, mut d) = (0, 0);
        let out = fusion_t2b(&base, &[&b, &a], &mut f, &mut d).expect("structures identiques");
        assert_eq!(
            out[0].variables,
            vec![Value::Int(42)],
            "seul un vrai changement compte"
        );
        assert_eq!((f, d), (1, 0));
    }

    #[test]
    fn une_structure_divergente_refuse_la_fusion_plutot_que_de_deviner() {
        let base = vec![entree("PERSO", &[1, 2])];
        let a = vec![entree("PERSO", &[1, 2, 3])];
        let (mut f, mut d) = (0, 0);
        assert!(fusion_t2b(&base, &[&a], &mut f, &mut d).is_none());
    }
}
