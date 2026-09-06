//! Listing de répertoire du VFS — la vue « dossier » que l'explorateur affiche.
//!
//! Le VFS est un index plat (`chemin interne → CPK conteneur`) : il n'a pas de notion de
//! répertoire. Cette vue-là est donc CALCULÉE, et elle l'était jusqu'ici en trois endroits qui
//! divergeaient : la commande Tauri `vfs_ls` de l'app desktop, `niers vfs ls` (CLI), et — sans
//! passer par le VFS du tout — l'index SQLite figé de l'explorateur web d'azalée, régénéré à la
//! main depuis un NDJSON.
//!
//! Une seule implémentation ici, trois façades : IPC Tauri, texte CLI, et JSON HTTP
//! (`nie-model-serve`, ce qui branche l'explorateur web sur le vrai VFS au lieu d'un index gelé).
//!
//! Coût : un balayage O(n) des ~250 800 entrées par appel. Mesuré sous la milliseconde à l'échelle
//! du VFS complet, contre une requête SQLite indexée — la différence ne se voit pas derrière le
//! réseau, et elle s'échange contre un index qui ne peut plus être périmé.

use nie_formats::vfs::Vfs;
use std::collections::BTreeMap;

/// Un sous-dossier direct, avec le nombre de fichiers qu'il contient (récursivement).
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Nom du segment, ex. `menu`.
    pub name: String,
    /// Nombre de fichiers sous ce sous-dossier, tous niveaux confondus.
    pub count: usize,
}

/// Un fichier directement contenu dans le répertoire listé.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Nom de fichier, ex. `c01000010_l.g4tx`.
    pub name: String,
    /// Extension en minuscules, sans point. Vide si le nom n'en porte pas.
    pub ext: String,
    /// Chemin interne complet, ex. `data/dx11/menu/.../c01000010_l.g4tx`.
    pub path: String,
    /// Taille décompressée, en octets.
    pub size: u32,
    /// Nom du CPK conteneur.
    pub cpk: String,
}

/// Le rôle catalogué d'un dossier (cf. [`crate::folder_roles`]), quand il en a un.
#[derive(Debug, Clone)]
pub struct Role {
    /// Ce à quoi le dossier sert, en clair.
    pub role: String,
    /// État de l'exploitation de ce dossier par le dépôt.
    pub status: String,
}

/// Vue « dossier » paginée.
#[derive(Debug, Clone)]
pub struct Listing {
    /// Le répertoire listé, normalisé (sans slash final).
    pub dir: String,
    /// Sous-dossiers directs, triés par nom. **Jamais** paginés : ils forment l'arbre.
    pub dirs: Vec<DirEntry>,
    /// Fichiers directs, triés par nom, tranche `[offset, offset + limit)`.
    pub files: Vec<FileEntry>,
    /// Nombre total de fichiers directs, avant pagination.
    pub file_total: usize,
    /// Décalage effectivement appliqué.
    pub file_offset: usize,
    /// Rôle catalogué du dossier, `None` si non catalogué — jamais un rôle deviné.
    pub role: Option<Role>,
}

/// Extension en minuscules d'un nom de fichier, sans le point. Vide si absente.
///
/// Un nom qui commence par un point (`.gitkeep`) n'a pas d'extension : `rfind` retournerait 0 et
/// produirait l'extension `gitkeep` pour un fichier qui n'en a pas.
fn ext_de(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[i + 1..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Ce qui reste du chemin sous `prefix`, ou `None` si le chemin n'est pas dessous.
///
/// `prefix` vide = racine : tout chemin est « dessous ».
fn sous_prefixe<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return Some(path);
    }
    if path == prefix {
        return None;
    }
    path.strip_prefix(prefix)?.strip_prefix('/')
}

/// Liste le contenu direct de `prefix`, fichiers paginés, sous-dossiers complets et comptés.
///
/// `limit` à 0 renvoie les sous-dossiers et `file_total` sans aucun fichier — c'est ce que veut
/// un arbre qui n'affiche que la structure.
#[must_use]
pub fn ls_paged(vfs: &Vfs, prefix: &str, limit: usize, offset: usize) -> Listing {
    let prefix = prefix.trim_matches('/');

    let mut dirs: BTreeMap<&str, usize> = BTreeMap::new();
    let mut files: Vec<FileEntry> = Vec::new();

    for (path, entry) in vfs.iter() {
        let Some(rest) = sous_prefixe(path, prefix) else {
            continue;
        };
        match rest.split_once('/') {
            // Un descendant : il compte pour le sous-dossier de premier niveau qui le porte.
            Some((seg, _)) => *dirs.entry(seg).or_insert(0) += 1,
            None => files.push(FileEntry {
                name: rest.to_string(),
                ext: ext_de(rest),
                path: path.to_string(),
                size: entry.file_size,
                cpk: entry.cpk_filename.clone(),
            }),
        }
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    let file_total = files.len();
    let files = files.into_iter().skip(offset).take(limit).collect();

    Listing {
        dir: prefix.to_string(),
        dirs: dirs
            .into_iter()
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, count)| DirEntry {
                name: name.to_string(),
                count,
            })
            .collect(),
        files,
        file_total,
        file_offset: offset,
        role: crate::folder_roles::describe_folder(prefix).map(|r| Role {
            role: r.role.to_string(),
            status: r.status.to_string(),
        }),
    }
}

/// Résultat d'une recherche : la tranche demandée, et le nombre TOTAL de correspondances.
#[derive(Debug, Clone)]
pub struct FindResult {
    /// Correspondances de la tranche `[offset, offset + limit)`, triées par chemin.
    pub files: Vec<FileEntry>,
    /// Nombre total de correspondances dans tout le VFS, avant pagination.
    pub total: usize,
    /// Décalage effectivement appliqué.
    pub offset: usize,
}

/// Cherche par sous-chaîne dans les chemins internes, filtrable par extension.
///
/// La comparaison est insensible à la casse des deux côtés : les chemins du VFS sont en
/// minuscules, mais la requête vient d'un champ de saisie.
///
/// Le balayage est TOTAL avant pagination : un appelant qui demande une page en connaît le
/// dénominateur. Tronquer pendant le balayage (l'ancienne forme) rendait `count` indiscernable
/// de la limite — 2 000 fichiers trouvés et 2 000 fichiers existants s'écrivaient pareil, ce qui
/// interdit à une galerie de savoir si elle est complète.
#[must_use]
pub fn find_paged(
    vfs: &Vfs,
    query: &str,
    ext: Option<&str>,
    limit: usize,
    offset: usize,
) -> FindResult {
    let besoin = query.to_ascii_lowercase();
    let ext_voulue = ext.map(str::to_ascii_lowercase);

    let mut out: Vec<FileEntry> = Vec::new();
    for (path, entry) in vfs.iter() {
        if !besoin.is_empty() && !path.to_ascii_lowercase().contains(&besoin) {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let e = ext_de(name);
        if let Some(voulue) = &ext_voulue
            && &e != voulue
        {
            continue;
        }
        out.push(FileEntry {
            name: name.to_string(),
            ext: e,
            path: path.to_string(),
            size: entry.file_size,
            cpk: entry.cpk_filename.clone(),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    let total = out.len();
    FindResult {
        files: out.into_iter().skip(offset).take(limit).collect(),
        total,
        offset,
    }
}

/// Première page de résultats — forme courte de [`find_paged`].
#[must_use]
pub fn find(vfs: &Vfs, query: &str, ext: Option<&str>, limit: usize) -> Vec<FileEntry> {
    find_paged(vfs, query, ext, limit, 0).files
}

/// Métadonnées d'une entrée précise, `None` si le chemin n'est pas un fichier du VFS.
#[must_use]
pub fn stat(vfs: &Vfs, path: &str) -> Option<FileEntry> {
    let entry = vfs.find(path)?;
    let name = path.rsplit('/').next().unwrap_or(path);
    Some(FileEntry {
        name: name.to_string(),
        ext: ext_de(name),
        path: path.to_string(),
        size: entry.file_size,
        cpk: entry.cpk_filename.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_minuscule_sans_point() {
        assert_eq!(ext_de("c01.G4TX"), "g4tx");
        assert_eq!(ext_de("sans_extension"), "");
        // Un fichier caché n'a pas d'extension : le point en tête n'en ouvre pas une.
        assert_eq!(ext_de(".gitkeep"), "");
        // Double extension : seule la dernière compte, comme partout ailleurs dans le dépôt.
        assert_eq!(ext_de("item.cfg.bin"), "bin");
    }

    #[test]
    fn prefixe_racine_prend_tout() {
        assert_eq!(sous_prefixe("data/a/b.g4tx", ""), Some("data/a/b.g4tx"));
        assert_eq!(sous_prefixe("data/a/b.g4tx", "data/a"), Some("b.g4tx"));
        // Le dossier lui-même n'est pas son propre descendant.
        assert_eq!(sous_prefixe("data/a", "data/a"), None);
        // Un préfixe qui n'est pas une frontière de segment ne matche pas.
        assert_eq!(sous_prefixe("data/abc/x", "data/a"), None);
    }
}
