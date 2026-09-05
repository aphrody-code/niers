//! Live-modding du process `nie.exe` — **lecture ET écriture** de la mémoire du jeu en cours.
//!
//! # Pourquoi un module séparé de [`crate::re_trace`]
//!
//! `re_trace` est, par décision documentée, **strictement lecture seule**. Ce module-ci assume
//! l'écriture : il a été demandé explicitement par l'utilisateur pour modder le jeu en direct
//! depuis l'explorateur. Le cadre reste le même — RE et modding single-player offline d'un jeu
//! possédé, sous l'accord `RG-L5-VR-2026-001`. Aucune des deux surfaces ne touche à EAC : le jeu
//! se lance directement (`nie.exe`), sans `EACLauncher`.
//!
//! # La structure éditée
//!
//! L'équipe active est un tableau de `CraftResidentsStatusP` (0x38 octets par entrée). Ses 24
//! premiers octets sont un `CraftResidentsCharaInfo`, dont les champs sont **nommés par la table
//! de réflexion embarquée dans le binaire** (chaque champ y est enregistré avec son nom, son
//! offset et sa taille ; la clé est le CRC-32 du nom) :
//!
//! | Offset | Type | Champ | CRC-32 du nom |
//! |---|---|---|---|
//! | `+0x00` | u32 | `charaParamId` | `0xF9A1342D` |
//! | `+0x04` | u32 | `uniformId`    | `0xA8E04439` |
//! | `+0x08` | u32 | `shoesId`      | `0x8E02F856` |
//! | `+0x0C` | u32 | `gloveId`      | `0xDC05252E` |
//! | `+0x10` | u32 | `emblemId`     | `0x292566C1` |
//! | `+0x14` | u16 | `uniformNo`    | `0x70730B76` |
//! | `+0x16` | u8  | `scPosNo`      | `0x709A88E9` |
//! | `+0x17` | u8  | `isCaptain`    | `0xC8F2EC9B` |
//!
//! # Localisation
//!
//! L'adresse du tableau change à chaque lancement (allocation dynamique) : elle se retrouve par
//! [`live_find_team`], qui scanne un `charaParamId` connu puis **valide la forme** — deux voisins
//! à ±0x38 portant le même `uniformId` et un `charaParamId` non nul. On ne rend une adresse que
//! si la structure se confirme, jamais sur la seule présence d'une valeur.

use nie_trace::{find_pid_by_name, module_regions, read_exact, write_exact};
use serde::{Deserialize, Serialize};

/// Taille d'une entrée du tableau d'équipe (`CraftResidentsStatusP`).
const STRIDE: u64 = 0x38;
/// Nombre de slots lus autour de la base.
const SLOTS: usize = 29;

/// Noms de process essayés, dans l'ordre.
const PROCESS: [&str; 2] = ["nie.exe", "nie_eacpatched.exe"];

/// Un membre de l'équipe active, champs nommés d'après la réflexion du binaire.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LiveMember {
    /// Index du slot dans le tableau.
    pub slot: u32,
    /// Adresse absolue de l'entrée.
    pub address: String,
    /// `charaParamId` — la variante jouable occupant le slot (0 = slot vide).
    pub chara_param_id: u32,
    /// `uniformId` — maillot de l'équipe.
    pub uniform_id: u32,
    /// `shoesId` — chaussures équipées.
    pub shoes_id: u32,
    /// `gloveId` — gants (0 = aucun).
    pub glove_id: u32,
    /// `emblemId` — emblème de l'équipe.
    pub emblem_id: u32,
    /// `uniformNo` — numéro de maillot.
    pub uniform_no: u16,
    /// `scPosNo` — position sur le terrain.
    pub sc_pos_no: u8,
    /// `isCaptain` — brassard.
    pub is_captain: bool,
}

/// État du live-modding : le jeu tourne-t-il, et où est son équipe.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LiveStatus {
    /// `true` si un process du jeu est attaché.
    pub running: bool,
    /// PID trouvé.
    pub pid: Option<i32>,
    /// Nom du process trouvé.
    pub process: Option<String>,
    /// Base de chargement du module principal (hex).
    pub module_base: Option<String>,
    /// Décalage ASLR par rapport à la base statique `0x140000000` (hex).
    pub aslr_slide: Option<String>,
}

/// Cherche le process du jeu.
fn pid_du_jeu() -> Option<(i32, &'static str)> {
    PROCESS.iter().find_map(|n| find_pid_by_name(n).map(|p| (p, *n)))
}

/// État du jeu, sans rien lire de sa mémoire au-delà de ses modules.
#[tauri::command]
#[specta::specta]
pub fn live_status() -> LiveStatus {
    let Some((pid, nom)) = pid_du_jeu() else {
        return LiveStatus {
            running: false,
            pid: None,
            process: None,
            module_base: None,
            aslr_slide: None,
        };
    };
    let base = nie_trace::find_module_base(pid, nom);
    LiveStatus {
        running: true,
        pid: Some(pid),
        process: Some(nom.to_string()),
        module_base: base.map(|b| format!("0x{b:X}")),
        aslr_slide: base.map(|b| format!("0x{:X}", b.wrapping_sub(0x1_4000_0000))),
    }
}

/// Décode une entrée de 0x38 octets.
fn decoder(slot: usize, addr: u64, buf: &[u8]) -> LiveMember {
    let u32_at = |o: usize| -> u32 {
        buf.get(o..o + 4)
            .and_then(|s| s.try_into().ok())
            .map_or(0, u32::from_le_bytes)
    };
    LiveMember {
        slot: slot as u32,
        address: format!("0x{addr:X}"),
        chara_param_id: u32_at(0x00),
        uniform_id: u32_at(0x04),
        shoes_id: u32_at(0x08),
        glove_id: u32_at(0x0C),
        emblem_id: u32_at(0x10),
        uniform_no: buf
            .get(0x14..0x16)
            .and_then(|s| s.try_into().ok())
            .map_or(0, u16::from_le_bytes),
        sc_pos_no: buf.get(0x16).copied().unwrap_or(0),
        is_captain: buf.get(0x17).copied().unwrap_or(0) != 0,
    }
}

/// Retrouve l'adresse du tableau d'équipe en scannant un `charaParamId` connu.
///
/// Le scan seul ne suffit pas : un identifiant apparaît dans les tables de données comme dans le
/// roster. On ne retient un candidat que si la **forme** se confirme — l'entrée suivante, à
/// `+0x38`, porte le même `uniformId` et un `charaParamId` non nul. C'est la signature d'un
/// tableau d'équipe, pas d'une occurrence isolée.
///
/// Renvoie l'adresse de la **première** entrée du tableau (en remontant tant que la forme tient).
#[tauri::command]
#[specta::specta]
pub fn live_find_team(chara_param_id: u32) -> Result<String, String> {
    let (pid, nom) = pid_du_jeu().ok_or("le jeu ne tourne pas")?;
    let motif = chara_param_id.to_le_bytes();

    for region in module_regions(pid, nom, true) {
        if !region.is_readable() || !region.is_writable() {
            continue;
        }
        let Ok(buf) = read_exact(pid, region.start, region.size() as usize) else {
            continue;
        };
        let mut i = 0usize;
        while i + 4 <= buf.len() {
            if buf[i..i + 4] != motif {
                i += 4;
                continue;
            }
            // Validation de forme : le voisin de droite doit avoir le même uniformId.
            let uniform = |o: usize| -> Option<u32> {
                buf.get(o + 0x04..o + 0x08)
                    .and_then(|s| s.try_into().ok())
                    .map(u32::from_le_bytes)
            };
            let param = |o: usize| -> Option<u32> {
                buf.get(o..o + 4).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes)
            };
            let suivant = i + STRIDE as usize;
            let forme_ok = matches!((uniform(i), uniform(suivant)), (Some(a), Some(b)) if a == b && a != 0)
                && param(suivant).is_some_and(|p| p != 0);
            if !forme_ok {
                i += 4;
                continue;
            }
            // Remonter au premier slot tant que la forme tient.
            let mut debut = i;
            while debut >= STRIDE as usize {
                let prec = debut - STRIDE as usize;
                if uniform(prec) == uniform(debut) && param(prec).is_some_and(|p| p != 0) {
                    debut = prec;
                } else {
                    break;
                }
            }
            return Ok(format!("0x{:X}", region.start + debut as u64));
        }
    }
    Err(format!("aucun tableau d'équipe portant 0x{chara_param_id:08X}"))
}

/// Lit les membres de l'équipe active à partir de l'adresse donnée.
#[tauri::command]
#[specta::specta]
pub fn live_read_team(address: String) -> Result<Vec<LiveMember>, String> {
    let (pid, _) = pid_du_jeu().ok_or("le jeu ne tourne pas")?;
    let base = parse_addr(&address)?;
    let buf = read_exact(pid, base, SLOTS * STRIDE as usize).map_err(|e| e.to_string())?;
    Ok((0..SLOTS)
        .map(|s| {
            let o = s * STRIDE as usize;
            decoder(s, base + o as u64, &buf[o..o + STRIDE as usize])
        })
        .collect())
}

/// Écrit un champ d'un membre. Le champ est nommé, pas donné en offset : on n'écrit que dans les
/// champs connus, à leur taille déclarée.
#[tauri::command]
#[specta::specta]
pub fn live_write_member(
    address: String,
    slot: u32,
    field: String,
    value: u32,
) -> Result<LiveMember, String> {
    let (pid, _) = pid_du_jeu().ok_or("le jeu ne tourne pas")?;
    let base = parse_addr(&address)?;
    let entree = base + u64::from(slot) * STRIDE;

    let (offset, taille) = match field.as_str() {
        "charaParamId" => (0x00u64, 4usize),
        "uniformId" => (0x04, 4),
        "shoesId" => (0x08, 4),
        "gloveId" => (0x0C, 4),
        "emblemId" => (0x10, 4),
        "uniformNo" => (0x14, 2),
        "scPosNo" => (0x16, 1),
        "isCaptain" => (0x17, 1),
        autre => return Err(format!("champ inconnu : {autre}")),
    };
    let octets = value.to_le_bytes();
    write_exact(pid, entree + offset, &octets[..taille]).map_err(|e| e.to_string())?;

    let buf = read_exact(pid, entree, STRIDE as usize).map_err(|e| e.to_string())?;
    Ok(decoder(slot as usize, entree, &buf))
}

/// Parse `0x…` ou un décimal.
fn parse_addr(s: &str) -> Result<u64, String> {
    let t = s.trim();
    let r = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .map_or_else(|| t.parse::<u64>().ok(), |h| u64::from_str_radix(h, 16).ok());
    r.ok_or_else(|| format!("adresse illisible : {s}"))
}

/// Une occurrence d'une valeur 32 bits dans la mémoire du jeu, avec son voisinage.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LiveHit {
    /// Adresse absolue de la valeur.
    pub address: String,
    /// Les 64 octets à partir de `address - 32`, en hexadécimal — de quoi reconnaître la
    /// structure porteuse sans relancer une lecture.
    pub context_hex: String,
    /// Offset de la valeur dans `context_hex` (toujours 32, sauf en début de région).
    pub context_offset: u32,
}

/// Cherche une valeur 32 bits dans toutes les pages accessibles en écriture du jeu.
///
/// Sert à localiser ce que les tables ne disent pas : l'identifiant d'une aura chargée, un slot
/// de compétence, une fiche de personnage. Chaque résultat rend son **voisinage**, parce qu'une
/// valeur seule ne dit pas dans quelle structure elle se trouve — c'est le contexte qui permet de
/// reconnaître un tableau (pas régulier) d'une occurrence isolée.
#[tauri::command]
#[specta::specta]
pub fn live_scan_u32(value: u32, limit: u32) -> Result<Vec<LiveHit>, String> {
    let (pid, nom) = pid_du_jeu().ok_or("le jeu ne tourne pas")?;
    let motif = value.to_le_bytes();
    let plafond = (limit as usize).clamp(1, 200);
    let mut hits = Vec::new();

    for region in module_regions(pid, nom, true) {
        if !region.is_readable() || !region.is_writable() {
            continue;
        }
        let Ok(buf) = read_exact(pid, region.start, region.size() as usize) else {
            continue;
        };
        let mut i = 0usize;
        while i + 4 <= buf.len() {
            if buf[i..i + 4] != motif {
                i += 4;
                continue;
            }
            let debut = i.saturating_sub(32);
            let fin = (i + 32).min(buf.len());
            hits.push(LiveHit {
                address: format!("0x{:X}", region.start + i as u64),
                context_hex: buf[debut..fin].iter().map(|b| format!("{b:02x}")).collect(),
                context_offset: (i - debut) as u32,
            });
            if hits.len() >= plafond {
                return Ok(hits);
            }
            i += 4;
        }
    }
    Ok(hits)
}

/// Écrit une valeur 32 bits à une adresse absolue.
///
/// Volontairement brut : c'est le complément de [`live_scan_u32`] pour tout ce qui n'a pas de
/// structure nommée — poser un identifiant d'aura dans un slot de compétence, par exemple. Rend
/// la valeur **relue** après écriture, jamais celle qu'on croyait écrire.
#[tauri::command]
#[specta::specta]
pub fn live_write_u32(address: String, value: u32) -> Result<u32, String> {
    let (pid, _) = pid_du_jeu().ok_or("le jeu ne tourne pas")?;
    let addr = parse_addr(&address)?;
    write_exact(pid, addr, &value.to_le_bytes()).map_err(|e| e.to_string())?;
    let buf = read_exact(pid, addr, 4).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(buf[..4].try_into().map_err(|_| "relecture courte")?))
}

/// Résultat d'un lancement d'outil externe.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LaunchResult {
    /// Ce qui a été lancé.
    pub launched: Vec<String>,
    /// Ce qui a été demandé mais n'existe pas sur le disque.
    pub missing: Vec<String>,
}

/// Lance l'éditeur de sauvegarde livré avec le dépôt, puis le jeu.
///
/// L'éditeur est cherché à la racine du dépôt (`InazumaElevenVRSaveEditor.exe`) ; le jeu est
/// lancé **directement** (`nie.exe`), sans `EACLauncher` — c'est ce qui permet d'attacher le
/// live-modding derrière.
///
/// L'ordre compte : l'éditeur d'abord, pour pouvoir préparer la sauvegarde avant que le jeu ne
/// la charge.
#[tauri::command]
#[specta::specta]
pub fn launch_save_editor(
    repo_dir: Option<String>,
    game_dir: Option<String>,
    also_game: bool,
) -> Result<LaunchResult, String> {
    use std::path::PathBuf;
    use std::process::Command;

    let mut launched = Vec::new();
    let mut missing = Vec::new();

    let racine = repo_dir
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("répertoire du dépôt introuvable")?;
    let editeur = racine.join("InazumaElevenVRSaveEditor.exe");
    if editeur.is_file() {
        Command::new(&editeur)
            .current_dir(&racine)
            .spawn()
            .map_err(|e| format!("lancement de l'éditeur : {e}"))?;
        launched.push(editeur.display().to_string());
    } else {
        missing.push(editeur.display().to_string());
    }

    if also_game {
        let jeu = game_dir
            .map(PathBuf::from)
            .or_else(crate::steam::detect_game_dir)
            .map(|d| d.join("nie.exe"))
            .ok_or("répertoire du jeu introuvable")?;
        if jeu.is_file() {
            Command::new(&jeu)
                .current_dir(jeu.parent().unwrap_or(&racine))
                .spawn()
                .map_err(|e| format!("lancement du jeu : {e}"))?;
            launched.push(jeu.display().to_string());
        } else {
            missing.push(jeu.display().to_string());
        }
    }

    Ok(LaunchResult { launched, missing })
}
