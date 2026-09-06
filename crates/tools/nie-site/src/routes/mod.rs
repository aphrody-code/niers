//! Les routes du serveur, une par module, plus le DTO de pagination qu'elles partagent.

pub mod api_v1;
pub mod aphrody;
pub mod assets;
pub mod couverture;
pub mod episodes;
pub mod feed;
pub mod formats;
pub mod geometrie;
pub mod health;
pub mod level5;
pub mod lua;
pub mod modeles3d;
pub mod pages;
pub mod static_files;
pub mod vfs;
pub mod well_known;

use serde::{Deserialize, Serialize};

use crate::config::Pagination;

/// Demande de pagination telle qu'elle arrive en query (`?page=&per_page=`).
///
/// Les deux champs sont optionnels et **bornés** par [`Pagination::borner`] : une demande
/// absurde ne provoque pas d'erreur, elle est ramenée dans les bornes et la réponse annonce ce
/// qui a été appliqué.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct DemandePage {
    /// Numéro de page, à partir de 1.
    pub page: Option<u32>,
    /// Nombre d'éléments par page, plafonné à [`crate::config::PER_PAGE_MAX`].
    pub per_page: Option<u32>,
    /// Motif de recherche, comparé sans casse au chemin ENTIER. Vide ou absent : aucun filtre.
    pub q: Option<String>,
}

impl DemandePage {
    /// Bornes effectives de la demande.
    #[must_use]
    pub fn bornee(&self) -> Pagination {
        Pagination::borner(self.page, self.per_page)
    }
}

/// Une page de résultats. Tous les catalogues de l'API en passent par là — il n'existe aucune
/// route qui rende une collection entière.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    /// Éléments de la page.
    pub elements: Vec<T>,
    /// Page rendue (après bornage).
    pub page: u32,
    /// Taille de page appliquée (après bornage).
    pub per_page: u32,
    /// Nombre total d'éléments, toutes pages confondues.
    pub total: usize,
    /// Nombre total de pages.
    pub pages: usize,
}

impl<T> Page<T> {
    /// Assemble une page à partir des éléments déjà découpés et du total connu.
    #[must_use]
    pub fn nouvelle(elements: Vec<T>, p: Pagination, total: usize) -> Self {
        let per_page = p.per_page as usize;
        Self {
            elements,
            page: p.page,
            per_page: p.per_page,
            total,
            pages: total.div_ceil(per_page.max(1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compte_des_pages() {
        let p = Pagination::borner(Some(1), Some(50));
        assert_eq!(Page::nouvelle(vec![1, 2], p, 101).pages, 3);
        assert_eq!(Page::nouvelle(Vec::<u8>::new(), p, 0).pages, 0);
        assert_eq!(Page::nouvelle(vec![1], p, 50).pages, 1);
    }
}
