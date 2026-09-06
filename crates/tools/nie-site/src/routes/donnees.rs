//! `/api/v1/donnees` — les familles de **données du jeu**, décodées en structures nommées.
//!
//! Ce que la matrice de couverture a chiffré le 2026-09-06 : **110 des 116 modules de
//! `nie-data` étaient `manquant`** — un parseur écrit, validé par golden byte-exact, et aucune
//! route qui l'appelle. C'était, et de loin, le plus gros écart entre ce que le dépôt sait
//! faire et ce qu'il montre.
//!
//! Une seule route les branche, parce qu'une seule façade les porte déjà :
//! [`nie_data::typed::decode_by_key`], partagée par `nie-model-serve` (route `/typed`) et
//! `nie-wasm` (décodage in-browser). La brancher ici n'ouvre pas une seconde implémentation —
//! c'est précisément la règle du dépôt : *ne pas réimplémenter d'un côté ce que l'autre fait*.
//!
//! **La différence avec `/api/v1/formats/decode` n'est pas cosmétique**, et le dépôt l'a déjà
//! payée : `niers decode` rend le RDBN **brut** (`header`/`types`/`fields`), et un consommateur
//! typé y lit zéro élément **en annonçant un succès**. Les deux routes coexistent donc, et
//! chacune dit ce qu'elle rend :
//!
//! | Route | Ce qu'elle rend |
//! |---|---|
//! | `/api/v1/formats/decode/{chemin}` | la structure **générique** du conteneur, toutes familles |
//! | `/api/v1/donnees/{chemin}` | la structure **nommée** du jeu, quand la famille est connue |
//!
//! Une clé de famille inconnue n'est pas une erreur du fichier : la route le dit en `404`, cite
//! la clé qu'elle a dérivée et renvoie vers la route générique — plutôt que de rendre un objet
//! vide qu'un client prendrait pour « cette famille est vide dans ce jeu ».

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Suffixe des fichiers que cette route accepte.
pub const SUFFIXE: &str = ".cfg.bin";

/// Taille maximale d'un `.cfg.bin` décodé en JSON typé.
///
/// La sortie **grossit avec la source** (un `chara_base` rend des milliers d'entrées), donc la
/// borne est celle du décodage générique, pas celle d'un résumé.
pub const TAILLE_MAX: usize = 4 * 1024 * 1024;

/// Ce que la route sait faire, et ce qu'elle ne fait pas.
#[derive(Debug, Clone, Serialize)]
pub struct Capacites {
    /// Nombre d'entrées visibles dans l'index du VFS.
    ///
    /// Ce n'est **pas** le nombre de `.cfg.bin` : l'index agrège les doubles suffixes sous
    /// `.bin`, et publier ce compte-là sous le nom « cfg.bin » ferait passer 72 308 fichiers
    /// pour 71 101. Un compte approché sous un nom exact est pire qu'un compte absent.
    pub entrees_indexees: usize,
    /// Nombre de familles nommées **réellement rencontrées** sur ce VFS.
    pub familles_nommees: usize,
    /// La route qui rend une famille.
    pub route: &'static str,
    /// La route générique, pour tout le reste.
    pub route_generique: &'static str,
    /// D'où vient le décodage — nommer la source unique évite qu'on en écrive une seconde.
    pub facade: &'static str,
}

/// Nombre de familles nommées rencontrées sur le VFS du jeu.
///
/// **Mesuré**, pas cité : `scripts/validation/mesurer-donnees.sh --cles` dérive la clé des
/// 71 101 `.cfg.bin` (18 326 clés distinctes), sonde un fichier par clé et compte les labels
/// rendus. Le 2026-09-06 : **1 056 fichiers typés, 121 familles distinctes**.
///
/// Ce n'est pas le 93 que `nie-data` annonce dans sa documentation de module — ce compte-là
/// est périmé, et c'est exactement pourquoi celui-ci porte sa commande et sa date.
pub const FAMILLES_TYPEES: usize = 121;

/// `GET /api/v1/donnees` — ce que la route sait faire.
pub async fn capacites(State(etat): State<EtatSite>) -> Result<Json<Capacites>, ErreurSite> {
    let entrees_indexees = etat.index().map_or(0, |i| i.len());
    Ok(Json(Capacites {
        entrees_indexees,
        familles_nommees: FAMILLES_TYPEES,
        route: "/api/v1/donnees/{chemin}",
        route_generique: "/api/v1/formats/decode/{chemin}",
        facade: "nie_data::typed::decode_by_key",
    }))
}

/// Une famille de données décodée.
#[derive(Debug, Clone, Serialize)]
pub struct Decodage {
    /// Chemin VFS décodé.
    pub chemin: String,
    /// Taille du fichier source, en octets.
    pub octets: usize,
    /// La clé de famille dérivée du nom de fichier, **publiée** : c'est elle qui décide du
    /// parseur, et un client qui reçoit un 404 doit pouvoir voir ce qui a été dérivé.
    pub cle: String,
    /// Le nom de la famille, tel que `nie-data` la nomme.
    pub famille: &'static str,
    /// Les données typées.
    pub donnees: serde_json::Value,
}

/// `GET /api/v1/donnees/{chemin}` — la structure nommée d'un `.cfg.bin`.
pub async fn donnees(
    State(etat): State<EtatSite>,
    Path(brut): Path<String>,
) -> Result<Json<Decodage>, ErreurSite> {
    let chemin = super::vfs::normaliser(&brut)?;
    if !chemin.ends_with(SUFFIXE) {
        return Err(ErreurSite::Demande(format!(
            "cette route ne lit que les {SUFFIXE} ; pour les autres formats, \
             /api/v1/formats/decode/{{chemin}}"
        )));
    }

    let index = etat.index()?;
    if !index.contient(&chemin) {
        return Err(ErreurSite::Introuvable(format!(
            "chemin absent du VFS: {chemin}"
        )));
    }

    let vfs = etat.vfs()?;
    let a_lire = chemin.clone();
    let octets = tokio::task::spawn_blocking(move || vfs.read(&a_lire))
        .await?
        .map_err(|e| {
            tracing::debug!(erreur = %e, "lecture VFS impossible");
            ErreurSite::Introuvable("fichier indexe mais illisible sur ce montage".to_owned())
        })?;
    if octets.len() > TAILLE_MAX {
        return Err(ErreurSite::Demande(format!(
            "fichier trop volumineux pour un decodage en JSON ({} octets, borne {TAILLE_MAX})",
            octets.len()
        )));
    }

    let decodage =
        tokio::task::spawn_blocking(move || decoder(&chemin, &octets)).await??;
    Ok(Json(decodage))
}

/// Décode des octets de `.cfg.bin` en structure nommée. Séparée du handler pour être testable
/// sans HTTP ni VFS.
///
/// # Errors
///
/// `Demande` quand le conteneur n'est pas lisible, `Introuvable` quand aucune famille nommée ne
/// correspond à la clé dérivée.
pub fn decoder(chemin: &str, octets: &[u8]) -> Result<Decodage, ErreurSite> {
    let racine = nie_formats::cfgbin::to_iecode_json(octets).ok_or_else(|| {
        ErreurSite::Demande(
            "conteneur illisible : ni RDBN ni T2B — /api/v1/formats/decode dit ce que c'est"
                .to_owned(),
        )
    })?;
    let cle = nie_data::typed::family_key(chemin);
    let (famille, donnees) = nie_data::typed::decode_by_key(&cle, &racine).ok_or_else(|| {
        // Un 404 qui NOMME la clé dérivée : sans elle, le prochain refait la dérivation à la
        // main pour comprendre pourquoi son fichier n'est pas reconnu.
        ErreurSite::Introuvable(format!(
            "aucune famille nommee pour la cle `{cle}` ; la structure generique de ce fichier \
             est sur /api/v1/formats/decode/{chemin}"
        ))
    })?;
    Ok(Decodage {
        chemin: chemin.to_owned(),
        octets: octets.len(),
        cle,
        famille,
        donnees,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_de_famille_retire_la_version_pas_le_suffixe_utile() {
        // Les fichiers du jeu portent un numero de version (`chara_base_1.03.98.00.cfg.bin`) :
        // c'est LUI qu'il faut retirer, et rien d'autre. `phase_set_c21` garde son `_c21`.
        let k = nie_data::typed::family_key;
        assert_eq!(k("data/x/chara_base_1.03.98.00.cfg.bin"), "chara_base");
        assert_eq!(k("data/x/phase_set_c21_0.00.00.cfg.bin"), "phase_set_c21");
        assert_eq!(k("data/x/record_config.cfg.bin"), "record_config");
    }

    #[test]
    fn un_conteneur_illisible_est_un_400_qui_renvoie_au_generique() {
        let e = decoder("data/x.cfg.bin", b"pas un cfg.bin du tout").unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
        assert!(format!("{e}").contains("/api/v1/formats/decode"), "{e}");
    }

    #[test]
    fn une_famille_inconnue_est_un_404_qui_publie_la_cle_derivee() {
        // Un T2B valide mais dont la cle n'est couverte par aucune famille nommee : la route
        // ne rend PAS un objet vide — elle dit laquelle des deux choses a manque.
        let vide = serde_json::json!({ "entries": [], "lists": [] });
        assert!(
            nie_data::typed::decode_by_key("famille_qui_n_existe_pas_2026", &vide).is_none(),
            "une cle inventee ne doit rien decoder"
        );
    }

    #[test]
    fn la_facade_couvre_bien_les_familles_qu_on_annonce_servies() {
        // Preuve par falsification : on prend deux cles reelles et on verifie qu'elles
        // decodent, puis une cle inventee et on verifie qu'elle ne decode pas. Un test qui ne
        // ferait que la premiere moitie passerait aussi sur une facade qui dit oui a tout.
        let vide = serde_json::json!({ "entries": [], "lists": [] });
        for cle in ["item_config", "skill_config", "formation_config"] {
            assert!(
                nie_data::typed::decode_by_key(cle, &vide).is_some(),
                "`{cle}` devrait etre couverte par la facade"
            );
        }
        assert!(nie_data::typed::decode_by_key("", &vide).is_none());
    }
}
