//! Opérations de modding LEVEL-5 — le périmètre de l'outil **Viola** (`SuperTavor/Viola`), en
//! Rust pur : *dump*, *pack*, *merge*, et le (dé)chiffrement Criware.
//!
//! # Pourquoi ce crate existe
//!
//! Le dépôt portait déjà ces opérations deux fois — en C# (`csharp/IECODE.Core`) et en C++
//! (`src/viola/`) — mais aucune n'était atteignable depuis `nie-explorer` ni `niers` sans
//! lancer un binaire construit à côté. La doctrine du dépôt met la CLI, la GUI et le cœur en
//! Rust ; ce crate est ce cœur, et il s'appelle en process.
//!
//! # Ce qui n'est pas ici
//!
//! Le cinquième bouton de Viola, *Decrypt/Encrypt Criware*, n'a pas eu besoin d'être porté : il
//! existait déjà dans [`nie_formats::cpk::decrypt_block`], port fidèle de
//! `CriwareCrypt.DecryptBlock`. Le module [`crypto`] ne fait que lui donner une façade fichier
//! et nommer la propriété qui rend l'opération unique : le XOR est **involutif**, chiffrer et
//! déchiffrer sont le même calcul.
//!
//! # Écarts assumés avec l'amont
//!
//! * Le `pack` C++ ne connaît que l'enveloppe XorShift, alors que le `cpk_list.cfg.bin` d'IEVR
//!   est chiffré en **AES-256-CBC** (clé et IV reversés de `nie.exe`). [`pack`] relit et
//!   réécrit dans l'enveloppe réellement trouvée, quelle qu'elle soit.
//! * Le `dump` amont attribue un thread par pack en piochant dans une liste non triée ; [`dump`]
//!   ordonnance les packs par volume décroissant, les mappe en mémoire et sait reprendre un
//!   dump interrompu. Le détail et les raisons sont dans la documentation de ce module.

#![warn(missing_docs)]

pub mod crypto;
pub mod dump;
pub mod filtre;
pub mod manifeste;
pub mod merge;
pub mod pack;
pub mod patch;
pub mod presets;
pub mod verify;

pub use crypto::{CriwareKey, crypt_bytes, crypt_file};
pub use dump::{DumpOptions, DumpProgress, DumpReport, Echec, Raison, dump_all};
pub use filtre::Filtre;
pub use manifeste::{Dependance, FichierMod, Manifeste, Version, ordonner};
pub use merge::{Conflit, MergeReport, MergeStrategy, merge_dirs};
pub use pack::{CpkListCrypto, PackReport, Platform, decode_cpk_list, encode_cpk_list, pack_mod};
pub use verify::{Anomalie, Constat, VerifOptions, VerifReport, verifier};

use std::path::{Path, PathBuf};

/// Filtre glob minimal — seul `*` est reconnu, ce qui couvre les motifs réellement utilisés
/// (`*.g4tx`, `data/chr/*`). Écrit à la main plutôt qu'en tirant une dépendance de glob : la
/// syntaxe complète (classes, `**`, échappements) n'a aucun appelant ici et coûterait un crate.
#[must_use]
pub fn glob_match(motif: &str, texte: &str) -> bool {
    let mut morceaux = motif.split('*');
    let Some(debut) = morceaux.next() else {
        return true;
    };
    if !texte.starts_with(debut) {
        return false;
    }
    let mut pos = debut.len();
    let mut dernier = "";
    let mut a_joker = false;
    for m in morceaux {
        a_joker = true;
        dernier = m;
        if m.is_empty() {
            continue;
        }
        match texte[pos..].find(m) {
            Some(i) => pos += i + m.len(),
            None => return false,
        }
    }
    if !a_joker {
        return texte == motif; // aucun `*` : égalité stricte
    }
    // Un motif qui ne finit pas par `*` doit coller jusqu'au bout du texte.
    dernier.is_empty() || texte.ends_with(dernier)
}

/// Énumère récursivement les fichiers d'un dossier, en chemins relatifs normalisés (`/`).
///
/// L'ordre est **stable** (tri par chemin) : deux exécutions du même *pack* ou du même *merge*
/// produisent alors des sorties identiques, ce qui rend les résultats comparables et
/// diffusables sans bruit.
///
/// # Errors
/// Si un répertoire est illisible.
pub fn enumerer(racine: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out = Vec::new();
    let mut pile = vec![racine.to_path_buf()];
    while let Some(dir) = pile.pop() {
        let lecture = std::fs::read_dir(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        for e in lecture.flatten() {
            let chemin = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => pile.push(chemin),
                Ok(t) if t.is_file() => {
                    let Ok(rel) = chemin.strip_prefix(racine) else {
                        continue;
                    };
                    out.push((chemin.clone(), rel.to_string_lossy().replace('\\', "/")));
                }
                // Les liens et les entrées illisibles sont ignorés plutôt que de faire échouer
                // toute l'opération sur un cas marginal.
                _ => {}
            }
        }
    }
    out.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_glob_ne_reconnait_que_l_etoile() {
        assert!(glob_match("*.g4tx", "data/chr/c01.g4tx"));
        assert!(!glob_match("*.g4tx", "data/chr/c01.g4md"));
        assert!(glob_match("data/chr/*", "data/chr/c01.g4md"));
        assert!(!glob_match("data/chr/*", "data/menu/x"));
        assert!(glob_match("*chr*g4md", "data/chr/c01.g4md"));
        assert!(glob_match("*", "n'importe quoi"));
        assert!(glob_match("exact", "exact"));
        assert!(
            !glob_match("exact", "exactement"),
            "sans joker, l'égalité est stricte"
        );
        assert!(!glob_match("exact", "inexact"));
    }

    #[test]
    fn l_enumeration_est_recursive_et_ordonnee() {
        let dir = std::env::temp_dir().join(format!("nie-viola-enum-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("b/c")).expect("arborescence de test");
        std::fs::write(dir.join("z.bin"), b"z").expect("z");
        std::fs::write(dir.join("b/c/a.bin"), b"a").expect("a");
        let v = enumerer(&dir).expect("énumération");
        let rels: Vec<&str> = v.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(
            rels,
            vec!["b/c/a.bin", "z.bin"],
            "récursif, trié, séparateurs normalisés"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
