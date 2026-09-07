//! `nie-trace` — RE **en direct** (runtime) d'`nie.exe` : lecture de sa mémoire vivante pour
//! valider au réel les structures C++ reversées statiquement (offsets, vtables, état de scène).
//!
//! ## Deux backends, une API
//!
//! - **Windows natif** ([`win_memory`], `cfg(windows)`) — le vrai jeu Steam tourne sous Windows ;
//!   on lit via `OpenProcess`+`ReadProcessMemory` (Toolhelp32 pour les PID/modules, `VirtualQueryEx`
//!   pour les plages). Le binaire [`nie-mem`](../bin/nie-mem.rs) se **cross-compile depuis WSL**
//!   (`x86_64-pc-windows-gnu`, mingw) et se lance via l'interop WSL→Windows : WSL2 est une VM
//!   distincte, ses `/proc` ne voient PAS les process Windows — il FAUT l'API Windows.
//! - **Wine/Linux** ([`wine_memory`], `cfg(target_os="linux")`) — si le jeu tourne sous Wine, le PE
//!   est chargé dans l'espace d'adressage Linux → `process_vm_readv(2)` sur `/proc/<pid>/maps`.
//!
//! L'API publique ([`read`], [`find_pid_by_name`], [`find_module_base`], [`module_range`],
//! [`enumerate_regions`]) dispatch vers le backend de la plateforme courante. Les helpers neutres
//! ([`scan_regions`], [`dump_regions`], [`patch_eac`]) s'appuient dessus.
//!
//! ## EAC
//!
//! [`patch_eac`] neutralise la **modale fatale** d'init EAC (NOP du `call` @ [`EAC_PATCH_OFFSET`])
//! sur une **copie** : permet de lancer `nie.exe` **directement** (sans `EACLauncher.exe`), donc
//! sans driver EAC kernel → `ReadProcessMemory` autorisé. RE single-player offline d'un jeu possédé.
//!
//! ## `unsafe`
//!
//! Seul crate de niers à faire de la FFI OS (libc / windows-sys), confinée aux backends et
//! documentée `SAFETY`. Le reste (types, scan, patch) est sûr. Pas de `forbid(unsafe_code)`.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::Path;

use thiserror::Error;

pub mod aob;
pub mod catalog;
pub mod lancement;
pub mod recette;
#[cfg(windows)]
pub mod win_memory;
#[cfg(target_os = "linux")]
pub mod wine_memory;

pub use aob::Pattern;

// Extras spécifiques au backend Wine/Linux (avertissement ptrace côté CLI).
#[cfg(target_os = "linux")]
pub use wine_memory::{likely_permitted, read_ptrace_scope};

// ─── Types partagés ────────────────────────────────────────────────────────────────

/// Une plage mémoire du process cible (issue de `/proc/<pid>/maps` sous Linux, de `VirtualQueryEx`
/// sous Windows). `perms` est normalisé `rwx` (`-` si absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEntry {
    pub start: u64,
    pub end: u64,
    pub perms: String,
    pub offset: u64,
    pub path: String,
}

impl MapEntry {
    #[must_use]
    pub fn size(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
    #[must_use]
    pub fn is_readable(&self) -> bool {
        self.perms.as_bytes().first() == Some(&b'r')
    }
    #[must_use]
    pub fn is_writable(&self) -> bool {
        self.perms.as_bytes().get(1) == Some(&b'w')
    }
    #[must_use]
    pub fn is_executable(&self) -> bool {
        self.perms.as_bytes().get(2) == Some(&b'x')
    }
}

/// Échec d'une lecture mémoire, détail OS décodé.
#[derive(Debug, Error)]
pub enum MemError {
    #[error("lecture mémoire refusée: longueur {length} supérieure à la limite {max}")]
    InvalidLength { length: usize, max: usize },
    #[error("{op} a échoué pid={pid} addr=0x{addr:x} len={len} errno={errno}{hint}")]
    Syscall {
        op: &'static str,
        pid: i32,
        addr: u64,
        len: usize,
        errno: i32,
        hint: &'static str,
    },
    #[error("{op} a échoué pid={pid} addr=0x{addr:x} len={len} GetLastError={code}{hint}")]
    Win {
        op: &'static str,
        pid: i32,
        addr: u64,
        len: usize,
        code: u32,
        hint: &'static str,
    },
    #[error("{op} lecture partielle pid={pid} addr=0x{addr:x} demandé={requested} lu={got}")]
    Partial {
        op: &'static str,
        pid: i32,
        addr: u64,
        requested: usize,
        got: usize,
    },
    #[error("lecture mémoire indisponible sur cette plateforme")]
    Unsupported,
}

// ─── API publique : dispatch par backend ───────────────────────────────────────────

/// Lit jusqu'à `dest.len()` octets à l'adresse virtuelle `addr` du process `pid`. Ne stoppe pas la
/// cible. Renvoie le nombre d'octets lus (peut être `< dest.len()` aux frontières de plage).
pub fn read(pid: i32, addr: u64, dest: &mut [u8]) -> Result<usize, MemError> {
    #[cfg(target_os = "linux")]
    return wine_memory::read(pid, addr, dest);
    #[cfg(windows)]
    return win_memory::read(pid, addr, dest);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (pid, addr);
        if dest.is_empty() {
            Ok(0)
        } else {
            Err(MemError::Unsupported)
        }
    }
}

/// Écrit `src` à l'adresse virtuelle `addr` du process `pid`. Renvoie le nombre d'octets écrits.
///
/// RE / patch live d'un jeu **possédé** tournant en local : modifier la mémoire d'un process actif
/// peut le déstabiliser ou le faire planter. Le backend Windows déverrouille puis restaure la
/// protection de page ; le backend Wine exige une page déjà inscriptible.
pub fn write(pid: i32, addr: u64, src: &[u8]) -> Result<usize, MemError> {
    #[cfg(target_os = "linux")]
    return wine_memory::write(pid, addr, src);
    #[cfg(windows)]
    return win_memory::write(pid, addr, src);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (pid, addr);
        if src.is_empty() {
            Ok(0)
        } else {
            Err(MemError::Unsupported)
        }
    }
}

/// PID du premier process dont le nom d'image vaut `name` (ex. `"nie.exe"`).
#[must_use]
pub fn find_pid_by_name(name: &str) -> Option<i32> {
    #[cfg(target_os = "linux")]
    return wine_memory::find_pid_by_name(name);
    #[cfg(windows)]
    return win_memory::find_pid_by_name(name);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = name;
        None
    }
}

/// Adresse de chargement (base) du module dont le chemin/nom contient `fragment`.
#[must_use]
pub fn find_module_base(pid: i32, fragment: &str) -> Option<u64> {
    #[cfg(target_os = "linux")]
    return wine_memory::find_module_base(pid, fragment);
    #[cfg(windows)]
    return win_memory::find_module_base(pid, fragment);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (pid, fragment);
        None
    }
}

/// Étendue VA `[base, base+taille)` du module (taille = `SizeOfImage` PE sous Linux, `modBaseSize`
/// Toolhelp sous Windows).
#[must_use]
pub fn module_range(pid: i32, fragment: &str) -> Option<(u64, u64)> {
    #[cfg(target_os = "linux")]
    return wine_memory::module_range(pid, fragment);
    #[cfg(windows)]
    return win_memory::module_range(pid, fragment);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (pid, fragment);
        None
    }
}

/// Toutes les plages mémoire (committées) du process.
#[must_use]
pub fn enumerate_regions(pid: i32) -> Vec<MapEntry> {
    #[cfg(target_os = "linux")]
    return wine_memory::enumerate_regions(pid);
    #[cfg(windows)]
    return win_memory::enumerate_regions(pid);
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = pid;
        Vec::new()
    }
}

/// Plages du module `fragment` uniquement (intersection avec [`module_range`]), ou toutes si `all`.
#[must_use]
pub fn module_regions(pid: i32, fragment: &str, all: bool) -> Vec<MapEntry> {
    let regions = enumerate_regions(pid);
    if all {
        return regions;
    }
    match module_range(pid, fragment) {
        Some((base, end)) if end > base => regions
            .into_iter()
            .filter(|m| m.start < end && m.end > base)
            .collect(),
        _ => regions
            .into_iter()
            .filter(|m| m.path.to_lowercase().contains(&fragment.to_lowercase()))
            .collect(),
    }
}

/// Lit exactement `length` octets ou échoue (lecture partielle = échec).
pub fn read_exact(pid: i32, addr: u64, length: usize) -> Result<Vec<u8>, MemError> {
    let mut buf = vec![0u8; length];
    let got = read(pid, addr, &mut buf)?;
    if got != length {
        return Err(MemError::Partial {
            op: "read",
            pid,
            addr,
            requested: length,
            got,
        });
    }
    Ok(buf)
}

/// Lit un `u32` little-endian (x86-64).
pub fn read_u32(pid: i32, addr: u64) -> Result<u32, MemError> {
    let b = read_exact(pid, addr, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Lit un `u64` little-endian (x86-64).
pub fn read_u64(pid: i32, addr: u64) -> Result<u64, MemError> {
    let b = read_exact(pid, addr, 8)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// Lit un `u8`.
pub fn read_u8(pid: i32, addr: u64) -> Result<u8, MemError> {
    Ok(read_exact(pid, addr, 1)?[0])
}

/// Lit un `u16` little-endian.
pub fn read_u16(pid: i32, addr: u64) -> Result<u16, MemError> {
    let b = read_exact(pid, addr, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// Lit un `i32` little-endian (entier signé — niveaux, rangs, deltas).
pub fn read_i32(pid: i32, addr: u64) -> Result<i32, MemError> {
    let b = read_exact(pid, addr, 4)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Lit un `f32` little-endian (jauges, cooldowns, temps de match).
pub fn read_f32(pid: i32, addr: u64) -> Result<f32, MemError> {
    let b = read_exact(pid, addr, 4)?;
    Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Écrit exactement `src.len()` octets ou échoue (écriture partielle = échec).
pub fn write_exact(pid: i32, addr: u64, src: &[u8]) -> Result<(), MemError> {
    let put = write(pid, addr, src)?;
    if put != src.len() {
        return Err(MemError::Partial {
            op: "write",
            pid,
            addr,
            requested: src.len(),
            got: put,
        });
    }
    Ok(())
}

/// Écrit un `u8`.
pub fn write_u8(pid: i32, addr: u64, v: u8) -> Result<(), MemError> {
    write_exact(pid, addr, &[v])
}

/// Écrit un `u16` little-endian.
pub fn write_u16(pid: i32, addr: u64, v: u16) -> Result<(), MemError> {
    write_exact(pid, addr, &v.to_le_bytes())
}

/// Écrit un `u32` little-endian.
pub fn write_u32(pid: i32, addr: u64, v: u32) -> Result<(), MemError> {
    write_exact(pid, addr, &v.to_le_bytes())
}

/// Écrit un `i32` little-endian.
pub fn write_i32(pid: i32, addr: u64, v: i32) -> Result<(), MemError> {
    write_exact(pid, addr, &v.to_le_bytes())
}

/// Écrit un `u64` little-endian.
pub fn write_u64(pid: i32, addr: u64, v: u64) -> Result<(), MemError> {
    write_exact(pid, addr, &v.to_le_bytes())
}

/// Écrit un `f32` little-endian.
pub fn write_f32(pid: i32, addr: u64, v: f32) -> Result<(), MemError> {
    write_exact(pid, addr, &v.to_le_bytes())
}

/// Résout une **chaîne de pointeurs** façon Cheat-Engine depuis `base` : pour chaque offset, on
/// ajoute l'offset à l'adresse courante puis on **déréférence** (lecture d'un `u64`) — sauf le
/// dernier, qui n'est qu'ajouté. Renvoie l'adresse finale (où lit/écrit la valeur).
///
/// Ex. `[singleton+0x69A0]+0x5C` (rang du dump) = `resolve_chain(pid, singleton, &[0x69A0, 0x5C])`.
/// `offsets` vide renvoie `base`.
pub fn resolve_chain(pid: i32, base: u64, offsets: &[i64]) -> Result<u64, MemError> {
    let mut addr = base;
    for (i, &off) in offsets.iter().enumerate() {
        addr = add_offset(addr, off);
        if i + 1 < offsets.len() {
            addr = read_u64(pid, addr)?;
        }
    }
    Ok(addr)
}

/// `addr + off` avec `off` signé, saturé dans `u64`.
#[must_use]
fn add_offset(addr: u64, off: i64) -> u64 {
    if off >= 0 {
        addr.wrapping_add(off as u64)
    } else {
        addr.wrapping_sub(off.unsigned_abs())
    }
}

// ─── EAC patch (port de scripts/patch-eac.sh) ──────────────────────────────────────

/// File offset du `call` vers le constructeur de modale fatale (VA `0x14114ea02`,
/// image base `0x140000000`). NOP-er ces 5 octets rend l'échec d'init EAC non-fatal.
pub const EAC_PATCH_OFFSET: u64 = 0x0114_DE02;
/// Octets d'origine attendus : `call 0x140afa1a0` (`e8 99 b7 9a ff`). Garde-fou anti-mauvais-build.
pub const EAC_PATCH_ORIG: [u8; 5] = [0xE8, 0x99, 0xB7, 0x9A, 0xFF];
/// 5× `nop`.
pub const EAC_PATCH_NOP: [u8; 5] = [0x90, 0x90, 0x90, 0x90, 0x90];

/// Erreur du patch EAC.
#[derive(Debug, Error)]
pub enum EacPatchError {
    #[error("E/S sur le patch EAC : {0}")]
    Io(#[from] std::io::Error),
    #[error("octets @0x{offset:X} = {got}, attendu {want} — build de nie.exe différent, abandon")]
    Mismatch {
        offset: u64,
        got: String,
        want: String,
    },
}

/// Compte-rendu d'un patch EAC réussi.
#[derive(Debug, Clone)]
pub struct EacPatchReport {
    pub offset: u64,
    pub original: [u8; 5],
    pub patched: [u8; 5],
    pub dst_len: u64,
}

/// Crée `dst` comme **copie** de `src` puis NOP le `call` de modale fatale EAC @ [`EAC_PATCH_OFFSET`].
/// Vérifie que les 5 octets valent [`EAC_PATCH_ORIG`] avant d'écrire. **Ne touche jamais `src`**.
pub fn patch_eac(src: &Path, dst: &Path) -> Result<EacPatchReport, EacPatchError> {
    fs::copy(src, dst)?;
    let mut f = fs::OpenOptions::new().read(true).write(true).open(dst)?;

    f.seek(SeekFrom::Start(EAC_PATCH_OFFSET))?;
    let mut got = [0u8; 5];
    f.read_exact(&mut got)?;
    if got != EAC_PATCH_ORIG {
        return Err(EacPatchError::Mismatch {
            offset: EAC_PATCH_OFFSET,
            got: hex5(&got),
            want: hex5(&EAC_PATCH_ORIG),
        });
    }

    f.seek(SeekFrom::Start(EAC_PATCH_OFFSET))?;
    f.write_all(&EAC_PATCH_NOP)?;
    f.flush()?;
    let dst_len = f.metadata()?.len();

    Ok(EacPatchReport {
        offset: EAC_PATCH_OFFSET,
        original: EAC_PATCH_ORIG,
        patched: EAC_PATCH_NOP,
        dst_len,
    })
}

fn hex5(b: &[u8; 5]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ─── Dump / scan de plages ──────────────────────────────────────────────────────────

/// Un hit de [`scan_regions`].
#[derive(Debug, Clone)]
pub struct ScanHit {
    pub addr: u64,
    pub perms: String,
    /// RVA module-relative si une base est connue et `addr >= base`.
    pub rva: Option<u64>,
}

/// Résultat de [`dump_regions`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DumpStats {
    pub regions: usize,
    pub bytes: u64,
}

/// Dumpe les plages **lisibles** vers `out_dir` (`<start>-<end>.bin` par plage). Saute les plages
/// volatiles/refusées.
pub fn dump_regions(pid: i32, regions: &[MapEntry], out_dir: &Path) -> std::io::Result<DumpStats> {
    fs::create_dir_all(out_dir)?;
    let mut stats = DumpStats::default();
    for m in regions {
        if !m.is_readable() || m.size() == 0 {
            continue;
        }
        let mut buf = vec![0u8; m.size() as usize];
        let got = match read(pid, m.start, &mut buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };
        let name = format!("{:012x}-{:012x}.bin", m.start, m.end);
        fs::write(out_dir.join(name), &buf[..got])?;
        stats.regions += 1;
        stats.bytes += got as u64;
    }
    Ok(stats)
}

/// Cherche `needle` dans les plages **lisibles**, jusqu'à `limit` hits. `base` sert à calculer la RVA.
#[must_use]
pub fn scan_regions(
    pid: i32,
    regions: &[MapEntry],
    base: Option<u64>,
    needle: &[u8],
    limit: usize,
) -> Vec<ScanHit> {
    let mut hits = Vec::new();
    if needle.is_empty() {
        return hits;
    }
    for m in regions {
        if hits.len() >= limit || !m.is_readable() || m.size() == 0 {
            continue;
        }
        let mut buf = vec![0u8; m.size() as usize];
        let got = match read(pid, m.start, &mut buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };
        let span = &buf[..got];
        let mut from = 0usize;
        while hits.len() < limit {
            let Some(idx) = find_subslice(&span[from..], needle) else {
                break;
            };
            let at = m.start + (from + idx) as u64;
            let rva = base.filter(|&b| at >= b).map(|b| at - b);
            hits.push(ScanHit {
                addr: at,
                perms: m.perms.clone(),
                rva,
            });
            from += idx + 1;
        }
    }
    hits
}

/// Cherche un [`Pattern`] **à masque** (wildcards) dans les plages lisibles, jusqu'à `limit` hits.
/// `base` sert au calcul de la RVA. Pendant des signatures reversées (`44 8B ?? 10`) — voir
/// [`crate::catalog`].
///
/// Chaque plage est lue et scannée **indépendamment** (comme [`scan_regions`]) : un motif qui
/// *chevauche la couture* entre deux plages contiguës (`m.end == m_suivante.start`) ne serait pas
/// trouvé. Sans conséquence ici — une signature de code AOB vit entièrement dans une section
/// exécutable (une seule plage).
#[must_use]
pub fn scan_regions_masked(
    pid: i32,
    regions: &[MapEntry],
    base: Option<u64>,
    pattern: &Pattern,
    limit: usize,
) -> Vec<ScanHit> {
    let mut hits = Vec::new();
    if pattern.is_empty() {
        return hits;
    }
    for m in regions {
        if hits.len() >= limit || !m.is_readable() || m.size() == 0 {
            continue;
        }
        let mut buf = vec![0u8; m.size() as usize];
        let got = match read(pid, m.start, &mut buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };
        let remaining = limit - hits.len();
        for idx in pattern.find_all(&buf[..got], remaining) {
            let at = m.start + idx as u64;
            let rva = base.filter(|&b| at >= b).map(|b| at - b);
            hits.push(ScanHit {
                addr: at,
                perms: m.perms.clone(),
                rva,
            });
        }
    }
    hits
}

/// Première occurrence de `needle` dans `hay` (recherche naïve).
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eac_constants_are_self_consistent() {
        assert_eq!(EAC_PATCH_OFFSET, 0x0114_DE02);
        assert_eq!(hex5(&EAC_PATCH_ORIG), "e899b79aff");
        assert_eq!(hex5(&EAC_PATCH_NOP), "9090909090");
    }

    #[test]
    fn map_entry_perms() {
        let e = MapEntry {
            start: 0x1000,
            end: 0x2000,
            perms: "r-x".into(),
            offset: 0,
            path: "m".into(),
        };
        assert!(e.is_readable() && !e.is_writable() && e.is_executable());
        assert_eq!(e.size(), 0x1000);
    }

    #[test]
    fn patch_eac_nops_only_when_bytes_match() {
        let dir = std::env::temp_dir().join(format!("nie-trace-eac-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("src.bin");
        let dst = dir.join("dst.bin");

        // Faux binaire assez grand pour contenir l'offset EAC (~18 Mo), avec le `call` à l'offset.
        let size = EAC_PATCH_OFFSET as usize + 0x1000;
        let mut data = vec![0u8; size];
        data[EAC_PATCH_OFFSET as usize..EAC_PATCH_OFFSET as usize + 5]
            .copy_from_slice(&EAC_PATCH_ORIG);
        fs::write(&src, &data).unwrap();

        let report = patch_eac(&src, &dst).expect("patch ok");
        assert_eq!(report.original, EAC_PATCH_ORIG);
        assert_eq!(report.patched, EAC_PATCH_NOP);

        let src_after = fs::read(&src).unwrap();
        assert_eq!(
            &src_after[EAC_PATCH_OFFSET as usize..EAC_PATCH_OFFSET as usize + 5],
            &EAC_PATCH_ORIG
        );
        let dst_after = fs::read(&dst).unwrap();
        assert_eq!(
            &dst_after[EAC_PATCH_OFFSET as usize..EAC_PATCH_OFFSET as usize + 5],
            &EAC_PATCH_NOP
        );

        let bad_src = dir.join("bad.bin");
        let bad_dst = dir.join("bad_dst.bin");
        let mut bad = vec![0u8; size];
        bad[EAC_PATCH_OFFSET as usize] = 0xCC;
        fs::write(&bad_src, &bad).unwrap();
        assert!(matches!(
            patch_eac(&bad_src, &bad_dst),
            Err(EacPatchError::Mismatch { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_offset_signed() {
        assert_eq!(add_offset(0x1000, 0x58), 0x1058);
        assert_eq!(add_offset(0x1000, -8), 0x0FF8);
        // résolution d'une chaîne vide = base.
        assert_eq!(resolve_chain(-1, 0xDEAD_0000, &[]).unwrap(), 0xDEAD_0000);
    }

    #[test]
    fn scan_find_subslice() {
        let hay = b"....DEADBEEF....DEADBEEF";
        assert_eq!(find_subslice(hay, b"DEAD"), Some(4));
        // 2e "DEAD" @ index absolu 16 → dans hay[5..] : 16 - 5 = 11.
        assert_eq!(find_subslice(&hay[5..], b"DEAD"), Some(11));
        assert_eq!(find_subslice(hay, b"ZZZZ"), None);
        assert_eq!(find_subslice(b"ab", b"abc"), None);
    }
}
