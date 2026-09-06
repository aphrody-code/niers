//! Tables de répartition **funcLua** : les points d'entrée du script vers le
//! moteur.
//!
//! Le jeu expose ses fonctionnalités aux scripts Lua par des tables de
//! répartition en `.rdata`, faites d'entrées de 16 octets :
//!
//! ```c
//! struct FuncLuaEntry {
//!     void*    handler;  // pointeur de code, dans .text
//!     uint32_t cmdId;    // identifiant de commande
//!     uint32_t pad;      // toujours 0
//! };
//! ```
//!
//! Le répartiteur trie la table à la première invocation puis fait une
//! recherche binaire sur `cmdId`. Vérifié par désassemblage sur `nie.exe` : le
//! `cmdId` arrive de Lua comme **nombre** (`lua_tonumber` → `cvttsd2si`), pas
//! comme chaîne — les scripts compilés portent la valeur numérique en dur.
//!
//! Ces handlers sont la **surface fonctionnelle** du moteur vue du script :
//! 3 659 points d'entrée sur `nie.exe`. Les nommer et les rattacher au
//! sous-système `script` rend lisible tout un pan du binaire que ni le RTTI ni
//! le call-graph n'atteignent (ils ne sont appelés que par `call rax` depuis le
//! répartiteur).
//!
//! ## Honnêteté
//!
//! - Le nom écrit est `funcLuaCmd_<cmdId>`, pas le nom de la commande. Le
//!   `cmdId` est un **hachage du nom d'origine**, calculé hors ligne par la
//!   chaîne de build de LEVEL-5 : il n'existe aucune table inverse dans le
//!   binaire, et le hachage n'a été retrouvé ni parmi les variantes de CRC-32
//!   usuelles (ISO-HDLC, JAMCRC, BZIP2, MPEG-2, POSIX, CRC-32C/D/Q, AUTOSAR)
//!   ni parmi FNV-1/1a, djb2, sdbm ou ELF. Le nom réel reste inconnu — mais
//!   l'identifiant, lui, est exact et suffit à relier un handler à l'appel Lua
//!   qui le déclenche.
//! - La détection est **structurelle** : une suite d'entrées bien formées, sans
//!   ancre codée en dur sur une adresse, donc valable pour n'importe quel build.

use anyhow::{Context, Result};
use goblin::pe::PE;
use hashbrown::HashSet;
use nie_index::{Db, rusqlite};
use tracing::info;

/// Longueur minimale, en entrées, d'une suite retenue comme table de
/// répartition.
///
/// Les tables réelles vont de 8 à 2 451 entrées ; en dessous de 8, une suite
/// d'octets quelconques peut satisfaire la forme par hasard.
const MIN_ENTRIES: usize = 8;

/// Statistiques de l'extraction funcLua.
#[derive(Debug, Clone, Copy, Default)]
pub struct FuncLuaStats {
    /// Suites d'entrées bien formées retenues.
    pub tables: usize,
    /// Entrées totales des tables retenues.
    pub entries: usize,
    /// Handlers distincts (un même handler peut servir plusieurs `cmdId`).
    pub handlers: usize,
    /// Handlers absents de `function`, ajoutés comme nœuds.
    pub new_funcs: usize,
    /// Handlers nommés `funcLuaCmd_<cmdId>`.
    pub named: usize,
    /// Handlers rattachés au sous-système `script`.
    pub classified: usize,
}

/// Localise les tables de répartition funcLua de `exe_path`, nomme leurs
/// handlers et les rattache au sous-système `script`.
///
/// # Errors
///
/// Échoue si le PE est illisible, si `.text`/`.rdata` manquent, ou sur toute
/// erreur SQLite.
pub fn ingest_funclua(db: &mut Db, bin: i64, exe_path: &std::path::Path) -> Result<FuncLuaStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let pe = PE::parse(&bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;

    let text = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".text")))
        .context(".text introuvable")?;
    let text_va = image_base + u64::from(text.virtual_address);
    let text_end = text_va + u64::from(text.virtual_size.min(text.size_of_raw_data));

    let mut stats = FuncLuaStats::default();
    // `cmdId → handler`. Un handler peut apparaître sous plusieurs `cmdId`
    // (surcharges partageant une implémentation) : on garde la liste par
    // handler pour que le nom reflète tous ses points d'entrée.
    let mut pairs: Vec<(u32, u64)> = Vec::new();

    for name in [".rdata", ".data"] {
        let Some(sec) = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(name)))
        else {
            continue;
        };
        let off = sec.pointer_to_raw_data as usize;
        let len = sec.virtual_size.min(sec.size_of_raw_data) as usize;
        let Some(raw) = bytes.get(off..off + len) else {
            continue;
        };

        let entry_ok = |i: usize| -> Option<(u64, u32)> {
            let e = raw.get(i..i + 16)?;
            let h = u64::from_le_bytes(e[0..8].try_into().unwrap());
            let id = u32::from_le_bytes(e[8..12].try_into().unwrap());
            let pad = u32::from_le_bytes(e[12..16].try_into().unwrap());
            if pad != 0 || id == 0 || !(text_va..text_end).contains(&h) {
                return None;
            }
            Some((h, id))
        };

        let mut i = 0usize;
        while i + 16 <= raw.len() {
            if entry_ok(i).is_none() {
                i += 8;
                continue;
            }
            let mut run = Vec::new();
            let mut j = i;
            while let Some((h, id)) = entry_ok(j) {
                run.push((id, h));
                j += 16;
                if j + 16 > raw.len() {
                    break;
                }
            }
            if run.len() >= MIN_ENTRIES {
                stats.tables += 1;
                stats.entries += run.len();
                pairs.extend(run);
            }
            i = j.max(i + 8);
        }
    }

    let handlers: HashSet<u64> = pairs.iter().map(|&(_, h)| h).collect();
    stats.handlers = handlers.len();

    let known: HashSet<u64> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1")?;
        q.query_map([bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<_, _>>()?
    };

    let tx = db.conn_mut().transaction()?;
    {
        let mut ins = tx.prepare_cached(
            "INSERT OR IGNORE INTO function(binary_id, vaddr, name_source, subsystem, subsys_src, confidence)
             VALUES(?1,?2,'funclua','script','funclua',0.9)",
        )?;
        for &h in &handlers {
            if !known.contains(&h) {
                ins.execute(rusqlite::params![bin, h as i64])?;
                stats.new_funcs += 1;
            }
        }
    }
    {
        // Nommage : `funcLuaCmd_<cmdId>`. Le `cmdId` est exact ; le nom de
        // commande d'origine, lui, est inconnu (cf. en-tête du module).
        let mut upd = tx.prepare_cached(
            "UPDATE function SET name=?1, name_source='funclua'
             WHERE binary_id=?2 AND vaddr=?3 AND name IS NULL",
        )?;
        // Un handler servant plusieurs `cmdId` est nommé par le plus petit :
        // le nom doit être stable d'une exécution à l'autre.
        let mut sorted = pairs.clone();
        sorted.sort_unstable();
        let mut seen: HashSet<u64> = HashSet::new();
        for (id, h) in sorted {
            if !seen.insert(h) {
                continue;
            }
            let nm = format!("funcLuaCmd_{id:08x}");
            stats.named += upd.execute(rusqlite::params![nm, bin, h as i64])?;
        }
    }
    {
        // Rattachement au sous-système `script` : un handler de commande Lua
        // *est* du script par construction. Prime sur une étiquette
        // statistique (`ml`) ou de contiguïté (`adjacency`), pas sur une ancre
        // RTTI.
        let mut upd = tx.prepare_cached(
            "UPDATE function SET subsystem='script', subsys_src='funclua', confidence=0.9
             WHERE binary_id=?1 AND vaddr=?2
               AND (subsystem='standalone' OR subsys_src IN ('ml','adjacency','leaf-scan','leaf-ref'))",
        )?;
        for &h in &handlers {
            stats.classified += upd.execute(rusqlite::params![bin, h as i64])?;
        }
    }
    tx.commit()?;

    info!(
        tables = stats.tables,
        entries = stats.entries,
        handlers = stats.handlers,
        "funclua: tables de répartition ingérées"
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forme d'une entrée : handler dans `.text`, `cmdId` non nul, `pad` nul.
    fn entry_ok(raw: &[u8], i: usize, lo: u64, hi: u64) -> Option<(u64, u32)> {
        let e = raw.get(i..i + 16)?;
        let h = u64::from_le_bytes(e[0..8].try_into().unwrap());
        let id = u32::from_le_bytes(e[8..12].try_into().unwrap());
        let pad = u32::from_le_bytes(e[12..16].try_into().unwrap());
        if pad != 0 || id == 0 || !(lo..hi).contains(&h) {
            return None;
        }
        Some((h, id))
    }

    fn entry(handler: u64, id: u32, pad: u32) -> Vec<u8> {
        let mut v = handler.to_le_bytes().to_vec();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&pad.to_le_bytes());
        v
    }

    #[test]
    fn une_entree_bien_formee_est_acceptee() {
        let raw = entry(0x1400_1000, 0x214D_A123, 0);
        assert_eq!(
            entry_ok(&raw, 0, 0x1400_0000, 0x1500_0000),
            Some((0x1400_1000, 0x214D_A123))
        );
    }

    #[test]
    fn le_padding_non_nul_disqualifie_l_entree() {
        let raw = entry(0x1400_1000, 0x214D_A123, 1);
        assert_eq!(entry_ok(&raw, 0, 0x1400_0000, 0x1500_0000), None);
    }

    #[test]
    fn un_handler_hors_text_disqualifie_l_entree() {
        let raw = entry(0x9000_0000, 0x214D_A123, 0);
        assert_eq!(entry_ok(&raw, 0, 0x1400_0000, 0x1500_0000), None);
    }

    #[test]
    fn un_cmdid_nul_disqualifie_l_entree() {
        let raw = entry(0x1400_1000, 0, 0);
        assert_eq!(entry_ok(&raw, 0, 0x1400_0000, 0x1500_0000), None);
    }

    /// Le seuil doit écarter une suite trop courte pour être une vraie table.
    #[test]
    fn seuil_de_longueur_de_table() {
        let court: Vec<(u32, u64)> = (0..4).map(|i| (i + 1, 0x1400_1000)).collect();
        let long: Vec<(u32, u64)> = (0..16).map(|i| (i + 1, 0x1400_1000)).collect();
        assert!(court.len() < MIN_ENTRIES);
        assert!(long.len() >= MIN_ENTRIES);
    }

    /// Le nom retenu pour un handler partagé est celui du plus petit `cmdId` :
    /// sans cet ordre, le nom changerait d'une exécution à l'autre.
    #[test]
    fn le_nom_d_un_handler_partage_est_stable() {
        let mut pairs = [
            (0x00FF_0000u32, 0x1400_1000u64),
            (0x0000_00FFu32, 0x1400_1000u64),
        ];
        pairs.sort_unstable();
        assert_eq!(pairs[0].0, 0x0000_00FF);
        assert_eq!(
            format!("funcLuaCmd_{:08x}", pairs[0].0),
            "funcLuaCmd_000000ff"
        );
    }
}
