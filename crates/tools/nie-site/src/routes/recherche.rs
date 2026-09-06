//! `/api/v1/recherche` — chercher un fichier dans **tout** le VFS.
//!
//! Ce qui manquait, mesuré le 2026-09-06 : `/b/{préfixe}` parcourt un dossier **niveau par
//! niveau** et son `q` ne filtre que le niveau courant. Vérifié —
//! `/b/data?q=chara_base` rend `total_fichiers: 0` alors que le jeu porte des dizaines de
//! `chara_base*`. Il n'existait donc **aucune** façon de trouver un fichier par son nom sur ce
//! site, alors que l'index qui le permet est monté au démarrage et porte déjà tous les filtres.
//!
//! C'est le manque que la matrice de couverture désignait sous trois noms : `niers vfs find`,
//! `vfs_find` et `vfs_find_paged`. Un seul câblage les sert tous les trois.
//!
//! **La réponse publie ce qui a été réellement appliqué** (`filtres`), et le total **filtré**
//! séparément du total brut. Les deux, parce qu'un filtre silencieusement ignoré est le pire
//! des défauts — le client croit filtrer — et parce qu'un total déduit du nombre d'éléments
//! rendus donne une dernière page qui ne finit jamais.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::error::ErreurSite;
use crate::routes::DemandePage;
use crate::state::EtatSite;
use crate::vfs_index::{DemandeFiltre, Fichier, FiltresAppliques};

/// Ce que le client demande : la pagination, le motif, et les filtres de l'index.
///
/// **Les champs sont écrits à plat, et non composés par `#[serde(flatten)]`** — piège payé ici
/// le 2026-09-06 : `flatten` fait passer la désérialisation par un tampon de contenu où toute
/// valeur d'une query string est une **chaîne**, et `?per_page=2` échoue alors en
/// « invalid type: string "2", expected u32 ». La réponse est un `400` sur une requête
/// parfaitement valide, et rien dans le code des deux structures ne le laisse voir.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Demande {
    /// Numéro de page, à partir de 1.
    pub page: Option<u32>,
    /// Nombre d'éléments par page, plafonné à [`crate::config::PER_PAGE_MAX`].
    pub per_page: Option<u32>,
    /// Motif comparé sans casse au chemin **entier**.
    pub q: Option<String>,
    /// Extension exacte, sans point (`ext=g4mg`).
    pub ext: Option<String>,
    /// Sous-arbre auquel restreindre la recherche (`prefixe=data/dx11/menu`).
    pub prefixe: Option<String>,
    /// Motif glob du jeu (`glob=data/dx11/**,!**/movie/**`).
    pub glob: Option<String>,
    /// Nom du CPK d'origine. Sans effet sur un montage dump.
    pub cpk: Option<String>,
    /// Critère de tri : `nom` (défaut) ou `taille`.
    pub tri: Option<String>,
    /// Sens de tri : `asc` (défaut) ou `desc`.
    pub ordre: Option<String>,
    /// Taille minimale en octets, incluse.
    pub taille_min: Option<u32>,
    /// Taille maximale en octets, incluse.
    pub taille_max: Option<u32>,
}

impl Demande {
    /// La pagination demandée, déjà bornée.
    #[must_use]
    pub fn page(&self) -> DemandePage {
        DemandePage {
            page: self.page,
            per_page: self.per_page,
            q: self.q.clone(),
        }
    }

    /// Les filtres de l'index tels que l'index les attend.
    #[must_use]
    pub fn filtre(&self) -> DemandeFiltre {
        DemandeFiltre {
            prefixe: self.prefixe.clone(),
            glob: self.glob.clone(),
            ext: self.ext.clone(),
            cpk: self.cpk.clone(),
            tri: self.tri.clone(),
            ordre: self.ordre.clone(),
            taille_min: self.taille_min,
            taille_max: self.taille_max,
        }
    }
}

/// Le résultat d'une recherche.
#[derive(Debug, Clone, Serialize)]
pub struct Resultat {
    /// Les fichiers de la page demandée.
    pub fichiers: Vec<Fichier>,
    /// Nombre de fichiers que la recherche retient, **tous** confondus.
    pub total: usize,
    /// Nombre de fichiers de l'index, sans aucun filtre — le dénominateur.
    pub total_sans_filtre: usize,
    /// Page rendue, à partir de 1.
    pub page: u32,
    /// Taille de page réellement appliquée (bornée à [`crate::config::PER_PAGE_MAX`]).
    pub per_page: u32,
    /// Ce que le serveur a **réellement** appliqué.
    pub filtres: FiltresAppliques,
}

/// `GET /api/v1/recherche` — cherche dans tout le VFS.
///
/// # Errors
///
/// `503` tant que l'index n'est pas monté : le montage se fait en fond au démarrage, et
/// répondre « aucun résultat » pendant ce temps ferait passer une indisponibilité pour un
/// corpus vide.
pub async fn recherche(
    State(etat): State<EtatSite>,
    Query(demande): Query<Demande>,
) -> Result<Json<Resultat>, ErreurSite> {
    let index = etat.index()?;
    let bornes = demande.page().bornee();
    let requete = index
        .resoudre(demande.q.as_deref(), &demande.filtre())
        .paginer(bornes.offset(), bornes.per_page as usize);
    // `vue = None` : la recherche porte sur l'index ENTIER, pas sur l'une des quatre vues
    // enregistrées — celles-ci ne couvrent que 143 246 des 255 308 entrées.
    let (fichiers, total) = index.page_filtree(None, &requete);
    Ok(Json(Resultat {
        fichiers,
        total,
        total_sans_filtre: index.len(),
        page: bornes.page,
        per_page: bornes.per_page,
        filtres: requete.applique,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs_index::IndexVfs;

    fn index_temoin() -> IndexVfs {
        IndexVfs::depuis(vec![
            (
                "data/common/chara/chara_base_1.03.98.00.cfg.bin".to_owned(),
                4096,
            ),
            ("data/common/chara/chara_text.cfg.bin".to_owned(), 2048),
            ("data/dx11/menu/title/title.g4tx".to_owned(), 65_536),
            // Un `.bin` HORS du sous-arbre `data/common/chara/` : sans lui, `ext=bin` rendait
            // deja le bon compte et le test de la combinaison prefixe+extension ne pouvait pas
            // echouer. Verifie par falsification — garde retiree, il rougit.
            ("data/dx11/menu/title/title_layout.cfg.bin".to_owned(), 128),
            ("data/common/sound/en/ev01.p3lip".to_owned(), 512),
        ])
    }

    #[test]
    fn la_recherche_porte_sur_tout_l_index_pas_sur_un_niveau() {
        // Le defaut que cette route corrige : `/b/data?q=chara_base` rend 0 parce qu'il ne
        // regarde que les fichiers DIRECTEMENT sous `data/`. Ici le motif traverse l'arbre.
        let index = index_temoin();
        let r = index.resoudre(Some("chara_base"), &DemandeFiltre::default());
        let (fichiers, total) = index.page_filtree(None, &r);
        assert_eq!(total, 1, "le motif traverse les dossiers");
        assert_eq!(fichiers.len(), 1);
        assert!(fichiers[0].chemin.ends_with("chara_base_1.03.98.00.cfg.bin"));
    }

    #[test]
    fn le_prefixe_restreint_le_motif_a_un_sous_arbre() {
        // La question qu'on pose vraiment sur 255 308 fichiers : « ce motif, mais sous CE
        // dossier ». Sans prefixe, `q=chara` traverse tout ; avec, il ne sort pas du sous-arbre.
        let index = index_temoin();
        let sans = index.resoudre(Some("cfg"), &DemandeFiltre::default());
        let avec = index.resoudre(
            Some("cfg"),
            &DemandeFiltre {
                prefixe: Some("data/common/chara".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert_eq!(index.page_filtree(None, &sans).1, 3, "trois `.cfg.bin` en tout");
        assert_eq!(index.page_filtree(None, &avec).1, 2, "deux sous ce sous-arbre");
        // La moitie qui compte : le meme motif sous un AUTRE sous-arbre ne rend pas le meme
        // compte. Un prefixe ignore rendrait 3 aux trois appels.
        let ailleurs = index.resoudre(
            Some("cfg"),
            &DemandeFiltre {
                prefixe: Some("data/dx11".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert_eq!(index.page_filtree(None, &ailleurs).1, 1);
    }

    #[test]
    fn un_prefixe_n_attrape_pas_le_sous_arbre_voisin() {
        // `data/dx1` ne doit PAS attraper `data/dx11` : c'est un voisin, pas un descendant.
        // La barre finale ajoutee a la normalisation est exactement ce qui l'empeche, et sans
        // ce test elle se ferait retirer un jour comme une coquetterie.
        let index = index_temoin();
        let voisin = index.resoudre(
            None,
            &DemandeFiltre {
                prefixe: Some("data/dx1".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert_eq!(index.page_filtree(None, &voisin).1, 0);
        let exact = index.resoudre(
            None,
            &DemandeFiltre {
                prefixe: Some("data/dx11".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert_eq!(index.page_filtree(None, &exact).1, 2);
    }

    #[test]
    fn un_glob_retient_ce_qu_il_inclut_et_l_exclusion_prime() {
        // Les trois constructions qui distinguent ce selecteur d'un `ext=` : `**` traverse les
        // `/`, la liste separee par virgules, et `!` qui PRIME sur les inclusions.
        let index = index_temoin();
        let compte = |spec: &str| {
            let r = index.resoudre(
                None,
                &DemandeFiltre {
                    glob: Some(spec.to_owned()),
                    ..DemandeFiltre::default()
                },
            );
            index.page_filtree(None, &r).1
        };
        assert_eq!(compte("data/**"), 5, "tout l'index");
        assert_eq!(compte("data/common/**"), 3);
        assert_eq!(compte("data/**/*.g4tx"), 1);
        assert_eq!(compte("data/common/**,data/dx11/**/*.g4tx"), 4, "la liste cumule");
        // La moitie qui compte : sans la priorite de l'exclusion, ce serait 3.
        assert_eq!(compte("data/common/**,!**/chara_text*"), 2, "l'exclusion prime");
        // Et un motif qui ne designe rien rend 0, pas tout.
        assert_eq!(compte("data/aucun/**"), 0);
    }

    #[test]
    fn un_glob_vide_le_dit_au_lieu_de_passer_pour_un_filtre() {
        // Un motif fait de separateurs compile en filtre qui accepte TOUT. Le republier comme
        // applique laisserait croire a un filtre actif sur une liste entiere — exactement le
        // defaut que ce depot a paye sur `/b?q=`.
        let index = index_temoin();
        let r = index.resoudre(
            None,
            &DemandeFiltre {
                glob: Some(",,".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert!(r.applique.glob_vide, "le service dit que le motif ne filtre rien");
        assert_eq!(r.applique.glob, None, "et ne le republie pas comme applique");
        assert_eq!(index.page_filtree(None, &r).1, 5);
    }

    #[test]
    fn le_prefixe_est_republie_normalise() {
        let index = index_temoin();
        let r = index.resoudre(
            None,
            &DemandeFiltre {
                prefixe: Some("/data/common/chara/".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        assert_eq!(r.applique.prefixe.as_deref(), Some("data/common/chara/"));
    }

    #[test]
    fn le_prefixe_survit_a_la_combinaison_avec_une_extension() {
        // Avec `ext`, `base` part de la liste par extension — qui n'est pas triee par chemin —
        // et c'est `retenu` qui doit finir le travail. Sans la garde dupliquee la, ce test
        // rendrait le fichier de l'autre sous-arbre.
        let index = index_temoin();
        let r = index.resoudre(
            None,
            &DemandeFiltre {
                prefixe: Some("data/common/chara".to_owned()),
                ext: Some("bin".to_owned()),
                ..DemandeFiltre::default()
            },
        );
        let (fichiers, total) = index.page_filtree(None, &r.paginer(0, 10));
        assert_eq!(total, 2);
        assert!(fichiers.iter().all(|f| f.chemin.starts_with("data/common/chara/")));
    }

    #[test]
    fn le_total_filtre_et_le_total_brut_sont_deux_comptes_distincts() {
        let index = index_temoin();
        let r = index.resoudre(Some("chara"), &DemandeFiltre::default());
        let (_, total) = index.page_filtree(None, &r);
        assert_eq!(total, 2);
        assert_eq!(index.len(), 5, "le denominateur ne bouge pas avec le filtre");
    }

    #[test]
    fn une_query_string_avec_des_nombres_se_deserialise() {
        // Non-regression du piege `#[serde(flatten)]` : avec lui, `per_page=2` echouait en
        // « invalid type: string "2", expected u32 » — un 400 sur une requete valide.
        let d: Demande = serde_urlencoded_temoin("q=chara&page=3&per_page=25&taille_min=100");
        assert_eq!(d.page, Some(3));
        assert_eq!(d.per_page, Some(25));
        assert_eq!(d.taille_min, Some(100));
        assert_eq!(d.q.as_deref(), Some("chara"));
        assert_eq!(d.page().bornee().per_page, 25);
        assert_eq!(d.filtre().taille_min, Some(100));
    }

    /// Deserialise comme axum le fait pour `Query<T>` : depuis une query string.
    fn serde_urlencoded_temoin(qs: &str) -> Demande {
        let uri: axum::http::Uri = format!("http://x/?{qs}").parse().expect("uri valide");
        let paires: Vec<(String, String)> = uri
            .query()
            .unwrap_or("")
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        let carte: serde_json::Map<String, serde_json::Value> = paires
            .into_iter()
            .map(|(k, v)| {
                let val = v.parse::<u64>().map_or_else(
                    |_| serde_json::Value::String(v.clone()),
                    |n| serde_json::Value::Number(n.into()),
                );
                (k, val)
            })
            .collect();
        serde_json::from_value(serde_json::Value::Object(carte)).expect("demande valide")
    }

    #[test]
    fn un_motif_qui_ne_designe_rien_rend_zero_et_le_dit() {
        // Preuve par falsification : sans cette moitie-la, un filtre ignore passerait pour un
        // filtre applique — c'est exactement le defaut 1 du lot 8.
        let index = index_temoin();
        let r = index.resoudre(Some("motif_absent_2026"), &DemandeFiltre::default());
        let (fichiers, total) = index.page_filtree(None, &r);
        assert_eq!(total, 0);
        assert!(fichiers.is_empty());
        assert_eq!(r.applique.q.as_deref(), Some("motif_absent_2026"));
    }
}
