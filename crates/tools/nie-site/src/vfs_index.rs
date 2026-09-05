//! Index des chemins du VFS, et les **filtres enregistrés** qui s'y appliquent.
//!
//! Amendement A3 : l'URL d'une ressource est son chemin de jeu verbatim. Les vues nommées
//! (`textures`, `modeles`, `sons`, `videos`) ne sont pas des dossiers ni des tables : ce sont
//! des **filtres enregistrés** sur l'espace `/f` et `/b`, définis par un jeu d'extensions.
//! Elles n'inventent aucun identifiant et ne renomment rien.
//!
//! L'index est une liste de chemins **triée**, ce qui rend le parcours d'un dossier
//! (`/b/<préfixe>`) équivalent à une recherche dichotomique suivie d'un balayage de plage — au
//! lieu d'un parcours des 255 000 entrées à chaque requête. Les vues sont pré-calculées à la
//! construction : servir `/api/v1/textures?page=1` ne coûte alors qu'une tranche de `Vec`.

use std::collections::BTreeSet;

use serde::Serialize;

/// Une vue nommée : un filtre enregistré sur l'espace VFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vue {
    /// Textures du jeu (`.g4tx`, `.dds`, `.png`).
    Textures,
    /// Modèles et leurs pièces (`.g4md`, `.g4mg`, `.g4sk`, `.g4mt`, `.g4pk`).
    Modeles,
    /// Sons et conteneurs audio CRI (`.acb`, `.awb`, `.hca`, `.adx`, `.wav`).
    Sons,
    /// Vidéos (`.usm`, `.mp4`, `.webm`).
    Videos,
}

/// Les quatre vues, dans l'ordre où elles sont exposées sous `/api/v1/`.
pub const VUES: [Vue; 4] = [Vue::Textures, Vue::Modeles, Vue::Sons, Vue::Videos];

impl Vue {
    /// Segment d'URL de la vue (`textures`, `modeles`, `sons`, `videos`).
    #[must_use]
    pub fn segment(self) -> &'static str {
        match self {
            Self::Textures => "textures",
            Self::Modeles => "modeles",
            Self::Sons => "sons",
            Self::Videos => "videos",
        }
    }

    /// Extensions retenues par le filtre, en minuscules et sans point.
    #[must_use]
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Textures => &["g4tx", "dds", "png"],
            Self::Modeles => &["g4md", "g4mg", "g4sk", "g4mt", "g4pk", "g4pkm"],
            Self::Sons => &["acb", "awb", "hca", "adx", "wav"],
            Self::Videos => &["usm", "mp4", "webm"],
        }
    }

    /// Reconnaît une vue depuis son segment d'URL.
    #[must_use]
    pub fn depuis_segment(s: &str) -> Option<Self> {
        VUES.into_iter().find(|v| v.segment() == s)
    }

    /// Dit si un chemin VFS entre dans le filtre.
    #[must_use]
    pub fn retient(self, chemin: &str) -> bool {
        let Some((_, ext)) = chemin.rsplit_once('.') else { return false };
        self.extensions().iter().any(|e| ext.eq_ignore_ascii_case(e))
    }
}

/// Une entrée de fichier telle qu'elle est rendue par `/b` et par les vues.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fichier {
    /// Chemin VFS verbatim — c'est aussi l'URL, sous `/f/`.
    pub chemin: String,
    /// Nom de la feuille, extension du jeu conservée.
    pub nom: String,
    /// Taille en octets telle que l'index du jeu la déclare.
    pub taille: u32,
}

/// Contenu direct d'un préfixe : ses sous-dossiers et ses fichiers.
#[derive(Debug, Clone, Serialize)]
pub struct Dossier {
    /// Préfixe demandé, normalisé (sans slash de fin).
    pub prefixe: String,
    /// Sous-dossiers directs, chemins complets, triés.
    pub dossiers: Vec<String>,
    /// Fichiers directs de la page demandée.
    pub fichiers: Vec<Fichier>,
    /// Nombre total de fichiers directs, toutes pages confondues.
    pub total_fichiers: usize,
}

/// Index trié des chemins du VFS, avec les vues pré-calculées.
#[derive(Debug, Default)]
pub struct IndexVfs {
    chemins: Vec<String>,
    tailles: Vec<u32>,
    /// Indices dans `chemins`, une liste par vue, dans l'ordre de [`VUES`].
    vues: [Vec<u32>; 4],
}

impl IndexVfs {
    /// Construit l'index depuis des couples `(chemin, taille)`. Les doublons sont écartés.
    ///
    /// Le tri est fait ici une fois pour toutes ; toutes les lectures suivantes sont
    /// dichotomiques.
    #[must_use]
    pub fn depuis(mut entrees: Vec<(String, u32)>) -> Self {
        entrees.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        entrees.dedup_by(|a, b| a.0 == b.0);
        let mut chemins = Vec::with_capacity(entrees.len());
        let mut tailles = Vec::with_capacity(entrees.len());
        for (c, t) in entrees {
            chemins.push(c);
            tailles.push(t);
        }
        let mut vues: [Vec<u32>; 4] = [const { Vec::new() }; 4];
        for (i, chemin) in chemins.iter().enumerate() {
            for (rang, vue) in VUES.into_iter().enumerate() {
                if vue.retient(chemin) {
                    vues[rang].push(u32::try_from(i).unwrap_or(u32::MAX));
                }
            }
        }
        Self { chemins, tailles, vues }
    }

    /// Nombre de chemins indexés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chemins.len()
    }

    /// Dit si l'index est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chemins.is_empty()
    }

    /// Nombre de chemins retenus par une vue.
    #[must_use]
    pub fn compte_vue(&self, vue: Vue) -> usize {
        self.vues[rang(vue)].len()
    }

    /// Une page d'une vue : les fichiers retenus, dans l'ordre du VFS.
    #[must_use]
    pub fn page_vue(&self, vue: Vue, offset: usize, limite: usize) -> Vec<Fichier> {
        self.vues[rang(vue)]
            .iter()
            .skip(offset)
            .take(limite)
            .filter_map(|i| self.fichier(*i as usize))
            .collect()
    }

    /// Une page d'une vue, restreinte aux chemins contenant `motif`.
    ///
    /// La comparaison est insensible à la casse et porte sur le CHEMIN entier, pas sur le seul
    /// nom : chercher `chr/` ou `title` doit fonctionner, et le rangement des fichiers du jeu
    /// porte autant de sens que leur nom.
    ///
    /// Sans elle, atteindre un fichier parmi 143 246 demandait de parcourir jusqu'à 904 pages.
    #[must_use]
    pub fn page_vue_filtree(
        &self,
        vue: Vue,
        motif: &str,
        offset: usize,
        limite: usize,
    ) -> Vec<Fichier> {
        let motif = motif.to_lowercase();
        self.vues[rang(vue)]
            .iter()
            .filter(|i| {
                self.chemins
                    .get(**i as usize)
                    .is_some_and(|c| c.to_lowercase().contains(&motif))
            })
            .skip(offset)
            .take(limite)
            .filter_map(|i| self.fichier(*i as usize))
            .collect()
    }

    /// Nombre de chemins d'une vue qui contiennent `motif`.
    ///
    /// Compté séparément de la page : le total conditionne la pagination, et le déduire du
    /// nombre d'éléments rendus donnerait une dernière page qui ne finit jamais.
    #[must_use]
    pub fn compte_vue_filtree(&self, vue: Vue, motif: &str) -> usize {
        let motif = motif.to_lowercase();
        self.vues[rang(vue)]
            .iter()
            .filter(|i| {
                self.chemins
                    .get(**i as usize)
                    .is_some_and(|c| c.to_lowercase().contains(&motif))
            })
            .count()
    }

    /// Contenu direct d'un préfixe. Le préfixe vide décrit la racine du VFS.
    ///
    /// Les sous-dossiers sont rendus en entier (ils sont peu nombreux à chaque niveau) ; seuls
    /// les fichiers sont paginés — c'est là que se trouvent les dossiers à 40 000 entrées.
    #[must_use]
    pub fn dossier(&self, prefixe: &str, offset: usize, limite: usize) -> Dossier {
        let prefixe = prefixe.trim_matches('/').to_owned();
        let base = if prefixe.is_empty() { String::new() } else { format!("{prefixe}/") };
        let debut = self.chemins.partition_point(|c| c.as_str() < base.as_str());
        let mut dossiers = BTreeSet::new();
        let mut fichiers = Vec::new();
        let mut total_fichiers = 0usize;
        for i in debut..self.chemins.len() {
            let chemin = &self.chemins[i];
            let Some(reste) = chemin.strip_prefix(base.as_str()) else { break };
            match reste.split_once('/') {
                Some((segment, _)) => {
                    if !segment.is_empty() {
                        dossiers.insert(format!("{base}{segment}"));
                    }
                }
                None => {
                    if total_fichiers >= offset
                        && fichiers.len() < limite
                        && let Some(f) = self.fichier(i)
                    {
                        fichiers.push(f);
                    }
                    total_fichiers += 1;
                }
            }
        }
        Dossier {
            prefixe,
            dossiers: dossiers.into_iter().collect(),
            fichiers,
            total_fichiers,
        }
    }

    /// Dit si un chemin exact est indexé.
    #[must_use]
    pub fn contient(&self, chemin: &str) -> bool {
        self.chemins.binary_search_by(|c| c.as_str().cmp(chemin)).is_ok()
    }

    /// Taille déclarée d'un chemin exact.
    #[must_use]
    pub fn taille(&self, chemin: &str) -> Option<u32> {
        let i = self.chemins.binary_search_by(|c| c.as_str().cmp(chemin)).ok()?;
        self.tailles.get(i).copied()
    }

    fn fichier(&self, i: usize) -> Option<Fichier> {
        let chemin = self.chemins.get(i)?;
        let nom = chemin.rsplit('/').next().unwrap_or(chemin).to_owned();
        Some(Fichier { chemin: chemin.clone(), nom, taille: self.tailles.get(i).copied().unwrap_or(0) })
    }
}

fn rang(vue: Vue) -> usize {
    match vue {
        Vue::Textures => 0,
        Vue::Modeles => 1,
        Vue::Sons => 2,
        Vue::Videos => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_exemple() -> IndexVfs {
        IndexVfs::depuis(vec![
            ("data/dx11/tex/a.g4tx".to_owned(), 10),
            ("data/dx11/tex/b.g4tx".to_owned(), 20),
            ("data/dx11/tex/sub/c.g4tx".to_owned(), 30),
            ("data/common/chr/c01000010.g4md".to_owned(), 40),
            ("data/common/sound/bgm.acb".to_owned(), 50),
            ("data/common/movie/op.usm".to_owned(), 60),
            ("data/common/movie/op.usm".to_owned(), 60),
        ])
    }

    #[test]
    fn vues_comptent_ce_qu_elles_filtrent() {
        let idx = index_exemple();
        assert_eq!(idx.len(), 6, "le doublon est ecarte");
        assert_eq!(idx.compte_vue(Vue::Textures), 3);
        assert_eq!(idx.compte_vue(Vue::Modeles), 1);
        assert_eq!(idx.compte_vue(Vue::Sons), 1);
        assert_eq!(idx.compte_vue(Vue::Videos), 1);
        assert_eq!(VUES.len(), 4);
    }

    #[test]
    fn dossier_separe_fichiers_et_sous_dossiers() {
        let idx = index_exemple();
        let d = idx.dossier("data/dx11/tex", 0, 50);
        assert_eq!(d.total_fichiers, 2);
        assert_eq!(d.dossiers, vec!["data/dx11/tex/sub".to_owned()]);
        assert_eq!(d.fichiers[0].nom, "a.g4tx");
        assert_eq!(d.fichiers[0].taille, 10);

        let racine = idx.dossier("", 0, 50);
        assert_eq!(racine.dossiers, vec!["data".to_owned()]);
        assert_eq!(racine.total_fichiers, 0);
    }

    #[test]
    fn pagination_de_vue() {
        let idx = index_exemple();
        assert_eq!(idx.page_vue(Vue::Textures, 0, 2).len(), 2);
        assert_eq!(idx.page_vue(Vue::Textures, 2, 2).len(), 1);
        assert_eq!(idx.page_vue(Vue::Textures, 9, 2).len(), 0);
    }

    #[test]
    fn extension_insensible_a_la_casse() {
        assert!(Vue::Textures.retient("data/A.G4TX"));
        assert!(!Vue::Textures.retient("data/sans_extension"));
        assert_eq!(Vue::depuis_segment("modeles"), Some(Vue::Modeles));
        assert_eq!(Vue::depuis_segment("inexistante"), None);
    }
}
