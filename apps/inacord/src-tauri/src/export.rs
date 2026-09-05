//! Façade IPC de l'export « au format voulu ».
//!
//! La table des formats, le nom de sortie et les conversions vivent dans
//! [`nie_explore::export`] — même règle que le reste de ce backend (« ce module n'est qu'une
//! façade au-dessus de `nie-formats`/`nie-explore` »), et c'est là que les tests s'exécutent
//! réellement : le harnais de test de ce paquet Tauri ne démarre pas sur cette plateforme
//! (`STATUS_ENTRYPOINT_NOT_FOUND` avant le premier test, cf. `CLAUDE.md`).
//!
//! Ne reste ici que le seul format qui demande un contexte propre à l'application
//! (cf. `nie_explore::export::necessite_contexte`) : **`glb`**, qui a besoin des fichiers FRÈRES
//! du VFS (`.g4mg`, `.g4tx` de même radical) et de la résolution de voisinage sachant dire
//! *pourquoi* un frère manque (`assemble_glb_for_preview`).
//!
//! `mp4` y figurait tant qu'il lançait `ffmpeg` ; le remux vit maintenant dans `nie-formats`, donc
//! il se produit comme les autres conversions.

use serde::Serialize;

use crate::{assemble_glb_for_preview, isoler};

/// Un format d'export proposé pour un fichier donné (miroir IPC de
/// [`nie_explore::export::FormatExport`]).
#[derive(Serialize, specta::Type)]
pub struct ExportFormatDto {
    /// Identifiant à repasser à `vfs_export_as` (`raw`, `png`, `glb`, `json`, `wav`, `mp4`…).
    pub id: String,
    /// Extension du fichier produit, sans le point.
    pub ext: String,
    /// Libellé affiché.
    pub label: String,
    /// Vrai pour « tel quel » : aucune conversion, donc aucune perte possible.
    pub brut: bool,
    /// Faux quand la conversion peut dégrader (JPEG, GIF).
    pub sans_perte: bool,
}

/// Formats d'export possibles pour `path`, le brut en tête.
#[must_use]
pub fn formats_pour(path: &str) -> Vec<ExportFormatDto> {
    nie_explore::export::formats_pour(path)
        .into_iter()
        .map(|f| ExportFormatDto {
            id: f.id,
            ext: f.ext,
            label: f.label,
            brut: f.brut,
            sans_perte: f.sans_perte,
        })
        .collect()
}

/// Nom de fichier proposé pour `path` exporté en `format`.
#[must_use]
pub fn nom_propose(path: &str, format: &str) -> String {
    nie_explore::export::nom_propose(path, format)
}

/// Produit les octets de `path` (déjà lu dans `data`) convertis vers `format`.
///
/// `vfs` ne sert qu'aux formats contextuels (cf. en-tête) ; tout le reste passe par
/// [`nie_explore::export::produire`].
///
/// # Erreurs
///
/// Rend un message si le format est inconnu pour ce fichier, si le décodage échoue, ou si un
/// outil externe manque (`ffmpeg` pour `mp4`).
pub fn produire(
    vfs: &nie_formats::vfs::Vfs,
    path: &str,
    data: Vec<u8>,
    format: &str,
) -> Result<Vec<u8>, String> {
    match format {
        "glb" => assemble_glb_for_preview(vfs, path).map(|(_stem, glb)| glb),
        // Isolé : décoder une texture de 2048² ou un banc audio complet sur un thread dédié à
        // pile large évite qu'un fichier atypique n'emporte la fenêtre entière (cf. `isoler`).
        autre => {
            let path = path.to_string();
            let autre = autre.to_string();
            isoler("conversion à l'export", move || {
                nie_explore::export::produire(&path, data, &autre)
            })?
        }
    }
}
