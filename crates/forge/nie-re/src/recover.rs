//! Récupération des fonctions **feuilles** de `.text` invisibles à `.pdata`.
//!
//! `.pdata` est la vérité terrain des bornes de fonction — mais seulement pour
//! les fonctions qui **ont** des données de déroulement. Sur `nie.exe`, les
//! 102 221 entrées `.pdata` se replient en 53 668 plages fusionnées qui
//! couvrent 22 625 021 des 25 601 760 octets de `.text` (88,37 %). Les
//! **2 976 739 octets restants**, répartis en 53 669 trous, sont des fonctions
//! *feuilles* (`/GS-`, pas de prologue, pas d'unwind) : codecs SIMD, thunks
//! IAT, accesseurs, helpers CRT. Elles n'existaient jusqu'ici dans aucune
//! table — ni comme nœuds, ni comme cibles d'appel.
//!
//! ## Méthode
//!
//! 1. Fusionner les plages `.pdata` → l'ensemble « couvert ». Le complément
//!    dans `.text` est l'ensemble des **trous**.
//! 2. Amorcer une file de travail avec toutes les fonctions connues et tous
//!    les pointeurs de données (`.rdata`/`.data`) qui tombent dans `.text`.
//! 3. Décoder chaque entrée jusqu'à son terminateur (`ret`, `jmp` terminal,
//!    `int3` de remplissage) ; chaque cible de `call`/`jmp` directe rencontrée
//!    qui tombe dans un **trou** est un début de fonction feuille — on
//!    l'empile. Itérer jusqu'à saturation (point fixe).
//! 4. Chaque début retenu est validé par un second décodage : il doit
//!    atteindre un terminateur sans instruction invalide et sans franchir le
//!    début suivant.
//!
//! ## Honnêteté
//!
//! - Le point fixe ne trouve **que** ce qui est atteignable par une référence
//!   directe (branchement `rel32` ou pointeur de données aligné). Une feuille
//!   appelée uniquement par un calcul d'adresse dynamique reste invisible :
//!   c'est le résidu assumé, et il est chiffré (`gap_bytes_left`).
//! - Aucun octet n'est « deviné » : une plage n'est retenue que si elle décode
//!   intégralement et se termine sur un terminateur réel.
//! - Le remplissage (`int3`, `nop` multi-octets) entre deux fonctions est
//!   compté séparément (`padding_bytes`), jamais comme du code récupéré.

use anyhow::{Context, Result};
use goblin::pe::PE;
use hashbrown::{HashMap, HashSet};
use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};
use nie_index::{Db, rusqlite};
use tracing::info;

/// Longueur maximale décodée pour un corps de fonction feuille.
///
/// Le plus gros trou de `nie.exe` fait 145 712 octets (le bloc SIMD en tête de
/// `.text`) ; la borne doit le laisser passer d'un seul tenant.
const MAX_LEAF_LEN: u64 = 256 * 1024;

/// Statistiques de la récupération de feuilles.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecoverStats {
    /// Octets de `.text` couverts par les plages `.pdata` fusionnées.
    pub pdata_bytes: u64,
    /// Octets de `.text` hors `.pdata` avant cette passe.
    pub gap_bytes: u64,
    /// Nombre de trous (plages maximales hors `.pdata`) dans `.text`.
    pub gaps: usize,
    /// Débuts de fonction feuille candidats découverts par le point fixe.
    pub candidates: usize,
    /// Feuilles retenues (décodage validé jusqu'à un terminateur).
    pub recovered: usize,
    /// Octets de code réellement attribués aux feuilles retenues.
    pub recovered_bytes: u64,
    /// Part de `recovered_bytes` tombant effectivement dans un trou `.pdata`.
    pub recovered_gap_bytes: u64,
    /// Octets de remplissage (`int3`/`nop`) identifiés dans les trous.
    pub padding_bytes: u64,
    /// Octets de trou restant sans propriétaire après la passe.
    pub gap_bytes_left: u64,
    /// Feuilles insérées comme nouveaux nœuds `function`.
    pub inserted: usize,
    /// Arêtes `call` nouvelles vers une feuille récupérée.
    pub edges_new: usize,
    /// Thunks IAT nommés (`jmp qword [rip+disp]` vers une entrée d'import).
    pub thunks_named: usize,
    /// Candidats rejetés (décodage invalide ou pas de terminateur).
    pub rejected: usize,
    /// Part des feuilles retenues découvertes par **référence** (appel direct
    /// ou pointeur de données) — la fraction de haute confiance.
    pub by_ref: usize,
    /// Part découverte par **balayage linéaire** seul (`leaf-scan`).
    pub by_scan: usize,
    /// Feuilles reconnues comme thunk (`jmp` vers une autre fonction).
    pub shape_thunk: usize,
    /// Feuilles reconnues comme accesseur de constante (`mov eax, K ; ret`).
    pub shape_const: usize,
    /// Feuilles reconnues comme accesseur de pointeur (`lea rax, [X] ; ret`).
    pub shape_ptr: usize,
    /// Feuilles reconnues comme implémentation vide (`ret` / `xor eax,eax ; ret`).
    pub shape_stub: usize,
    /// Noms structurels écrits par la reconnaissance de forme.
    pub shape_named: usize,
    /// Fonctions déjà connues mais sans taille, mesurées par cette passe.
    pub sized_late: usize,
    /// Feuilles ayant hérité du sous-système de la cible de leur thunk.
    pub shape_inherited: usize,
    /// Feuilles supprimées : leurs octets ne décodent pas intégralement.
    pub pruned: usize,
}

/// Géométrie d'une section utile au balayage.
struct Span {
    va: u64,
    end: u64,
    off: usize,
    len: usize,
}

impl Span {
    fn contains(&self, va: u64) -> bool {
        (self.va..self.end).contains(&va)
    }
}

/// Fusionne les plages `[begin, end)` triées en plages maximales disjointes.
fn merge(mut ivs: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ivs.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(ivs.len());
    for (b, e) in ivs {
        match out.last_mut() {
            Some(last) if b <= last.1 => last.1 = last.1.max(e),
            _ => out.push((b, e)),
        }
    }
    out
}

/// Vrai si `va` tombe dans une des plages triées disjointes `ranges`.
fn in_ranges(ranges: &[(u64, u64)], va: u64) -> bool {
    ranges
        .binary_search_by(|r| {
            if va < r.0 {
                std::cmp::Ordering::Greater
            } else if va >= r.1 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Lit les plages `.pdata` (`RUNTIME_FUNCTION`) du binaire, fusionnées.
fn pdata_ranges(bytes: &[u8], pe: &PE, image_base: u64) -> Result<Vec<(u64, u64)>> {
    let sec = pe
        .sections
        .iter()
        .find(|s| s.name().is_ok_and(|n| n.starts_with(".pdata")))
        .context(".pdata introuvable")?;
    let off = sec.pointer_to_raw_data as usize;
    let len = sec.virtual_size.min(sec.size_of_raw_data) as usize;
    let raw = bytes.get(off..off + len).context(".pdata hors limites")?;
    let mut ivs = Vec::with_capacity(raw.len() / 12);
    for e in raw.chunks_exact(12) {
        let begin = u32::from_le_bytes(e[0..4].try_into().unwrap());
        let end = u32::from_le_bytes(e[4..8].try_into().unwrap());
        if begin == 0 || end <= begin {
            continue;
        }
        ivs.push((image_base + u64::from(begin), image_base + u64::from(end)));
    }
    Ok(merge(ivs))
}

/// Résultat du décodage d'un corps : longueur et cibles de branchement directes.
struct Body {
    len: u64,
    targets: Vec<u64>,
    /// Adresse IAT du thunk si le corps est exactement `jmp qword [rip+disp]`.
    thunk_iat: Option<u64>,
}

/// Forme reconnue d'une fonction feuille.
///
/// Ces formes sont des faits **syntaxiques** lus dans les octets, pas des
/// hypothèses : elles donnent un nom structurel non ambigu (et, pour un thunk,
/// un sous-système hérité de sa cible), jamais le nom C++ d'origine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `jmp rel32`, éventuellement précédé d'un ajustement de `this`
    /// (`add rcx, N` / `lea rcx, [rdx+N]`) — thunk d'ajustement d'héritage
    /// multiple généré par MSVC.
    Thunk { target: u64 },
    /// `mov eax, imm32 ; ret` — accesseur de constante (identifiant de type,
    /// taille, drapeau) généré par instanciation de patron.
    ConstRet(u32),
    /// `lea rax, [rip+disp] ; ret` — accesseur de pointeur (singleton, table).
    PtrRet(u64),
    /// `ret` seul, ou `xor eax, eax ; ret` — implémentation vide d'une méthode
    /// virtuelle.
    Stub,
    /// Corps quelconque : aucun nom structurel n'est inventé.
    Other,
}

/// Reconnaît la forme d'une feuille en relisant ses octets.
fn shape_of(text: &Span, bytes: &[u8], start: u64, len: u64) -> Shape {
    if !text.contains(start) || len == 0 {
        return Shape::Other;
    }
    let off = text.off + (start - text.va) as usize;
    let Some(buf) = bytes.get(off..off + len as usize) else {
        return Shape::Other;
    };
    let mut dec = Decoder::with_ip(64, buf, start, DecoderOptions::NONE);
    let mut insns = Vec::new();
    let mut insn = Instruction::default();
    while dec.can_decode() && insns.len() < 4 {
        dec.decode_out(&mut insn);
        if insn.is_invalid() {
            return Shape::Other;
        }
        insns.push(insn);
    }
    match insns.as_slice() {
        // `ret` — méthode virtuelle vide.
        [a] if a.mnemonic() == Mnemonic::Ret => Shape::Stub,
        // `jmp rel32` nu : trampoline.
        [a] if a.mnemonic() == Mnemonic::Jmp && a.op0_kind() == OpKind::NearBranch64 => {
            Shape::Thunk {
                target: a.near_branch64(),
            }
        }
        [a, b] => match (a.mnemonic(), b.mnemonic()) {
            // `mov eax, imm32 ; ret`
            (Mnemonic::Mov, Mnemonic::Ret)
                if a.op0_kind() == OpKind::Register
                    && a.op0_register() == Register::EAX
                    && a.op1_kind() == OpKind::Immediate32 =>
            {
                Shape::ConstRet(a.immediate32())
            }
            // `xor eax, eax ; ret`
            (Mnemonic::Xor, Mnemonic::Ret)
                if a.op0_register() == Register::EAX && a.op1_register() == Register::EAX =>
            {
                Shape::Stub
            }
            // `lea rax, [rip+disp] ; ret`
            (Mnemonic::Lea, Mnemonic::Ret)
                if a.op0_register() == Register::RAX && a.is_ip_rel_memory_operand() =>
            {
                Shape::PtrRet(a.ip_rel_memory_address())
            }
            // Ajustement de `this` puis saut : thunk d'héritage multiple.
            (Mnemonic::Add | Mnemonic::Lea, Mnemonic::Jmp)
                if b.op0_kind() == OpKind::NearBranch64 =>
            {
                Shape::Thunk {
                    target: b.near_branch64(),
                }
            }
            _ => Shape::Other,
        },
        _ => Shape::Other,
    }
}

/// Décode un corps depuis `start`, borné par `limit`, et renvoie sa longueur
/// réelle (jusqu'au terminateur inclus) plus les cibles directes rencontrées.
///
/// Renvoie `None` si le décodage rencontre une instruction invalide avant
/// d'atteindre un terminateur, ou s'il dépasse `limit` sans terminer.
fn decode_body(text: &Span, bytes: &[u8], start: u64, limit: u64) -> Option<Body> {
    if !text.contains(start) || limit <= start {
        return None;
    }
    let off = text.off + (start - text.va) as usize;
    let span = ((limit - start).min(MAX_LEAF_LEN)) as usize;
    let buf = bytes.get(off..off + span)?;
    let mut dec = Decoder::with_ip(64, buf, start, DecoderOptions::NONE);
    let mut insn = Instruction::default();
    let mut targets = Vec::new();
    let mut end;
    // Un `jmp` en avant à l'intérieur du corps ne le termine pas : on ne
    // s'arrête que si le flot ne peut plus retomber au-delà.
    let mut furthest_branch = start;
    // Dernier terminateur franchi (`ret` / `jmp` inconditionnel) qui n'a pas pu
    // clore le corps parce qu'une branche visait plus loin. Si la borne est
    // atteinte sans jamais conclure — cas d'une fonction dont une branche saute
    // vers un bloc froid au-delà de `limit` — c'est ce point qui borne le corps
    // plutôt que de tout rejeter : rejeter perdait des fonctions entières
    // (94 736 octets d'un seul tenant en tête de `.text`).
    let mut last_term: Option<u64> = None;
    let mut n_insn = 0u32;
    while dec.can_decode() {
        dec.decode_out(&mut insn);
        if insn.is_invalid() {
            return None;
        }
        n_insn += 1;
        end = insn.next_ip();
        if n_insn == 1
            && insn.mnemonic() == Mnemonic::Jmp
            && insn.op0_kind() == OpKind::Memory
            && insn.is_ip_rel_memory_operand()
        {
            let iat = insn.ip_rel_memory_address();
            return Some(Body {
                len: end - start,
                targets,
                thunk_iat: Some(iat),
            });
        }
        match insn.flow_control() {
            FlowControl::Call
            | FlowControl::UnconditionalBranch
            | FlowControl::ConditionalBranch => {
                if insn.op0_kind() == OpKind::NearBranch64 {
                    let t = insn.near_branch64();
                    if insn.flow_control() == FlowControl::Call {
                        targets.push(t);
                    } else if text.contains(t) && t > start && t < start + MAX_LEAF_LEN {
                        // Saut interne (boucle / branche) : il prolonge le corps.
                        furthest_branch = furthest_branch.max(t);
                        if insn.flow_control() == FlowControl::UnconditionalBranch {
                            targets.push(t);
                        }
                    } else {
                        targets.push(t);
                    }
                }
                if insn.flow_control() == FlowControl::UnconditionalBranch {
                    if end > furthest_branch {
                        return Some(Body {
                            len: end - start,
                            targets,
                            thunk_iat: None,
                        });
                    }
                    last_term = Some(end);
                }
            }
            FlowControl::Return => {
                if end > furthest_branch {
                    return Some(Body {
                        len: end - start,
                        targets,
                        thunk_iat: None,
                    });
                }
                last_term = Some(end);
            }
            FlowControl::Interrupt => {
                // `int3` de remplissage : le corps s'arrête à l'instruction
                // précédente, sauf s'il s'agit du tout premier octet.
                if n_insn == 1 {
                    return None;
                }
                return Some(Body {
                    len: end - start - u64::from(insn.len() as u32),
                    targets,
                    thunk_iat: None,
                });
            }
            _ => {}
        }
        if end - start > MAX_LEAF_LEN {
            return None;
        }
    }
    // Borne atteinte sans conclusion : on retient jusqu'au dernier terminateur
    // franchi, s'il y en a eu un. Sans ce repli, une fonction dont une branche
    // vise au-delà de la borne était entièrement perdue.
    last_term.map(|e| Body {
        len: e - start,
        targets,
        thunk_iat: None,
    })
}

/// Récupère les fonctions feuilles de `.text` hors `.pdata` et les ingère dans
/// `binary_id = bin`.
///
/// Si `dry_run` est vrai, aucune écriture n'a lieu : seules les statistiques
/// sont calculées (mesure avant/après sans muter la base).
///
/// # Errors
///
/// Échoue si le PE est illisible, si `.text`/`.pdata` manquent, ou sur toute
/// erreur SQLite pendant l'ingestion.
#[allow(clippy::too_many_lines)]
pub fn recover_leaves(
    db: &mut Db,
    bin: i64,
    exe_path: &std::path::Path,
    dry_run: bool,
) -> Result<RecoverStats> {
    let bytes =
        std::fs::read(exe_path).with_context(|| format!("lecture {}", exe_path.display()))?;
    let pe = PE::parse(&bytes).context("goblin: parse PE")?;
    let image_base = pe.image_base;

    let span_of = |name: &str| -> Option<Span> {
        let s = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(name)))?;
        let len = s.virtual_size.min(s.size_of_raw_data) as usize;
        Some(Span {
            va: image_base + u64::from(s.virtual_address),
            end: image_base + u64::from(s.virtual_address) + len as u64,
            off: s.pointer_to_raw_data as usize,
            len,
        })
    };
    let text = span_of(".text").context(".text introuvable")?;

    let mut stats = RecoverStats::default();
    let covered = pdata_ranges(&bytes, &pe, image_base)?;
    stats.pdata_bytes = covered.iter().map(|(b, e)| e - b).sum();
    stats.gap_bytes = (text.end - text.va).saturating_sub(stats.pdata_bytes);
    // Trous = complément de `covered` dans `.text`.
    let mut gaps: Vec<(u64, u64)> = Vec::new();
    let mut cur = text.va;
    for &(b, e) in &covered {
        if b > cur {
            gaps.push((cur, b));
        }
        cur = cur.max(e);
    }
    if cur < text.end {
        gaps.push((cur, text.end));
    }
    stats.gaps = gaps.len();

    // Amorçage : fonctions connues (racines `.pdata` + feuilles déjà ingérées).
    // Les tailles déjà en base servent au calcul du résidu et à la
    // reconnaissance de forme : sans elles, une seconde exécution ne « verrait »
    // que ses propres découvertes et rapporterait un résidu faussement énorme.
    let known_sizes: Vec<(u64, u64)> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr, size FROM function WHERE binary_id=?1 AND size>0")?;
        q.query_map([bin], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let known: Vec<u64> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1")?;
        q.query_map([bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let sizeless: Vec<u64> = {
        let mut q = db
            .conn()
            .prepare("SELECT vaddr FROM function WHERE binary_id=?1 AND size=0")?;
        q.query_map([bin], |r| r.get::<_, i64>(0).map(|v| v as u64))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut seen: HashSet<u64> = known.iter().copied().collect();
    let mut queue: Vec<u64> = known;

    // Pointeurs de données (`.rdata`, `.data`, `.fptable`) tombant dans un trou :
    // vtables sans RTTI, tables de callback, tables de saut absolues.
    let mut leaves: HashSet<u64> = HashSet::new();
    for name in [".rdata", ".data", "_RDATA", ".fptable", ".rodata"] {
        let Some(sp) = span_of(name) else { continue };
        let Some(raw) = bytes.get(sp.off..sp.off + sp.len) else {
            continue;
        };
        for (i, w) in raw.chunks_exact(8).enumerate() {
            let v = u64::from_le_bytes(w.try_into().unwrap());
            if text.contains(v) && in_ranges(&gaps, v) && !seen.contains(&v) {
                let _ = i;
                leaves.insert(v);
            }
        }
    }
    for &l in &leaves {
        seen.insert(l);
        queue.push(l);
    }

    // Point fixe : décoder tout ce qu'on connaît, empiler les cibles directes
    // qui tombent dans un trou.
    let starts_sorted = {
        let mut v: Vec<u64> = seen.iter().copied().collect();
        v.sort_unstable();
        v
    };
    // Borne de décodage : début suivant connu, ou fin de `.text`.
    let next_start = |v: &Vec<u64>, a: u64| -> u64 {
        match v.binary_search(&a) {
            Ok(i) => v.get(i + 1).copied().unwrap_or(text.end),
            Err(i) => v.get(i).copied().unwrap_or(text.end),
        }
    };
    let mut bounds = starts_sorted;
    let mut edges: Vec<(u64, u64)> = Vec::new();
    // Feuilles déjà en base : celles qui tombent dans un trou `.pdata`. Elles
    // sont re-mesurées à chaque passe pour rester cohérentes avec les débuts
    // découverts depuis.
    let known_leaves: Vec<u64> = known_sizes
        .iter()
        .map(|&(a, _)| a)
        .filter(|&a| in_ranges(&gaps, a))
        .collect();
    let mut new_leaves: HashSet<u64> = leaves.clone();
    // Feuilles découvertes par balayage linéaire des résidus, sans qu'aucune
    // référence ne les désigne : confiance moindre, provenance distincte.
    let mut scanned: HashSet<u64> = HashSet::new();
    let mut sizes: HashMap<u64, u64> = HashMap::new();
    let mut thunks: HashMap<u64, u64> = HashMap::new();

    // Fin du trou contenant `a` (ou fin de `.text` si `a` n'est pas dans un trou).
    let gap_end_of = |a: u64| -> u64 {
        gaps.binary_search_by(|r| {
            if a < r.0 {
                std::cmp::Ordering::Greater
            } else if a >= r.1 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .map_or(text.end, |k| gaps[k].1)
    };

    // Boucle jusqu'au point fixe : le parcours par références découvre des
    // feuilles, leur mesure dégage les résidus, et le balayage linéaire des
    // résidus découvre des feuilles que rien ne référence directement — qui
    // relancent à leur tour le parcours par références.
    loop {
        // (a) parcours par références directes.
        while let Some(a) = queue.pop() {
            let limit = next_start(&bounds, a).max(a + 1);
            // Pour une fonction `.pdata`, la borne dure est la fin de sa plage.
            let limit = match covered.binary_search_by(|r| {
                if a < r.0 {
                    std::cmp::Ordering::Greater
                } else if a >= r.1 {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            }) {
                Ok(i) => covered[i].1.max(limit),
                Err(_) => limit,
            };
            let Some(body) = decode_body(&text, &bytes, a, limit.min(a + MAX_LEAF_LEN)) else {
                continue;
            };
            for t in body.targets {
                if !text.contains(t) {
                    continue;
                }
                edges.push((a, t));
                if in_ranges(&gaps, t) && seen.insert(t) {
                    new_leaves.insert(t);
                    queue.push(t);
                    // Le nouveau début resserre les bornes des voisins.
                    if let Err(i) = bounds.binary_search(&t) {
                        bounds.insert(i, t);
                    }
                }
            }
        }

        // (b) mesure : chaque feuille candidate doit décoder jusqu'à un
        // terminateur, bornée par la feuille suivante ou la fin de son trou.
        // La mesure porte sur **toutes** les feuilles (celles de cette passe et
        // celles déjà ingérées), sinon une feuille ancienne garde une taille
        // mesurée avec des bornes plus larges et recouvre une feuille neuve
        // apparue entre-temps.
        let mut cand: Vec<u64> = new_leaves.iter().copied().collect();
        cand.extend(known_leaves.iter().copied());
        cand.sort_unstable();
        cand.dedup();
        sizes.clear();
        thunks.clear();
        stats.rejected = 0;
        for (i, &a) in cand.iter().enumerate() {
            // La borne est le prochain début **connu**, toutes provenances
            // confondues (`bounds`), pas seulement le prochain candidat de
            // cette passe : sinon une feuille nouvelle se mesure par-dessus une
            // fonction ingérée par une passe antérieure.
            let next_known = match bounds.binary_search(&a) {
                Ok(k) => bounds.get(k + 1).copied().unwrap_or(u64::MAX),
                Err(k) => bounds.get(k).copied().unwrap_or(u64::MAX),
            };
            let next = cand
                .get(i + 1)
                .copied()
                .unwrap_or(u64::MAX)
                .min(next_known)
                .min(gap_end_of(a));
            match decode_body(&text, &bytes, a, next) {
                Some(b) => {
                    sizes.insert(a, b.len);
                    if let Some(iat) = b.thunk_iat {
                        thunks.insert(a, iat);
                    }
                }
                None => stats.rejected += 1,
            }
        }

        // (c) balayage linéaire des résidus : dans chaque trou, tout ce qui
        // n'appartient à aucun corps et n'est pas du remplissage est un début
        // de fonction candidat. MSVC aligne les fonctions, donc le début se
        // trouve juste après le run de remplissage.
        let owned = merge(sizes.iter().map(|(&a, &l)| (a, a + l)).collect());
        let mut found = 0usize;
        // `gaps` et `owned` sont triées et disjointes : un balayage à deux
        // curseurs suffit. Itérer `owned` en entier pour chaque trou serait
        // quadratique (53 669 × 12 731 comparaisons par tour).
        let mut oi = 0usize;
        for &(gb, ge) in &gaps {
            while oi < owned.len() && owned[oi].1 <= gb {
                oi += 1;
            }
            let mut c = gb;
            let mut j = oi;
            // Sous-plages du trou non couvertes par un corps retenu.
            let mut free: Vec<(u64, u64)> = Vec::new();
            while j < owned.len() && owned[j].0 < ge {
                let (b, e) = (owned[j].0.max(gb), owned[j].1.min(ge));
                if b > c {
                    free.push((c, b));
                }
                c = c.max(e);
                j += 1;
            }
            if c < ge {
                free.push((c, ge));
            }
            for (fb, fe) in free {
                // On avance **dans** le fragment : après un corps validé on
                // saute son remplissage et on reprend juste après. Ne tenter
                // qu'un début par tour rendrait la convergence quadratique en
                // nombre de fonctions. Si rien ne décode à une position, la
                // zone n'est pas du code (table de saut, données inline) et on
                // abandonne le fragment — un début inventé polluerait le graphe.
                let mut a = fb + count_filler(&text, &bytes, fb, fe);
                while a < fe {
                    if seen.contains(&a) {
                        // Début déjà connu mais sans taille mesurée : il ne doit
                        // pas condamner la suite du fragment.
                        a = (a + 16) & !15;
                        continue;
                    }
                    match decode_body(&text, &bytes, a, fe) {
                        Some(body) => {
                            seen.insert(a);
                            new_leaves.insert(a);
                            scanned.insert(a);
                            queue.push(a);
                            if let Err(i) = bounds.binary_search(&a) {
                                bounds.insert(i, a);
                            }
                            found += 1;
                            let next = a + body.len.max(1);
                            a = next + count_filler(&text, &bytes, next, fe);
                        }
                        // Rien de décodable ici (données inline, table de saut,
                        // milieu d'instruction) : on se recale sur la frontière
                        // de 16 octets suivante — MSVC aligne les fonctions à
                        // 16 — au lieu d'abandonner le fragment entier. Un
                        // échec en tête condamnait jusqu'ici tout ce qui suit,
                        // dont un bloc de 94 736 octets en tête de `.text`.
                        //
                        // Mais l'alignement seul ne suffit pas : une adresse
                        // multiple de 16 peut tomber au **milieu** d'une
                        // instruction, et le début inventé coupe alors la
                        // fonction qui la contient. On exige donc que la
                        // position soit précédée de remplissage — le marqueur
                        // fiable d'une frontière de fonction chez MSVC.
                        None => {
                            let mut n = (a + 16) & !15;
                            while n < fe && !preceded_by_filler(&text, &bytes, n) {
                                n += 16;
                            }
                            a = n;
                        }
                    }
                }
            }
        }
        if found == 0 {
            break;
        }
    }
    stats.candidates = new_leaves.len();
    stats.recovered = sizes.len();
    stats.recovered_bytes = sizes.values().sum();
    stats.by_scan = sizes.keys().filter(|&&a| scanned.contains(&a)).count();
    stats.by_ref = stats.recovered - stats.by_scan;
    // Le bilan de couverture porte sur **tout** ce dont on connaît les bornes :
    // les feuilles de cette passe et celles déjà ingérées.
    let mut all_sizes: HashMap<u64, u64> = sizes.clone();
    for &(a, l) in &known_sizes {
        all_sizes.entry(a).or_insert(l);
    }
    // Fonctions déjà connues mais sans taille (ingérées par `vtable`/`vtable-anon`,
    // qui ne connaissent que l'adresse d'entrée) : les mesurer, sinon leur code
    // reste compté comme résidu et leur forme n'est jamais reconnue.
    {
        let mut starts: Vec<u64> = all_sizes.keys().copied().collect();
        for &a in &sizeless {
            starts.push(a);
        }
        starts.sort_unstable();
        starts.dedup();
        for &a in &sizeless {
            if all_sizes.contains_key(&a) {
                continue;
            }
            let next = match starts.binary_search(&a) {
                Ok(i) => starts.get(i + 1).copied().unwrap_or(text.end),
                Err(i) => starts.get(i).copied().unwrap_or(text.end),
            };
            if let Some(b) = decode_body(&text, &bytes, a, next.max(a + 1)) {
                all_sizes.insert(a, b.len);
                stats.sized_late += 1;
            }
        }
    }

    // Résidu honnête : on n'attribue à la récupération que l'intersection
    // réelle des corps retenus avec les trous (un corps peut déborder sur une
    // plage `.pdata` voisine), et le remplissage se compte sur le complément.
    let owned = merge(all_sizes.iter().map(|(&a, &l)| (a, a + l)).collect());
    let mut pad = 0u64;
    let mut in_gap = 0u64;
    for &(gb, ge) in &gaps {
        let mut c = gb;
        for &(b, e) in &owned {
            if e <= gb || b >= ge {
                continue;
            }
            let (b, e) = (b.max(gb), e.min(ge));
            if b > c {
                pad += count_filler(&text, &bytes, c, b);
            }
            in_gap += e.saturating_sub(b.max(c));
            c = c.max(e);
        }
        if c < ge {
            pad += count_filler(&text, &bytes, c, ge);
        }
    }
    stats.padding_bytes = pad;
    stats.recovered_gap_bytes = in_gap;
    stats.gap_bytes_left = stats.gap_bytes.saturating_sub(in_gap + pad);

    // Nettoyage : une feuille dont les octets ne décodent pas *intégralement*
    // n'est pas une fonction — c'est un début inventé, tombé au milieu d'une
    // instruction, et il coupe en deux la fonction qui la contient. Ces
    // fausses bornes se paient plus loin : la forge ne peut pas relever une
    // unité tronquée. Les racines `.pdata` sont épargnées : elles sont la
    // vérité terrain, pas une inférence.
    let mut prune: Vec<u64> = Vec::new();
    for (&a, &len) in &all_sizes {
        if in_ranges(&covered, a) {
            continue; // racine `.pdata`
        }
        if !decodes_exactly(&text, &bytes, a, len) {
            prune.push(a);
        }
    }
    for a in &prune {
        all_sizes.remove(a);
    }
    stats.pruned = prune.len();

    // Noms d'import pour les thunks IAT.
    let iat_names = import_names(&pe, image_base);

    // Reconnaissance de forme : nom structurel + rôle pour chaque feuille.
    let mut shapes: HashMap<u64, Shape> = HashMap::new();
    for (&a, &len) in &all_sizes {
        let sh = shape_of(&text, &bytes, a, len);
        match sh {
            Shape::Thunk { .. } => stats.shape_thunk += 1,
            Shape::ConstRet(_) => stats.shape_const += 1,
            Shape::PtrRet(_) => stats.shape_ptr += 1,
            Shape::Stub => stats.shape_stub += 1,
            Shape::Other => {}
        }
        if sh != Shape::Other {
            shapes.insert(a, sh);
        }
    }

    if dry_run {
        info!(
            recovered = stats.recovered,
            bytes = stats.recovered_bytes,
            "recover: simulation, aucune écriture"
        );
        return Ok(stats);
    }

    let tx = db.conn_mut().transaction()?;
    {
        // Les fausses bornes sortent de la base : les garder ferait recouper
        // la forge au meme endroit au prochain `split`.
        let mut del = tx.prepare("DELETE FROM function WHERE binary_id=?1 AND vaddr=?2")?;
        for a in &prune {
            del.execute(rusqlite::params![bin, *a as i64])?;
        }
    }
    {
        let mut upd = tx.prepare(
            "UPDATE function SET size=?2, name=COALESCE(name,?3), name_source=COALESCE(name_source,?4) WHERE id=?1",
        )?;
        for (&a, &len) in &all_sizes {
            // Un thunk d'import porte le nom exact de la fonction importée ;
            // sinon la forme reconnue donne un nom structurel.
            let iat = thunks
                .get(&a)
                .and_then(|iat| iat_names.get(iat))
                .map(|n| format!("thunk_{n}"));
            if iat.is_some() {
                stats.thunks_named += 1;
            }
            let name = iat.or_else(|| match shapes.get(&a) {
                Some(Shape::Thunk { target }) => Some(format!("thunk_to_{target:x}")),
                Some(Shape::ConstRet(k)) => Some(format!("get_const_{k:08x}")),
                Some(Shape::PtrRet(p)) => Some(format!("get_ptr_{p:x}")),
                Some(Shape::Stub) => Some(format!("stub_{a:x}")),
                Some(Shape::Other) | None => None,
            });
            let src = if thunks.contains_key(&a) {
                "iat-thunk"
            } else {
                "leaf-shape"
            };
            let (nm, src) = match &name {
                Some(n) => (Some(n.as_str()), Some(src)),
                None => (None, None),
            };
            if nm.is_some() {
                stats.shape_named += 1;
            }
            let id: i64 = {
                let existing: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM function WHERE binary_id=?1 AND vaddr=?2",
                        rusqlite::params![bin, a as i64],
                        |r| r.get(0),
                    )
                    .ok();
                match existing {
                    Some(id) => id,
                    None => {
                        stats.inserted += 1;
                        // `leaf-scan` : trouvée par balayage linéaire seul.
                        // `leaf-ref` : désignée par un appel direct ou un
                        // pointeur de données — c'est une fonction au sens
                        // fort, pas seulement une suite d'octets décodable.
                        let prov = if scanned.contains(&a) {
                            "leaf-scan"
                        } else {
                            "leaf-ref"
                        };
                        tx.prepare_cached(
                            "INSERT INTO function(binary_id, vaddr, size, name, name_source, subsystem, subsys_src, role, confidence)
                             VALUES(?1,?2,?3,?4,?5,'standalone',?6,'leaf',0.0)",
                        )?
                        .execute(rusqlite::params![bin, a as i64, len as i64, nm, src, prov])?;
                        continue;
                    }
                }
            };
            upd.execute(rusqlite::params![id, len as i64, nm, src])?;
        }
    }
    {
        let mut ins = tx.prepare(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,'call')",
        )?;
        for (f, t) in edges {
            if sizes.contains_key(&t) {
                stats.edges_new += ins.execute(rusqlite::params![bin, f as i64, t as i64])?;
            }
        }
    }
    {
        // Un thunk s'exécute *pour le compte de* sa cible : il appartient au
        // même sous-système. C'est une identité structurelle, pas une
        // inférence statistique — elle prime donc sur une étiquette posée par
        // la propagation de labels (`ml`) ou laissée à `standalone` par la
        // récupération. Rien n'est écrit si la cible est elle-même non classée,
        // et un label déjà structurel (RTTI/vtable) n'est jamais écrasé.
        let mut upd = tx.prepare(
            "UPDATE function SET subsystem=?3, subsys_src='thunk-inherit', confidence=?4
             WHERE binary_id=?1 AND vaddr=?2
               AND (subsystem='standalone'
                    OR subsys_src IN ('ml','leaf-scan','leaf-ref','leaf-recover'))",
        )?;
        let mut get = tx.prepare(
            "SELECT subsystem, confidence FROM function WHERE binary_id=?1 AND vaddr=?2",
        )?;
        for (&a, sh) in &shapes {
            let Shape::Thunk { target } = *sh else {
                continue;
            };
            let tgt: Option<(String, f64)> = get
                .query_row(rusqlite::params![bin, target as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .ok();
            let Some((sub, conf)) = tgt else { continue };
            if sub == "standalone" {
                continue;
            }
            stats.shape_inherited += upd.execute(rusqlite::params![bin, a as i64, sub, conf])?;
        }
    }
    tx.commit()?;

    info!(
        gaps = stats.gaps,
        gap_bytes = stats.gap_bytes,
        recovered = stats.recovered,
        recovered_bytes = stats.recovered_bytes,
        thunks = stats.thunks_named,
        "recover: feuilles ingérées"
    );
    Ok(stats)
}

/// Vrai si `[va, va+len)` se décode en instructions complètes, sans reste.
///
/// C'est le test qu'appliquera la forge : une unité dont la dernière
/// instruction déborde ne peut pas être relevée.
fn decodes_exactly(text: &Span, bytes: &[u8], va: u64, len: u64) -> bool {
    if len == 0 || !text.contains(va) {
        return false;
    }
    let off = text.off + (va - text.va) as usize;
    let Some(buf) = bytes.get(off..off + len as usize) else {
        return false;
    };
    let mut dec = Decoder::with_ip(64, buf, va, DecoderOptions::NONE);
    let mut insn = Instruction::default();
    let mut consumed = 0u64;
    while dec.can_decode() {
        dec.decode_out(&mut insn);
        if insn.is_invalid() {
            return false;
        }
        consumed += insn.len() as u64;
    }
    consumed == len
}

/// Vrai si l'octet précédant `va` est du remplissage (`0xCC`, `0x90`, `0x00`).
///
/// Chez MSVC, une fonction est précédée du remplissage d'alignement de la
/// précédente. C'est le seul marqueur local fiable d'une frontière : une
/// adresse alignée à 16 ne l'est pas — elle peut tomber au milieu d'une
/// instruction, et le début inventé couperait la fonction qui la contient.
fn preceded_by_filler(text: &Span, bytes: &[u8], va: u64) -> bool {
    if va == 0 || !text.contains(va - 1) {
        return false;
    }
    let off = text.off + (va - 1 - text.va) as usize;
    bytes
        .get(off)
        .is_some_and(|&b| matches!(b, 0xCC | 0x90 | 0x00))
}

/// Longueur du **run** de remplissage (`0xCC`, `0x00`, `0x90`) en tête de
/// `[a, b)`.
///
/// Compter les octets de remplissage un à un sur toute la plage serait
/// malhonnête : un corps de fonction contient des `0x00` (déplacements,
/// immédiats) qui seraient comptés comme du vide. Seul le run contigu qui
/// suit immédiatement la fin d'une fonction est du vrai remplissage
/// d'alignement.
fn count_filler(text: &Span, bytes: &[u8], a: u64, b: u64) -> u64 {
    if b <= a || !text.contains(a) {
        return 0;
    }
    let off = text.off + (a - text.va) as usize;
    let Some(buf) = bytes.get(off..off + (b - a) as usize) else {
        return 0;
    };
    buf.iter()
        .take_while(|&&c| c == 0xCC || c == 0x00 || c == 0x90)
        .count() as u64
}

/// Table `adresse virtuelle de l'entrée IAT → nom d'import` (`DLL_Fonction`,
/// DLL sans extension).
///
/// Seul `Import::rva` est une adresse virtuelle ; `Import::offset` est un
/// offset fichier et n'a rien à faire dans cette table (il y créerait des
/// correspondances fortuites).
fn import_names(pe: &PE, image_base: u64) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    for imp in &pe.imports {
        let dll = imp.dll.trim_end_matches(".dll").trim_end_matches(".DLL");
        out.insert(image_base + imp.rva as u64, format!("{dll}_{}", imp.name));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un `Span` de test couvrant `bytes` à partir de `va`.
    fn span(va: u64, len: usize) -> Span {
        Span {
            va,
            end: va + len as u64,
            off: 0,
            len,
        }
    }

    #[test]
    fn merge_fusionne_les_plages_contigues_et_chevauchantes() {
        let m = merge(vec![(10, 20), (20, 30), (25, 40), (50, 60)]);
        assert_eq!(m, vec![(10, 40), (50, 60)]);
    }

    #[test]
    fn merge_trie_avant_de_fusionner() {
        let m = merge(vec![(50, 60), (10, 20), (15, 30)]);
        assert_eq!(m, vec![(10, 30), (50, 60)]);
    }

    #[test]
    fn in_ranges_respecte_les_bornes_semi_ouvertes() {
        let r = [(10u64, 20u64), (30, 40)];
        assert!(in_ranges(&r, 10));
        assert!(in_ranges(&r, 19));
        assert!(!in_ranges(&r, 20), "la borne haute est exclue");
        assert!(!in_ranges(&r, 25));
        assert!(in_ranges(&r, 30));
    }

    #[test]
    fn count_filler_ne_compte_que_le_run_de_tete() {
        // `cc cc 90 00` puis un octet de code, puis encore du remplissage :
        // seul le run initial compte — les `0x00` d'un corps ne sont pas du vide.
        let bytes = [0xCC, 0xCC, 0x90, 0x00, 0x48, 0xCC, 0xCC];
        let sp = span(0x1000, bytes.len());
        assert_eq!(count_filler(&sp, &bytes, 0x1000, 0x1007), 4);
        assert_eq!(
            count_filler(&sp, &bytes, 0x1004, 0x1007),
            0,
            "commence sur du code"
        );
        assert_eq!(count_filler(&sp, &bytes, 0x1005, 0x1007), 2);
    }

    #[test]
    fn shape_reconnait_un_accesseur_de_constante() {
        // b8 b1 2b 7d 53   mov eax, 0x537D2BB1
        // c3               ret
        let bytes = [0xB8, 0xB1, 0x2B, 0x7D, 0x53, 0xC3];
        let sp = span(0x1000, bytes.len());
        assert_eq!(
            shape_of(&sp, &bytes, 0x1000, 6),
            Shape::ConstRet(0x537D_2BB1)
        );
    }

    #[test]
    fn shape_reconnait_un_stub_vide() {
        let ret = [0xC3];
        let sp = span(0x1000, 1);
        assert_eq!(shape_of(&sp, &ret, 0x1000, 1), Shape::Stub);
        // 33 c0   xor eax, eax ; c3   ret
        let xor_ret = [0x33, 0xC0, 0xC3];
        let sp2 = span(0x2000, 3);
        assert_eq!(shape_of(&sp2, &xor_ret, 0x2000, 3), Shape::Stub);
    }

    #[test]
    fn shape_reconnait_un_thunk_avec_ajustement_de_this() {
        // 48 83 c1 08   add rcx, 8
        // e9 xx xx xx xx jmp rel32  (cible = fin de l'instruction + disp)
        let bytes = [0x48, 0x83, 0xC1, 0x08, 0xE9, 0x00, 0x01, 0x00, 0x00];
        let sp = span(0x1000, bytes.len());
        // fin du `jmp` = 0x1009, + 0x100 → 0x1109
        assert_eq!(
            shape_of(&sp, &bytes, 0x1000, 9),
            Shape::Thunk { target: 0x1109 }
        );
    }

    #[test]
    fn shape_ignore_un_corps_quelconque() {
        // 50 push rax ; 58 pop rax ; c3 ret — trois instructions, aucune forme.
        let bytes = [0x50, 0x58, 0xC3];
        let sp = span(0x1000, 3);
        assert_eq!(shape_of(&sp, &bytes, 0x1000, 3), Shape::Other);
    }

    #[test]
    fn decode_body_s_arrete_sur_le_ret_et_collecte_les_cibles() {
        // e8 xx.. call rel32 ; c3 ret
        let bytes = [0xE8, 0x00, 0x01, 0x00, 0x00, 0xC3];
        let sp = span(0x1000, bytes.len());
        let b = decode_body(&sp, &bytes, 0x1000, 0x1006).expect("doit décoder");
        assert_eq!(b.len, 6);
        assert_eq!(b.targets, vec![0x1105]);
        assert!(b.thunk_iat.is_none());
    }

    #[test]
    fn decode_body_refuse_un_debut_sur_du_remplissage() {
        let bytes = [0xCC, 0xCC];
        let sp = span(0x1000, 2);
        assert!(decode_body(&sp, &bytes, 0x1000, 0x1002).is_none());
    }
}
