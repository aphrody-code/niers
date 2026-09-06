//! Lecture et exploitation d'un minidump Windows (`MiniDumpWriteDump`) de `nie.exe`.
//!
//! Conçu pour exploiter un dump mémoire **live** (ASLR) : parsing des modules, des régions
//! mémoire (`Memory64List` + protections via `MemoryInfoList`), lecture aléatoire par adresse
//! virtuelle, scan de motifs AOB avec wildcards, résolution de noms de classes **RTTI MSVC**,
//! et census d'objets vivants par vtable.
//!
//! Format minidump (sous-ensemble) :
//! - en-tête `MINIDUMP_HEADER` (signature `MDMP`) + répertoire `MINIDUMP_DIRECTORY[]`,
//! - `ModuleListStream` (4) : base/taille/nom des modules chargés,
//! - `Memory64ListStream` (9) : régions mémoire concaténées à partir d'un `BaseRva`,
//! - `MemoryInfoListStream` (16) : état/protection/type de chaque région.
//!
//! `#![forbid(unsafe_code)]` au niveau crate : pas de `mmap`, lecture des régions à la
//! demande via `seek`/`read_exact`.
//!
//! Crate séparée de `nie-re` (qui la réexporte sous `nie_re::dump`) parce qu'elle est la seule
//! partie du moteur RE consommable par `nie-explorer` : elle n'a besoin ni de `rusqlite` (via
//! `nie-index`) ni du dépôt frère `aphrody`, deux dépendances rédhibitoires côté Tauri.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::pedantic)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use thiserror::Error;

const STREAM_MODULE_LIST: u32 = 4;
const STREAM_MEMORY64_LIST: u32 = 9;
const STREAM_MEMORY_INFO_LIST: u32 = 16;
const MODULE_RECORD_SIZE: usize = 108;
/// Image-base statique de `nie.exe` (PE `ImageBase`), pour traduire RVA → adresse r2.
pub const NIE_IMAGE_BASE: u64 = 0x1_4000_0000;

// Constantes Win32 mémoire.
const MEM_COMMIT: u32 = 0x1000;
const MEM_PRIVATE: u32 = 0x20000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

/// Erreurs de parsing/lecture d'un minidump.
#[derive(Debug, Error)]
pub enum DumpError {
    /// Erreur d'E/S sous-jacente.
    #[error("E/S : {0}")]
    Io(#[from] std::io::Error),
    /// Signature `MDMP` absente ou en-tête tronqué.
    #[error("minidump invalide : {0}")]
    Invalid(&'static str),
    /// Un stream requis (module list / memory64 list) est absent.
    #[error("stream minidump absent : {0}")]
    MissingStream(&'static str),
}

type Result<T> = std::result::Result<T, DumpError>;

/// Un module chargé dans l'espace d'adressage capturé.
#[derive(Debug, Clone)]
pub struct Module {
    /// Adresse de base virtuelle (avec ASLR).
    pub base: u64,
    /// Taille de l'image en mémoire.
    pub size: u32,
    /// Nom de fichier du module (ex. `nie.exe`).
    pub name: String,
}

/// Une région mémoire capturée : plage virtuelle + offset dans le fichier dump.
#[derive(Debug, Clone, Copy)]
struct Range {
    va: u64,
    size: u64,
    file_off: u64,
}

/// Métadonnées d'une région virtuelle (issues de `MemoryInfoList`).
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// Adresse de base de la région.
    pub base: u64,
    /// Taille de la région.
    pub size: u64,
    /// `State` Win32 (`MEM_COMMIT`/`MEM_RESERVE`/`MEM_FREE`).
    pub state: u32,
    /// `Protect` Win32 (`PAGE_*`).
    pub protect: u32,
    /// `Type` Win32 (`MEM_PRIVATE`/`MEM_MAPPED`/`MEM_IMAGE`).
    pub mtype: u32,
}

impl Region {
    /// `true` si la région est committed, privée, et accessible en écriture (le « tas »).
    #[must_use]
    pub fn is_heap(&self) -> bool {
        self.state == MEM_COMMIT
            && self.mtype == MEM_PRIVATE
            && (self.protect == PAGE_READWRITE || self.protect == PAGE_EXECUTE_READWRITE)
    }
}

/// Un motif octet à scanner : `bytes[i]` n'est comparé que si `mask[i]` est vrai.
#[derive(Debug, Clone)]
pub struct Pattern {
    /// Octets du motif (les positions wildcard sont ignorées).
    pub bytes: Vec<u8>,
    /// Masque : `true` = octet fixe, `false` = wildcard.
    pub mask: Vec<bool>,
}

impl Pattern {
    /// Parse un motif façon Cheat-Engine : `"44 8B ?? 10 ?? C0"`.
    ///
    /// Les jetons `??` (ou `?`, ou `*`) sont des wildcards ; les autres sont des octets hex.
    ///
    /// # Errors
    /// Renvoie [`DumpError::Invalid`] si un jeton non-wildcard n'est pas un octet hex valide.
    pub fn parse(s: &str) -> Result<Self> {
        let mut bytes = Vec::new();
        let mut mask = Vec::new();
        for tok in s.split_whitespace() {
            if tok == "??" || tok == "?" || tok == "*" {
                bytes.push(0);
                mask.push(false);
            } else {
                let b = u8::from_str_radix(tok, 16)
                    .map_err(|_| DumpError::Invalid("octet hex invalide dans le motif"))?;
                bytes.push(b);
                mask.push(true);
            }
        }
        if bytes.is_empty() {
            return Err(DumpError::Invalid("motif vide"));
        }
        Ok(Self { bytes, mask })
    }

    fn first_fixed(&self) -> usize {
        self.mask.iter().position(|&m| m).unwrap_or(0)
    }

    fn matches_at(&self, buf: &[u8], i: usize) -> bool {
        self.bytes
            .iter()
            .zip(&self.mask)
            .enumerate()
            .all(|(k, (b, &keep))| !keep || buf[i + k] == *b)
    }
}

/// Un coup : adresse virtuelle, et — si l'adresse tombe dans un module — `module + RVA`.
#[derive(Debug, Clone)]
pub struct Hit {
    /// Adresse virtuelle live du coup.
    pub va: u64,
    /// Nom du module contenant l'adresse, le cas échéant.
    pub module: Option<String>,
    /// Offset relatif à la base du module (RVA), le cas échéant.
    pub rva: Option<u64>,
}

impl Hit {
    /// Adresse statique r2 si le coup est dans `nie.exe` (`NIE_IMAGE_BASE + rva`).
    #[must_use]
    pub fn nie_static(&self) -> Option<u64> {
        match (self.module.as_deref(), self.rva) {
            (Some("nie.exe"), Some(rva)) => Some(NIE_IMAGE_BASE + rva),
            _ => None,
        }
    }
}

/// Un minidump ouvert : modules + régions mémoire, prêt à scanner et lire.
#[derive(Debug)]
pub struct Minidump {
    file: File,
    /// Modules chargés au moment de la capture.
    pub modules: Vec<Module>,
    ranges: Vec<Range>, // triées par va
    starts: Vec<u64>,   // ranges[i].va, pour bisect
    regions: Vec<Region>,
    region_starts: Vec<u64>,
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        b[o],
        b[o + 1],
        b[o + 2],
        b[o + 3],
        b[o + 4],
        b[o + 5],
        b[o + 6],
        b[o + 7],
    ])
}

fn read_at(file: &mut File, off: u64, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    file.seek(SeekFrom::Start(off))?;
    file.read_exact(&mut buf)?;
    Ok(buf)
}

/// Démangle un nom de type RTTI MSVC (`.?AVFoo@bar@@` → `bar::Foo`).
fn demangle(n: &str) -> String {
    let rest = n
        .strip_prefix(".?AV")
        .or_else(|| n.strip_prefix(".?AU"))
        .unwrap_or(n);
    let rest = rest.strip_suffix("@@").unwrap_or(rest);
    rest.split('@')
        .filter(|p| !p.is_empty())
        .rev()
        .collect::<Vec<_>>()
        .join("::")
}

impl Minidump {
    /// Ouvre et parse l'en-tête, les modules, les régions mémoire d'un `.dmp`.
    ///
    /// # Errors
    /// Renvoie [`DumpError`] si le fichier n'est pas un minidump `MDMP` ou si un stream
    /// requis (module list / memory64 list) est absent.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = File::open(path)?;

        let header = read_at(&mut file, 0, 32)?;
        if &header[0..4] != b"MDMP" {
            return Err(DumpError::Invalid("signature MDMP absente"));
        }
        let n_streams = u32le(&header, 8) as usize;
        let dir_rva = u32le(&header, 12) as u64;

        let dir = read_at(&mut file, dir_rva, n_streams * 12)?;
        let mut module_loc = None;
        let mut mem64_loc = None;
        let mut info_loc = None;
        for i in 0..n_streams {
            let o = i * 12;
            let stype = u32le(&dir, o);
            let size = u32le(&dir, o + 4) as usize;
            let rva = u32le(&dir, o + 8) as u64;
            match stype {
                STREAM_MODULE_LIST => module_loc = Some((rva, size)),
                STREAM_MEMORY64_LIST => mem64_loc = Some((rva, size)),
                STREAM_MEMORY_INFO_LIST => info_loc = Some((rva, size)),
                _ => {}
            }
        }

        let (mrva, msize) = module_loc.ok_or(DumpError::MissingStream("ModuleList"))?;
        let modules = Self::parse_modules(&mut file, mrva, msize)?;

        let (rrva, _) = mem64_loc.ok_or(DumpError::MissingStream("Memory64List"))?;
        let mut ranges = Self::parse_ranges(&mut file, rrva)?;
        ranges.sort_by_key(|r| r.va);
        let starts = ranges.iter().map(|r| r.va).collect();

        let mut regions = match info_loc {
            Some((irva, _)) => Self::parse_regions(&mut file, irva)?,
            None => Vec::new(),
        };
        regions.sort_by_key(|r| r.base);
        let region_starts = regions.iter().map(|r| r.base).collect();

        Ok(Self {
            file,
            modules,
            ranges,
            starts,
            regions,
            region_starts,
        })
    }

    fn parse_modules(file: &mut File, rva: u64, size: usize) -> Result<Vec<Module>> {
        let buf = read_at(file, rva, size.max(4))?;
        let count = u32le(&buf, 0) as usize;
        let mut mods = Vec::with_capacity(count);
        for i in 0..count {
            let o = 4 + i * MODULE_RECORD_SIZE;
            if o + MODULE_RECORD_SIZE > buf.len() {
                break;
            }
            let base = u64le(&buf, o);
            let isize = u32le(&buf, o + 8);
            let name_rva = u32le(&buf, o + 20) as u64;
            let len_buf = read_at(file, name_rva, 4)?;
            let nlen = u32le(&len_buf, 0) as usize;
            let name_bytes = read_at(file, name_rva + 4, nlen)?;
            let units: Vec<u16> = name_bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let full = String::from_utf16_lossy(&units);
            let name = full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string();
            mods.push(Module {
                base,
                size: isize,
                name,
            });
        }
        Ok(mods)
    }

    fn parse_ranges(file: &mut File, rva: u64) -> Result<Vec<Range>> {
        let head = read_at(file, rva, 16)?;
        let count = u64le(&head, 0) as usize;
        let base_rva = u64le(&head, 8);
        let descs = read_at(file, rva + 16, count * 16)?;
        let mut ranges = Vec::with_capacity(count);
        let mut off = base_rva;
        for i in 0..count {
            let o = i * 16;
            let va = u64le(&descs, o);
            let dsz = u64le(&descs, o + 8);
            ranges.push(Range {
                va,
                size: dsz,
                file_off: off,
            });
            off += dsz;
        }
        Ok(ranges)
    }

    fn parse_regions(file: &mut File, rva: u64) -> Result<Vec<Region>> {
        // MINIDUMP_MEMORY_INFO_LIST : u32 SizeOfHeader, u32 SizeOfEntry, u64 NumberOfEntries.
        let head = read_at(file, rva, 16)?;
        let soh = u32le(&head, 0) as u64;
        let soe = u32le(&head, 4) as usize;
        let count = u64le(&head, 8) as usize;
        let buf = read_at(file, rva + soh, count * soe)?;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let o = i * soe;
            // u64 Base, u64 Alloc, u32 AllocProt, u32 a1, u64 RegionSize, u32 State, u32 Protect, u32 Type, u32 a2
            out.push(Region {
                base: u64le(&buf, o),
                size: u64le(&buf, o + 24),
                state: u32le(&buf, o + 32),
                protect: u32le(&buf, o + 36),
                mtype: u32le(&buf, o + 40),
            });
        }
        Ok(out)
    }

    /// Module (nom, base, taille) par nom de fichier, casse-insensible.
    #[must_use]
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }

    /// Total d'octets mémoire capturés (toutes régions confondues).
    #[must_use]
    pub fn mapped_bytes(&self) -> u64 {
        self.ranges.iter().map(|r| r.size).sum()
    }

    /// Nombre de plages mémoire capturées.
    #[must_use]
    pub fn range_count(&self) -> usize {
        self.ranges.len()
    }

    /// Régions mémoire (avec protections), triées par adresse.
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    fn classify(&self, va: u64) -> (Option<String>, Option<u64>) {
        for m in &self.modules {
            if va >= m.base && va < m.base + u64::from(m.size) {
                return (Some(m.name.clone()), Some(va - m.base));
            }
        }
        (None, None)
    }

    /// Région contenant `va`, le cas échéant.
    #[must_use]
    pub fn region_of(&self, va: u64) -> Option<&Region> {
        if self.region_starts.is_empty() {
            return None;
        }
        let i = self.region_starts.partition_point(|&s| s <= va);
        if i == 0 {
            return None;
        }
        let r = &self.regions[i - 1];
        (va >= r.base && va < r.base + r.size).then_some(r)
    }

    fn seg(&self, va: u64) -> Option<Range> {
        if self.starts.is_empty() {
            return None;
        }
        let i = self.starts.partition_point(|&s| s <= va);
        if i == 0 {
            return None;
        }
        let r = self.ranges[i - 1];
        (va >= r.va && va < r.va + r.size).then_some(r)
    }

    fn read_into(&mut self, off: u64, buf: &mut [u8]) -> Option<()> {
        self.file.seek(SeekFrom::Start(off)).ok()?;
        self.file.read_exact(buf).ok()?;
        Some(())
    }

    /// Lit `n` octets à l'adresse virtuelle `va` (recolle les plages contiguës si besoin).
    /// Renvoie `None` si une partie de `[va, va+n)` n'est pas mappée.
    pub fn read(&mut self, va: u64, n: usize) -> Option<Vec<u8>> {
        let mut out = vec![0u8; n];
        let mut filled = 0usize;
        let mut cur = va;
        while filled < n {
            let r = self.seg(cur)?;
            let within = cur - r.va;
            let take = ((n - filled) as u64).min(r.size - within) as usize;
            self.read_into(r.file_off + within, &mut out[filled..filled + take])?;
            filled += take;
            cur += take as u64;
        }
        Some(out)
    }

    /// Lecture typée little-endian à l'adresse virtuelle `va`.
    pub fn u32_at(&mut self, va: u64) -> Option<u32> {
        self.read(va, 4).map(|b| u32le(&b, 0))
    }
    /// Lecture `i32` little-endian.
    pub fn i32_at(&mut self, va: u64) -> Option<i32> {
        self.u32_at(va).map(|v| v as i32)
    }
    /// Lecture `u16` little-endian.
    pub fn u16_at(&mut self, va: u64) -> Option<u16> {
        self.read(va, 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    /// Lecture `u64` / pointeur little-endian.
    pub fn u64_at(&mut self, va: u64) -> Option<u64> {
        self.read(va, 8).map(|b| u64le(&b, 0))
    }
    /// Lecture `f32` little-endian.
    pub fn f32_at(&mut self, va: u64) -> Option<f32> {
        self.u32_at(va).map(f32::from_bits)
    }

    /// Plage virtuelle `(va, taille)` d'une section PE d'un module chargé (ex. `b".rdata"`).
    pub fn module_section(&mut self, base: u64, name: &[u8]) -> Option<(u64, u32)> {
        let e = u64::from(self.u32_at(base + 0x3c)?);
        let pe = base + e;
        let nsec = self.u16_at(pe + 6)?;
        let opt = u64::from(self.u16_at(pe + 20)?);
        let sect = pe + 24 + opt;
        for i in 0..u64::from(nsec) {
            let o = sect + i * 40;
            let raw = self.read(o, 8)?;
            let nm: Vec<u8> = raw.into_iter().take_while(|&c| c != 0).collect();
            if nm == name {
                let vsize = self.u32_at(o + 8)?;
                let va = base + u64::from(self.u32_at(o + 12)?);
                return Some((va, vsize));
            }
        }
        None
    }

    /// Résout le nom de classe RTTI MSVC (x64) d'une vtable (`.?AVFoo@@` → `Foo`).
    /// Renvoie `None` si la vtable n'a pas de `CompleteObjectLocator` valide.
    pub fn rtti_name(&mut self, vtable_va: u64) -> Option<String> {
        let col = self.u64_at(vtable_va.checked_sub(8)?)?;
        let pself = u64::from(self.u32_at(col + 20)?);
        let ptd = u64::from(self.u32_at(col + 12)?);
        let image_base = col.checked_sub(pself)?;
        let td = image_base + ptd;
        let raw = self.read(td + 16, 256)?;
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        let name = std::str::from_utf8(&raw[..end]).ok()?;
        name.starts_with(".?A").then(|| demangle(name))
    }

    /// Census d'objets vivants : compte, dans le tas (committed/private/RW), les qwords
    /// pointant dans `[vtable_lo, vtable_hi)` (typiquement la `.rdata` de `nie.exe`).
    /// Renvoie `(vtable_va, count)` trié par count décroissant.
    pub fn vtable_census(&mut self, vtable_lo: u64, vtable_hi: u64) -> Vec<(u64, u32)> {
        let ranges = self.ranges.clone();
        let mut counts: HashMap<u64, u32> = HashMap::new();
        let mut buf = Vec::new();
        for r in ranges {
            if !self.region_of(r.va).is_some_and(Region::is_heap) {
                continue;
            }
            let n = (r.size as usize) & !7;
            if n == 0 {
                continue;
            }
            buf.clear();
            buf.resize(n, 0);
            if self.read_into(r.file_off, &mut buf).is_none() {
                continue;
            }
            for chunk in buf.chunks_exact(8) {
                let q = u64::from_le_bytes(chunk.try_into().unwrap());
                if q >= vtable_lo && q < vtable_hi {
                    *counts.entry(q).or_insert(0) += 1;
                }
            }
        }
        let mut v: Vec<(u64, u32)> = counts.into_iter().collect();
        v.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Scanne un motif unique sur toute la mémoire capturée.
    ///
    /// # Errors
    /// Propage les erreurs de lecture du fichier dump.
    pub fn scan(&mut self, pattern: &Pattern) -> Result<Vec<Hit>> {
        Ok(self
            .scan_many(std::slice::from_ref(pattern))?
            .pop()
            .unwrap_or_default())
    }

    /// Scanne plusieurs motifs en **une seule passe disque** (lit chaque région une fois).
    ///
    /// Renvoie un vecteur parallèle à `patterns` : `out[i]` = coups du motif `patterns[i]`.
    ///
    /// # Errors
    /// Propage les erreurs de lecture du fichier dump.
    pub fn scan_many(&mut self, patterns: &[Pattern]) -> Result<Vec<Vec<Hit>>> {
        let mut out: Vec<Vec<Hit>> = vec![Vec::new(); patterns.len()];
        let ranges = self.ranges.clone();
        let mut buf = Vec::new();
        for r in &ranges {
            let len = r.size as usize;
            buf.clear();
            buf.resize(len, 0);
            self.file.seek(SeekFrom::Start(r.file_off))?;
            self.file.read_exact(&mut buf)?;
            for (pi, pat) in patterns.iter().enumerate() {
                let n = pat.bytes.len();
                if len < n {
                    continue;
                }
                let anchor = pat.first_fixed();
                let anchor_byte = pat.bytes[anchor];
                let last = len - n;
                let mut i = 0;
                while i <= last {
                    if buf[i + anchor] == anchor_byte && pat.matches_at(&buf, i) {
                        let va = r.va + i as u64;
                        let (module, rva) = self.classify(va);
                        out[pi].push(Hit { va, module, rva });
                    }
                    i += 1;
                }
            }
        }
        Ok(out)
    }

    /// Comme [`Minidump::scan`], mais s'arrête dès `limite` coups — le scan relit des régions
    /// de plusieurs centaines de Mo, borner la sortie borne aussi le travail restant.
    ///
    /// # Errors
    /// Propage les erreurs de lecture du fichier dump.
    pub fn scan_limited(&mut self, pattern: &Pattern, limite: usize) -> Result<Vec<Hit>> {
        let mut out: Vec<Hit> = Vec::new();
        let ranges = self.ranges.clone();
        let mut buf = Vec::new();
        let n = pattern.bytes.len();
        let anchor = pattern.first_fixed();
        let anchor_byte = pattern.bytes[anchor];
        for r in &ranges {
            if out.len() >= limite {
                break;
            }
            let len = r.size as usize;
            if len < n {
                continue;
            }
            buf.clear();
            buf.resize(len, 0);
            self.file.seek(SeekFrom::Start(r.file_off))?;
            self.file.read_exact(&mut buf)?;
            let last = len - n;
            let mut i = 0;
            while i <= last {
                if buf[i + anchor] == anchor_byte && pattern.matches_at(&buf, i) {
                    let va = r.va + i as u64;
                    let (module, rva) = self.classify(va);
                    out.push(Hit { va, module, rva });
                    if out.len() >= limite {
                        break;
                    }
                }
                i += 1;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aob_with_wildcards() {
        let p = Pattern::parse("44 8B ?? 10 ?? C0").unwrap();
        assert_eq!(p.bytes, vec![0x44, 0x8B, 0x00, 0x10, 0x00, 0xC0]);
        assert_eq!(p.mask, vec![true, true, false, true, false, true]);
        assert_eq!(p.first_fixed(), 0);
    }

    #[test]
    fn parse_aob_rejects_garbage() {
        assert!(Pattern::parse("44 ZZ").is_err());
        assert!(Pattern::parse("   ").is_err());
    }

    #[test]
    fn matches_respects_mask() {
        let p = Pattern::parse("AA ?? CC").unwrap();
        assert!(p.matches_at(&[0xAA, 0x12, 0xCC], 0));
        assert!(p.matches_at(&[0xAA, 0xFF, 0xCC], 0));
        assert!(!p.matches_at(&[0xAB, 0x12, 0xCC], 0));
    }

    #[test]
    fn nie_static_address() {
        let h = Hit {
            va: 0,
            module: Some("nie.exe".to_string()),
            rva: Some(0xD7_2145),
        };
        assert_eq!(h.nie_static(), Some(0x1_40D7_2145));
    }

    #[test]
    fn demangle_msvc_rtti() {
        assert_eq!(
            demangle(".?AVCUniformBlock@lives@@"),
            "lives::CUniformBlock"
        );
        assert_eq!(demangle(".?AUFoo@@"), "Foo");
        assert_eq!(demangle(".?AVCObjLua@game@@"), "game::CObjLua");
    }

    #[test]
    fn heap_region_predicate() {
        let r = Region {
            base: 0x1000,
            size: 0x1000,
            state: MEM_COMMIT,
            protect: PAGE_READWRITE,
            mtype: MEM_PRIVATE,
        };
        assert!(r.is_heap());
        let img = Region {
            mtype: 0x1000000,
            ..r
        };
        assert!(!img.is_heap());
    }
}
