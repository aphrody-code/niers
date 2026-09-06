//! Manipulation de noms de chemins d'assets, portée **byte-exact** du moteur Level-5 (`lives::`).
//!
//! [`file_stem`] reproduit `FUN_140452820` (@`0x140452820`, `nie_eacpatched.exe`, ~171 callers) :
//! le *file-stem* d'un chemin d'asset = **basename sans dernière extension** (retire le répertoire
//! au dernier `/` ou `\`, puis l'extension au dernier `.`). Utilitaire central de la couche asset.
//!
//! ## Quirks byte-exact (≠ [`core`]/std)
//!
//! Le scan du séparateur ne s'exécute que si la longueur est `> 2`, et seulement sur les indices
//! `[1, len-2]` : un séparateur en **position 0** ou en **dernière position** n'est PAS reconnu
//! (vérifié via l'oracle uemu). L'extension est coupée au **dernier** `.` (`strrchr`,
//! `FUN_14098f4e0`), y compris en tête (`".foo"` → stem vide).
//!
//! Validation : `scripts/validate_path_stem.py` (1 220 cas fuzz + bords, byte-exact vs uemu) ;
//! la chaîne du jeu étant un C-string (lu jusqu'au `\0`), passer ici le contenu **sans** le `\0`.

/// File-stem byte-exact du moteur (`FUN_140452820`) : basename sans dernière extension.
///
/// Renvoie un sous-slice de `path` (zéro-copie). Voir le module-doc pour les quirks (séparateur
/// ignoré en position 0 ou dernière ; coupe au **dernier** `.`).
#[must_use]
pub fn file_stem(path: &[u8]) -> &[u8] {
    let n = path.len();
    // 1) basename : dernier '/' ou '\\' parmi les indices [1, n-2], seulement si n > 2.
    let mut base = path;
    if n > 2 {
        for i in (1..=n - 2).rev() {
            if path[i] == b'/' || path[i] == b'\\' {
                base = &path[i + 1..];
                break;
            }
        }
    }
    // 2) retire l'extension au DERNIER '.' (strrchr) ; absent → basename entier.
    match base.iter().rposition(|&c| c == b'.') {
        Some(dot) => &base[..dot],
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::file_stem;

    /// Cas golden capturés de l'oracle uemu (`FUN_140452820`).
    #[test]
    fn matches_uemu_oracle() {
        // (chemin, stem attendu)
        let cases: &[(&[u8], &[u8])] = &[
            (b"data/menu/main_menu_1.02.lua.bin", b"main_menu_1.02.lua"),
            (b"foo\\bar.baz.qux", b"bar.baz"),
            (b"chr/k0001/k0001.g4mdl", b"k0001"),
            (b"noext", b"noext"),
            (b"a.b", b"a"),
            (b"a/b/c", b"c"),
            (b"a/.config", b""), // basename ".config", dernier '.' en tête → vide
            (b"", b""),
            (b".", b""), // n<=2, pas de scan ; dernier '.' index 0 → vide
            (b"..", b"."),
            (b"a.", b"a"),
            (b".a", b""),
            (b"/x", b"/x"), // séparateur en position 0 ignoré + n<=2
            (b"x/", b"x/"), // séparateur en dernière position ignoré + n<=2
            (b"...", b".."),
            (b"a..b", b"a."),
        ];
        for &(path, want) in cases {
            assert_eq!(
                file_stem(path),
                want,
                "file_stem({:?})",
                core::str::from_utf8(path).unwrap_or("<non-utf8>")
            );
        }
    }

    /// Le résultat est bien un sous-slice contigu de l'entrée (zéro-copie).
    #[test]
    fn is_subslice_zero_copy() {
        let p = b"dir/name.ext";
        let s = file_stem(p);
        assert_eq!(s, b"name");
        // l'adresse du stem doit tomber à l'intérieur du buffer d'entrée
        let base = p.as_ptr() as usize;
        let got = s.as_ptr() as usize;
        assert!((base..base + p.len()).contains(&got));
    }
}
