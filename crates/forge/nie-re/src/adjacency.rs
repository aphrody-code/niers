//! Classement par **contiguïté d'adresse** du résidu que le graphe n'atteint pas.
//!
//! Après la récupération des feuilles, 36 251 des 117 521 fonctions de
//! `nie.exe` n'ont **aucune arête entrante** : rien ne les référence
//! directement, elles ne sont atteintes que par un pointeur calculé, une table
//! de sauts ou une vtable non identifiée. La propagation de labels, qui vote
//! sur le voisinage du call-graph, ne peut structurellement rien pour elles.
//!
//! Reste un signal indépendant du graphe : **l'éditeur de liens MSVC émet les
//! fonctions d'une même unité de compilation de façon contiguë**. Deux
//! fonctions voisines en adresse viennent le plus souvent du même `.obj`, donc
//! du même sous-système.
//!
//! ## Règle appliquée
//!
//! Une fonction `standalone` est classée `S` si le **plus proche voisin classé
//! à gauche** et le **plus proche voisin classé à droite** valent tous deux `S`
//! (dans la limite de [`MAX_GAP`] fonctions de distance). L'encadrement
//! concordant est exigé : un seul voisin ne suffit pas, et un bloc encadré par
//! deux sous-systèmes différents n'est pas classé du tout.
//!
//! ## Ce que vaut cette règle — mesuré, pas supposé
//!
//! `precision_estimate` rejoue la règle sur les fonctions **déjà classées** :
//! on prédit leur sous-système à partir de leur encadrement, et on compare.
//! Sur `nie.exe` au 2026-08-29 : **87,9 % de concordance sur 54 742 cas** de
//! contrôle.
//!
//! Cette mesure dit exactement une chose, et il ne faut pas lui en faire dire
//! plus : la règle est cohérente à 87,9 % avec **l'étiquetage existant** — qui
//! provient lui-même en majorité de la propagation statistique, et n'est donc
//! pas une vérité terrain. C'est une mesure de cohérence, pas d'exactitude.
//! D'où une confiance délibérément basse ([`ADJACENCY_CONFIDENCE`]) et une
//! source distincte (`subsys_src='adjacency'`) : ces étiquettes restent
//! reconnaissables et révocables, et n'écrasent jamais un label structurel
//! (RTTI, vtable, héritage de thunk).

use anyhow::Result;
use nie_index::{Db, rusqlite};
use tracing::info;

/// Distance maximale, **en nombre de fonctions**, à laquelle un voisin classé
/// est encore considéré comme encadrant. `None` = pas de limite : l'encadrement
/// porte sur le bloc contigu entier.
///
/// Le compromis a été mesuré sur `nie.exe` (2026-08-29) plutôt que choisi :
///
/// | Distance | Cas de contrôle | Cohérence | Fonctions classables |
/// |---|---|---|---|
/// | 1 (voisins immédiats) | 44 179 | **89,8 %** | 3 937 |
/// | 2 | 49 566 | 88,8 % | — |
/// | 8 | 53 885 | 88,0 % | — |
/// | illimitée (bloc entier) | 54 742 | 87,9 % | **20 998** |
///
/// Restreindre aux voisins immédiats ne gagne que 1,9 point de cohérence et
/// perd 5 fois la couverture : les fonctions non classées forment des **blocs**
/// contigus, et exiger un voisin classé à distance 1 exclut tout l'intérieur
/// d'un bloc. La règle retenue encadre donc le bloc entier — ce qui reste
/// fidèle à l'hypothèse de départ, l'unité de compilation étant précisément un
/// bloc contigu.
const MAX_GAP: Option<usize> = None;

/// Confiance attribuée à un label de contiguïté.
///
/// Sous les 0,7 des ancres structurelles (RTTI/vtable) : la contiguïté est un
/// indice de disposition mémoire, pas une identité.
const ADJACENCY_CONFIDENCE: f64 = 0.5;

/// Résultat du classement par contiguïté.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdjacencyStats {
    /// Fonctions examinées.
    pub total: usize,
    /// Fonctions `standalone` avant la passe.
    pub unclassified: usize,
    /// Fonctions classées par cette passe.
    pub classified: usize,
    /// Cas de contrôle utilisés pour l'auto-évaluation (fonctions déjà
    /// classées dont l'encadrement concorde).
    pub control_cases: usize,
    /// Parmi eux, ceux où la règle retrouve le label existant.
    pub control_hits: usize,
}

impl AdjacencyStats {
    /// Cohérence de la règle avec l'étiquetage existant, en pourcentage.
    ///
    /// Voir la mise en garde du module : c'est une mesure de **cohérence**,
    /// l'étiquetage de référence n'étant pas une vérité terrain.
    #[must_use]
    pub fn precision_estimate(&self) -> f64 {
        if self.control_cases == 0 {
            return 0.0;
        }
        100.0 * self.control_hits as f64 / self.control_cases as f64
    }
}

/// Index du plus proche voisin classé et distance en nombre de fonctions.
type Neighbour = Option<(usize, usize)>;

/// Pour chaque position, le plus proche voisin classé à gauche puis à droite.
fn neighbours(subs: &[String]) -> (Vec<Neighbour>, Vec<Neighbour>) {
    let n = subs.len();
    let mut left: Vec<Neighbour> = vec![None; n];
    let mut right: Vec<Neighbour> = vec![None; n];
    let mut last: Option<usize> = None;
    for i in 0..n {
        left[i] = last.map(|k| (k, i - k));
        if subs[i] != "standalone" {
            last = Some(i);
        }
    }
    last = None;
    for i in (0..n).rev() {
        right[i] = last.map(|k| (k, k - i));
        if subs[i] != "standalone" {
            last = Some(i);
        }
    }
    (left, right)
}

/// Classe les fonctions `standalone` encadrées par deux voisins classés
/// concordants, et rapporte la cohérence de la règle mesurée sur les fonctions
/// déjà classées.
///
/// Si `dry_run` est vrai, rien n'est écrit : seule l'auto-évaluation est
/// calculée.
///
/// # Errors
///
/// Échoue sur toute erreur SQLite.
pub fn classify_by_adjacency(db: &mut Db, bin: i64, dry_run: bool) -> Result<AdjacencyStats> {
    let rows: Vec<(u64, String, String)> = {
        let mut q = db.conn().prepare(
            "SELECT vaddr, COALESCE(subsystem,'standalone'), COALESCE(subsys_src,'')
             FROM function WHERE binary_id=?1 ORDER BY vaddr",
        )?;
        q.query_map([bin], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    let subs: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    let (left, right) = neighbours(&subs);

    let mut stats = AdjacencyStats {
        total: rows.len(),
        ..Default::default()
    };
    let mut to_write: Vec<(u64, String)> = Vec::new();

    for i in 0..rows.len() {
        // Encadrement concordant et suffisamment proche ?
        let (Some((li, ld)), Some((ri, rd))) = (left[i], right[i]) else {
            continue;
        };
        if MAX_GAP.is_some_and(|m| ld > m || rd > m) || subs[li] != subs[ri] {
            continue;
        }
        let predicted = &subs[li];
        if subs[i] == "standalone" {
            stats.unclassified += 1;
            to_write.push((rows[i].0, predicted.clone()));
        } else {
            // Cas de contrôle : la règle retrouve-t-elle un label déjà posé ?
            stats.control_cases += 1;
            if subs[i] == *predicted {
                stats.control_hits += 1;
            }
        }
    }

    if dry_run {
        info!(
            candidats = to_write.len(),
            coherence = stats.precision_estimate(),
            "adjacency: simulation, aucune écriture"
        );
        return Ok(stats);
    }

    let tx = db.conn_mut().transaction()?;
    {
        // N'écrase jamais un label structurel : la garde `subsystem='standalone'`
        // suffit, une fonction déjà classée n'étant pas candidate.
        let mut upd = tx.prepare(
            "UPDATE function SET subsystem=?3, subsys_src='adjacency', confidence=?4
             WHERE binary_id=?1 AND vaddr=?2 AND subsystem='standalone'",
        )?;
        for (va, sub) in &to_write {
            stats.classified += upd.execute(rusqlite::params![
                bin,
                *va as i64,
                sub,
                ADJACENCY_CONFIDENCE
            ])?;
        }
    }
    tx.commit()?;

    info!(
        classees = stats.classified,
        coherence = stats.precision_estimate(),
        "adjacency: résidu classé par contiguïté"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn voisins_reperent_le_plus_proche_classe_de_chaque_cote() {
        let subs = s(&["menu", "standalone", "standalone", "audio"]);
        let (l, r) = neighbours(&subs);
        // Index 1 : voisin gauche = 0 à distance 1, voisin droit = 3 à distance 2.
        assert_eq!(l[1], Some((0, 1)));
        assert_eq!(r[1], Some((3, 2)));
        // Index 0 n'a pas de voisin classé à gauche.
        assert_eq!(l[0], None);
    }

    #[test]
    fn encadrement_discordant_n_est_pas_retenu() {
        let subs = s(&["menu", "standalone", "audio"]);
        let (l, r) = neighbours(&subs);
        let (li, ri) = (l[1].unwrap(), r[1].unwrap());
        assert_ne!(
            subs[li.0], subs[ri.0],
            "les deux bords divergent : pas de prédiction"
        );
    }

    #[test]
    fn encadrement_concordant_predit_le_sous_systeme() {
        let subs = s(&["menu", "standalone", "menu"]);
        let (l, r) = neighbours(&subs);
        let (li, ld) = l[1].unwrap();
        let (ri, rd) = r[1].unwrap();
        assert!(!MAX_GAP.is_some_and(|m| ld > m || rd > m));
        assert_eq!(subs[li], subs[ri]);
        assert_eq!(subs[li], "menu");
    }

    #[test]
    fn la_coherence_est_nulle_sans_cas_de_controle() {
        let st = AdjacencyStats::default();
        assert!((st.precision_estimate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn la_coherence_se_calcule_sur_les_cas_de_controle() {
        let st = AdjacencyStats {
            control_cases: 200,
            control_hits: 180,
            ..Default::default()
        };
        assert!((st.precision_estimate() - 90.0).abs() < 1e-9);
    }
}
