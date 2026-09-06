//! Projection des builds BASARA et notation de la synergie d'équipe.
//!
//! Portage de `packages/inagle/src/analysis/optimizer.ts` (495 l.).
//!
//! # Ce qui est porté, et ce qui ne l'est pas
//!
//! `calculateTeamSynergy` mêle deux choses : **résoudre** onze joueurs
//! (`createCharactersAPI` / `createBasaraAPI` / `createSkillsAPI`, qui lisent le
//! disque) puis **noter** le résultat. Seule la notation est du calcul ; c'est
//! elle qui est portée, sur des joueurs déjà résolus ([`JoueurCharge`]). La
//! résolution reste hors périmètre — c'est de l'I/O, et la carte
//! `docs/inagle/02-sortie-et-domaines.md` le dit : « le calcul est isolable,
//! l'assemblage non ».
//!
//! Ce découpage n'invente rien : le TS construit lui-même un enregistrement
//! intermédiaire (`loadedPlayers`, `optimizer.ts:305-313`) avec exactement les
//! champs de [`JoueurCharge`]. Les deux prédicats purs de la boucle de
//! résolution — « cette technique est-elle un passif » et « quel nom lui
//! donner » — sont portés séparément ([`est_passif`], [`nom_passif`]).
//!
//! # Fidélité
//!
//! - Les cinq éléments sont comptés dans l'ordre `Wind, Forest, Fire, Mountain,
//!   Void` et le dominant est déterminé par un `>` strict : à égalité, le
//!   premier de cet ordre gagne. Changer l'ordre changerait le résultat sans
//!   qu'aucune valeur ne paraisse fausse.
//! - Un élément inconnu n'est compté nulle part (`optimizer.ts:277`).
//! - Les libellés et les recommandations sont verbatim, emoji compris : ce sont
//!   des données de sortie, pas de la prose.

use crate::stats::StatBlock;

/// Nombre de types de build BASARA (0 à 5).
pub const NB_BUILDS_BASARA: u8 = 6;

/// Ordre d'itération des éléments comptés, verbatim de `elementCounts`
/// (`optimizer.ts:194-200`). Le premier maximum dans CET ordre est déclaré
/// dominant.
pub const ORDRE_ELEMENTS: [&str; 5] = ["Wind", "Forest", "Fire", "Mountain", "Void"];

/// Projection d'un build BASARA : nom, description et multiplicateurs.
///
/// Reproduit une entrée de `BASARA_BUILD_PROJECTIONS` (`optimizer.ts:48-118`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionBuild {
    /// Nom affiché du build.
    pub nom: &'static str,
    /// Description française du build.
    pub description: &'static str,
    /// Multiplicateurs par statistique, dans l'ordre de [`StatBlock::as_array`]
    /// (`[Kc, Cr, Tc, Pr, Ps, Ag, It]`). `None` = statistique inchangée.
    pub multiplicateurs: [Option<f64>; 7],
}

/// Les 6 builds BASARA, indexés par leur `buildType` (0 à 5).
///
/// Portage verbatim de `BASARA_BUILD_PROJECTIONS`. Les clés du TS
/// (`kick, control, technique, pressure, physical, agility, intelligence`) sont
/// replacées à leur position dans [`StatBlock`] : `Pr` porte *physical*, `Ps`
/// porte *pressure*. Les intervertir renverrait des builds plausibles et faux.
pub const BUILDS_BASARA: [ProjectionBuild; NB_BUILDS_BASARA as usize] = [
    ProjectionBuild {
        nom: "Polyvalent (All-Rounder)",
        description: "Statistiques équilibrées, aucune pénalité.",
        multiplicateurs: [None, None, None, None, None, None, None],
    },
    ProjectionBuild {
        nom: "Attaquant (Striker)",
        description: "Augmente la Frappe et la Vitesse, diminue le Contrôle et la Pression.",
        multiplicateurs: [
            Some(1.25), // kick
            Some(0.9),  // control
            None,       // technique
            Some(1.1),  // physical
            Some(0.8),  // pressure
            Some(1.15), // agility
            None,       // intelligence
        ],
    },
    ProjectionBuild {
        nom: "Muraille (Defender / Wall)",
        description: "Augmente la Pression et le Physique, diminue la Frappe et la Vitesse.",
        multiplicateurs: [
            Some(0.7),
            None,
            None,
            Some(1.2),
            Some(1.25),
            Some(0.9),
            Some(1.1),
        ],
    },
    ProjectionBuild {
        nom: "Meneur (Playmaker)",
        description: "Augmente le Contrôle et la Technique, diminue la Frappe et la Pression.",
        multiplicateurs: [
            Some(0.9),
            Some(1.25),
            Some(1.2),
            None,
            Some(0.9),
            None,
            Some(1.15),
        ],
    },
    ProjectionBuild {
        nom: "Voltigeur (Speedster)",
        description: "Augmente drastiquement la Vitesse, diminue la Pression.",
        multiplicateurs: [
            Some(0.9),
            Some(1.1),
            Some(1.1),
            None,
            Some(0.8),
            Some(1.3),
            None,
        ],
    },
    ProjectionBuild {
        nom: "Gardien Infranchissable (GK Wall)",
        description: "Augmente la Technique et le Physique, diminue drastiquement la Frappe.",
        multiplicateurs: [
            Some(0.5),
            Some(0.8),
            Some(1.3),
            Some(1.15),
            Some(1.15),
            None,
            None,
        ],
    },
];

/// Un build projeté et sa puissance totale.
///
/// Reproduit `OptimizedBuildResult` (`optimizer.ts:26-31`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultatBuild {
    /// Type de build (0 à 5).
    pub type_build: u8,
    /// Nom du build.
    pub nom: &'static str,
    /// Statistiques projetées.
    pub stats: StatBlock,
    /// Somme des statistiques projetées.
    pub puissance_totale: u32,
}

/// Projette les statistiques niveau 99 d'un BASARA sous un build donné.
///
/// Reproduit `projectBasaraBuildStats` (`optimizer.ts:121-143`). Un
/// `type_build` hors de 0-5 retombe sur le build 0 (polyvalent), comme le
/// `|| BASARA_BUILD_PROJECTIONS[0]` du TS.
///
/// L'arrondi est celui de `Math.round` : les statistiques étant positives,
/// `f64::round` (moitié à l'opposé de zéro) lui est identique.
#[must_use]
pub fn projeter_stats_build(base: StatBlock, type_build: u8) -> StatBlock {
    let projection = BUILDS_BASARA
        .get(type_build as usize)
        .unwrap_or(&BUILDS_BASARA[0]);

    let base_arr = base.as_array();
    let mut sortie = base_arr;
    for (i, multiplicateur) in projection.multiplicateurs.iter().enumerate() {
        if let Some(m) = multiplicateur {
            let valeur = (f64::from(base_arr[i]) * m).round();
            // Bornage explicite : une stat projetée ne peut ni être négative
            // (multiplicateurs > 0) ni dépasser u16 (max réel ≈ 261 × 1,3).
            sortie[i] = valeur.clamp(0.0, f64::from(u16::MAX)) as u16;
        }
    }
    StatBlock::from_array(sortie)
}

/// Calcule les 6 builds d'un BASARA, classés par puissance totale décroissante.
///
/// Reproduit `getOptimizedBasaraBuilds` (`optimizer.ts:148-171`). Le tri est
/// **stable** — comme `Array.prototype.sort` en JavaScript moderne : à égalité
/// de puissance, l'ordre des types de build est conservé.
#[must_use]
pub fn builds_basara_classes(base: StatBlock) -> Vec<ResultatBuild> {
    let mut resultats: Vec<ResultatBuild> = (0..NB_BUILDS_BASARA)
        .map(|type_build| {
            let stats = projeter_stats_build(base, type_build);
            ResultatBuild {
                type_build,
                nom: BUILDS_BASARA[type_build as usize].nom,
                stats,
                puissance_totale: stats.total(),
            }
        })
        .collect();
    resultats.sort_by_key(|r| std::cmp::Reverse(r.puissance_totale));
    resultats
}

/// Une technique est-elle traitée comme un passif par l'analyseur de synergie ?
///
/// Prédicat pur extrait de `calculateTeamSynergy` (`optimizer.ts:388-393`) :
/// catégorie anglaise `Passive`, **ou** catégorie française `Passif`, **ou**
/// nom (FR ou EN) contenant `boost`. La chaîne vide est traitée comme absente,
/// comme en JavaScript où `""` est falsy.
#[must_use]
pub fn est_passif(
    categorie_en: Option<&str>,
    categorie_fr: Option<&str>,
    nom_fr: Option<&str>,
    nom_en: Option<&str>,
) -> bool {
    let contient_boost = |s: Option<&str>| {
        s.is_some_and(|v| !v.is_empty() && v.to_lowercase().contains("boost"))
    };
    categorie_en == Some("Passive")
        || categorie_fr == Some("Passif")
        || contient_boost(nom_fr)
        || contient_boost(nom_en)
}

/// Nom retenu pour un passif : FR, sinon EN, sinon `Passif`.
///
/// Reproduit `skillMeta.name_FR || skillMeta.name_EN || "Passif"`
/// (`optimizer.ts:396`) — chaîne vide comprise, qui bascule sur le suivant.
#[must_use]
pub fn nom_passif(nom_fr: Option<&str>, nom_en: Option<&str>) -> String {
    fn non_vide(s: Option<&str>) -> Option<&str> {
        s.filter(|v| !v.is_empty())
    }
    non_vide(nom_fr)
        .or_else(|| non_vide(nom_en))
        .unwrap_or("Passif")
        .to_string()
}

/// Un joueur déjà résolu, prêt à être noté.
///
/// Correspond à l'enregistrement intermédiaire `loadedPlayers`
/// (`optimizer.ts:305-313`). Pour un BASARA, `stats` est le résultat de
/// [`projeter_stats_build`] ; pour un personnage ordinaire, ses stats lv99.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JoueurCharge {
    /// Nom affiché du joueur.
    pub nom: String,
    /// Élément (`Wind`, `Forest`, `Fire`, `Mountain`, `Void`, ou inconnu).
    pub element: String,
    /// Poste naturel du joueur.
    pub position_naturelle: String,
    /// Poste occupé sur le terrain.
    pub position_terrain: String,
    /// Statistiques retenues pour la puissance brute.
    pub stats: StatBlock,
    /// Noms des passifs déjà filtrés par [`est_passif`] et nommés par
    /// [`nom_passif`].
    pub passifs: Vec<String>,
}

/// L'entraîneur, s'il a été résolu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entraineur {
    /// Nom affiché.
    pub nom: String,
    /// Élément.
    pub element: String,
}

/// Une synergie déclenchée par la composition de l'équipe.
///
/// Reproduit `ActiveSynergy` (`optimizer.ts:18-23`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SynergieActive {
    /// Nom de la synergie.
    pub nom: String,
    /// Description française.
    pub description: String,
    /// Famille de bonus — sert au plafonnement des passifs à 20 points.
    pub type_bonus: String,
    /// Valeur du bonus.
    pub valeur: i32,
}

/// Rapport de synergie d'une équipe.
///
/// Reproduit `TeamSynergyReport` (`optimizer.ts:39-44`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RapportSynergie {
    /// Somme des statistiques de tous les joueurs.
    pub puissance_totale: u32,
    /// Score de synergie, borné à 100.
    pub score_synergie: i32,
    /// Synergies déclenchées, dans l'ordre où le TS les empile.
    pub synergies_actives: Vec<SynergieActive>,
    /// Recommandations, dans l'ordre où le TS les empile.
    pub recommandations: Vec<String>,
}

/// Compteur ordonné : conserve l'ordre de première apparition, comme un objet JS.
fn compter_ordonne(valeurs: impl Iterator<Item = String>) -> Vec<(String, i32)> {
    let mut compteurs: Vec<(String, i32)> = Vec::new();
    for v in valeurs {
        if let Some(entree) = compteurs.iter_mut().find(|(nom, _)| *nom == v) {
            entree.1 += 1;
        } else {
            compteurs.push((v, 1));
        }
    }
    compteurs
}

/// Traduit un élément anglais vers le nom français utilisé par les libellés.
///
/// Reproduit `localizedElement` (`optimizer.ts:344-352`) : tout ce qui n'est ni
/// `Fire`, ni `Wind`, ni `Forest` y devient « Montagne » — la branche `Void`
/// étant déjà exclue par la garde qui précède.
fn element_en_francais(element: &str) -> &'static str {
    match element {
        "Fire" => "Feu",
        "Wind" => "Vent",
        "Forest" => "Forêt",
        _ => "Montagne",
    }
}

/// Note la synergie d'une équipe déjà résolue.
///
/// Reproduit la moitié « notation » de `calculateTeamSynergy`
/// (`optimizer.ts:176-495`), dans son ordre exact :
/// 1. bonus de placement (≥ 90 % → 15 pts, ≥ 70 % → 8 pts) ;
/// 2. cohésion élémentaire (élément dominant ≥ 55 %, hors `Void`) ;
/// 3. harmonie avec l'entraîneur ;
/// 4. passifs d'équipe (élémentaires, et défensif via le nombre de DF) ;
/// 5. score final : placement ×40, cohésion 30/15, entraîneur 10, passifs
///    plafonnés à 20, le tout borné à 100.
///
/// Une équipe vide rend un rapport à zéro, sans division par zéro (le TS teste
/// `loadedPlayers.length > 0`).
///
/// # Asymétrie portée telle quelle
///
/// La garde `dominantElement !== "Void"` protège la **synergie** « Cohésion
/// Élémentaire » (`optimizer.ts:341`) mais **pas** les 30 points de **score**
/// (`optimizer.ts:465`, qui ne teste que `elementRatio >= 0.55`). Une équipe
/// entièrement `Void` n'affiche donc aucune cohésion et encaisse quand même son
/// bonus. C'est reproduit à l'identique, et figé par un test.
#[must_use]
pub fn calculer_synergie_equipe(
    joueurs: &[JoueurCharge],
    entraineur: Option<&Entraineur>,
) -> RapportSynergie {
    let mut synergies_actives: Vec<SynergieActive> = Vec::new();
    let mut recommandations: Vec<String> = Vec::new();

    let mut puissance_totale: u32 = 0;
    let mut nb_positions_correctes = 0usize;
    let mut comptes_elements = [0usize; ORDRE_ELEMENTS.len()];

    for joueur in joueurs {
        if let Some(i) = ORDRE_ELEMENTS.iter().position(|e| *e == joueur.element) {
            comptes_elements[i] += 1;
        }

        if joueur.position_naturelle == joueur.position_terrain {
            nb_positions_correctes += 1;
        } else {
            recommandations.push(format!(
                "⚠️ {} ({}) joue en tant que {}. Pensez à le repositionner pour maximiser son efficacité.",
                joueur.nom, joueur.position_naturelle, joueur.position_terrain
            ));
        }

        puissance_totale += joueur.stats.total();
    }

    let nb_joueurs = joueurs.len();
    let ratio_positions = if nb_joueurs > 0 {
        nb_positions_correctes as f64 / nb_joueurs as f64
    } else {
        0.0
    };

    // 1. Synergie de placement.
    if ratio_positions >= 0.9 {
        synergies_actives.push(SynergieActive {
            nom: "Discipline Tactique".to_string(),
            description: "Plus de 90% des joueurs sont placés à leur position naturelle."
                .to_string(),
            type_bonus: "Statistiques de placement".to_string(),
            valeur: 15,
        });
    } else if ratio_positions >= 0.7 {
        synergies_actives.push(SynergieActive {
            nom: "Coordination de Base".to_string(),
            description: "Plus de 70% des joueurs sont à leur position naturelle.".to_string(),
            type_bonus: "Statistiques de placement".to_string(),
            valeur: 8,
        });
    }

    // 2. Cohésion élémentaire — `>` strict : à égalité, ORDRE_ELEMENTS tranche.
    let mut element_dominant = "Void";
    let mut compte_dominant = 0usize;
    for (i, nom) in ORDRE_ELEMENTS.iter().enumerate() {
        if comptes_elements[i] > compte_dominant {
            compte_dominant = comptes_elements[i];
            element_dominant = nom;
        }
    }

    let ratio_element = if nb_joueurs > 0 {
        compte_dominant as f64 / nb_joueurs as f64
    } else {
        0.0
    };

    if ratio_element >= 0.55 && element_dominant != "Void" {
        let element_fr = element_en_francais(element_dominant);
        synergies_actives.push(SynergieActive {
            nom: format!("Cohésion Élémentaire ({element_fr})"),
            description: format!(
                "Dominance forte de l'élément {element_fr} (>= 55% de l'équipe)."
            ),
            type_bonus: "Multiplicateur de Hissatsu".to_string(),
            valeur: 12,
        });
    }

    // 3. Harmonie avec l'entraîneur.
    if let Some(coach) = entraineur {
        if coach.element == element_dominant && element_dominant != "Void" {
            synergies_actives.push(SynergieActive {
                nom: "Harmonie Tactique".to_string(),
                description: format!(
                    "L'élément de l'entraîneur {} ({}) correspond à la dominance de l'équipe.",
                    coach.nom, coach.element
                ),
                type_bonus: "Boost Global de Puissance".to_string(),
                valeur: 10,
            });
        } else {
            recommandations.push(format!(
                "💡 L'entraîneur {} est d'élément {}. Envisagez un coach d'élément {} pour activer l'Harmonie Tactique.",
                coach.nom, coach.element, element_dominant
            ));
        }
    }

    // 4. Passifs d'équipe.
    let compte = |nom: &str| -> usize {
        ORDRE_ELEMENTS
            .iter()
            .position(|e| *e == nom)
            .map_or(0, |i| comptes_elements[i])
    };
    let nb_df = joueurs
        .iter()
        .filter(|j| j.position_terrain == "DF")
        .count();

    let compteurs = compter_ordonne(joueurs.iter().flat_map(|j| j.passifs.iter().cloned()));
    for (nom, n) in &compteurs {
        let minuscule = nom.to_lowercase();
        let n = *n;
        let bonus_element = |nom_synergie: &str, element: &str, element_fr: &str| {
            let effectif = compte(element);
            (effectif >= 3).then(|| SynergieActive {
                nom: format!("{nom_synergie} (x{n})"),
                description: format!(
                    "Boost de puissance de {}% sur {effectif} joueurs {element_fr}.",
                    5 * n
                ),
                type_bonus: format!("Boost Élémentaire {element_fr}"),
                valeur: 5 * n,
            })
        };

        let synergie = if minuscule.contains("feu") || minuscule.contains("fire") {
            bonus_element("Fureur du Feu", "Fire", "Feu")
        } else if minuscule.contains("vent") || minuscule.contains("wind") {
            bonus_element("Souffle du Vent", "Wind", "Vent")
        } else if minuscule.contains("forêt") || minuscule.contains("forest") {
            bonus_element("Emprise de la Forêt", "Forest", "Forêt")
        } else if minuscule.contains("montagne") || minuscule.contains("mountain") {
            bonus_element("Force de la Montagne", "Mountain", "Montagne")
        } else if minuscule.contains("mur")
            || minuscule.contains("wall")
            || minuscule.contains("défense")
        {
            (nb_df >= 3).then(|| SynergieActive {
                nom: "Mur d'Acier Tactique".to_string(),
                description: format!(
                    "Augmentation de la défense collective (+{}%) pour les {nb_df} défenseurs.",
                    6 * n
                ),
                type_bonus: "Boost Positionnel DF".to_string(),
                valeur: 6 * n,
            })
        } else {
            None
        };

        if let Some(s) = synergie {
            synergies_actives.push(s);
        }
    }

    // 5. Score final.
    let mut score_synergie = (ratio_positions * 40.0).round() as i32;

    if ratio_element >= 0.55 {
        score_synergie += 30;
    } else if ratio_element >= 0.35 {
        score_synergie += 15;
    }

    if synergies_actives
        .iter()
        .any(|s| s.nom == "Harmonie Tactique")
    {
        score_synergie += 10;
    }

    let somme_passifs: i32 = synergies_actives
        .iter()
        .filter(|s| {
            s.type_bonus.starts_with("Boost Élémentaire")
                || s.type_bonus.starts_with("Boost Positionnel")
        })
        .map(|s| s.valeur)
        .sum();
    score_synergie += somme_passifs.min(20);

    if score_synergie < 50 {
        recommandations.push(
            "💡 L'équipe manque de cohésion élémentaire. Essayez de regrouper au moins 5 joueurs du même élément pour débloquer des bonus."
                .to_string(),
        );
    }

    RapportSynergie {
        puissance_totale,
        score_synergie: score_synergie.min(100),
        synergies_actives,
        recommandations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joueur(nom: &str, element: &str, naturelle: &str, terrain: &str) -> JoueurCharge {
        JoueurCharge {
            nom: nom.to_string(),
            element: element.to_string(),
            position_naturelle: naturelle.to_string(),
            position_terrain: terrain.to_string(),
            stats: StatBlock::from_array([10; 7]),
            passifs: Vec::new(),
        }
    }

    // ── Builds BASARA ───────────────────────────────────────────────────────

    #[test]
    fn build_0_ne_touche_a_rien() {
        let base = StatBlock::from_array([100, 100, 100, 100, 100, 100, 100]);
        assert_eq!(projeter_stats_build(base, 0), base);
    }

    #[test]
    fn build_striker_applique_les_multiplicateurs_aux_bonnes_stats() {
        let base = StatBlock::from_array([100, 100, 100, 100, 100, 100, 100]);
        let p = projeter_stats_build(base, 1);
        // kick ×1,25 ; control ×0,9 ; technique inchangée ; physical ×1,1 ;
        // pressure ×0,8 ; agility ×1,15 ; intelligence inchangée.
        assert_eq!(p.as_array(), [125, 90, 100, 110, 80, 115, 100]);
    }

    #[test]
    fn build_gk_wall_ecrase_la_frappe() {
        let base = StatBlock::from_array([200, 200, 200, 200, 200, 200, 200]);
        let p = projeter_stats_build(base, 5);
        assert_eq!(p.kc, 100, "kick ×0,5");
        assert_eq!(p.tc, 260, "technique ×1,3");
        assert_eq!(p.ag, 200, "agility inchangée");
    }

    #[test]
    fn multiplicateurs_visent_physical_et_pressure_dans_le_bon_ordre() {
        // Le piège : Pr = physical, Ps = pressure. Le build Muraille monte
        // pressure à 1,25 et physical à 1,2 — les intervertir donnerait un
        // résultat plausible et faux.
        let base = StatBlock::from_array([0, 0, 0, 100, 100, 0, 0]);
        let p = projeter_stats_build(base, 2);
        assert_eq!(p.pr, 120, "physical ×1,2");
        assert_eq!(p.ps, 125, "pressure ×1,25");
    }

    #[test]
    fn type_de_build_hors_bornes_retombe_sur_polyvalent() {
        let base = StatBlock::from_array([100; 7]);
        assert_eq!(projeter_stats_build(base, 99), base);
    }

    #[test]
    fn builds_classes_rendent_six_entrees_triees() {
        let base = StatBlock::from_array([100; 7]);
        let r = builds_basara_classes(base);
        assert_eq!(r.len(), 6);
        for paire in r.windows(2) {
            assert!(paire[0].puissance_totale >= paire[1].puissance_totale);
        }
        // 700 pour le polyvalent ; le classement doit donc être mené par un
        // build à somme de multiplicateurs > 7.
        assert!(r[0].puissance_totale >= 700);
    }

    #[test]
    fn tri_des_builds_est_stable_a_egalite() {
        // Stats nulles : les six builds valent 0, l'ordre 0..5 doit survivre.
        let r = builds_basara_classes(StatBlock::default());
        let types: Vec<u8> = r.iter().map(|b| b.type_build).collect();
        assert_eq!(types, [0, 1, 2, 3, 4, 5]);
    }

    // ── Prédicats de passif ─────────────────────────────────────────────────

    #[test]
    fn est_passif_reconnait_les_quatre_criteres() {
        assert!(est_passif(Some("Passive"), None, None, None));
        assert!(est_passif(None, Some("Passif"), None, None));
        assert!(est_passif(None, None, Some("Boost de Feu"), None));
        assert!(est_passif(None, None, None, Some("Fire BOOST")));
        assert!(!est_passif(Some("Shoot"), Some("Tir"), Some("Tornade"), None));
    }

    #[test]
    fn nom_passif_traite_la_chaine_vide_comme_absente() {
        assert_eq!(nom_passif(Some("Fureur"), Some("Fury")), "Fureur");
        assert_eq!(nom_passif(Some(""), Some("Fury")), "Fury");
        assert_eq!(nom_passif(None, None), "Passif");
        assert_eq!(nom_passif(Some(""), Some("")), "Passif");
    }

    // ── Synergie d'équipe ───────────────────────────────────────────────────

    #[test]
    fn equipe_vide_ne_divise_pas_par_zero() {
        let r = calculer_synergie_equipe(&[], None);
        assert_eq!(r.puissance_totale, 0);
        assert_eq!(r.score_synergie, 0);
        assert!(r.synergies_actives.is_empty());
        // Score < 50 → la recommandation générale tombe quand même.
        assert_eq!(r.recommandations.len(), 1);
    }

    #[test]
    fn tous_bien_places_declenche_discipline_tactique() {
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Void", "MF", "MF"))
            .collect();
        let r = calculer_synergie_equipe(&equipe, None);
        assert_eq!(r.synergies_actives[0].nom, "Discipline Tactique");
        assert_eq!(r.synergies_actives[0].valeur, 15);
        assert_eq!(r.puissance_totale, 700);
        // Placement 40 pts + 30 pts de ratio élémentaire : `Void` est exclu de
        // la SYNERGIE mais pas du SCORE (cf. asymétrie ci-dessous) → 70.
        assert_eq!(r.score_synergie, 70);
    }

    /// Asymétrie du TS, portée telle quelle : la garde `dominantElement !==
    /// "Void"` protège la synergie « Cohésion Élémentaire » (`optimizer.ts:341`)
    /// mais **pas** les 30 points de score (`optimizer.ts:465`, qui ne teste que
    /// `elementRatio >= 0.55`). Une équipe entièrement `Void` n'affiche donc
    /// aucune cohésion tout en encaissant son bonus. Si ce test rougit, c'est
    /// qu'une des deux gardes a bougé — c'est un arbitrage, pas un détail.
    #[test]
    fn void_est_exclu_de_la_synergie_mais_pas_du_score() {
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Void", "MF", "MF"))
            .collect();
        let r = calculer_synergie_equipe(&equipe, None);
        assert!(
            !r.synergies_actives
                .iter()
                .any(|s| s.nom.starts_with("Cohésion")),
            "aucune cohésion affichée pour Void"
        );
        assert_eq!(r.score_synergie, 70, "mais les 30 points sont bien comptés");
    }

    #[test]
    fn ratio_de_placement_entre_70_et_90_donne_coordination_de_base() {
        let mut equipe: Vec<JoueurCharge> = (0..8)
            .map(|i| joueur(&format!("J{i}"), "Void", "MF", "MF"))
            .collect();
        equipe.push(joueur("Hors poste 1", "Void", "FW", "GK"));
        equipe.push(joueur("Hors poste 2", "Void", "FW", "GK"));
        let r = calculer_synergie_equipe(&equipe, None);
        assert_eq!(r.synergies_actives[0].nom, "Coordination de Base");
        // 32 (placement) + 30 (ratio élémentaire, Void compris) = 62 ≥ 50 :
        // pas de recommandation générale, seulement les 2 repositionnements.
        assert_eq!(r.score_synergie, 62);
        assert_eq!(r.recommandations.len(), 2, "2 repositionnements");
        assert!(r.recommandations[0].contains("Hors poste 1"));
    }

    #[test]
    fn element_dominant_declenche_la_cohesion() {
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| {
                joueur(
                    &format!("J{i}"),
                    if i < 6 { "Fire" } else { "Wind" },
                    "MF",
                    "MF",
                )
            })
            .collect();
        let r = calculer_synergie_equipe(&equipe, None);
        let cohesion = r
            .synergies_actives
            .iter()
            .find(|s| s.nom.starts_with("Cohésion"))
            .expect("cohésion attendue à 60 % de Feu");
        assert_eq!(cohesion.nom, "Cohésion Élémentaire (Feu)");
        assert_eq!(cohesion.valeur, 12);
        // 40 (placement) + 30 (cohésion) = 70.
        assert_eq!(r.score_synergie, 70);
    }

    #[test]
    fn egalite_d_elements_tranche_par_l_ordre_wind_forest_fire() {
        // 5 Wind, 5 Fire : `>` strict → Wind gagne, car il est vu en premier.
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| {
                joueur(
                    &format!("J{i}"),
                    if i < 5 { "Fire" } else { "Wind" },
                    "MF",
                    "MF",
                )
            })
            .collect();
        let r = calculer_synergie_equipe(&equipe, Some(&Entraineur {
            nom: "Coach".to_string(),
            element: "Wind".to_string(),
        }));
        assert!(
            r.synergies_actives.iter().any(|s| s.nom == "Harmonie Tactique"),
            "le dominant doit être Wind, pas Fire"
        );
    }

    #[test]
    fn element_inconnu_n_est_compte_nulle_part() {
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Lumière", "MF", "MF"))
            .collect();
        let r = calculer_synergie_equipe(&equipe, None);
        assert!(!r.synergies_actives.iter().any(|s| s.nom.starts_with("Cohésion")));
        assert_eq!(r.score_synergie, 40);
    }

    #[test]
    fn entraineur_du_mauvais_element_produit_une_recommandation() {
        let equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "MF"))
            .collect();
        let coach = Entraineur {
            nom: "Endou".to_string(),
            element: "Wind".to_string(),
        };
        let r = calculer_synergie_equipe(&equipe, Some(&coach));
        assert!(!r.synergies_actives.iter().any(|s| s.nom == "Harmonie Tactique"));
        assert!(r.recommandations[0].contains("Envisagez un coach d'élément Fire"));
    }

    #[test]
    fn passif_elementaire_donne_un_bonus_plafonne_a_vingt() {
        let mut equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "MF"))
            .collect();
        // 10 joueurs portant le même passif → count = 10 → valeur 50, plafonnée.
        for j in &mut equipe {
            j.passifs.push("Fureur du Feu".to_string());
        }
        let r = calculer_synergie_equipe(&equipe, None);
        let passif = r
            .synergies_actives
            .iter()
            .find(|s| s.type_bonus == "Boost Élémentaire Feu")
            .expect("passif Feu attendu");
        assert_eq!(passif.valeur, 50);
        assert_eq!(passif.nom, "Fureur du Feu (x10)");
        // 40 (placement) + 30 (cohésion) + min(20, 50) = 90.
        assert_eq!(r.score_synergie, 90);
    }

    #[test]
    fn passif_elementaire_sans_trois_joueurs_ne_declenche_rien() {
        let mut equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Void", "MF", "MF"))
            .collect();
        equipe[0].element = "Fire".to_string();
        equipe[1].element = "Fire".to_string();
        equipe[0].passifs.push("Boost de Feu".to_string());
        let r = calculer_synergie_equipe(&equipe, None);
        assert!(!r.synergies_actives.iter().any(|s| s.type_bonus.starts_with("Boost Élémentaire")));
    }

    #[test]
    fn passif_defensif_compte_les_df_du_terrain() {
        let mut equipe: Vec<JoueurCharge> = (0..4)
            .map(|i| joueur(&format!("D{i}"), "Void", "DF", "DF"))
            .collect();
        equipe[0].passifs.push("Mur de fer".to_string());
        let r = calculer_synergie_equipe(&equipe, None);
        let mur = r
            .synergies_actives
            .iter()
            .find(|s| s.nom == "Mur d'Acier Tactique")
            .expect("mur attendu avec 4 DF");
        assert_eq!(mur.valeur, 6);
        assert_eq!(mur.type_bonus, "Boost Positionnel DF");
        assert!(mur.description.contains("pour les 4 défenseurs"));
    }

    #[test]
    fn score_est_borne_a_cent() {
        let mut equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "MF"))
            .collect();
        for j in &mut equipe {
            j.passifs.push("Fureur du Feu".to_string());
        }
        let coach = Entraineur {
            nom: "Coach".to_string(),
            element: "Fire".to_string(),
        };
        let r = calculer_synergie_equipe(&equipe, Some(&coach));
        // 40 + 30 + 10 + 20 = 100.
        assert_eq!(r.score_synergie, 100);
        assert!(r.recommandations.is_empty());
    }

    #[test]
    fn passifs_comptes_dans_l_ordre_de_premiere_apparition() {
        let mut equipe: Vec<JoueurCharge> = (0..10)
            .map(|i| joueur(&format!("J{i}"), "Fire", "DF", "DF"))
            .collect();
        equipe[0].passifs = vec!["Mur de fer".to_string(), "Fureur du Feu".to_string()];
        equipe[1].passifs = vec!["Fureur du Feu".to_string()];
        let r = calculer_synergie_equipe(&equipe, None);
        let noms: Vec<&str> = r
            .synergies_actives
            .iter()
            .filter(|s| s.type_bonus.starts_with("Boost "))
            .map(|s| s.nom.as_str())
            .collect();
        assert_eq!(noms, ["Mur d'Acier Tactique", "Fureur du Feu (x2)"]);
    }
}
