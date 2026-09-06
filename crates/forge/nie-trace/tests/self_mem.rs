//! Tests d'intégration des primitives mémoire **contre le propre process du test**.
//!
//! Sous Linux, `process_vm_readv`/`writev` opèrent sur le `mm` partagé quand cible == appelant, donc
//! on peut exercer les *chemins de succès* lecture/écriture/scan/chaîne sans `nie.exe` lancé (yama
//! `ptrace_scope` n'entrave pas l'accès au même process). Sur les plateformes sans backend, ces
//! tests sont neutralisés.

#![cfg(target_os = "linux")]

use nie_trace::aob::Pattern;
use nie_trace::{
    MapEntry, dump_regions, enumerate_regions, find_module_base, find_pid_by_name, module_range,
    module_regions, read, read_exact, read_f32, read_i32, read_u8, read_u16, read_u32, read_u64,
    resolve_chain, scan_regions, scan_regions_masked, write, write_exact, write_f32, write_i32,
    write_u8, write_u16, write_u32, write_u64,
};

fn me() -> i32 {
    std::process::id() as i32
}

fn comm() -> String {
    std::fs::read_to_string(format!("/proc/{}/comm", me()))
        .unwrap()
        .trim()
        .to_owned()
}

fn region_over(buf: &[u8]) -> MapEntry {
    let start = buf.as_ptr() as u64;
    MapEntry {
        start,
        end: start + buf.len() as u64,
        perms: "rw-".into(),
        offset: 0,
        path: "self".into(),
    }
}

#[test]
fn read_roundtrip_typed() {
    let buf = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let a = buf.as_ptr() as u64;
    assert_eq!(read_u8(me(), a).unwrap(), 0x11);
    assert_eq!(read_u16(me(), a).unwrap(), 0x2211);
    assert_eq!(read_u32(me(), a).unwrap(), 0x4433_2211);
    assert_eq!(read_i32(me(), a).unwrap(), 0x4433_2211);
    assert_eq!(read_u64(me(), a).unwrap(), 0x8877_6655_4433_2211);
    assert_eq!(read_exact(me(), a, 3).unwrap(), vec![0x11, 0x22, 0x33]);
    let f = 1.5f32.to_le_bytes();
    assert_eq!(read_f32(me(), f.as_ptr() as u64).unwrap(), 1.5);
}

#[test]
fn write_roundtrip_typed() {
    let buf = [0u8; 32];
    let a = buf.as_ptr() as u64; // adresse stable tant que `buf` vit
    write_u8(me(), a, 0xAB).unwrap();
    assert_eq!(buf[0], 0xAB);
    write_u16(me(), a, 0xBEEF).unwrap();
    assert_eq!(&buf[0..2], &[0xEF, 0xBE]);
    write_u32(me(), a, 0xDEAD_BEEF).unwrap();
    assert_eq!(&buf[0..4], &0xDEAD_BEEFu32.to_le_bytes());
    write_i32(me(), a, -2).unwrap();
    assert_eq!(read_i32(me(), a).unwrap(), -2);
    write_u64(me(), a, 0x0102_0304_0506_0708).unwrap();
    assert_eq!(read_u64(me(), a).unwrap(), 0x0102_0304_0506_0708);
    write_f32(me(), a, 2.5).unwrap();
    assert_eq!(read_f32(me(), a).unwrap(), 2.5);
    write_exact(me(), a, &[1, 2, 3, 4]).unwrap();
    assert_eq!(&buf[0..4], &[1, 2, 3, 4]);
}

#[test]
fn pointer_chain_self() {
    // buf : [ ptr→valeur (8o) | valeur u32 @ +8 ]
    let mut buf = [0u8; 16];
    let base = buf.as_ptr() as u64;
    let value_addr = base + 8;
    buf[0..8].copy_from_slice(&value_addr.to_le_bytes());
    buf[8..12].copy_from_slice(&1234u32.to_le_bytes());
    // [base+0] déréférencé → value_addr ; +0 final → value_addr.
    let resolved = resolve_chain(me(), base, &[0, 0]).unwrap();
    assert_eq!(resolved, value_addr);
    assert_eq!(read_u32(me(), resolved).unwrap(), 1234);
    // chaîne vide → base.
    assert_eq!(resolve_chain(me(), base, &[]).unwrap(), base);
}

#[test]
fn scan_self_exact_and_masked() {
    let buf: Vec<u8> = vec![0x00, 0x44, 0x8B, 0x6F, 0x10, 0x8B, 0x47, 0x04, 0x00];
    let regions = vec![region_over(&buf)];
    let base = Some(buf.as_ptr() as u64);

    let exact = scan_regions(me(), &regions, base, &[0x44, 0x8B, 0x6F, 0x10], 8);
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].rva, Some(1));

    let pat = Pattern::parse("44 8B ?? 10 8B 47 04").unwrap();
    let masked = scan_regions_masked(me(), &regions, base, &pat, 8);
    assert_eq!(masked.len(), 1);
    assert_eq!(masked[0].rva, Some(1));
    assert!(masked[0].perms.starts_with('r'));

    // motif absent → 0 hit.
    let none = scan_regions_masked(
        me(),
        &regions,
        base,
        &Pattern::parse("DE AD BE EF").unwrap(),
        8,
    );
    assert!(none.is_empty());

    // garde-fous : aiguille/motif vide → 0 hit.
    assert!(scan_regions(me(), &regions, base, &[], 8).is_empty());
    let empty_pat = Pattern {
        bytes: vec![],
        mask: vec![],
    };
    assert!(scan_regions_masked(me(), &regions, base, &empty_pat, 8).is_empty());
    // limite 0 → 0 hit même si présent.
    assert!(
        scan_regions_masked(me(), &regions, base, &Pattern::parse("44 8B").unwrap(), 0).is_empty()
    );
}

#[test]
fn lib_dispatch_over_self() {
    let me = me();
    let c = comm();
    // dispatch lib → backend Wine, contre notre propre process.
    assert!(!enumerate_regions(me).is_empty());
    assert_eq!(find_pid_by_name(&c), Some(me));
    // module_regions : filtré par fragment, et tout (--all).
    assert!(!module_regions(me, "", true).is_empty());
    let _filtered = module_regions(me, &c, false); // peut être vide si le nom ne matche pas un chemin
    if let Some(base) = find_module_base(me, &c) {
        let (b, end) = module_range(me, &c).unwrap();
        assert_eq!(b, base);
        assert!(end >= base);
    }
    // read/write nuls = no-op via le dispatch.
    assert_eq!(read(me, 0x1000, &mut []).unwrap(), 0);
    assert_eq!(write(me, 0x1000, &[]).unwrap(), 0);
}

#[test]
fn read_exact_partial_at_mapping_gap() {
    // Trouve une plage lisible suivie d'un TROU (process_vm_readv s'arrête au 1er octet non mappé).
    let maps = std::fs::read_to_string(format!("/proc/{}/maps", me())).unwrap();
    let mut ranges: Vec<(u64, u64, bool)> = Vec::new();
    for line in maps.lines() {
        let mut it = line.split_whitespace();
        let (Some(r), Some(perms)) = (it.next(), it.next()) else {
            continue;
        };
        let Some(dash) = r.find('-') else { continue };
        let (Ok(s), Ok(e)) = (
            u64::from_str_radix(&r[..dash], 16),
            u64::from_str_radix(&r[dash + 1..], 16),
        ) else {
            continue;
        };
        ranges.push((s, e, perms.starts_with('r')));
    }
    // Cherche une plage lisible dont la fin n'est PAS le début de la suivante (trou).
    let gap_end = ranges.windows(2).find_map(|w| {
        let (_, e0, readable) = w[0];
        let (s1, _, _) = w[1];
        (readable && e0 < s1).then_some(e0)
    });
    if let Some(end) = gap_end {
        // 4 octets à cheval sur la frontière : 2 mappés (end-2,end-1) + 2 dans le trou.
        let r = read_exact(me(), end - 2, 4);
        assert!(
            r.is_err(),
            "lecture à cheval sur un trou devrait être partielle/erreur"
        );
    }
}

#[test]
fn scan_and_dump_skip_bad_regions() {
    let buf: Vec<u8> = vec![0x44, 0x8B, 0x6F, 0x10];
    let good = MapEntry {
        start: buf.as_ptr() as u64,
        end: buf.as_ptr() as u64 + 4,
        perms: "r--".into(),
        offset: 0,
        path: "ok".into(),
    };
    let unreadable = MapEntry {
        start: buf.as_ptr() as u64,
        end: buf.as_ptr() as u64 + 4,
        perms: "---".into(),
        offset: 0,
        path: "noread".into(),
    };
    let zero = MapEntry {
        start: 0x4000,
        end: 0x4000,
        perms: "r--".into(),
        offset: 0,
        path: "zero".into(),
    };
    let unmapped = MapEntry {
        start: 0x1,
        end: 0x100,
        perms: "rw-".into(),
        offset: 0,
        path: "gap".into(),
    };
    let regions = vec![unreadable, zero, unmapped, good.clone()];

    // scan saute illisible/vide/non-mappé, ne garde que `good`.
    let needle = [0x44u8, 0x8B, 0x6F, 0x10];
    let hits = scan_regions(me(), &regions, None, &needle, 8);
    assert_eq!(hits.len(), 1);
    let pat = Pattern::parse("44 8B ?? 10").unwrap();
    assert_eq!(scan_regions_masked(me(), &regions, None, &pat, 8).len(), 1);

    // limite atteinte → la 2e plage valide est sautée (branche hits.len() >= limit).
    let two = vec![good.clone(), good.clone()];
    assert_eq!(scan_regions(me(), &two, None, &needle, 1).len(), 1);
    assert_eq!(scan_regions_masked(me(), &two, None, &pat, 1).len(), 1);

    // dump saute pareil : seule `good` est écrite.
    let dir = std::env::temp_dir().join(format!("nie-trace-skip-{}", me()));
    let stats = dump_regions(me(), &regions, &dir).unwrap();
    assert_eq!(stats.regions, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_exact_partial_or_err_at_gap() {
    // process_vm_writev s'arrête au 1er octet non inscriptible → write_exact détecte le partiel/erreur.
    // (page nulle non mappée : écriture impossible → Err, ce qui exerce le chemin d'échec.)
    assert!(write_exact(me(), 0x1, &[1, 2, 3, 4]).is_err());
}

#[test]
fn dump_regions_writes_self_region() {
    let buf: Vec<u8> = (0..64u8).collect();
    let start = buf.as_ptr() as u64;
    let region = MapEntry {
        start,
        end: start + 64,
        perms: "rw-".into(),
        offset: 0,
        path: "self".into(),
    };
    let dir = std::env::temp_dir().join(format!("nie-trace-dump-{}", me()));
    let stats = dump_regions(me(), &[region], &dir).unwrap();
    assert_eq!(stats.regions, 1);
    assert_eq!(stats.bytes, 64);
    let _ = std::fs::remove_dir_all(&dir);
}
