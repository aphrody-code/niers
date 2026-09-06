//! `/api/v1/team` — la **synergie d'une équipe**, notée par le moteur.
//!
//! # Ce que cette route ferme
//!
//! `nie_core::optimisation` a deux moitiés. La première, `builds_basara_classes`, est une
//! fonction pure d'un bloc de sept entiers vers six projections : elle tient dans une query, et
//! `/api/v1/regles/builds` la sert depuis le 2026-09-06. La seconde,
//! `calculer_synergie_equipe`, prend un **effectif complet** et un entraîneur — `routes::regles`
//! écrit noir sur blanc qu'« elle n'est pas routée ici ».
//!
//! Elle l'est maintenant, et c'est ce qui manquait pour qu'Aphrody vaille l'outil « Mon équipe »
//! d'Azalée : là-bas, les moyennes d'équipe et le score sont **recalculés en TypeScript** dans
//! le navigateur (`TeamStats.tsx`), à côté d'un moteur Rust qui sait déjà le faire. Deux
//! implémentations d'une même règle divergent au premier ajustement.
//!
//! # Pourquoi un espace de noms neuf, en anglais
//!
//! `/api/v1/regles/*` est **déjà servi**, en français, et ne se renomme pas au fil de l'eau —
//! renommer une route casse ses consommateurs. Mais la règle du 2026-09-06 est sans exception
//! pour les **nouveaux** noms : URLs et clés JSON en anglais. Greffer `synergie` sur `regles`
//! aurait mêlé les deux langues dans un même espace ; `/api/v1/team/synergy` en ouvre un propre,
//! entièrement anglais, sans toucher à l'existant. Les commentaires restent en français.
//!
//! # DTO écrits à la main, et c'est délibéré
//!
//! `nie-core` est lié **sans** sa feature `serde` : la dériver publierait le nom Rust des
//! variantes dans un JSON destiné à être lu. Chaque champ publié ici est donc choisi et traduit,
//! comme le fait `routes::regles`. C'est plus de lignes, et c'est la garantie qu'aucun `derive`
//! ne peut fuiter un identifiant interne.
//!
//! # Les routes
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `GET /api/v1/team/synergy` | le contrat : le corps attendu, les bornes, les éléments reconnus |
//! | `POST /api/v1/team/synergy` | le rapport noté : puissance, score, synergies, recommandations |
//!
//! Les formations, elles, ne sont pas ici : elles sont déjà servies par
//! `/api/v1/donnees/famille/formation_config`, qui décode le fichier réel du jeu. Une seconde
//! source de formations serait le doublon que ce dépôt interdit.

use axum::Json;
use serde::{Deserialize, Serialize};

use nie_core::optimisation::{
    Entraineur, JoueurCharge, ORDRE_ELEMENTS, RapportSynergie, calculer_synergie_equipe,
};
use nie_core::stats::StatBlock;

use crate::error::ErreurSite;

/// Nombre maximal de joueurs acceptés dans un corps de requête.
///
/// Une équipe du jeu en compte **11** sur le terrain ; la borne est posée à 32 pour laisser
/// passer un banc et une réserve sans jamais transformer la route en calculateur de masse.
pub const JOUEURS_MAX: usize = 32;

/// Un joueur, tel que le client le décrit.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayerInput {
    /// Nom affiché — il n'entre dans aucun calcul, il ressort dans les recommandations.
    pub name: String,
    /// Élément du joueur. Une valeur hors [`ORDRE_ELEMENTS`] n'est **pas** refusée : elle ne
    /// compte simplement dans aucun élément dominant, exactement comme dans le moteur.
    pub element: String,
    /// Poste naturel.
    pub natural_position: String,
    /// Poste réellement occupé sur le terrain.
    pub field_position: String,
    /// Les sept statistiques, dans l'ordre `[kc, cr, tc, pr, ps, ag, it]`.
    pub stats: [u16; 7],
    /// Les passifs déjà nommés. Le client les filtre avec `est_passif` / `nom_passif`, dont la
    /// logique est publiée par le contrat.
    #[serde(default)]
    pub passives: Vec<String>,
}

/// L'entraîneur, s'il y en a un.
#[derive(Debug, Clone, Deserialize)]
pub struct CoachInput {
    /// Nom affiché.
    pub name: String,
    /// Élément.
    pub element: String,
}

/// Le corps de `POST /api/v1/team/synergy`.
#[derive(Debug, Clone, Deserialize)]
pub struct SynergyRequest {
    /// Les joueurs, au plus [`JOUEURS_MAX`].
    pub players: Vec<PlayerInput>,
    /// L'entraîneur, facultatif.
    #[serde(default)]
    pub coach: Option<CoachInput>,
}

/// Une synergie déclenchée.
#[derive(Debug, Clone, Serialize)]
pub struct Synergy {
    /// Son nom.
    pub name: String,
    /// Sa description, en français — c'est la chaîne que le moteur produit, reprise telle
    /// quelle : la traduire ici ferait diverger le site du jeu.
    pub description: String,
    /// La famille de bonus, qui sert au plafonnement des passifs.
    pub bonus_type: String,
    /// La valeur du bonus.
    pub value: i32,
}

/// Le rapport rendu.
#[derive(Debug, Clone, Serialize)]
pub struct SynergyReport {
    /// Nombre de joueurs pris en compte.
    pub players: usize,
    /// Somme des statistiques de tous les joueurs.
    pub total_power: u32,
    /// Score de synergie, borné à 100 par le moteur.
    pub synergy_score: i32,
    /// Les synergies déclenchées, dans l'ordre du moteur.
    pub synergies: Vec<Synergy>,
    /// Les recommandations, dans l'ordre du moteur.
    pub recommendations: Vec<String>,
    /// La fonction qui a produit ce rapport — pour qu'un compte cité porte sa commande.
    pub engine: &'static str,
}

/// Le contrat publié par `GET /api/v1/team/synergy`.
#[derive(Debug, Clone, Serialize)]
pub struct SynergyContract {
    /// La méthode HTTP à employer.
    pub method: &'static str,
    /// Le chemin.
    pub path: &'static str,
    /// Les clés attendues au premier niveau du corps.
    pub body: &'static [&'static str],
    /// Les clés attendues pour un joueur.
    pub player_fields: &'static [&'static str],
    /// Les éléments que le moteur compte, dans **son** ordre d'itération : à égalité, c'est
    /// cet ordre qui désigne l'élément dominant.
    pub elements: &'static [&'static str],
    /// Les sept statistiques, dans l'ordre du tableau `stats`.
    pub stat_order: &'static [&'static str],
    /// Nombre maximal de joueurs.
    pub players_max: usize,
    /// Les asymétries du moteur, publiées plutôt que corrigées en douce.
    pub caveats: &'static [&'static str],
}

/// Ce que le moteur fait et qui surprendrait sans être écrit.
///
/// Ces deux points sont des **ports fidèles** du TypeScript d'origine, figés par des tests dans
/// `nie-core`. Les corriger ici ferait diverger le site du jeu ; les taire ferait passer un
/// comportement voulu pour un défaut.
const CAVEATS: &[&str] = &[
    "un element hors des cinq comptes n'est compte dans aucun element dominant, et abaisse donc \
     mecaniquement le ratio de cohesion",
    "une equipe entierement `Void` n'affiche aucune synergie de cohesion mais encaisse quand \
     meme les 30 points de score : la garde protege la synergie, pas le score",
];

/// Traduit un joueur du corps de requête vers le type du moteur.
fn vers_moteur(p: PlayerInput) -> JoueurCharge {
    JoueurCharge {
        nom: p.name,
        element: p.element,
        position_naturelle: p.natural_position,
        position_terrain: p.field_position,
        stats: StatBlock::from_array(p.stats),
        passifs: p.passives,
    }
}

/// Traduit le rapport du moteur vers le DTO public.
fn vers_dto(r: RapportSynergie, players: usize) -> SynergyReport {
    SynergyReport {
        players,
        total_power: r.puissance_totale,
        synergy_score: r.score_synergie,
        synergies: r
            .synergies_actives
            .into_iter()
            .map(|s| Synergy {
                name: s.nom,
                description: s.description,
                bonus_type: s.type_bonus,
                value: s.valeur,
            })
            .collect(),
        recommendations: r.recommandations,
        engine: "nie_core::optimisation::calculer_synergie_equipe",
    }
}

/// `GET /api/v1/team/synergy` — le contrat.
pub async fn contract() -> Json<SynergyContract> {
    Json(SynergyContract {
        method: "POST",
        path: "/api/v1/team/synergy",
        body: &["players", "coach"],
        player_fields: &[
            "name",
            "element",
            "natural_position",
            "field_position",
            "stats",
            "passives",
        ],
        elements: &ORDRE_ELEMENTS,
        stat_order: &["kc", "cr", "tc", "pr", "ps", "ag", "it"],
        players_max: JOUEURS_MAX,
        caveats: CAVEATS,
    })
}

/// `POST /api/v1/team/synergy` — noter une équipe.
///
/// Une équipe vide n'est pas une erreur du moteur (il rend un rapport à zéro), mais c'en est une
/// ici : un client qui envoie zéro joueur et lit un score de 0 croirait avoir mesuré quelque
/// chose. On le lui dit.
///
/// # Errors
///
/// `400` si `players` est vide ou dépasse [`JOUEURS_MAX`].
pub async fn synergy(
    Json(demande): Json<SynergyRequest>,
) -> Result<Json<SynergyReport>, ErreurSite> {
    if demande.players.is_empty() {
        return Err(ErreurSite::Demande(
            "`players` est vide : une equipe sans joueur rendrait un score de 0 qui ne mesure \
             rien. Le contrat est sur GET /api/v1/team/synergy"
                .to_owned(),
        ));
    }
    if demande.players.len() > JOUEURS_MAX {
        return Err(ErreurSite::Demande(format!(
            "trop de joueurs : {} (borne {JOUEURS_MAX})",
            demande.players.len()
        )));
    }

    let players = demande.players.len();
    let joueurs: Vec<JoueurCharge> = demande.players.into_iter().map(vers_moteur).collect();
    let entraineur = demande.coach.map(|c| Entraineur {
        nom: c.name,
        element: c.element,
    });
    let rapport = calculer_synergie_equipe(&joueurs, entraineur.as_ref());
    Ok(Json(vers_dto(rapport, players)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joueur(nom: &str, element: &str, naturelle: &str, terrain: &str) -> PlayerInput {
        PlayerInput {
            name: nom.to_owned(),
            element: element.to_owned(),
            natural_position: naturelle.to_owned(),
            field_position: terrain.to_owned(),
            stats: [100, 100, 100, 100, 100, 100, 100],
            passives: Vec::new(),
        }
    }

    #[tokio::test]
    async fn une_equipe_vide_est_refusee_et_une_equipe_pleine_acceptee() {
        // Preuve par falsification : sans la seconde moitie, un handler qui refuserait TOUT
        // passerait la premiere.
        let vide = SynergyRequest {
            players: Vec::new(),
            coach: None,
        };
        let e = synergy(Json(vide)).await.unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);

        let pleine = SynergyRequest {
            players: vec![joueur("Mark", "Fire", "GK", "GK")],
            coach: None,
        };
        assert!(synergy(Json(pleine)).await.is_ok());
    }

    #[tokio::test]
    async fn la_borne_haute_refuse_au_dela_de_joueurs_max() {
        let trop = SynergyRequest {
            players: (0..=JOUEURS_MAX)
                .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "MF"))
                .collect(),
            coach: None,
        };
        let e = synergy(Json(trop)).await.unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("32"), "la borne doit etre dite");
    }

    #[tokio::test]
    async fn le_placement_change_reellement_le_score() {
        // Le test qui interdit un handler qui rendrait une constante : deux equipes qui ne
        // different QUE par le placement doivent rendre deux scores differents.
        let bien = SynergyRequest {
            players: (0..10)
                .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "MF"))
                .collect(),
            coach: None,
        };
        let mal = SynergyRequest {
            players: (0..10)
                .map(|i| joueur(&format!("J{i}"), "Fire", "MF", "FW"))
                .collect(),
            coach: None,
        };
        let a = synergy(Json(bien)).await.unwrap().0;
        let b = synergy(Json(mal)).await.unwrap().0;
        assert!(
            a.synergy_score > b.synergy_score,
            "onze joueurs a leur poste doivent scorer plus haut : {} vs {}",
            a.synergy_score,
            b.synergy_score
        );
        assert_eq!(
            a.total_power, b.total_power,
            "la puissance brute, elle, ne depend pas du placement"
        );
        assert!(
            !b.recommendations.is_empty(),
            "un joueur hors poste doit produire une recommandation"
        );
        assert!(a.recommendations.is_empty());
    }

    #[tokio::test]
    async fn le_contrat_annonce_les_elements_dans_l_ordre_du_moteur() {
        let c = contract().await.0;
        assert_eq!(c.elements, ORDRE_ELEMENTS);
        assert_eq!(c.elements[0], "Wind", "l'ordre tranche les egalites");
        assert_eq!(c.stat_order.len(), 7);
        assert!(!c.caveats.is_empty(), "les asymetries se publient");
    }
}
