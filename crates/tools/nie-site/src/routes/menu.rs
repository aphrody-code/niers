//! Relais de l'arbre de navigation des menus de `nie-model-serve`.
//!
//! Le catalogue `/menu-tree.json` est déjà construit par l'amont à partir des vrais
//! `*_setting.cfg.bin`, de leurs calques et de leurs commandes. `nie-site` ne le recopie pas et
//! ne le reconstruit pas : il l'adresse sous son API publique, en réutilisant le proxy borné de
//! [`super::assets`] (cache, ETag, timeout et plafond de réponse).
//!
//! Les deux paramètres d'écran ne désignent pas le calque `mainmenu01` ni un script Lua : ils
//! désignent le stem du fichier `*_setting.cfg.bin`, exactement comme le sélecteur de
//! `nie-model-serve`.

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Chemin public du catalogue de navigation.
pub const SCREENS_ROUTE: &str = "/api/v1/menu/screens";

/// Chemin public d'une entrée du catalogue de navigation.
pub const SCREEN_ROUTE: &str = "/api/v1/menu/screens/{stem}";

/// Chemin d'amont du catalogue complet.
const UPSTREAM_INDEX: &str = "menu-tree.json";

/// Construit le chemin d'amont d'un écran précis.
fn upstream_screen(stem: &str) -> Result<String, ErreurSite> {
    // Le routeur ne capture qu'un segment, mais la garde reste ici aussi : cette fonction est
    // le point qui transforme une entrée client en chemin VFS adressé à l'amont.
    if stem.is_empty()
        || stem == "."
        || stem == ".."
        || stem.contains('/')
        || stem.contains('\\')
        || stem.contains("..")
        || stem.ends_with(".json")
    {
        return Err(ErreurSite::Demande(
            "stem de menu invalide : attendez le nom sans chemin ni suffixe .json".to_owned(),
        ));
    }
    Ok(format!("menu-tree/{stem}.json"))
}

/// Relaie une ressource de menu par le proxy partagé.
async fn relay(state: EtatSite, path: String, query: RawQuery, headers: HeaderMap) -> Response {
    match super::assets::proxy(State(state), Path(path), query, headers).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

/// `GET /api/v1/menu/screens` — l'arbre de navigation complet des menus.
pub async fn screens(
    State(state): State<EtatSite>,
    query: RawQuery,
    headers: HeaderMap,
) -> Response {
    relay(state, UPSTREAM_INDEX.to_owned(), query, headers).await
}

/// `GET /api/v1/menu/screens/{stem}` — une entrée de l'arbre de navigation.
pub async fn screen(
    State(state): State<EtatSite>,
    Path(stem): Path<String>,
    query: RawQuery,
    headers: HeaderMap,
) -> Response {
    let path = match upstream_screen(&stem) {
        Ok(path) => path,
        Err(error) => return error.into_response(),
    };
    relay(state, path, query, headers).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_stem_devient_un_fichier_menu_tree() {
        assert_eq!(
            upstream_screen("mainmenu01").unwrap(),
            "menu-tree/mainmenu01.json"
        );
    }

    #[test]
    fn le_stem_ne_peut_pas_sortir_de_l_espace_menu() {
        for stem in ["", ".", "..", "../secret", "a/b", "a\\b", "a.json"] {
            assert!(upstream_screen(stem).is_err(), "stem accepté : {stem:?}");
        }
    }
}
