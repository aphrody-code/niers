//! `nie-edit` — éditeur mémoire **live** d'`nie.exe`, piloté par le catalogue dérivé du dump.
//!
//! Instrument de RE : lit **et écrit** la mémoire du vrai jeu Windows natif (via
//! `ReadProcessMemory`/`WriteProcessMemory`, backend [`nie_trace::win_memory`]) pour *valider au réel*
//! les structures reversées — change une valeur, observe l'effet, confirme l'offset. Cross-compilé
//! depuis WSL (`cargo build -p nie-trace --bin nie-edit --target x86_64-pc-windows-gnu`) et lancé via
//! l'interop. Compile aussi sous Linux (backend Wine `process_vm_writev`).
//!
//! Les écritures sur un process **actif** peuvent le déstabiliser : elles exigent `--force`
//! (sinon « dry-run » qui n'imprime que ce qui serait écrit). Jeu **possédé**, en local, hors-ligne.

use std::collections::HashMap;
use std::process::ExitCode;

use nie_trace::aob::Pattern;
use nie_trace::catalog::{self, Category, Entry, Kind, Ty};
use nie_trace::{
    find_module_base, find_pid_by_name, module_regions, read_exact, resolve_chain,
    scan_regions_masked, write_exact,
};

/// Octets exacts de l'AOB `max-abilities` (`44 8B 6F 10 8B 47 04`), implantés **deux fois** dans
/// l'image du binaire pour que les tests d'auto-résolution (`resolve`/`scan`) trouvent des hits
/// déterministes (≥2 → branche multi-hits) sans `nie.exe`.
#[cfg(test)]
#[used]
static MARKER: [u8; 7] = [0x44, 0x8B, 0x6F, 0x10, 0x8B, 0x47, 0x04];
#[cfg(test)]
#[used]
static MARKER_DUP: [u8; 8] = [0x44, 0x8B, 0x6F, 0x10, 0x8B, 0x47, 0x04, 0xC3];
/// Octets de l'AOB `unlimited-spirits` (`75 03 8B 58 10 49`), entrée **sans RVA au dump** → verdict
/// `New` lors de la résolution live.
#[cfg(test)]
#[used]
static MARKER_NEW: [u8; 6] = [0x75, 0x03, 0x8B, 0x58, 0x10, 0x49];

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
nie-edit — éditeur mémoire live d'nie.exe (catalogue dérivé du dump)

INSPECTION
  nie-edit list [--category player|match|shop|spirit|passive]
  nie-edit info  <name>
  nie-edit slide [--pid N]                  base de chargement + slide ASLR
  nie-edit resolve <name | --all> [--pid N] scanne l'AOB live, vérifie vs la RVA du dump

VALEURS (lecture/écriture typées)
  nie-edit get  <name> [--base 0xADDR]      StructField : --base requis (objet)
  nie-edit set  <name> <val> [--base 0xADDR] --force
  nie-edit get-va <0xVA | nie.exe+0xRVA> :TYPE
  nie-edit set-va <0xVA | nie.exe+0xRVA> :TYPE <val> --force
  nie-edit ptr  <0xBASE | nie.exe+0xRVA> +off1 +off2 ... [:TYPE]
  nie-edit watch <name --base 0xADDR | 0xVA :TYPE> [--interval 500] [--count 0]

BAS NIVEAU (RE)
  nie-edit scan  <'48 8B ?? 10'> [--limit 20] [--pid N]
  nie-edit patch <0xVA> <'90 90'> [--save FICHIER] --force
  nie-edit nop   <0xVA> <len> [--save FICHIER] --force

TYPE ∈ u8 u16 u32 i32 u64 f32. --pid 0 (défaut) auto-détecte nie.exe.
Les écritures exigent --force (sinon dry-run).";

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let (pos, flags) = parse(&args[1..]);
    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        "list" => cmd_list(&flags),
        "info" => cmd_info(&pos),
        "slide" => cmd_slide(&flags),
        "resolve" => cmd_resolve(&pos, &flags),
        "get" => cmd_get(&pos, &flags),
        "set" => cmd_set(&pos, &flags),
        "get-va" => cmd_get_va(&pos, &flags),
        "set-va" => cmd_set_va(&pos, &flags),
        "ptr" => cmd_ptr(&pos, &flags),
        "watch" => cmd_watch(&pos, &flags),
        "scan" => cmd_scan(&pos, &flags),
        "patch" => cmd_patch(&pos, &flags),
        "nop" => cmd_nop(&pos, &flags),
        other => Err(format!("commande inconnue « {other} »\n\n{USAGE}")),
    }
}

// ─── Inspection ─────────────────────────────────────────────────────────────────────

fn cmd_list(flags: &Flags) -> Result<(), String> {
    let filter = flags.get("category").and_then(Clone::clone);
    let want = filter.as_deref().map(parse_category).transpose()?;
    println!(
        "  {:<26} {:<8} {:<11} {:<4} rva",
        "id", "cat", "kind", "type"
    );
    for e in catalog::CATALOG {
        if let Some(w) = want
            && e.category != w
        {
            continue;
        }
        let rva = e.rva.map_or_else(|| "—".to_owned(), |r| format!("0x{r:X}"));
        println!(
            "  {:<26} {:<8} {:<11} {:<4} {}",
            e.id,
            e.category.label(),
            kind_label(e.kind),
            e.ty.name(),
            rva
        );
    }
    Ok(())
}

fn cmd_info(pos: &[String]) -> Result<(), String> {
    let e = entry(pos.first())?;
    println!("  id        {}", e.id);
    println!("  feature   {}", e.feature);
    println!(
        "  catégorie {}  ({})",
        e.category.label(),
        kind_label(e.kind)
    );
    println!("  type      {}", e.ty.name());
    if let Some(p) = e.aob {
        println!("  aob       {p}");
    }
    match (e.rva, e.static_addr()) {
        (Some(r), Some(s)) => println!("  rva       0x{r:X}   (statique 0x{s:X})"),
        _ => println!("  rva       — (AOB non retrouvé au dump)"),
    }
    if let Some(f) = e.field {
        println!("  field     +0x{f:X}");
    }
    if let Some(c) = e.chain {
        let parts: Vec<String> = c.iter().map(|o| format!("+0x{o:X}")).collect();
        println!("  chain     {}", parts.join(" "));
    }
    println!("  doc       {}", e.doc);
    Ok(())
}

fn cmd_slide(flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let module = module_of(flags);
    let base = find_module_base(pid, &module)
        .ok_or_else(|| format!("« {module} » introuvable dans le process"))?;
    let slide = base.wrapping_sub(catalog::NIE_IMAGE_BASE);
    println!("  {module} @ 0x{base:X}  (pid {pid})");
    println!(
        "  base statique 0x{:X}  →  slide ASLR 0x{slide:X}",
        catalog::NIE_IMAGE_BASE
    );
    println!("  live = statique + slide  |  statique = live − slide");
    Ok(())
}

/// Verdict de résolution d'un site : la RVA live colle-t-elle à celle du dump ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RvaVerdict {
    /// RVA live == RVA du dump.
    Match,
    /// RVA live ≠ RVA du dump (dérive de build) — porte la RVA attendue.
    Drift(u64),
    /// Le dump n'avait pas de RVA pour ce site (AOB non retrouvé au dump).
    New,
}

/// Classe une RVA live contre la RVA attendue du dump (pur, testable).
fn classify_rva(expected: Option<u64>, got: u64) -> RvaVerdict {
    match expected {
        Some(e) if e == got => RvaVerdict::Match,
        Some(e) => RvaVerdict::Drift(e),
        None => RvaVerdict::New,
    }
}

/// Noyau d'une signature AOB : son plus long préfixe d'octets **concrets**, borné à 8.
///
/// Nos signatures mêlent deux choses de nature différente : l'instruction qui porte
/// l'information (un offset de champ, une chaîne de pointeurs) et le code qui l'entoure,
/// retenu pour lever l'ambiguïté. Le second ne survit pas à une recompilation, le premier si.
/// Isoler le préfixe concret rend donc au scan ce qui a du sens quand la signature entière
/// échoue.
///
/// Le seuil de six octets n'est pas cosmétique : en deçà, le motif attrape n'importe quelle
/// instruction courante et la liste de candidats ne vaut rien. Les signatures faites presque
/// entièrement de jokers (les cooldowns, `F3 0F 5C ?? …`) rendent donc `None` — dire qu'il n'y
/// a rien à rattraper est plus honnête que produire trois cents adresses au hasard.
fn noyau_aob(aob: &str) -> Option<String> {
    let mut octets = Vec::new();
    for jeton in aob.split_whitespace() {
        if jeton.contains('?') || octets.len() == 8 {
            break;
        }
        octets.push(jeton);
    }
    (octets.len() >= 6).then(|| octets.join(" "))
}

fn cmd_resolve(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let module = module_of(flags);
    let base = find_module_base(pid, &module).ok_or_else(|| format!("« {module} » introuvable"))?;
    let regions = module_regions(pid, &module, false);
    let entries: Vec<&Entry> = if flags.contains_key("all") {
        catalog::CATALOG.iter().collect()
    } else {
        vec![entry(pos.first())?]
    };

    let (mut ok, mut drift, mut miss) = (0u32, 0u32, 0u32);
    for e in entries {
        let Some(aob) = e.aob else {
            println!("  {:<26} (pas d'AOB)", e.id);
            continue;
        };
        let pat = Pattern::parse(aob).map_err(|err| format!("{}: {err}", e.id))?;
        let hits = scan_regions_masked(pid, &regions, Some(base), &pat, 4);
        if hits.is_empty() {
            // Une signature absente ne veut pas dire que le site a disparu : nos AOB portent du
            // contexte de code autour de l'instruction utile, et ce contexte change à chaque
            // recompilation. Constaté sur le build Steam d'août 2026 — `tension` était donné
            // « introuvable » alors que son instruction porteuse, `mov eax,[rax+0x1058]`, était
            // présente six fois. Rapporter les candidats du noyau vaut mieux qu'un verdict faux.
            match noyau_aob(aob).and_then(|n| Pattern::parse(&n).ok().map(|p| (n, p))) {
                Some((noyau, pat_noyau)) => {
                    let cands = scan_regions_masked(pid, &regions, Some(base), &pat_noyau, 32);
                    if cands.is_empty() {
                        println!(
                            "  {:<26} ✗ aucun hit (noyau « {noyau} » absent aussi)",
                            e.id
                        );
                    } else {
                        println!(
                            "  {:<26} ✗ signature complète absente — noyau « {noyau} » : {} candidat(s)",
                            e.id,
                            cands.len()
                        );
                        for h in cands.iter().take(8) {
                            println!(
                                "      candidat  0x{:X}  rva 0x{:X}",
                                h.addr,
                                h.rva.unwrap_or(0)
                            );
                        }
                    }
                }
                // Pas de noyau exploitable : la signature est presque entièrement du contexte
                // (cas des cooldowns), il n'y a rien de sémantique à rattraper.
                None => println!("  {:<26} ✗ aucun hit", e.id),
            }
            miss += 1;
            continue;
        }
        for h in &hits {
            let rva = h.rva.unwrap_or(0);
            let tag = match classify_rva(e.rva, rva) {
                RvaVerdict::Match => {
                    ok += 1;
                    "✓ = dump".to_owned()
                }
                RvaVerdict::Drift(expected) => {
                    drift += 1;
                    format!("≠ dump 0x{expected:X}")
                }
                RvaVerdict::New => "• nouveau (dump: —)".to_owned(),
            };
            let extra = if hits.len() > 1 {
                format!("  [{} hits]", hits.len())
            } else {
                String::new()
            };
            println!("  {:<26} 0x{:X}  rva 0x{rva:X}  {tag}{extra}", e.id, h.addr);
        }
    }
    println!("\n  {ok} ✓   {drift} drift   {miss} introuvable");
    Ok(())
}

// ─── Valeurs ────────────────────────────────────────────────────────────────────────

/// Résout l'adresse d'un `StructField` à partir d'une base d'objet (`--base`).
fn struct_field_addr(pid: i32, e: &Entry, flags: &Flags) -> Result<u64, String> {
    let base = flags.get("base").and_then(Clone::clone).ok_or_else(|| {
        format!(
            "« {} » est un champ d'objet : précise --base 0xADDR (base de l'objet)",
            e.id
        )
    })?;
    let base = parse_addr(&base, pid)?;
    if let Some(chain) = e.chain {
        return resolve_chain(pid, base, chain).map_err(|err| err.to_string());
    }
    if let Some(off) = e.field {
        return Ok(add_off(base, off));
    }
    Err(format!("« {} » n'a ni field ni chain", e.id))
}

fn cmd_get(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let e = entry(pos.first())?;
    if e.kind != Kind::StructField {
        return Err(format!(
            "« {}» est un site de code ({}) : utilise `resolve` pour le localiser, `get-va` pour lire une \
             adresse, ou `patch`/`nop` pour le modifier — ce n'est pas un scalaire à une adresse fixe",
            e.id,
            kind_label(e.kind)
        ));
    }
    let addr = struct_field_addr(pid, e, flags)?;
    let v = read_typed(pid, addr, e.ty)?;
    println!("  {} @ 0x{addr:X} = {v}  ({})", e.id, e.ty.name());
    Ok(())
}

fn cmd_set(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let e = entry(pos.first())?;
    let val = pos.get(1).ok_or("valeur manquante")?;
    if e.kind != Kind::StructField {
        return Err(format!(
            "« {} » est un site de code ({}) : `set` n'écrit que des champs d'objet (StructField). Pour \
             modifier le code, vois `patch`/`nop` (RE)",
            e.id,
            kind_label(e.kind)
        ));
    }
    let addr = struct_field_addr(pid, e, flags)?;
    let bytes = e.ty.encode(val)?;
    write_guarded(
        pid,
        addr,
        &bytes,
        flags,
        &format!("{} ({})", e.id, e.ty.name()),
    )
}

fn cmd_get_va(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let addr_s = pos.first().ok_or("adresse manquante")?;
    let ty = type_arg(pos, flags)?;
    let addr = parse_addr(addr_s, pid)?;
    let v = read_typed(pid, addr, ty)?;
    println!("  0x{addr:X} = {v}  ({})", ty.name());
    Ok(())
}

fn cmd_set_va(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let addr_s = pos.first().ok_or("adresse manquante")?;
    let ty = type_arg(pos, flags)?;
    let val = pos
        .iter()
        .skip(1)
        .find(|p| !p.starts_with(':'))
        .ok_or("valeur manquante")?;
    let addr = parse_addr(addr_s, pid)?;
    let bytes = ty.encode(val)?;
    write_guarded(
        pid,
        addr,
        &bytes,
        flags,
        &format!("0x{addr:X} ({})", ty.name()),
    )
}

fn cmd_ptr(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let base_s = pos.first().ok_or("base manquante")?;
    let base = parse_addr(base_s, pid)?;
    let mut offsets = Vec::new();
    let mut ty = None;
    for p in &pos[1..] {
        if let Some(t) = p.strip_prefix(':') {
            ty = Some(Ty::from_tag(t).ok_or_else(|| format!("type inconnu: {p}"))?);
        } else {
            offsets.push(parse_signed(p.trim_start_matches('+'))?);
        }
    }
    let addr = resolve_chain(pid, base, &offsets).map_err(|e| e.to_string())?;
    match ty {
        Some(t) => {
            let v = read_typed(pid, addr, t)?;
            println!("  chaîne → 0x{addr:X} = {v}  ({})", t.name());
        }
        None => println!("  chaîne → 0x{addr:X}"),
    }
    Ok(())
}

fn cmd_watch(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let first = pos.first().ok_or("cible manquante (name ou 0xVA)")?;
    let (addr, ty) = if let Some(e) = catalog::find(first) {
        if e.kind != Kind::StructField {
            return Err(format!(
                "« {} » n'est pas un champ adressable ; vois `watch 0xVA :TYPE`",
                e.id
            ));
        }
        (struct_field_addr(pid, e, flags)?, e.ty)
    } else {
        (parse_addr(first, pid)?, type_arg(pos, flags)?)
    };
    let interval = flag_num(flags, "interval").unwrap_or(500u64).max(10);
    let count: u64 = flag_num(flags, "count").unwrap_or(0); // 0 = infini
    println!(
        "  watch 0x{addr:X} ({}) toutes les {interval}ms — Ctrl-C pour stopper",
        ty.name()
    );
    let mut last: Option<String> = None;
    let mut i = 0u64;
    loop {
        match read_typed(pid, addr, ty) {
            Ok(v) => {
                if last.as_deref() != Some(v.as_str()) {
                    println!("  [{i:>5}] {v}");
                    last = Some(v);
                }
            }
            Err(e) => {
                println!("  [{i:>5}] <lecture impossible: {e}>");
            }
        }
        i += 1;
        if count != 0 && i >= count {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(interval));
    }
    Ok(())
}

// ─── Bas niveau (RE) ────────────────────────────────────────────────────────────────

fn cmd_scan(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let raw = pos.first().ok_or("motif manquant")?;
    let pat = Pattern::parse(raw).map_err(|e| e.to_string())?;
    let module = module_of(flags);
    let base = find_module_base(pid, &module);
    let regions = module_regions(pid, &module, flags.contains_key("all"));
    let limit: usize = flag_num(flags, "limit").unwrap_or(20);
    let hits = scan_regions_masked(pid, &regions, base, &pat, limit);
    for h in &hits {
        let rva = h
            .rva
            .map(|r| {
                format!(
                    "  nie.exe+0x{r:X}  (statique 0x{:X})",
                    catalog::NIE_IMAGE_BASE + r
                )
            })
            .unwrap_or_default();
        println!("  0x{:X}  [{}]{rva}", h.addr, h.perms);
    }
    let capped = if hits.len() >= limit {
        format!(" (limité à {limit})")
    } else {
        String::new()
    };
    println!("\n  {} hit(s){capped}", hits.len());
    Ok(())
}

fn cmd_patch(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let addr = parse_addr(pos.first().ok_or("adresse manquante")?, pid)?;
    let bytes = parse_hex_bytes(pos.get(1).ok_or("octets manquants")?)?;
    save_original(pid, addr, bytes.len(), flags)?;
    write_guarded(
        pid,
        addr,
        &bytes,
        flags,
        &format!("0x{addr:X} ({} octets)", bytes.len()),
    )
}

/// Garde-fou : un patch de code ne dépasse jamais quelques Ko ; borne la longueur saisie pour éviter
/// qu'un doigt qui fourche (`nop 0xVA 999999999999`) ne demande une allocation géante (abort).
const MAX_PATCH_LEN: usize = 0x1_0000;

fn cmd_nop(pos: &[String], flags: &Flags) -> Result<(), String> {
    let pid = resolve_pid(flags)?;
    let addr = parse_addr(pos.first().ok_or("adresse manquante")?, pid)?;
    let len: usize = pos
        .get(1)
        .ok_or("longueur manquante")?
        .parse()
        .map_err(|_| "longueur invalide")?;
    if len > MAX_PATCH_LEN {
        return Err(format!(
            "longueur {len} déraisonnable (max {MAX_PATCH_LEN})"
        ));
    }
    save_original(pid, addr, len, flags)?;
    let bytes = vec![0x90u8; len];
    write_guarded(
        pid,
        addr,
        &bytes,
        flags,
        &format!("0x{addr:X} ({len}× nop)"),
    )
}

/// Sauvegarde les octets d'origine vers `--save FICHIER` avant un patch (restauration manuelle).
fn save_original(pid: i32, addr: u64, len: usize, flags: &Flags) -> Result<(), String> {
    let Some(path) = flags.get("save").and_then(Clone::clone) else {
        return Ok(());
    };
    let orig = read_exact(pid, addr, len).map_err(|e| e.to_string())?;
    std::fs::write(&path, &orig).map_err(|e| e.to_string())?;
    println!(
        "  sauvegarde {} octets @ 0x{addr:X} → {path}  ({})",
        len,
        hex_join(&orig)
    );
    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────────────────────

type Flags = HashMap<String, Option<String>>;

fn read_typed(pid: i32, addr: u64, ty: Ty) -> Result<String, String> {
    let b = read_exact(pid, addr, ty.size()).map_err(|e| e.to_string())?;
    Ok(ty.decode(&b))
}

/// Écrit `bytes` si `--force`, sinon imprime le dry-run.
fn write_guarded(
    pid: i32,
    addr: u64,
    bytes: &[u8],
    flags: &Flags,
    what: &str,
) -> Result<(), String> {
    if !flags.contains_key("force") {
        println!(
            "  DRY-RUN  écrirait {what} @ 0x{addr:X} ← {}  (ajoute --force pour appliquer)",
            hex_join(bytes)
        );
        return Ok(());
    }
    write_exact(pid, addr, bytes).map_err(|e| e.to_string())?;
    println!("  écrit {what} @ 0x{addr:X} ← {}", hex_join(bytes));
    Ok(())
}

fn entry(name: Option<&String>) -> Result<&'static Entry, String> {
    let n = name.ok_or("nom d'entrée manquant")?;
    catalog::find(n).ok_or_else(|| format!("« {n} » absent du catalogue (voir `list`)"))
}

fn type_arg(pos: &[String], flags: &Flags) -> Result<Ty, String> {
    if let Some(t) = flags.get("type").and_then(Clone::clone) {
        return Ty::from_tag(&t).ok_or_else(|| format!("type inconnu: {t}"));
    }
    pos.iter()
        .find_map(|p| p.strip_prefix(':').and_then(Ty::from_tag))
        .ok_or_else(|| "type manquant (suffixe :u32 / :f32 …)".to_owned())
}

fn kind_label(k: Kind) -> &'static str {
    match k {
        Kind::Toggle => "toggle",
        Kind::Value => "value",
        Kind::StructField => "structfield",
    }
}

fn parse_category(s: &str) -> Result<Category, String> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "player" => Category::Player,
        "match" => Category::Match,
        "shop" => Category::Shop,
        "spirit" => Category::Spirit,
        "passive" => Category::Passive,
        other => return Err(format!("catégorie inconnue: {other}")),
    })
}

fn add_off(addr: u64, off: i64) -> u64 {
    if off >= 0 {
        addr.wrapping_add(off as u64)
    } else {
        addr.wrapping_sub(off.unsigned_abs())
    }
}

/// `0xADDR` (absolu), `nie.exe+0xRVA`, ou décimal.
fn parse_addr(s: &str, pid: i32) -> Result<u64, String> {
    let s = s.trim();
    if let Some(plus) = s.find('+') {
        let module = &s[..plus];
        let rva = parse_u64(&s[plus + 1..])?;
        let base = find_module_base(pid, module)
            .ok_or_else(|| format!("module « {module} » introuvable"))?;
        return Ok(base.wrapping_add(rva));
    }
    parse_u64(s)
}

fn parse_u64(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|_| format!("hex invalide: {s}"))
    } else {
        s.parse::<u64>()
            .map_err(|_| format!("nombre invalide: {s}"))
    }
}

fn parse_signed(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let (neg, body) = s.strip_prefix('-').map_or((false, s), |b| (true, b));
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).map_err(|_| format!("hex invalide: {s}"))?
    } else {
        body.parse::<i64>()
            .map_err(|_| format!("offset invalide: {s}"))?
    };
    Ok(if neg { -v } else { v })
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let hex: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if !hex.is_ascii() {
        return Err("octets hex non-ASCII".to_owned());
    }
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return Err("octets hex de longueur impaire/vide".to_owned());
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        out.push(
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("octet hex invalide à {i}"))?,
        );
    }
    Ok(out)
}

fn hex_join(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_pid(flags: &Flags) -> Result<i32, String> {
    let pid = flag_num::<i32>(flags, "pid").unwrap_or(0);
    if pid > 0 {
        return Ok(pid);
    }
    find_pid_by_name("nie.exe")
        .ok_or_else(|| "nie.exe introuvable — lance le jeu, ou précise --pid".to_owned())
}

/// Module ciblé (`--module`, défaut `nie.exe`) — pour `slide`/`resolve`/`scan`.
fn module_of(flags: &Flags) -> String {
    flags
        .get("module")
        .and_then(Clone::clone)
        .unwrap_or_else(|| "nie.exe".to_owned())
}

fn flag_num<T: std::str::FromStr>(flags: &Flags, key: &str) -> Option<T> {
    flags
        .get(key)
        .and_then(Clone::clone)
        .and_then(|v| v.parse::<T>().ok())
}

/// Sépare positionnels et flags. `--k v` (valeur) ou `--k` (booléen → None). Les booléens connus
/// (`--all`, `--force`) ne consomment jamais l'argument suivant.
fn parse(args: &[String]) -> (Vec<String>, Flags) {
    const BOOLS: &[&str] = &["all", "force"];
    let mut pos = Vec::new();
    let mut flags: Flags = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        // Uniquement les flags `--xxx` : un argument `-5` reste une **valeur** (ex. `set rank -5`).
        if let Some(key) = a.strip_prefix("--") {
            let key = key.to_owned();
            if BOOLS.contains(&key.as_str()) {
                flags.insert(key, None);
            } else if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                flags.insert(key, Some(args[i + 1].clone()));
                i += 1;
            } else {
                flags.insert(key, None);
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    (pos, flags)
}

// Les tests s'auto-ciblent : ils lisent la mémoire du process de test et le nomment via
// `/proc/self/comm`. C'est du Linux pur — sous Windows ils échouaient tous les cinq sur
// « chemin d'accès introuvable » avant même de tester quoi que ce soit. Le code testé
// (parseurs d'adresses, dispatch get/set/scan) reste compilé partout ; seul le harnais,
// qui suppose /proc, est gaté.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn me() -> String {
        std::process::id().to_string()
    }
    fn comm() -> String {
        std::fs::read_to_string("/proc/self/comm")
            .unwrap()
            .trim()
            .to_owned()
    }
    fn run_ok(args: &[&str]) {
        let v: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        run(&v).unwrap_or_else(|e| panic!("attendu Ok pour {args:?}, eu Err: {e}"));
    }
    fn run_err(args: &[&str]) {
        let v: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        assert!(run(&v).is_err(), "attendu Err pour {args:?}");
    }
    fn hexaddr(a: u64) -> String {
        format!("0x{a:x}")
    }

    // ── helpers purs ────────────────────────────────────────────────────────────────

    #[test]
    fn pure_number_parsers() {
        assert_eq!(parse_u64("0x10").unwrap(), 16);
        assert_eq!(parse_u64("16").unwrap(), 16);
        assert!(parse_u64("zz").is_err());
        assert_eq!(parse_signed("5").unwrap(), 5);
        assert_eq!(parse_signed("-5").unwrap(), -5);
        assert_eq!(parse_signed("0x10").unwrap(), 16);
        assert_eq!(parse_signed("-0x10").unwrap(), -16);
        assert!(parse_signed("zz").is_err());
        assert!(parse_signed("-zz").is_err());
        assert_eq!(add_off(0x1000, 0x58), 0x1058);
        assert_eq!(add_off(0x1000, -8), 0x0FF8);
    }

    #[test]
    fn pure_addr_parser() {
        assert_eq!(parse_addr("0x20", 0).unwrap(), 0x20);
        // branche module+rva : résout la base du binaire de test puis +0.
        let base = find_module_base(std::process::id() as i32, &comm()).unwrap();
        assert_eq!(
            parse_addr(&format!("{}+0x0", comm()), std::process::id() as i32).unwrap(),
            base
        );
        assert!(parse_addr("zzz-absent+0x0", std::process::id() as i32).is_err());
        assert!(parse_addr("nothex", 0).is_err());
    }

    #[test]
    fn pure_hex_bytes() {
        assert_eq!(parse_hex_bytes("90 90").unwrap(), vec![0x90, 0x90]);
        assert_eq!(parse_hex_bytes("44-8B").unwrap(), vec![0x44, 0x8B]);
        assert!(parse_hex_bytes("909").is_err()); // longueur impaire
        assert!(parse_hex_bytes("").is_err()); // vide
        assert!(parse_hex_bytes("ZZ").is_err()); // octet invalide
        assert!(parse_hex_bytes("€€").is_err()); // non-ASCII (pas de panic)
        assert_eq!(hex_join(&[0x90, 0xAB]), "90 AB");
    }

    #[test]
    fn pure_category_kind_type() {
        for (s, ok) in [
            ("player", true),
            ("match", true),
            ("shop", true),
            ("spirit", true),
            ("passive", true),
            ("bogus", false),
        ] {
            assert_eq!(parse_category(s).is_ok(), ok);
        }
        assert_eq!(kind_label(Kind::Toggle), "toggle");
        assert_eq!(kind_label(Kind::Value), "value");
        assert_eq!(kind_label(Kind::StructField), "structfield");
        // type_arg : positionnel :type, flag --type, et absent.
        let mut f = Flags::new();
        assert!(type_arg(&[":u32".to_owned()], &f).is_ok());
        f.insert("type".to_owned(), Some("f32".to_owned()));
        assert_eq!(type_arg(&[], &f).unwrap(), Ty::F32);
        f.insert("type".to_owned(), Some("bad".to_owned()));
        assert!(type_arg(&[], &f).is_err());
        assert!(type_arg(&[], &Flags::new()).is_err());
    }

    #[test]
    fn pure_arg_splitter() {
        // --all/--force sont booléens et ne consomment pas l'argument suivant.
        let (pos, flags) = parse(&[
            "--all".into(),
            "--pid".into(),
            "5".into(),
            "p".into(),
            "--force".into(),
        ]);
        assert_eq!(pos, vec!["p"]);
        assert!(flags.contains_key("all") && flags.contains_key("force"));
        assert_eq!(flags.get("pid"), Some(&Some("5".to_owned())));
        // un flag valeur dont le suivant est --xxx reste sans valeur.
        let (_, f2) = parse(&["--base".into(), "--force".into()]);
        assert_eq!(f2.get("base"), Some(&None));
        assert!(f2.contains_key("force"));
        // un argument `-5` (mono-tiret) reste un positionnel (valeur signée, pas un flag).
        let (pos2, f3) = parse(&["set".into(), "rank".into(), "-5".into(), "--force".into()]);
        assert_eq!(pos2, vec!["set", "rank", "-5"]);
        assert!(f3.contains_key("force"));
    }

    #[test]
    fn classify_rva_three_verdicts() {
        assert_eq!(classify_rva(Some(0x10), 0x10), RvaVerdict::Match);
        assert_eq!(classify_rva(Some(0x10), 0x20), RvaVerdict::Drift(0x10));
        assert_eq!(classify_rva(None, 0x99), RvaVerdict::New);
    }

    // ── inspection (pas de process requis) ──────────────────────────────────────────

    #[test]
    fn run_inspection() {
        run_ok(&["help"]);
        run_ok(&["-h"]);
        run_ok(&[]); // sans argument → usage
        run_ok(&["list"]);
        run_ok(&["list", "--category", "match"]);
        run_err(&["list", "--category", "bogus"]);
        run_ok(&["info", "tension"]); // field + (pas de chain)
        run_ok(&["info", "rank"]); // chain
        run_ok(&["info", "max-abilities"]); // field, rva connu
        run_err(&["info", "bogus"]);
        run_err(&["info"]); // nom manquant
        run_err(&["commande-inconnue"]);
    }

    #[test]
    fn run_slide_self_and_missing() {
        run_ok(&["slide", "--module", &comm(), "--pid", &me()]); // succès (binaire de test)
        run_err(&["slide", "--pid", &me()]); // nie.exe introuvable
    }

    // ── I/O contre le propre process ────────────────────────────────────────────────

    #[test]
    fn run_get_set_va_self() {
        let buf = [0u8; 64];
        let a = hexaddr(buf.as_ptr() as u64);
        run_ok(&["get-va", &a, ":u32", "--pid", &me()]);
        run_ok(&["get-va", &a, "--type", "u32", "--pid", &me()]); // type via flag
        run_err(&["get-va", &a, "--pid", &me()]); // type manquant
        run_ok(&["set-va", &a, ":u32", "1234", "--pid", &me(), "--force"]); // écriture
        assert_eq!(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]), 1234);
        run_ok(&["set-va", &a, ":u32", "7", "--pid", &me()]); // dry-run (sans --force)
        run_err(&["set-va", &a, ":u32", "--pid", &me()]); // valeur manquante
        // lecture d'une adresse non mappée → erreur.
        run_err(&["get-va", "0x1", ":u32", "--pid", &me()]);
        // adresse module+rva.
        run_ok(&["get-va", &format!("{}+0x0", comm()), ":u8", "--pid", &me()]);
    }

    #[test]
    fn run_ptr_self() {
        let mut buf = [0u8; 16];
        let base = buf.as_ptr() as u64;
        let val_addr = base + 8;
        buf[0..8].copy_from_slice(&val_addr.to_le_bytes());
        buf[8..12].copy_from_slice(&4242u32.to_le_bytes());
        run_ok(&["ptr", &hexaddr(base), "+0", "+0", ":u32", "--pid", &me()]); // chaîne + lecture typée
        run_ok(&["ptr", &hexaddr(base), "+0", "--pid", &me()]); // sans type → adresse seule
        run_err(&["ptr", "--pid", &me()]); // base manquante
        run_err(&["ptr", &hexaddr(base), "+zz", "--pid", &me()]); // offset invalide
        run_err(&["ptr", &hexaddr(base), ":bad", "--pid", &me()]); // type inconnu
    }

    #[test]
    fn run_get_set_structfield_self() {
        // spirit-id : StructField, field +0x18 — tient dans un petit tampon.
        let buf = [0u8; 64];
        let a = hexaddr(buf.as_ptr() as u64);
        run_ok(&["get", "spirit-id", "--base", &a, "--pid", &me()]);
        run_ok(&[
            "set",
            "spirit-id",
            "9",
            "--base",
            &a,
            "--pid",
            &me(),
            "--force",
        ]);
        run_ok(&["set", "spirit-id", "9", "--base", &a, "--pid", &me()]); // dry-run
        run_err(&["get", "tension", "--pid", &me()]); // --base manquant
        run_err(&["get", "max-abilities", "--base", &a, "--pid", &me()]); // toggle, pas un champ
        run_err(&["set", "max-abilities", "1", "--base", &a, "--pid", &me()]); // toggle
        run_err(&["set", "spirit-id", "--base", &a, "--pid", &me()]); // valeur manquante
        run_err(&["get", "bogus", "--pid", &me()]); // entrée inconnue
    }

    #[test]
    fn run_get_chain_structfield_self() {
        // rank : chaîne [+0x69A0, +0x5C] — tampon assez grand, pointeur implanté à +0x69A0.
        let mut buf = vec![0u8; 0x6A00];
        let base = buf.as_ptr() as u64;
        buf[0x69A0..0x69A8].copy_from_slice(&base.to_le_bytes()); // *(base+0x69A0) = base ; +0x5C dans le tampon
        run_ok(&["get", "rank", "--base", &hexaddr(base), "--pid", &me()]);
    }

    #[test]
    fn run_watch_self() {
        let buf = [0u8; 8];
        let a = hexaddr(buf.as_ptr() as u64);
        run_ok(&[
            "watch",
            &a,
            ":u32",
            "--count",
            "2",
            "--interval",
            "10",
            "--pid",
            &me(),
        ]);
        run_ok(&["watch", "0x1", ":u32", "--count", "1", "--pid", &me()]); // lecture impossible
        run_ok(&[
            "watch",
            "tension",
            "--base",
            &a,
            "--count",
            "1",
            "--pid",
            &me(),
        ]); // nom StructField
        run_err(&["watch", "max-abilities", "--base", &a, "--pid", &me()]); // toggle
        run_err(&["watch", "--pid", &me()]); // cible manquante
    }

    #[test]
    fn run_scan_self_finds_marker() {
        let _ = std::hint::black_box(&MARKER); // garantit la rétention du marqueur
        run_ok(&[
            "scan",
            "44 8B 6F 10 8B 47 04",
            "--pid",
            &me(),
            "--module",
            &comm(),
        ]);
        run_ok(&[
            "scan",
            "44 8B ?? 10",
            "--limit",
            "3",
            "--pid",
            &me(),
            "--module",
            &comm(),
        ]);
        run_err(&["scan", "ZZ", "--pid", &me()]); // motif invalide
        run_err(&["scan", "--pid", &me()]); // motif manquant
    }

    #[test]
    fn run_resolve_self() {
        let _ = std::hint::black_box((&MARKER, &MARKER_DUP, &MARKER_NEW));
        // max-abilities : MARKER(+DUP) trouvés → hits, RVA ≠ dump → drift (+ branche multi-hits).
        run_ok(&[
            "resolve",
            "max-abilities",
            "--module",
            &comm(),
            "--pid",
            &me(),
        ]);
        // unlimited-spirits : entrée sans RVA au dump → verdict « nouveau » (New).
        run_ok(&[
            "resolve",
            "unlimited-spirits",
            "--module",
            &comm(),
            "--pid",
            &me(),
        ]);
        run_ok(&["resolve", "--all", "--module", &comm(), "--pid", &me()]); // miss + drift + new + résumé
        run_err(&["resolve", "tension", "--pid", &me()]); // nie.exe introuvable
        run_err(&["resolve", "--module", &comm(), "--pid", &me()]); // nom manquant
    }

    #[test]
    fn struct_field_addr_branches() {
        let me_pid = std::process::id() as i32;
        let mut flags = Flags::new();
        // --base manquant → erreur.
        let field_entry = catalog::find("spirit-id").unwrap();
        assert!(struct_field_addr(me_pid, field_entry, &flags).is_err());
        // base présente → branche field (offset ajouté).
        flags.insert("base".to_owned(), Some("0x1000".to_owned()));
        assert_eq!(
            struct_field_addr(me_pid, field_entry, &flags).unwrap(),
            0x1000 + 0x18
        );
        // base invalide → erreur parse_addr.
        flags.insert("base".to_owned(), Some("pas-hex".to_owned()));
        assert!(struct_field_addr(me_pid, field_entry, &flags).is_err());
        // StructField sans field ni chain (synthétique) → erreur « ni field ni chain ».
        flags.insert("base".to_owned(), Some("0x1000".to_owned()));
        let bogus = Entry {
            id: "b",
            feature: "b",
            category: Category::Match,
            kind: Kind::StructField,
            ty: Ty::U32,
            aob: None,
            rva: None,
            field: None,
            chain: None,
            doc: "",
        };
        assert!(struct_field_addr(me_pid, &bogus, &flags).is_err());
    }

    #[test]
    fn resolve_pid_defaults_to_nie_exe() {
        // sans --pid, resolve_pid cherche « nie.exe » (absent ici) → la commande échoue.
        run_err(&["get-va", "0x1000", ":u32"]);
    }

    #[test]
    fn run_patch_and_nop_self() {
        let buf = [0xFFu8; 8];
        let a = hexaddr(buf.as_ptr() as u64);
        let save = std::env::temp_dir().join(format!("nie-edit-orig-{}.bin", std::process::id()));
        let save_s = save.to_string_lossy().into_owned();
        run_ok(&[
            "patch",
            &a,
            "90 90",
            "--pid",
            &me(),
            "--force",
            "--save",
            &save_s,
        ]); // patch + sauvegarde
        assert_eq!(buf[0], 0x90);
        assert!(save.exists());
        run_ok(&["patch", &a, "90", "--pid", &me()]); // dry-run, sans --save
        run_err(&["patch", &a, "--pid", &me()]); // octets manquants
        run_ok(&["nop", &a, "4", "--pid", &me(), "--force"]); // nop
        run_err(&["nop", &a, "999999999999", "--pid", &me()]); // longueur déraisonnable
        run_err(&["nop", &a, "abc", "--pid", &me()]); // longueur invalide
        run_err(&["nop", &a, "--pid", &me()]); // longueur manquante
        let _ = std::fs::remove_file(&save);
    }
}
