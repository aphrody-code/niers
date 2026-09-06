//! `nie-mem` — lecteur **et éditeur** mémoire autonome d'`nie.exe`.
//!
//! Cross-compilé depuis WSL (`cargo build -p nie-trace --bin nie-mem --target x86_64-pc-windows-gnu`)
//! et lancé via l'interop WSL→Windows pour inspecter le **vrai jeu Windows natif** :
//! `nie-mem.exe maps`, `... scan wstr:Title`, `... read nie.exe+0xF600CA -n 64`, etc.
//! Compile aussi sous Linux (backend Wine/process_vm_readv).
//!
//! Parsing d'arguments minimal (pas de clap) pour un binaire léger et un cross-compile sans surface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

use nie_trace::lancement::lancer_chaine;
use nie_trace::recette;
use nie_trace::{
    dump_regions, find_module_base, find_pid_by_name, module_regions, patch_eac, read_exact,
    scan_regions, write_exact,
};

/// Lit une recette, l'applique, et imprime son rapport règle par règle.
///
/// En mode à blanc (le défaut, sans `--force`), rien n'est écrit : le rapport dit ce qui **aurait**
/// été touché. C'est ce qui permet de vérifier qu'une recette vise bien ce qu'on croit avant de
/// la lancer sur le process.
fn appliquer_recette_fichier(
    pid: i32,
    module: &str,
    chemin: &str,
    a_blanc: bool,
) -> Result<(), String> {
    let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{chemin} : {e}"))?;
    let r = recette::parser(&texte)?;
    println!(
        "\n  recette « {} » — {} règle(s){}",
        if r.nom.is_empty() { chemin } else { &r.nom },
        r.regles.len(),
        if a_blanc { "  [À BLANC]" } else { "" }
    );

    let rapport = recette::appliquer(pid, module, &r, a_blanc);
    for res in &rapport.resultats {
        let quoi = match &res.regle {
            recette::Regle::RemplacerU32 {
                de,
                vers,
                max,
                garde,
            } => {
                let borne = max.map_or_else(String::new, |m| format!(" max {m}"));
                let cond = garde.map_or_else(String::new, |(o, v)| {
                    let signe = if o < 0 { "-" } else { "+" };
                    format!(" si {signe}0x{:X} == 0x{v:08X}", o.abs())
                });
                format!("u32 0x{de:08X} -> 0x{vers:08X}{cond}{borne}")
            }
            recette::Regle::Ecrire { adresse, octets } => {
                format!("at {adresse} = {} octet(s)", octets.len())
            }
        };
        let apercu: Vec<&str> = res.adresses.iter().take(4).map(String::as_str).collect();
        let suite = if res.adresses.len() > 4 {
            format!(" … +{}", res.adresses.len() - 4)
        } else {
            String::new()
        };
        println!(
            "    {quoi}\n      {} trouvée(s), {} écrite(s)  [{}{suite}]{}",
            res.trouvees,
            res.ecrites,
            apercu.join(", "),
            res.erreur
                .as_ref()
                .map_or_else(String::new, |e| format!("  ⚠ {e}"))
        );
    }
    println!(
        "\n  {} écriture(s), {} règle(s) en échec",
        rapport.total_ecrites(),
        rapport.echecs()
    );
    if a_blanc {
        println!("  ajoute --force pour appliquer réellement");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
nie-mem — lecteur/éditeur mémoire d'nie.exe (Windows natif via Read/WriteProcessMemory, ou Wine via process_vm_readv/writev)

USAGE:
  nie-mem find-pid [nom=nie.exe]
  nie-mem maps   [--pid N] [--module nie.exe] [--all]
  nie-mem base   [--pid N] [--module nie.exe]
  nie-mem read   <0xADDR | module+0xRVA> [--len N=256] [--pid N] [--out FICHIER]
  nie-mem write  <0xADDR | module+0xRVA> <hex '48 8B' | str:Texte> [--in FICHIER] [--pid N] --force
  nie-mem dump   [--pid N] [--module nie.exe] [--all] [--out DOSSIER=./memdump]
  nie-mem scan   <hex '48 8B 0D' | str:Texte | wstr:Titre> [--pid N] [--module nie.exe] [--all] [--limit N=20]
  nie-mem patch-eac <src nie.exe> <dst nie_eacpatched.exe>

LIVE MODDING
  nie-mem apply  <recette.txt> [--pid N] [--force]
  nie-mem live   [recette.txt] --game <chemin/nie.exe> [--repo DOSSIER] [--save-editor EXE]
                 [--no-save-editor] [--wait SECS=120] [--force]
      Lance l'editeur de sauvegarde puis le jeu SANS EAC (nie.exe direct), attend que le
      process reponde, puis applique la recette. Sans --force, tout est a blanc.

--pid 0 (défaut) auto-détecte nie.exe.";

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let (positionals, flags) = parse(&args[1..]);

    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        "find-pid" => {
            let name = positionals.first().map_or("nie.exe", String::as_str);
            match find_pid_by_name(name) {
                Some(pid) => {
                    println!("{name} → pid {pid}");
                    Ok(())
                }
                None => Err(format!("{name} introuvable (le jeu tourne-t-il ?)")),
            }
        }
        "maps" => {
            let pid = resolve_pid(&flags)?;
            let module = module_of(&flags);
            let all = flags.contains_key("all");
            let regions = module_regions(pid, &module, all);
            let mut total = 0u64;
            for m in &regions {
                total += m.size();
                println!(
                    "  0x{:012x}-0x{:012x}  {}  {:>12}  {}",
                    m.start,
                    m.end,
                    m.perms,
                    m.size(),
                    m.path
                );
            }
            let suffix = if all {
                String::new()
            } else {
                format!(" (module « {module} »)")
            };
            println!("\n  {} plage(s), {total} octets{suffix}", regions.len());
            Ok(())
        }
        "base" => {
            let pid = resolve_pid(&flags)?;
            let module = module_of(&flags);
            match find_module_base(pid, &module) {
                Some(b) => println!("  {module} @ 0x{b:x} (pid {pid})"),
                None => return Err(format!("module « {module} » introuvable (pid {pid})")),
            }
            Ok(())
        }
        "read" => {
            let pid = resolve_pid(&flags)?;
            let addr_s = positionals.first().ok_or("adresse manquante")?;
            let addr = resolve_addr(addr_s, pid)?;
            let len: usize = flag_num(&flags, "len").unwrap_or(256);
            let buf = read_exact(pid, addr, len).map_err(|e| e.to_string())?;
            match flags.get("out").and_then(|v| v.clone()) {
                Some(path) => {
                    std::fs::write(&path, &buf).map_err(|e| e.to_string())?;
                    println!("  {} octets @ 0x{addr:x} → {path}", buf.len());
                }
                None => hexdump(&buf, addr),
            }
            Ok(())
        }
        // Écriture — le pendant de `read`. La lib porte `write_exact` depuis toujours ; seul le
        // binaire ne l'exposait pas. Une écriture est **relue** juste après et le hexdump montre
        // l'état réel de la mémoire, pas ce qu'on croyait y mettre.
        "write" => {
            let pid = resolve_pid(&flags)?;
            let addr_s = positionals.first().ok_or("adresse manquante")?;
            let addr = resolve_addr(addr_s, pid)?;

            // Deux sources d'octets : un motif hex en argument, ou un fichier via `--in`.
            let octets: Vec<u8> = match flags.get("in").and_then(Clone::clone) {
                Some(path) => std::fs::read(&path).map_err(|e| e.to_string())?,
                None => {
                    let motif = positionals
                        .get(1)
                        .ok_or("octets manquants (hex, ou --in <fichier>)")?;
                    parse_pattern(motif)?.0
                }
            };
            if octets.is_empty() {
                return Err("rien à écrire".to_owned());
            }
            // Garde-fou : une écriture mémoire est irréversible pour le process visé. On exige
            // `--force`, comme `nie-edit`, pour qu'un `write` ne parte jamais par accident.
            if !flags.contains_key("force") {
                let avant = read_exact(pid, addr, octets.len()).map_err(|e| e.to_string())?;
                println!("  à blanc — {} octet(s) @ 0x{addr:x}", octets.len());
                hexdump(&avant, addr);
                println!("  deviendrait :");
                hexdump(&octets, addr);
                println!("\n  ajoute --force pour écrire réellement");
                return Ok(());
            }
            write_exact(pid, addr, &octets).map_err(|e| e.to_string())?;
            let relu = read_exact(pid, addr, octets.len()).map_err(|e| e.to_string())?;
            println!("  écrit {} octet(s) @ 0x{addr:x} — relu :", octets.len());
            hexdump(&relu, addr);
            if relu != octets {
                return Err("la relecture diffère de ce qui a été écrit".to_owned());
            }
            Ok(())
        }
        // Chaîne complète de live modding : éditeur de sauvegarde → jeu (sans EAC) → recette.
        // C'est le mode « je relance et tout se réapplique » : les adresses changent à chaque
        // lancement, la recette s'exprime en valeurs, donc elle survit au redémarrage.
        "live" => {
            let racine = flags.get("repo").and_then(Clone::clone).map_or_else(
                || std::env::current_dir().unwrap_or_default(),
                PathBuf::from,
            );
            let editeur = flags.get("save-editor").and_then(Clone::clone).map_or_else(
                || racine.join("InazumaElevenVRSaveEditor.exe"),
                PathBuf::from,
            );
            let jeu = flags
                .get("game")
                .and_then(Clone::clone)
                .map(PathBuf::from)
                .ok_or("--game <chemin de nie.exe> est requis")?;
            let attente =
                std::time::Duration::from_secs(flag_num(&flags, "wait").unwrap_or(120) as u64);
            let sans_editeur = flags.contains_key("no-save-editor");

            let l = lancer_chaine(
                (!sans_editeur).then_some(editeur.as_path()),
                Some(jeu.as_path()),
                attente,
            );
            for d in &l.demarres {
                println!("  démarré  {}", d.display());
            }
            for a in &l.absents {
                println!("  ABSENT   {}", a.display());
            }
            for e in &l.echecs {
                println!(
                    "  ÉCHEC    {} (démarrage refusé, même via le shell)",
                    e.display()
                );
            }
            let Some(pid) = l.pid else {
                return Err(format!(
                    "le jeu n'a pas répondu en {} s — rien n'a été appliqué",
                    attente.as_secs()
                ));
            };
            println!("  jeu prêt (pid {pid})");

            // Sans recette, on s'arrête au lancement : `live` sert alors juste à démarrer la
            // chaîne sans EAC.
            let Some(chemin) = positionals.first() else {
                println!("\n  aucune recette donnée — chaîne lancée, rien appliqué");
                return Ok(());
            };
            appliquer_recette_fichier(
                pid,
                &module_of(&flags),
                chemin,
                !flags.contains_key("force"),
            )
        }
        // Applique une recette au jeu déjà lancé.
        "apply" => {
            let pid = resolve_pid(&flags)?;
            let chemin = positionals.first().ok_or("recette manquante")?;
            appliquer_recette_fichier(
                pid,
                &module_of(&flags),
                chemin,
                !flags.contains_key("force"),
            )
        }
        "dump" => {
            let pid = resolve_pid(&flags)?;
            let module = module_of(&flags);
            let all = flags.contains_key("all");
            let out = flags
                .get("out")
                .and_then(Clone::clone)
                .unwrap_or_else(|| "./memdump".to_owned());
            let regions = module_regions(pid, &module, all);
            let stats =
                dump_regions(pid, &regions, &PathBuf::from(&out)).map_err(|e| e.to_string())?;
            println!(
                "  {} plage(s) dumpée(s), {} octets → {out}",
                stats.regions, stats.bytes
            );
            Ok(())
        }
        "scan" => {
            let pid = resolve_pid(&flags)?;
            let module = module_of(&flags);
            let all = flags.contains_key("all");
            let limit: usize = flag_num(&flags, "limit").unwrap_or(20);
            let pattern = positionals.first().ok_or("motif manquant")?;
            let (needle, label) = parse_pattern(pattern)?;
            let regions = module_regions(pid, &module, all);
            let base = find_module_base(pid, &module);
            let hits = scan_regions(pid, &regions, base, &needle, limit);
            for h in &hits {
                let rva = h
                    .rva
                    .map(|r| format!(" ({module}+0x{r:x})"))
                    .unwrap_or_default();
                println!("  0x{:012x}{rva}  [{}]", h.addr, h.perms);
            }
            let capped = if hits.len() >= limit {
                format!(" (limité à {limit})")
            } else {
                String::new()
            };
            println!("\n  {} hit(s) pour {label}{capped}", hits.len());
            Ok(())
        }
        "patch-eac" => {
            let src = positionals.first().ok_or("src manquant")?;
            let dst = positionals.get(1).ok_or("dst manquant")?;
            let r =
                patch_eac(&PathBuf::from(src), &PathBuf::from(dst)).map_err(|e| e.to_string())?;
            println!(
                "  OK  offset 0x{:X}: {} -> {}  ({} octets)  {dst}",
                r.offset,
                hexs(&r.original),
                hexs(&r.patched),
                r.dst_len
            );
            Ok(())
        }
        other => Err(format!("commande inconnue « {other} »\n\n{USAGE}")),
    }
}

/// Sépare positionnels et flags. `--k v` (valeur) ou `--k` (booléen → valeur None).
fn parse(args: &[String]) -> (Vec<String>, HashMap<String, Option<String>>) {
    let mut pos = Vec::new();
    let mut flags: HashMap<String, Option<String>> = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(key) = a
            .strip_prefix("--")
            .or_else(|| a.strip_prefix('-'))
            .filter(|_| a.starts_with('-'))
        {
            // booléens connus sans valeur
            if key == "all" {
                flags.insert("all".to_owned(), None);
            } else if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                flags.insert(normalize(key), Some(args[i + 1].clone()));
                i += 1;
            } else {
                flags.insert(normalize(key), None);
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    (pos, flags)
}

fn normalize(key: &str) -> String {
    match key {
        "p" => "pid",
        "m" => "module",
        "n" => "len",
        "o" => "out",
        "l" => "limit",
        k => k,
    }
    .to_owned()
}

fn resolve_pid(flags: &HashMap<String, Option<String>>) -> Result<i32, String> {
    let pid = flag_num::<i32>(flags, "pid").unwrap_or(0);
    if pid > 0 {
        return Ok(pid);
    }
    find_pid_by_name("nie.exe")
        .ok_or_else(|| "nie.exe introuvable — lance le jeu, ou précise --pid".to_owned())
}

fn module_of(flags: &HashMap<String, Option<String>>) -> String {
    flags
        .get("module")
        .and_then(Clone::clone)
        .unwrap_or_else(|| "nie.exe".to_owned())
}

fn flag_num<T: std::str::FromStr>(flags: &HashMap<String, Option<String>>, key: &str) -> Option<T> {
    flags
        .get(key)
        .and_then(Clone::clone)
        .and_then(|v| v.parse::<T>().ok())
}

/// `0x…` (absolu) ou `module+0xRVA`.
fn resolve_addr(addr: &str, pid: i32) -> Result<u64, String> {
    let s = addr.trim();
    if let Some(plus) = s.find('+') {
        let module = &s[..plus];
        let rva = parse_hex(&s[plus + 1..])?;
        let base = find_module_base(pid, module)
            .ok_or_else(|| format!("module « {module} » introuvable"))?;
        return Ok(base + rva);
    }
    parse_hex(s)
}

fn parse_hex(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(s, 16).map_err(|_| format!("adresse hex invalide: {s}"))
}

/// `wstr:` (UTF-16LE), `str:` (UTF-8), ou octets hex.
fn parse_pattern(pattern: &str) -> Result<(Vec<u8>, String), String> {
    if let Some(t) = pattern.strip_prefix("wstr:") {
        let n: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
        if n.is_empty() {
            return Err("motif wstr: vide".to_owned());
        }
        return Ok((n, format!("wstr \"{t}\"")));
    }
    if let Some(t) = pattern.strip_prefix("str:") {
        if t.is_empty() {
            return Err("motif str: vide".to_owned());
        }
        return Ok((t.as_bytes().to_vec(), format!("\"{t}\"")));
    }
    let hex: String = pattern
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return Err("motif hex de longueur impaire/vide".to_owned());
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        bytes.push(
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("octet hex invalide à {i}"))?,
        );
    }
    let label = format!("hex {}", hexs_join(&bytes));
    Ok((bytes, label))
}

fn hexdump(data: &[u8], base: u64) {
    for (i, line) in data.chunks(16).enumerate() {
        let off = base + (i * 16) as u64;
        let mut hex = String::new();
        for j in 0..16 {
            if j < line.len() {
                hex.push_str(&format!("{:02x} ", line[j]));
            } else {
                hex.push_str("   ");
            }
        }
        let ascii: String = line
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("  0x{off:012x}  {hex} {ascii}");
    }
}

fn hexs(b: &[u8; 5]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hexs_join(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Motif distinctif en `.rodata` du binaire de test : `scan` doit le retrouver dans une plage
    /// mappée du process (chemin = binaire de test → contient le `comm`). `#[used]` + une lecture
    /// runtime (`black_box`) garantissent sa rétention par l'éditeur de liens.
    #[used]
    static MARKER: [u8; 16] = [
        0x4E, 0x49, 0x45, 0x4D, 0x45, 0x4D, 0x5A, 0x5A, 0xC0, 0xFF, 0xEE, 0xBA, 0xDD, 0xF0, 0x0D,
        0x55,
    ];

    /// Construit un `Vec<String>` à partir de `&str` et appelle `run`.
    fn r(args: &[&str]) -> Result<(), String> {
        let owned: Vec<String> = args.iter().copied().map(String::from).collect();
        run(&owned)
    }

    /// Chemin temporaire unique au process (nettoyé par chaque test).
    fn tmp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("niemem-test-{}-{name}", std::process::id()))
    }

    // ── Helpers purs (toutes plateformes) ────────────────────────────────────

    #[test]
    fn marker_is_retained() {
        assert_eq!(std::hint::black_box(&MARKER).len(), 16);
    }

    #[test]
    fn parse_positionals_flags_and_booleans() {
        let raw: Vec<String> = [
            "pos1", "--all", "--pid", "42", "-m", "modx", "--solo", "--a", "--b",
        ]
        .iter()
        .copied()
        .map(String::from)
        .collect();
        let (pos, flags) = parse(&raw);
        assert_eq!(pos, vec!["pos1".to_owned()]);
        assert!(flags.contains_key("all") && flags["all"].is_none()); // booléen connu
        assert_eq!(flags["pid"].as_deref(), Some("42")); // flag à valeur
        assert_eq!(flags["module"].as_deref(), Some("modx")); // alias -m → module
        assert!(flags["solo"].is_none()); // suivi d'un flag → booléen
        assert!(flags["a"].is_none()); // suivi d'un flag → booléen
        assert!(flags["b"].is_none()); // dernier argument → booléen
    }

    #[test]
    fn normalize_all_aliases() {
        assert_eq!(normalize("p"), "pid");
        assert_eq!(normalize("m"), "module");
        assert_eq!(normalize("n"), "len");
        assert_eq!(normalize("o"), "out");
        assert_eq!(normalize("l"), "limit");
        assert_eq!(normalize("xyz"), "xyz"); // bras par défaut
    }

    #[test]
    fn parse_hex_variants() {
        assert_eq!(parse_hex("0x1F").unwrap(), 0x1F);
        assert_eq!(parse_hex("0XfF").unwrap(), 0xFF); // préfixe majuscule
        assert_eq!(parse_hex(" ff ").unwrap(), 0xFF); // sans préfixe + trim
        assert!(parse_hex("zz").is_err());
    }

    #[test]
    fn parse_pattern_all_branches() {
        assert!(parse_pattern("wstr:Hi").is_ok());
        assert!(parse_pattern("wstr:").is_err()); // wstr vide
        assert!(parse_pattern("str:Hi").is_ok());
        assert!(parse_pattern("str:").is_err()); // str vide
        let (bytes, label) = parse_pattern("41 42-43").unwrap(); // hex + séparateurs filtrés
        assert_eq!(bytes, vec![0x41, 0x42, 0x43]);
        assert!(label.contains("41"));
        assert!(parse_pattern("abc").is_err()); // longueur impaire
        assert!(parse_pattern("").is_err()); // vide
        assert!(parse_pattern("zz").is_err()); // octet hex invalide
    }

    #[test]
    fn hexdump_and_hexs_helpers() {
        // 20 octets → 2e ligne partielle (couvre le padding) ; octets imprimables et non imprimables.
        let sample: [u8; 20] = [
            0x00, 0x41, 0x7e, 0x7f, 0x20, 0x1f, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
            0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
        ];
        hexdump(&sample, 0x1_4000_0000);
        assert_eq!(hexs(&nie_trace::EAC_PATCH_NOP), "9090909090");
        assert_eq!(hexs_join(&[0xDE, 0xAD]), "DE-AD");
    }

    // ── Commandes via run() : chemins indépendants de la plateforme ──────────

    #[test]
    fn run_help_empty_unknown() {
        r(&["help"]).unwrap();
        r(&["-h"]).unwrap();
        r(&["--help"]).unwrap();
        r(&[]).unwrap(); // aucun argument → USAGE
        assert!(r(&["commande-bidon"]).is_err()); // commande inconnue
    }

    #[test]
    fn run_find_pid_absent_is_err() {
        // Un nom qui ne tourne jamais : la seule assertion inconditionnelle.
        assert!(r(&["find-pid", "zzz-process-inexistant"]).is_err());
        // Le défaut est "nie.exe" : il ÉCHOUE si le jeu n'est pas lancé, RÉUSSIT s'il l'est.
        // Assener l'échec ferait rougir la suite pendant toute session de RE en direct.
        let _ = r(&["find-pid"]);
    }

    #[test]
    fn run_patch_eac_paths() {
        let dir = tmp_path("eac");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("nie.exe");
        let dst = dir.join("nie_eacpatched.exe");
        let off = nie_trace::EAC_PATCH_OFFSET as usize;
        let mut data = vec![0u8; off + 0x1000];
        data[off..off + 5].copy_from_slice(&nie_trace::EAC_PATCH_ORIG);
        std::fs::write(&src, &data).unwrap();

        r(&["patch-eac", src.to_str().unwrap(), dst.to_str().unwrap()]).unwrap(); // succès
        assert!(r(&["patch-eac"]).is_err()); // src manquant
        assert!(r(&["patch-eac", src.to_str().unwrap()]).is_err()); // dst manquant

        // erreur de patch : src trop petit → read_exact échoue à l'offset EAC.
        let small = dir.join("small.exe");
        std::fs::write(&small, b"tiny").unwrap();
        let small_dst = dir.join("small_out.exe");
        assert!(
            r(&[
                "patch-eac",
                small.to_str().unwrap(),
                small_dst.to_str().unwrap()
            ])
            .is_err()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn main_entrypoint_runs() {
        // Exerce `main` : lit les arguments ambiants du binaire de test et dispatch vers `run`.
        let _ = main();
    }

    // ── Commandes via run() pilotées contre le propre process (Linux) ────────

    #[cfg(target_os = "linux")]
    fn me_pid() -> String {
        (std::process::id() as i32).to_string()
    }

    #[cfg(target_os = "linux")]
    fn comm_name() -> String {
        std::fs::read_to_string("/proc/self/comm")
            .unwrap()
            .trim()
            .to_owned()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_maps_self() {
        let pid = me_pid();
        let comm = comm_name();
        r(&["maps", "--pid", &pid, "--module", &comm]).unwrap(); // plages non vides → corps de boucle
        r(&["maps", "--pid", &pid, "--all"]).unwrap(); // --all → suffixe vide
        assert!(r(&["maps"]).is_err()); // pas de --pid → nie.exe introuvable
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_base_self() {
        let pid = me_pid();
        let comm = comm_name();
        r(&["base", "--pid", &pid, "--module", &comm]).unwrap(); // base trouvée
        assert!(r(&["base", "--pid", &pid, "--module", "zzz-no-such-module"]).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_read_self() {
        let pid = me_pid();
        let comm = comm_name();
        let buf: Vec<u8> = (0..32u8)
            .map(|i| if i % 3 == 0 { 0 } else { 0x40 + i })
            .collect();
        let addr = format!("0x{:x}", buf.as_ptr() as u64);

        r(&["read", &addr, "--pid", &pid, "--len", "24"]).unwrap(); // hexdump (out None)
        let out = tmp_path("read.bin");
        let outs = out.to_string_lossy().into_owned();
        r(&["read", &addr, "--pid", &pid, "--len", "16", "--out", &outs]).unwrap(); // out Some
        assert!(out.exists());
        let _ = std::fs::remove_file(&out);

        assert!(r(&["read", "--pid", &pid]).is_err()); // adresse manquante
        assert!(r(&["read", "zz", "--pid", &pid]).is_err()); // hex invalide
        let modrva = format!("{comm}+0x0");
        r(&["read", &modrva, "--pid", &pid, "--len", "8"]).unwrap(); // module+RVA
        assert!(r(&["read", "zzz-no-such-module+0x10", "--pid", &pid]).is_err()); // module introuvable
        assert!(r(&["read", "0x1", "--pid", &pid, "--len", "8"]).is_err()); // page nulle → EFAULT
        assert_eq!(buf.len(), 32); // garde `buf` vivant jusqu'ici
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_dump_self() {
        let pid = me_pid();
        let comm = comm_name();
        let dir = tmp_path("dump");
        let ds = dir.to_string_lossy().into_owned();
        // Limité à --module (binaire de test, quelques Mo), JAMAIS --all.
        r(&["dump", "--pid", &pid, "--module", &comm, "--out", &ds]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_scan_self() {
        let pid = me_pid();
        let comm = comm_name();
        std::hint::black_box(&MARKER);
        let pat: String = MARKER
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");

        // --module : base connue → rva Some ; limite 1 → branche « limité ».
        r(&[
            "scan", &pat, "--pid", &pid, "--module", &comm, "--limit", "1",
        ])
        .unwrap();
        // --all : base "nie.exe" introuvable → rva None (au moins MARKER est trouvé).
        r(&["scan", &pat, "--pid", &pid, "--all", "--limit", "1"]).unwrap();
        // wstr / str : module défaut "nie.exe" → plages vides → 0 hit (branche non « limité »).
        r(&["scan", "wstr:Title", "--pid", &pid]).unwrap();
        r(&["scan", "str:Hello", "--pid", &pid]).unwrap();
        assert!(r(&["scan", "--pid", &pid]).is_err()); // motif manquant
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn run_find_pid_self() {
        let comm = comm_name();
        r(&["find-pid", &comm]).unwrap(); // notre propre comm → Some
    }

    /// Couvre les sous-régions d'erreur (`?` propagé, fermeture `--out` par défaut) de chaque
    /// commande, sans `nie.exe` lancé.
    #[cfg(target_os = "linux")]
    #[test]
    fn run_error_subregions() {
        let comm = comm_name();
        let pid = me_pid();

        // resolve_pid échoue (pas de --pid → nie.exe introuvable) dans base/read/dump/scan.
        assert!(r(&["base", "--module", &comm]).is_err());
        assert!(r(&["read", "0x1000", "--module", &comm]).is_err());
        assert!(r(&["dump", "--module", &comm]).is_err());
        assert!(r(&["scan", "str:x", "--module", &comm]).is_err());

        // parse_pattern échoue (octet hex invalide) après resolve_pid OK.
        assert!(r(&["scan", "zz", "--pid", &pid]).is_err());

        // parse_hex échoue dans la branche module+RVA de resolve_addr.
        let bad_rva = format!("{comm}+0xZZ");
        assert!(r(&["read", &bad_rva, "--pid", &pid]).is_err());

        // std::fs::write échoue (dossier inexistant) sur read --out.
        let buf = [0u8; 8];
        let addr = format!("0x{:x}", buf.as_ptr() as u64);
        assert!(
            r(&[
                "read",
                &addr,
                "--pid",
                &pid,
                "--out",
                "/no_such_dir_xyz/out.bin"
            ])
            .is_err()
        );
        assert_eq!(buf.len(), 8);

        // dump_regions échoue : --out pointe sur un FICHIER (create_dir_all impossible).
        let as_file = tmp_path("not_a_dir");
        std::fs::write(&as_file, b"x").unwrap();
        let afs = as_file.to_string_lossy().into_owned();
        assert!(r(&["dump", "--pid", &pid, "--module", &comm, "--out", &afs]).is_err());
        let _ = std::fs::remove_file(&as_file);

        // dump sans --out : couvre la fermeture par défaut "./memdump" (plages vides).
        r(&["dump", "--pid", &pid, "--module", "zzz-no-such-module"]).unwrap();
        let _ = std::fs::remove_dir_all("./memdump");
    }
}
