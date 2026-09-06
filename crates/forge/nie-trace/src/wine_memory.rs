//! Backend **Wine/Linux** : lecture de la mémoire d'un `nie.exe` exécuté sous Wine, sans `ptrace`
//! (pas de stop), via `process_vm_readv(2)` / `process_vm_writev(2)`.
//!
//! Wine ne virtualise pas : il charge le PE dans l'**espace d'adressage Linux du même processus**.
//! Une VA « Windows » *est* une VA Linux réelle → lire le module = lire `/proc/<pid>/maps`.
//!
//! Contrainte ptrace (Yama) : avec `kernel.yama.ptrace_scope=1`, `process_vm_readv` n'est permis
//! que si l'appelant est **ancêtre** de la cible (ou a `CAP_SYS_PTRACE`) — d'où le harnais qui
//! *lance* le jeu en enfant (`scripts/boot-nie-direct.sh`).
//!
//! Module `cfg(target_os = "linux")` (dispatché par `lib.rs`).

use std::fs;

use crate::{MapEntry, MemError};

// ─── process_vm_readv / writev ──────────────────────────────────────────────────────

/// Libellé humain d'un errno de `process_vm_{read,write}v` (pur, testable).
#[must_use]
fn errno_hint(errno: i32) -> &'static str {
    match errno {
        1 => " (EPERM — ptrace_scope refuse : pas ancêtre et pas CAP_SYS_PTRACE)",
        3 => " (ESRCH — pid inexistant)",
        14 => " (EFAULT — adresse/plage non mappée côté cible)",
        _ => "",
    }
}

fn err_from_errno(pid: i32, addr: u64, len: usize, is_write: bool) -> MemError {
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    let op = if is_write {
        "process_vm_writev"
    } else {
        "process_vm_readv"
    };
    MemError::Syscall {
        op,
        pid,
        addr,
        len,
        errno,
        hint: errno_hint(errno),
    }
}

/// Lit jusqu'à `dest.len()` octets à `addr` du process `pid`. Ne stoppe pas la cible.
pub fn read(pid: i32, addr: u64, dest: &mut [u8]) -> Result<usize, MemError> {
    if dest.is_empty() {
        return Ok(0);
    }
    let local = libc::iovec {
        iov_base: dest.as_mut_ptr().cast::<libc::c_void>(),
        iov_len: dest.len(),
    };
    let remote = libc::iovec {
        iov_base: addr as usize as *mut libc::c_void,
        iov_len: dest.len(),
    };
    // SAFETY: `local.iov_base` pointe sur `dest` (valide pour écriture, `iov_len` == dest.len()).
    // `remote` n'est qu'une (adresse, longueur) transmise au noyau ; il copie au plus dest.len()
    // octets de la cible vers `dest`, sans déréférencer `remote` côté appelant.
    let n = unsafe { libc::process_vm_readv(pid, &local, 1, &remote, 1, 0) };
    if n < 0 {
        return Err(err_from_errno(pid, addr, dest.len(), false));
    }
    Ok(n as usize)
}

/// Écrit `src` à `addr` (RE / patch live ; la page doit être inscriptible côté cible).
pub fn write(pid: i32, addr: u64, src: &[u8]) -> Result<usize, MemError> {
    if src.is_empty() {
        return Ok(0);
    }
    let local = libc::iovec {
        iov_base: src.as_ptr() as *mut libc::c_void,
        iov_len: src.len(),
    };
    let remote = libc::iovec {
        iov_base: addr as usize as *mut libc::c_void,
        iov_len: src.len(),
    };
    // SAFETY: `local.iov_base` pointe sur `src` (valide pour lecture, `iov_len` == src.len()) ;
    // process_vm_writev lit `src` et écrit au plus src.len() octets côté cible.
    let n = unsafe { libc::process_vm_writev(pid, &local, 1, &remote, 1, 0) };
    if n < 0 {
        return Err(err_from_errno(pid, addr, src.len(), true));
    }
    Ok(n as usize)
}

// ─── /proc/<pid>/maps ─────────────────────────────────────────────────────────────

/// Parse pure (testable) d'une ligne de `/proc/<pid>/maps`.
#[must_use]
pub fn parse_map_line(line: &str) -> Option<MapEntry> {
    let mut parts = line.split_whitespace();
    let range = parts.next()?;
    let perms = parts.next()?;
    let offset_s = parts.next()?;
    let _dev = parts.next()?;
    let _inode = parts.next()?;
    let path = parts.collect::<Vec<_>>().join(" ");

    let dash = range.find('-')?;
    let start = u64::from_str_radix(&range[..dash], 16).ok()?;
    let end = u64::from_str_radix(&range[dash + 1..], 16).ok()?;
    let offset = u64::from_str_radix(offset_s, 16).ok()?;
    Some(MapEntry {
        start,
        end,
        perms: perms.to_owned(),
        offset,
        path,
    })
}

/// Toutes les plages de `/proc/<pid>/maps`.
pub fn read_maps(pid: i32) -> std::io::Result<Vec<MapEntry>> {
    let content = fs::read_to_string(format!("/proc/{pid}/maps"))?;
    Ok(content.lines().filter_map(parse_map_line).collect())
}

/// API commune : toutes les plages committées du process.
#[must_use]
pub fn enumerate_regions(pid: i32) -> Vec<MapEntry> {
    read_maps(pid).unwrap_or_default()
}

/// Base d'un module dont le chemin contient `fragment` (variante pure).
#[must_use]
pub fn find_module_base_in(maps: &[MapEntry], fragment: &str) -> Option<u64> {
    let needle = fragment.to_lowercase();
    let mut best: Option<u64> = None;
    for m in maps {
        if !m.path.is_empty() && m.path.to_lowercase().contains(&needle) {
            best = Some(best.map_or(m.start, |b| b.min(m.start)));
        }
    }
    best
}

/// Base du module (plus petit `start` parmi ses plages).
#[must_use]
pub fn find_module_base(pid: i32, fragment: &str) -> Option<u64> {
    find_module_base_in(&read_maps(pid).ok()?, fragment)
}

/// Plages d'un module donné, triées.
#[must_use]
pub fn find_module_regions(pid: i32, fragment: &str) -> Vec<MapEntry> {
    let needle = fragment.to_lowercase();
    let mut list: Vec<MapEntry> = enumerate_regions(pid)
        .into_iter()
        .filter(|m| m.path.to_lowercase().contains(&needle))
        .collect();
    list.sort_by_key(|m| m.start);
    list
}

/// API commune : étendue VA `[base, base+SizeOfImage)` du module, via l'en-tête PE en mémoire.
#[must_use]
pub fn module_range(pid: i32, fragment: &str) -> Option<(u64, u64)> {
    let base = find_module_base(pid, fragment)?;
    let dos = match crate::read_exact(pid, base, 0x40) {
        Ok(d) => d,
        Err(_) => return Some((base, base)),
    };
    let lfanew = i32::from_le_bytes([dos[0x3c], dos[0x3d], dos[0x3e], dos[0x3f]]);
    if lfanew <= 0 || lfanew > 0x1000 {
        return Some((base, base));
    }
    match crate::read_u32(pid, base + lfanew as u64 + 0x50) {
        Ok(0) | Err(_) => Some((base, base)),
        Ok(size) => Some((base, base + u64::from(size))),
    }
}

// ─── ptrace_scope / généalogie ───────────────────────────────────────────────────

/// Valeur de `kernel.yama.ptrace_scope` (0–3). `-1` si illisible.
#[must_use]
pub fn read_ptrace_scope() -> i32 {
    fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(-1)
}

/// `ancestor_pid` est-il un ancêtre (parent transitif) de `pid` ?
#[must_use]
pub fn is_ancestor_of(pid: i32, ancestor_pid: i32) -> bool {
    let mut cur = pid;
    let mut i = 0;
    while i < 4096 && cur > 1 {
        let ppid = read_ppid(cur);
        if ppid <= 0 {
            return false;
        }
        if ppid == ancestor_pid {
            return true;
        }
        cur = ppid;
        i += 1;
    }
    false
}

/// Extrait le PPid d'un contenu `/proc/<pid>/status` (pur, testable). `-1` si absent/illisible.
#[must_use]
fn parse_ppid(status: &str) -> i32 {
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse::<i32>().unwrap_or(-1);
        }
    }
    -1
}

fn read_ppid(pid: i32) -> i32 {
    match fs::read_to_string(format!("/proc/{pid}/status")) {
        Ok(status) => parse_ppid(&status),
        Err(_) => -1,
    }
}

/// Décision pure (testable) : un `ptrace_scope` donné permet-il l'accès, sachant si l'appelant est
/// ancêtre de la cible ? `scope<=0` = oui ; `scope==1` = ssi ancêtre ; `>=2` = non (sans `CAP_SYS_PTRACE`).
#[must_use]
fn permitted_for_scope(scope: i32, is_ancestor: bool) -> bool {
    match scope {
        s if s <= 0 => true,
        1 => is_ancestor,
        _ => false,
    }
}

/// `process_vm_readv` est-il *probablement* permis sur `pid` depuis le process courant ?
#[must_use]
pub fn likely_permitted(pid: i32) -> bool {
    let scope = read_ptrace_scope();
    permitted_for_scope(scope, is_ancestor_of(pid, std::process::id() as i32))
}

/// API commune : PID du premier process dont `/proc/<pid>/comm` vaut `name`.
#[must_use]
pub fn find_pid_by_name(name: &str) -> Option<i32> {
    let dir = fs::read_dir("/proc").ok()?;
    for entry in dir.flatten() {
        let fname = entry.file_name();
        let Some(s) = fname.to_str() else { continue };
        let Ok(pid) = s.parse::<i32>() else { continue };
        if let Ok(c) = fs::read_to_string(format!("/proc/{pid}/comm"))
            && c.trim() == name
        {
            return Some(pid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_map_line_basic() {
        let e =
            parse_map_line("00400000-00452000 r-xp 00000000 08:01 1234 /path/to/nie.exe").unwrap();
        assert_eq!(e.start, 0x40_0000);
        assert_eq!(e.end, 0x45_2000);
        assert_eq!(e.perms, "r-xp");
        assert_eq!(e.path, "/path/to/nie.exe");
        assert!(e.is_readable() && e.is_executable());
    }

    #[test]
    fn parse_map_line_spaces_and_anon() {
        assert_eq!(
            parse_map_line("7ffd0000-7ffd1000 rw-p 0 00:00 0")
                .unwrap()
                .path,
            ""
        );
        let s =
            parse_map_line("140000000-140001000 r--p 0 fd:01 42 /home/u/My Games/nie.exe").unwrap();
        assert_eq!(s.path, "/home/u/My Games/nie.exe");
    }

    #[test]
    fn parse_map_line_rejects_malformed() {
        assert!(parse_map_line("").is_none());
        assert!(parse_map_line("nodash r-xp 0 0:0 0").is_none());
    }

    #[test]
    fn find_module_base_smallest_case_insensitive() {
        let maps = vec![
            MapEntry {
                start: 0x1_4000_5000,
                end: 0x1_4000_6000,
                perms: "rw-p".into(),
                offset: 0,
                path: "/g/NIE.exe".into(),
            },
            MapEntry {
                start: 0x1_4000_0000,
                end: 0x1_4000_1000,
                perms: "r--p".into(),
                offset: 0,
                path: "/g/nie.exe".into(),
            },
        ];
        assert_eq!(find_module_base_in(&maps, "nie.exe"), Some(0x1_4000_0000));
        assert_eq!(find_module_base_in(&maps, "absent"), None);
    }

    #[test]
    fn ptrace_scope_in_range() {
        assert!((-1..=3).contains(&read_ptrace_scope()));
    }

    #[test]
    fn read_rejects_bad_pid() {
        let mut buf = [0u8; 16];
        assert!(read(-1, 0x1000, &mut buf).is_err());
    }

    #[test]
    fn errno_hint_all_arms() {
        assert!(errno_hint(1).contains("EPERM"));
        assert!(errno_hint(3).contains("ESRCH"));
        assert!(errno_hint(14).contains("EFAULT"));
        assert_eq!(errno_hint(0), "");
        assert_eq!(errno_hint(42), "");
    }

    #[test]
    fn permitted_for_scope_all_arms() {
        assert!(permitted_for_scope(0, false)); // scope désactivé → toujours permis
        assert!(permitted_for_scope(-1, false));
        assert!(permitted_for_scope(1, true)); // scope restreint → ssi ancêtre
        assert!(!permitted_for_scope(1, false));
        assert!(!permitted_for_scope(2, true)); // admin only
        assert!(!permitted_for_scope(3, true));
    }

    #[test]
    fn empty_slices_are_noops() {
        assert_eq!(read(123, 0x1000, &mut []).unwrap(), 0);
        assert_eq!(write(123, 0x1000, &[]).unwrap(), 0);
    }

    #[test]
    fn write_rejects_bad_pid() {
        assert!(write(-1, 0x1000, &[1, 2, 3]).is_err());
    }

    #[test]
    fn read_errnos_esrch_and_efault() {
        // ESRCH : pid quasi-certainement inexistant.
        let mut buf = [0u8; 8];
        let esrch = read(0x3FFF_FFFF, 0x1000, &mut buf);
        assert!(matches!(esrch, Err(MemError::Syscall { errno: 3, .. })));
        // EFAULT : adresse non mappée dans NOTRE propre process (page nulle).
        let efault = read(std::process::id() as i32, 0x1, &mut buf);
        assert!(matches!(efault, Err(MemError::Syscall { errno: 14, .. })));
    }

    #[test]
    fn self_introspection_via_proc() {
        let me = std::process::id() as i32;
        // /proc/self/maps non vide.
        assert!(!enumerate_regions(me).is_empty());
        // notre propre comm est retrouvable.
        let comm = fs::read_to_string(format!("/proc/{me}/comm"))
            .unwrap()
            .trim()
            .to_owned();
        assert_eq!(find_pid_by_name(&comm), Some(me));
        // base + étendue du binaire de test (son chemin /proc/self/maps contient le nom du comm).
        let base = find_module_base(me, &comm).expect("binaire de test mappé");
        let (b, end) = module_range(me, &comm).unwrap();
        assert_eq!(b, base);
        assert!(end >= base);
        assert!(!find_module_regions(me, &comm).is_empty());
        // ppid réel → is_ancestor_of vrai ; pid bidon → faux.
        let ppid = read_ppid(me);
        assert!(ppid > 0);
        assert!(is_ancestor_of(me, ppid));
        assert!(!is_ancestor_of(me, 0x3FFF_FFFF));
        // likely_permitted ne panique pas (résultat dépend du scope système).
        let _ = likely_permitted(me);
    }

    #[test]
    fn read_ppid_rejects_bad_pid() {
        assert_eq!(read_ppid(0x3FFF_FFFF), -1);
    }

    #[test]
    fn parse_ppid_variants() {
        assert_eq!(parse_ppid("Name:\tx\nPPid:\t42\nUid:\t0"), 42);
        assert_eq!(parse_ppid("Name:\tx\nUid:\t0"), -1); // pas de ligne PPid
        assert_eq!(parse_ppid("PPid:\tnotnum"), -1); // PPid illisible
        assert_eq!(parse_ppid(""), -1);
    }

    #[test]
    fn is_ancestor_of_short_circuits_on_missing_status() {
        // pid bidon : read_ppid(cur) == -1 dès la 1re itération → false (branche ppid<=0).
        assert!(!is_ancestor_of(0x3FFF_FFFF, 1));
    }

    #[test]
    fn find_pid_by_name_absent() {
        assert_eq!(find_pid_by_name("zzz-process-inexistant-zzz"), None);
    }
}
