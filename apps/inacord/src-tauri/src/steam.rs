//! Détection réelle de l'installation Steam d'*Inazuma Eleven: Victory Road* — remplace
//! l'ancien repli codé en dur (`/home/aphrody/niers`, un chemin de dev WSL qui n'existe sur
//! aucune machine utilisatrice) par une VRAIE résolution via le registre Windows + le format
//! `libraryfolders.vdf` de Steam, exactement comme le fait le client Steam lui-même.
//!
//! AppID confirmé sur cette machine (`steamapps/appmanifest_2799860.acf`, champ `"appid"
//! "2799860"`, `"name" "INAZUMA ELEVEN: Victory Road"`) — pas deviné.
//!
//! Algorithme (identique à celui que Steam utilise pour retrouver ses propres jeux) :
//! 1. Registre : `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath` (build 64 bits, vue
//!    32 bits où Steam s'enregistre), repli `HKCU\Software\Valve\Steam\SteamPath`.
//! 2. Le dossier d'installation Steam lui-même compte aussi comme bibliothèque (cas le plus
//!    courant : le jeu est sur le même disque que Steam) — vérifié en premier, sans attendre
//!    le parsing VDF.
//! 3. `<install Steam>/steamapps/libraryfolders.vdf` : ce fichier texte (format VDF de Valve)
//!    liste toutes les bibliothèques Steam (disque secondaire compris) — on en extrait chaque
//!    valeur `"path"`.
//! 4. Pour chaque bibliothèque, on cherche `steamapps/appmanifest_2799860.acf` ; s'il existe,
//!    son champ `"installdir"` donne le nom du dossier sous `steamapps/common/`.
//! 5. Le chemin candidat est validé par la présence réelle de `data/cpk_list.cfg.bin` (même
//!    critère que [`nie_formats::vfs::resolve_game_dir`]) avant d'être retourné — jamais un
//!    chemin deviné/non vérifié.

use std::path::{Path, PathBuf};

/// AppID Steam d'*Inazuma Eleven: Victory Road*, confirmé via `appmanifest_2799860.acf` réel
/// sur cette installation (pas une valeur supposée).
const STEAM_APP_ID: &str = "2799860";

/// Sous-dossier d'installation attendu (`"installdir"` de l'appmanifest), utilisé comme
/// dernier repli si l'`.acf` est absent/illisible mais que la bibliothèque est valide.
const INSTALL_DIR_FALLBACK: &str = "INAZUMA ELEVEN Victory Road";

/// Vérifie qu'un dossier candidat est bien une installation du jeu (même critère que
/// `nie_formats::vfs::resolve_game_dir`) : `data/cpk_list.cfg.bin` doit exister.
fn is_valid_install(dir: &Path) -> bool {
    dir.join("data").join("cpk_list.cfg.bin").is_file()
}

/// Lit le chemin d'installation de Steam depuis le registre Windows.
#[cfg(target_os = "windows")]
fn steam_install_path() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("InstallPath") {
            return Some(PathBuf::from(path));
        }
    }
    if let Ok(key) = hklm.open_subkey(r"SOFTWARE\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("InstallPath") {
            return Some(PathBuf::from(path));
        }
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"Software\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            return Some(PathBuf::from(path));
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn steam_install_path() -> Option<PathBuf> {
    None
}

/// Extrait toutes les valeurs `"path" "..."` d'un `libraryfolders.vdf` (format texte VDF de
/// Valve — un scan ligne à ligne suffit, pas besoin d'un parseur VDF complet pour cette seule
/// clé). Les `\\` échappés du VDF sont dépliés en `\`.
fn parse_library_paths(vdf: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in vdf.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("\"path\"") else { continue };
        let rest = rest.trim();
        let Some(start) = rest.find('"') else { continue };
        let Some(end) = rest[start + 1..].find('"') else { continue };
        let raw = &rest[start + 1..start + 1 + end];
        out.push(PathBuf::from(raw.replace("\\\\", "\\")));
    }
    out
}

/// Lit `"installdir"` d'un fichier `appmanifest_*.acf` (même format VDF minimal).
fn parse_installdir(acf: &str) -> Option<String> {
    for line in acf.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("\"installdir\"") {
            let rest = rest.trim();
            let start = rest.find('"')?;
            let end = rest[start + 1..].find('"')?;
            return Some(rest[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

/// Bibliothèques Steam candidates : dossier d'installation de Steam lui-même en premier
/// (cas le plus courant), puis toutes celles listées dans `libraryfolders.vdf` (disques
/// secondaires).
fn candidate_libraries(steam_install: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_install.to_path_buf()];
    let vdf_path = steam_install.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(&vdf_path) {
        for p in parse_library_paths(&text) {
            if !libs.contains(&p) {
                libs.push(p);
            }
        }
    }
    libs
}

/// Résout le dossier d'installation du jeu via une VRAIE détection Steam (registre +
/// bibliothèques + appmanifest), validée par la présence de `data/cpk_list.cfg.bin`.
/// Retourne `None` si Steam n'est pas installé ou que le jeu n'est trouvé dans aucune
/// bibliothèque — JAMAIS un chemin deviné.
pub fn detect_game_dir() -> Option<PathBuf> {
    let steam_install = steam_install_path()?;
    for lib in candidate_libraries(&steam_install) {
        let steamapps = lib.join("steamapps");
        let acf = steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf"));
        let installdir = std::fs::read_to_string(&acf)
            .ok()
            .and_then(|t| parse_installdir(&t))
            .unwrap_or_else(|| INSTALL_DIR_FALLBACK.to_string());
        let candidate = steamapps.join("common").join(installdir);
        if is_valid_install(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Candidats de sauvegarde `*-USERDATALIVE` sous TOUTES les bibliothèques Steam, TOUS les
/// `userdata/<steamid>/2799860/remote/` trouvés (plusieurs comptes Windows/Steam sur le même
/// poste sont possibles — pas d'API simple pour « le compte actif » sans le client Steam en
/// cours d'exécution, donc on énumère large et on laisse [`pick_best_save`] trancher par preuve
/// — mtime + validité réelle — plutôt que de deviner un seul compte). `remote/` (pas la racine
/// `2799860/`) : c'est le sous-dossier synchronisé par Steam Cloud, confirmé sur cette
/// installation (`002AB8F4-USERDATALIVE`, `002AB8F4-SYSTEMLIVE`).
pub fn userdata_save_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(steam_install) = steam_install_path() else { return out };
    for lib in candidate_libraries(&steam_install) {
        let userdata = lib.join("userdata");
        let Ok(accounts) = std::fs::read_dir(&userdata) else { continue };
        for account in accounts.flatten() {
            let remote = account.path().join(STEAM_APP_ID).join("remote");
            let Ok(files) = std::fs::read_dir(&remote) else { continue };
            for f in files.flatten() {
                let name = f.file_name();
                let name = name.to_string_lossy();
                if name.ends_with("-USERDATALIVE") {
                    out.push(f.path());
                }
            }
        }
    }
    out
}

/// Choisit LA meilleure sauvegarde parmi [`userdata_save_candidates`] : trie par date de
/// modification décroissante (le fichier le plus récent est quasi-toujours le plus complet — la
/// taille croît avec le temps de jeu, cf. `002AB8F4-USERDATALIVE` 12,5 Mo/récent vs
/// `002B8D10-USERDATALIVE` 2,2 Mo/2024 sur cette installation), puis VALIDE réellement chaque
/// candidat via `is_valid` (déchiffrement/parse `nie-save` réel, pas une supposition sur le nom
/// de fichier) jusqu'à en trouver un qui parse — un fichier corrompu/tronqué ne doit jamais être
/// silencieusement retenu juste parce qu'il est le plus récent.
pub fn pick_best_save(is_valid: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut candidates = userdata_save_candidates();
    candidates.sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    candidates.into_iter().find(|p| is_valid(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_library_paths_extrait_les_chemins() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;
        let paths = parse_library_paths(vdf);
        assert_eq!(paths, vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]);
    }

    #[test]
    fn parse_installdir_extrait_le_dossier() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"2799860\"\n\t\"installdir\"\t\t\"INAZUMA ELEVEN Victory Road\"\n}\n";
        assert_eq!(parse_installdir(acf).as_deref(), Some("INAZUMA ELEVEN Victory Road"));
    }

    /// Détection de bout en bout sur la VRAIE installation Steam de ce poste (registre +
    /// `libraryfolders.vdf` + `appmanifest_2799860.acf` réels) — prouve que
    /// `detect_game_dir()` retrouve le jeu sans variable d'environnement ni chemin codé en
    /// dur, contrairement à l'ancien repli `/home/aphrody/niers`. Skip (pas d'échec) sur une
    /// machine sans Steam/le jeu.
    #[test]
    fn detect_game_dir_trouve_le_vrai_jeu_steam() {
        let Some(dir) = detect_game_dir() else {
            eprintln!("skip detect_game_dir_trouve_le_vrai_jeu_steam : Steam/le jeu absent de ce poste");
            return;
        };
        assert!(is_valid_install(&dir), "chemin détecté invalide : {}", dir.display());
        eprintln!("jeu détecté via Steam : {}", dir.display());
    }

    /// Bout en bout sur la VRAIE sauvegarde Steam Cloud de ce poste : `pick_best_save` doit
    /// retrouver un `*-USERDATALIVE` qui déchiffre/parse RÉELLEMENT via `nie-save` (pas juste un
    /// nom de fichier qui matche). Skip (pas d'échec) sur un poste sans sauvegarde du jeu.
    #[test]
    fn pick_best_save_trouve_une_vraie_sauvegarde_valide() {
        let candidates = userdata_save_candidates();
        if candidates.is_empty() {
            eprintln!("skip pick_best_save_trouve_une_vraie_sauvegarde_valide : aucune sauvegarde locale");
            return;
        }
        let best = pick_best_save(|p| nie_save::io::read_save(p).is_ok());
        let Some(best) = best else {
            panic!("des candidats existent ({}) mais aucun ne déchiffre — régression réelle", candidates.len());
        };
        eprintln!("meilleure sauvegarde détectée : {}", best.display());
        assert!(nie_save::io::read_save(&best).is_ok());
    }
}
