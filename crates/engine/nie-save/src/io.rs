//! Utilitaires `std`-dépendants pour lire/écrire les saves depuis le disque.
//!
//! Ce module est séparé du reste de la bibliothèque (qui reste `no_std + alloc`-friendly)
//! pour garder le cœur portable wasm/no_std.

use std::fs;
use std::path::Path;

use crate::{LivesContainer, parse};

/// Lit, déchiffre et parse un fichier de sauvegarde depuis `path`.
///
/// Le nom de fichier est extrait automatiquement de `path` pour dériver la clé CRC32.
///
/// # Erreurs
///
/// Propage les erreurs d'I/O comme [`SaveError::Corrupt`] (message inclus).
pub fn read_save(path: &Path) -> Result<LivesContainer, Box<dyn std::error::Error>> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("chemin invalide : {}", path.display()))?;

    let data = fs::read(path).map_err(|e| format!("lecture {} : {e}", path.display()))?;

    Ok(parse(&data, filename)?)
}

/// Rechiffre et écrit un conteneur de sauvegarde vers `path`.
///
/// # Erreurs
///
/// Propage les erreurs d'I/O et de sérialisation.
pub fn write_save(
    container: &LivesContainer,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let encrypted = container.encrypt()?;
    fs::write(path, encrypted).map_err(|e| format!("écriture {} : {e}", path.display()).into())
}

/// Affiche un résumé terse du conteneur (format `clé=val`, convention niers).
pub fn print_summary(container: &LivesContainer) {
    println!(
        "slot={} key=0x{:08X} entries={} blobs={}",
        container.slot_name,
        container.key,
        container.entries.len(),
        container.blobs.len(),
    );
    for (i, (entry, blob)) in container
        .entries
        .iter()
        .zip(container.blobs.iter())
        .enumerate()
    {
        println!(
            "  blob[{i}] name={} size={} crc32=0x{:08X} subtype={:?} field8=0x{:08X}",
            entry.filename, entry.size, entry.crc32, blob.header.subtype, blob.header.field8,
        );
    }
}

/// Imprime les N premiers octets du corps d'un blob en hexadécimal.
pub fn hexdump_blob_body(blob: &crate::Blob, limit: usize) {
    let n = limit.min(blob.body.len());
    let hex: String = blob.body[..n]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("body[0..{n}]: {hex}");
    if n < blob.body.len() {
        println!("  ... ({} octets restants)", blob.body.len() - n);
    }
}

/// Édite un byte dans le corps d'un blob par nom de fichier interne.
///
/// Retourne `true` si l'édition a réussi, `false` si le blob ou l'offset est introuvable.
pub fn edit_blob_byte(
    container: &mut LivesContainer,
    internal_filename: &str,
    offset: usize,
    value: u8,
) -> bool {
    for (entry, blob) in container.entries.iter().zip(container.blobs.iter_mut()) {
        if entry.filename == internal_filename && offset < blob.body.len() {
            blob.body[offset] = value;
            // Ne pas mettre à jour payload_size : ce champ est interne au moteur jeu
            // et n'est pas nécessairement la taille du corps (ex. AUTOSAVE).
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Blob, BlobHeader, BlobSubtype, DirEntry, LivesContainer};

    fn fake_container() -> LivesContainer {
        let body = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let blob = Blob {
            header: BlobHeader {
                subtype: BlobSubtype::System,
                payload_size: body.len() as u32,
                field8: 0,
            },
            body: body.clone(),
        };
        let blob_bytes = blob.to_bytes();
        let crc32 = crate::crc32_of_pub(&blob_bytes);

        LivesContainer {
            slot_name: "TEST-SYSTEMLIVE".to_string(),
            key: crate::key_from_filename("TEST-SYSTEMLIVE"),
            entries: vec![DirEntry {
                crc32,
                size: blob_bytes.len() as u32,
                offset: 0,
                filename: "SYSTEM_data.bin".to_string(),
            }],
            blobs: vec![blob],
        }
    }

    #[test]
    fn round_trip_io_fichier() {
        let container = fake_container();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write_save(&container, tmp.path()).unwrap();

        let data = std::fs::read(tmp.path()).unwrap();
        let container2 = crate::parse(&data, "TEST-SYSTEMLIVE").unwrap();

        assert_eq!(container2.slot_name, container.slot_name);
        assert_eq!(container2.blobs[0].body, container.blobs[0].body);
    }

    #[test]
    fn edit_blob_byte_modifie_valeur() {
        let mut container = fake_container();
        let ok = edit_blob_byte(&mut container, "SYSTEM_data.bin", 0, 0xAA);
        assert!(ok);
        assert_eq!(container.blobs[0].body[0], 0xAA);
    }

    #[test]
    fn edit_blob_byte_hors_limites_retourne_false() {
        let mut container = fake_container();
        let ok = edit_blob_byte(&mut container, "SYSTEM_data.bin", 999, 0xFF);
        assert!(!ok);
    }

    #[test]
    fn edit_blob_byte_mauvais_nom_retourne_false() {
        let mut container = fake_container();
        let ok = edit_blob_byte(&mut container, "INCONNU.bin", 0, 0xFF);
        assert!(!ok);
    }
}
