//! Comparaison des variantes d'un même personnage.
//!
//! Portage de `packages/inagle/src/characters/comparison-engine.ts` (161 l.),
//! qui classe chaque variante d'un personnage face à sa variante de base : le
//! poste a-t-il changé, l'élément, les stats sont-elles toutes meilleures, quelles
//! techniques ont été gagnées ou perdues.
//!
//! # Ce qui est porté, et ce qui ne l'est pas
//!
//! Le TS travaille sur `CharacterVariant`/`BaseCharacter` (`core/types.ts`), des
//! entités assemblées par jointure de treize sources de disque. Le calcul, lui,
//! ne lit que **six** champs. Ce module déclare donc son propre
//! [`VarianteComparable`], un sous-ensemble minimal — la même méthode que celle
//! que le TS applique déjà côté zukan (`ZukanMatchEntry` est un sous-ensemble
//! déclaré de `ZukanCharacter`). L'assemblage de l'entité reste hors périmètre :
//! c'est de l'I/O, pas du calcul.
//!
//! # Fidélité
//!
//! - Ordre des stats : `kick, control, technique, physical, pressure, agility,
//!   intelligence` (`comparison-engine.ts:66`), identique à
//!   [`StatBlock::as_array`] (`Pr` = *Power/Physical*, `Ps` = *Pressure*).
//! - `addedSkills`/`removedSkills` sortent d'un `new Set(...)` JavaScript, qui
//!   **conserve l'ordre de première insertion** : la déduplication ici est
//!   ordonnée, pas triée.
//! - La classification `Series Evolution` est déclarée par le TS mais **jamais
//!   produite** (`seriesChanged` y est une constante `false`,
//!   `comparison-engine.ts:105`). La variante est portée pour que le type reste
//!   complet ; [`comparer_variantes`] ne la rend jamais.

use crate::stats::StatBlock;
use std::collections::{HashMap, HashSet};

/// Noms des 7 statistiques comparées, dans l'ordre de [`StatBlock::as_array`].
///
/// Verbatim de `statsKeys` (`comparison-engine.ts:66`). Ce sont ces chaînes-là
/// qui apparaissent dans [`EcartStat::stat`].
pub const NOMS_STATS_COMPAREES: [&str; 7] = [
    "kick",
    "control",
    "technique",
    "physical",
    "pressure",
    "agility",
    "intelligence",
];

/// Sous-ensemble d'une variante de personnage nécessaire à la comparaison.
///
/// Correspond aux seuls champs de `CharacterVariant` que
/// `compareVariants` lit réellement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VarianteComparable {
    /// Identifiant de la variante (`charaParamId`, hash hex `0x…`).
    pub chara_param_id: String,
    /// Libellé de rareté, tel qu'affiché (cf. [`crate::stats::rarity_code_to_name`]).
    pub rarete: String,
    /// Élément, sous sa forme d'affichage.
    pub element: String,
    /// Poste, sous sa forme d'affichage.
    pub position: String,
    /// Statistiques au niveau 99 (le seul palier comparé par le TS).
    pub stats_lv99: StatBlock,
    /// Identifiants des techniques apprises, dans l'ordre du jeu.
    pub techniques: Vec<String>,
}

/// Nature du lien entre une variante et la variante de base.
///
/// Reproduit l'union de chaînes `VariantComparisonResult["classification"]`
/// (`comparison-engine.ts:14`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Classification {
    /// La variante EST la variante de base.
    VersionBase,
    /// Toutes les stats sont ≥ et au moins une est >.
    AmeliorationPure,
    /// Seul l'élément change.
    ChangementElement,
    /// Seul le poste change.
    ChangementPoste,
    /// Ni l'un ni l'autre, et pas une amélioration pure.
    VariationTactique,
    /// Déclarée par le TS, **jamais produite** — cf. doc du module.
    EvolutionSerie,
    /// Le poste **et** l'élément changent.
    EvolutionHybride,
}

impl Classification {
    /// Libellé exact utilisé par inagle, pour comparer les sorties sans ambiguïté.
    #[must_use]
    pub fn libelle_inagle(self) -> &'static str {
        match self {
            Self::VersionBase => "Base Version",
            Self::AmeliorationPure => "Pure Upgrade",
            Self::ChangementElement => "Element Shift",
            Self::ChangementPoste => "Position Shift",
            Self::VariationTactique => "Tactical Variation",
            Self::EvolutionSerie => "Series Evolution",
            Self::EvolutionHybride => "Hybrid Evolution",
        }
    }
}

/// Écart d'une statistique entre la variante de base et la variante comparée.
///
/// **`Serialize` seul, jamais `Deserialize`** : le champ `stat` est un `&'static str` pris
/// dans [`NOMS_STATS_COMPAREES`], et `serde` ne sait pas produire une référence `'static`
/// depuis une entrée de durée de vie quelconque. Le `derive(Deserialize)` qui figurait ici
/// **cassait la compilation** de `nie-core --features serde`, donc de `nie-ffi` et de
/// `nie-wasm` qui l'activent tous deux (`error: lifetime may not live long enough`, mesuré le
/// 2026-09-06). Un type qui ne se relit pas se sérialise quand même : c'est un résultat de
/// calcul, il se recalcule, il ne se recharge pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EcartStat {
    /// Nom de la stat, parmi [`NOMS_STATS_COMPAREES`].
    pub stat: &'static str,
    /// Valeur chez la variante de base.
    pub valeur_base: i32,
    /// Valeur chez la variante comparée.
    pub valeur_variante: i32,
    /// `valeur_variante - valeur_base`.
    pub ecart: i32,
}

/// Résultat complet de la comparaison d'une variante à sa base.
///
/// Reproduit `VariantComparisonResult` (`comparison-engine.ts:11-25`).
///
/// `Serialize` seul, pour la raison écrite sur [`EcartStat`] : il en porte un `Vec`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ResultatComparaison {
    /// `charaParamId` de la variante comparée.
    pub variante_id: String,
    /// Rareté de la variante comparée.
    pub rarete: String,
    /// Élément de la variante comparée.
    pub element: String,
    /// Poste de la variante comparée.
    pub position: String,
    /// Classification déduite.
    pub classification: Classification,
    /// L'élément diffère-t-il de celui de la base ?
    pub element_change: bool,
    /// Le poste diffère-t-il de celui de la base ?
    pub position_changee: bool,
    /// Écarts stat par stat, dans l'ordre de [`NOMS_STATS_COMPAREES`].
    pub ecarts_stats: Vec<EcartStat>,
    /// Somme des écarts.
    pub ecart_total: i32,
    /// Techniques présentes chez la variante et absentes de la base.
    pub techniques_ajoutees: Vec<String>,
    /// Techniques présentes chez la base et absentes de la variante.
    pub techniques_retirees: Vec<String>,
    /// Phrase d'explication, en français, telle que la construit inagle.
    pub explication: String,
}

/// Déduplique en conservant l'ordre de première apparition (sémantique `new Set`).
fn dedup_ordonne(ids: &[String]) -> Vec<String> {
    let mut vus = HashSet::new();
    let mut sortie = Vec::new();
    for id in ids {
        if vus.insert(id.as_str()) {
            sortie.push(id.clone());
        }
    }
    sortie
}

/// Compare une variante à la variante de base d'un personnage.
///
/// Reproduit `compareVariants` (`comparison-engine.ts:30-149`). `noms_techniques`
/// traduit un identifiant de technique en nom affichable ; un identifiant absent
/// de la table est rendu tel quel (`skillsMap.get(id) || id`).
///
/// Si la variante EST la base (même `chara_param_id`), le résultat est le
/// court-circuit du TS : classification [`Classification::VersionBase`], aucun
/// écart, aucune technique, explication figée.
#[must_use]
pub fn comparer_variantes(
    base: &VarianteComparable,
    variante: &VarianteComparable,
    noms_techniques: &HashMap<String, String>,
) -> ResultatComparaison {
    if base.chara_param_id == variante.chara_param_id {
        return ResultatComparaison {
            variante_id: variante.chara_param_id.clone(),
            rarete: variante.rarete.clone(),
            element: variante.element.clone(),
            position: variante.position.clone(),
            classification: Classification::VersionBase,
            element_change: false,
            position_changee: false,
            ecarts_stats: Vec::new(),
            ecart_total: 0,
            techniques_ajoutees: Vec::new(),
            techniques_retirees: Vec::new(),
            explication: "Il s'agit de la version d'origine (la plus jeune).".to_string(),
        };
    }

    let element_change = base.element != variante.element;
    let position_changee = base.position != variante.position;

    let stats_base = base.stats_lv99.as_array();
    let stats_var = variante.stats_lv99.as_array();

    let mut ecarts_stats = Vec::with_capacity(7);
    let mut ecart_total = 0i32;
    let mut toutes_superieures_ou_egales = true;
    let mut au_moins_une_superieure = false;

    for (i, nom) in NOMS_STATS_COMPAREES.iter().enumerate() {
        let valeur_base = i32::from(stats_base[i]);
        let valeur_variante = i32::from(stats_var[i]);
        let ecart = valeur_variante - valeur_base;
        ecart_total += ecart;

        if ecart < 0 {
            toutes_superieures_ou_egales = false;
        }
        if ecart > 0 {
            au_moins_une_superieure = true;
        }

        ecarts_stats.push(EcartStat {
            stat: nom,
            valeur_base,
            valeur_variante,
            ecart,
        });
    }

    let nom_technique = |id: &str| -> String {
        noms_techniques
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    };

    let base_ids = dedup_ordonne(&base.techniques);
    let var_ids = dedup_ordonne(&variante.techniques);
    let base_set: HashSet<&str> = base_ids.iter().map(String::as_str).collect();
    let var_set: HashSet<&str> = var_ids.iter().map(String::as_str).collect();

    let techniques_ajoutees: Vec<String> = var_ids
        .iter()
        .filter(|id| !base_set.contains(id.as_str()))
        .map(|id| nom_technique(id))
        .collect();
    let techniques_retirees: Vec<String> = base_ids
        .iter()
        .filter(|id| !var_set.contains(id.as_str()))
        .map(|id| nom_technique(id))
        .collect();

    let mut details: Vec<String> = Vec::new();
    let classification = if element_change && position_changee {
        details.push(format!(
            "Changement de poste ({} ➔ {}) et d'élément ({} ➔ {})",
            base.position, variante.position, base.element, variante.element
        ));
        Classification::EvolutionHybride
    } else if position_changee {
        details.push(format!(
            "Changement de poste de jeu : {} ➔ {}",
            base.position, variante.position
        ));
        Classification::ChangementPoste
    } else if element_change {
        details.push(format!(
            "Changement d'élément élémentaire : {} ➔ {}",
            base.element, variante.element
        ));
        Classification::ChangementElement
    } else if toutes_superieures_ou_egales && au_moins_une_superieure {
        details.push("Amélioration pure des statistiques globales".to_string());
        Classification::AmeliorationPure
    } else {
        details.push("Variante tactique avec ajustement de moveset et stats équilibrées".to_string());
        Classification::VariationTactique
    };

    if ecart_total != 0 {
        let signe = if ecart_total > 0 { "+" } else { "" };
        details.push(format!(
            "Différence totale de statistiques de {signe}{ecart_total}"
        ));
    }

    if !techniques_ajoutees.is_empty() {
        details.push(format!(
            "Nouvelles techniques apprises : {}",
            techniques_ajoutees.join(", ")
        ));
    }

    ResultatComparaison {
        variante_id: variante.chara_param_id.clone(),
        rarete: variante.rarete.clone(),
        element: variante.element.clone(),
        position: variante.position.clone(),
        classification,
        element_change,
        position_changee,
        ecarts_stats,
        ecart_total,
        techniques_ajoutees,
        techniques_retirees,
        explication: format!("{}.", details.join(". ")),
    }
}

/// Classe toutes les variantes d'un personnage face à la première d'entre elles.
///
/// Reproduit `analyzeCharacterVariants` (`comparison-engine.ts:154-161`). Le TS
/// suppose que `variants[0]` est la variante la plus jeune, donc la base ; ce
/// module reprend cette hypothèse sans la vérifier — le tri est fait en amont,
/// à l'assemblage de l'entité, qui n'est pas porté.
///
/// Un personnage sans variante rend un vecteur vide.
#[must_use]
pub fn analyser_variantes(
    variantes: &[VarianteComparable],
    noms_techniques: &HashMap<String, String>,
) -> Vec<ResultatComparaison> {
    let Some(base) = variantes.first() else {
        return Vec::new();
    };
    variantes
        .iter()
        .map(|v| comparer_variantes(base, v, noms_techniques))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variante(id: &str, position: &str, element: &str, stats: [u16; 7]) -> VarianteComparable {
        VarianteComparable {
            chara_param_id: id.to_string(),
            rarete: "Légendaire".to_string(),
            element: element.to_string(),
            position: position.to_string(),
            stats_lv99: StatBlock::from_array(stats),
            techniques: Vec::new(),
        }
    }

    fn sans_noms() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn meme_id_rend_la_version_de_base() {
        let v = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let r = comparer_variantes(&v, &v, &sans_noms());
        assert_eq!(r.classification, Classification::VersionBase);
        assert_eq!(r.classification.libelle_inagle(), "Base Version");
        assert_eq!(r.ecart_total, 0);
        assert!(r.ecarts_stats.is_empty());
        assert_eq!(
            r.explication,
            "Il s'agit de la version d'origine (la plus jeune)."
        );
    }

    #[test]
    fn toutes_stats_superieures_donne_amelioration_pure() {
        let base = variante("0xAAAA", "FW", "Fire", [10, 10, 10, 10, 10, 10, 10]);
        let up = variante("0xBBBB", "FW", "Fire", [11, 10, 12, 10, 10, 10, 10]);
        let r = comparer_variantes(&base, &up, &sans_noms());
        assert_eq!(r.classification, Classification::AmeliorationPure);
        assert_eq!(r.ecart_total, 3);
        assert!(!r.element_change);
        assert!(!r.position_changee);
        assert_eq!(
            r.explication,
            "Amélioration pure des statistiques globales. Différence totale de statistiques de +3."
        );
    }

    #[test]
    fn une_stat_en_baisse_donne_variation_tactique() {
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let mix = variante("0xBBBB", "FW", "Fire", [20, 10, 10, 10, 10, 10, 5]);
        let r = comparer_variantes(&base, &mix, &sans_noms());
        assert_eq!(r.classification, Classification::VariationTactique);
        assert_eq!(r.ecart_total, 5);
    }

    #[test]
    fn stats_identiques_donne_variation_tactique_sans_ligne_d_ecart() {
        // allBetterOrEqual = true mais atLeastOneBetter = false → pas une
        // amélioration pure ; et totalStatDiff = 0 → pas de phrase d'écart.
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let clone = variante("0xBBBB", "FW", "Fire", [10; 7]);
        let r = comparer_variantes(&base, &clone, &sans_noms());
        assert_eq!(r.classification, Classification::VariationTactique);
        assert_eq!(r.ecart_total, 0);
        assert_eq!(
            r.explication,
            "Variante tactique avec ajustement de moveset et stats équilibrées."
        );
    }

    #[test]
    fn poste_et_element_changes_donnent_evolution_hybride() {
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let autre = variante("0xBBBB", "GK", "Wind", [10; 7]);
        let r = comparer_variantes(&base, &autre, &sans_noms());
        assert_eq!(r.classification, Classification::EvolutionHybride);
        assert_eq!(r.classification.libelle_inagle(), "Hybrid Evolution");
        assert!(r.element_change && r.position_changee);
        assert_eq!(
            r.explication,
            "Changement de poste (FW ➔ GK) et d'élément (Fire ➔ Wind)."
        );
    }

    #[test]
    fn poste_seul_prime_sur_l_amelioration_de_stats() {
        // Ordre des branches du TS : poste/élément AVANT « amélioration pure ».
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let autre = variante("0xBBBB", "MF", "Fire", [20; 7]);
        let r = comparer_variantes(&base, &autre, &sans_noms());
        assert_eq!(r.classification, Classification::ChangementPoste);
        assert_eq!(
            r.explication,
            "Changement de poste de jeu : FW ➔ MF. Différence totale de statistiques de +70."
        );
    }

    #[test]
    fn element_seul_donne_changement_element() {
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let autre = variante("0xBBBB", "FW", "Void", [10; 7]);
        let r = comparer_variantes(&base, &autre, &sans_noms());
        assert_eq!(r.classification, Classification::ChangementElement);
        assert_eq!(
            r.explication,
            "Changement d'élément élémentaire : Fire ➔ Void."
        );
    }

    #[test]
    fn ecart_negatif_n_a_pas_de_signe_plus() {
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let faible = variante("0xBBBB", "FW", "Fire", [5; 7]);
        let r = comparer_variantes(&base, &faible, &sans_noms());
        assert_eq!(r.ecart_total, -35);
        assert!(r.explication.contains("de -35."), "{}", r.explication);
    }

    #[test]
    fn ordre_des_stats_identique_a_inagle() {
        let base = variante("0xAAAA", "FW", "Fire", [1, 2, 3, 4, 5, 6, 7]);
        let autre = variante("0xBBBB", "FW", "Fire", [1, 2, 3, 4, 5, 6, 8]);
        let r = comparer_variantes(&base, &autre, &sans_noms());
        let noms: Vec<&str> = r.ecarts_stats.iter().map(|e| e.stat).collect();
        assert_eq!(
            noms,
            [
                "kick",
                "control",
                "technique",
                "physical",
                "pressure",
                "agility",
                "intelligence"
            ]
        );
        // `physical` est le 4e (Pr), `pressure` le 5e (Ps) — l'inverse casserait
        // silencieusement toute lecture des écarts.
        assert_eq!(r.ecarts_stats[3].valeur_base, 4);
        assert_eq!(r.ecarts_stats[4].valeur_base, 5);
        assert_eq!(r.ecarts_stats[6].ecart, 1);
    }

    #[test]
    fn techniques_ajoutees_et_retirees_resolvent_leur_nom() {
        let mut base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        base.techniques = vec!["s_perdue".to_string(), "s_commune".to_string()];
        let mut autre = variante("0xBBBB", "FW", "Fire", [10; 7]);
        autre.techniques = vec!["s_commune".to_string(), "s_gagnee".to_string()];

        let mut noms = HashMap::new();
        noms.insert("s_gagnee".to_string(), "Tornade".to_string());
        // `s_perdue` n'est pas dans la table → rendu tel quel.

        let r = comparer_variantes(&base, &autre, &noms);
        assert_eq!(r.techniques_ajoutees, ["Tornade"]);
        assert_eq!(r.techniques_retirees, ["s_perdue"]);
        assert!(
            r.explication.ends_with("Nouvelles techniques apprises : Tornade."),
            "{}",
            r.explication
        );
    }

    #[test]
    fn techniques_dupliquees_ne_sortent_qu_une_fois_et_dans_l_ordre() {
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        let mut autre = variante("0xBBBB", "FW", "Fire", [10; 7]);
        autre.techniques = vec![
            "b".to_string(),
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ];
        let r = comparer_variantes(&base, &autre, &sans_noms());
        // Ordre de PREMIÈRE insertion, comme `new Set` en JS — pas un tri.
        assert_eq!(r.techniques_ajoutees, ["b", "a", "c"]);
    }

    #[test]
    fn analyser_variantes_prend_la_premiere_pour_base() {
        let v = vec![
            variante("0xAAAA", "FW", "Fire", [10; 7]),
            variante("0xBBBB", "GK", "Fire", [10; 7]),
        ];
        let r = analyser_variantes(&v, &sans_noms());
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].classification, Classification::VersionBase);
        assert_eq!(r[1].classification, Classification::ChangementPoste);
    }

    #[test]
    fn analyser_variantes_sans_variante_rend_vide() {
        assert!(analyser_variantes(&[], &sans_noms()).is_empty());
    }

    #[test]
    fn evolution_serie_n_est_jamais_produite() {
        // Le TS déclare "Series Evolution" mais son `seriesChanged` est une
        // constante `false` : aucune combinaison d'entrées ne peut la rendre.
        let base = variante("0xAAAA", "FW", "Fire", [10; 7]);
        for (pos, elem, stats) in [
            ("FW", "Fire", [10u16; 7]),
            ("GK", "Fire", [10; 7]),
            ("FW", "Wind", [10; 7]),
            ("GK", "Wind", [10; 7]),
            ("FW", "Fire", [20; 7]),
            ("FW", "Fire", [5; 7]),
        ] {
            let v = variante("0xBBBB", pos, elem, stats);
            let r = comparer_variantes(&base, &v, &sans_noms());
            assert_ne!(r.classification, Classification::EvolutionSerie);
        }
        // La variante existe malgré tout dans le type, avec son libellé exact.
        assert_eq!(
            Classification::EvolutionSerie.libelle_inagle(),
            "Series Evolution"
        );
    }
}
