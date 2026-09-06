//! Forme **textuelle** des instructions : la source assembleur du dépôt.
//!
//! Ce module donne aux corps régénérés une représentation lisible et
//! diff-able — `push rbx ; sub rsp, 0x20 ; call 0x140123456` — plutôt qu'un tas
//! d'octets. Le couple [`Insn::to_text`] / [`parse_insn`] est un aller-retour
//! strict : tout ce qui s'écrit se relit, et les tests le vérifient sur des corps
//! réels.
//!
//! Conventions propres au dialecte :
//! - les cibles de branchement sont des **adresses absolues** (`call 0x140123456`) ;
//! - le suffixe `.s` marque la forme courte (`jmp.s`, `jne.s`) — MSVC choisit
//!   l'une ou l'autre, et la source doit conserver ce choix pour rester exacte ;
//! - `[rip 0x140abc000]` est un opérande relatif au pointeur d'instruction, écrit
//!   par sa cible absolue.

use crate::{
    Alu, BitOp, Cond, CvtOp, Insn, Mem, NoOp, Reg, RepOp, Rm, Seg, ShiftOp, Size, SseMaskOp, SseOp,
    SseShiftOp, UnOp, VexOp, Xmm, XmmRm,
};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Erreur d'analyse d'une ligne assembleur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "assembleur : {}", self.0)
    }
}

const R64: [&str; 16] = [
    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15",
];
const R32: [&str; 16] = [
    "eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "r8d", "r9d", "r10d", "r11d", "r12d",
    "r13d", "r14d", "r15d",
];
const R16: [&str; 16] = [
    "ax", "cx", "dx", "bx", "sp", "bp", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w", "r13w",
    "r14w", "r15w",
];
const R8N: [&str; 16] = [
    "al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b", "r12b",
    "r13b", "r14b", "r15b",
];
const REGS: [Reg; 16] = [
    Reg::Rax,
    Reg::Rcx,
    Reg::Rdx,
    Reg::Rbx,
    Reg::Rsp,
    Reg::Rbp,
    Reg::Rsi,
    Reg::Rdi,
    Reg::R8,
    Reg::R9,
    Reg::R10,
    Reg::R11,
    Reg::R12,
    Reg::R13,
    Reg::R14,
    Reg::R15,
];
const ALUS: [(&str, Alu); 8] = [
    ("add", Alu::Add),
    ("or", Alu::Or),
    ("adc", Alu::Adc),
    ("sbb", Alu::Sbb),
    ("and", Alu::And),
    ("sub", Alu::Sub),
    ("xor", Alu::Xor),
    ("cmp", Alu::Cmp),
];
const CONDS: [(&str, Cond); 16] = [
    ("o", Cond::O),
    ("no", Cond::No),
    ("b", Cond::B),
    ("ae", Cond::Ae),
    ("e", Cond::E),
    ("ne", Cond::Ne),
    ("be", Cond::Be),
    ("a", Cond::A),
    ("s", Cond::S),
    ("ns", Cond::Ns),
    ("p", Cond::P),
    ("np", Cond::Np),
    ("l", Cond::L),
    ("ge", Cond::Ge),
    ("le", Cond::Le),
    ("g", Cond::G),
];
const SIZES: [(&str, Size); 4] = [
    ("byte", Size::B),
    ("word", Size::W),
    ("dword", Size::D),
    ("qword", Size::Q),
];

fn reg_name(r: Reg, size: Size) -> &'static str {
    if r.is_high_byte() {
        return match r {
            Reg::Ah => "ah",
            Reg::Ch => "ch",
            Reg::Dh => "dh",
            _ => "bh",
        };
    }
    let i = r.num() as usize;
    match size {
        Size::B => R8N[i],
        Size::W => R16[i],
        Size::D => R32[i],
        Size::Q => R64[i],
    }
}

fn size_name(s: Size) -> &'static str {
    SIZES[s as usize].0
}

/// Résout un nom de registre en `(registre, taille)`.
fn reg_of(name: &str) -> Option<(Reg, Size)> {
    let n = name.trim();
    // Octets hauts : mêmes numéros que `spl`/`bpl`/`sil`/`dil`, distingués par
    // l'absence de REX — ils ont donc leurs propres variantes.
    if let Some(r) = match n {
        "ah" => Some(Reg::Ah),
        "ch" => Some(Reg::Ch),
        "dh" => Some(Reg::Dh),
        "bh" => Some(Reg::Bh),
        _ => None,
    } {
        return Some((r, Size::B));
    }
    for (tbl, sz) in [
        (&R64, Size::Q),
        (&R32, Size::D),
        (&R16, Size::W),
        (&R8N, Size::B),
    ] {
        if let Some(i) = tbl.iter().position(|x| *x == n) {
            return Some((REGS[i], sz));
        }
    }
    None
}

fn cond_name(c: Cond) -> &'static str {
    CONDS[c.code() as usize].0
}

fn fmt_disp(d: i32) -> String {
    if d == 0 {
        String::new()
    } else if d > 0 {
        format!("+{d:#x}")
    } else {
        format!("-{:#x}", d.unsigned_abs())
    }
}

fn mem_text(m: Mem) -> String {
    if let Some(t) = m.rip {
        return format!("[rip {t:#x}]");
    }
    let mut s = String::new();
    if let Some(sg) = m.seg {
        s.push_str(match sg {
            Seg::Fs => "fs:",
            Seg::Gs => "gs:",
        });
    }
    s.push('[');
    if let Some(b) = m.base {
        s.push_str(reg_name(b, Size::Q));
    }
    if let Some((i, sc)) = m.index {
        if m.base.is_some() {
            s.push('+');
        }
        s.push_str(reg_name(i, Size::Q));
        s.push_str(&format!("*{sc}"));
    }
    if m.base.is_none() && m.index.is_none() {
        // Adresse absolue (`gs:[58h]`) : pas de terme précédent, donc pas de
        // signe de liaison — `[+0x58]` ne se relirait pas.
        s.push_str(&format!("{:#x}", m.disp));
    } else if m.disp == 0 && m.disp_explicite && !m.disp32 {
        // Le déplacement nul est écrit pour que la relecture le retrouve :
        // `[rbx]` s'encode `mod=00`, `[rbx+0x0]` garde le `disp8` de l'original.
        s.push_str("+0x0");
    } else if m.disp == 0 && m.disp32 {
        // Meme motif, pour la forme longue : `[rbx+0x0l]` garde le `disp32`
        // nul de l'original, distingue de `[rbx+0x0]` (`disp8`).
        s.push_str("+0x0l");
    } else {
        s.push_str(&fmt_disp(m.disp));
        if m.disp32 {
            // `l` marque la forme longue : `disp32` au lieu de `disp8`
            // alors que la valeur tiendrait sur un octet.
            s.push('l');
        }
    }
    s.push(']');
    s
}

fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim();
    let (neg, t) = t.strip_prefix('-').map_or((false, t), |r| (true, r));
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else {
        t.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

fn parse_u64(s: &str) -> Option<u64> {
    let t = s.trim();
    t.strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .and_then(|h| u64::from_str_radix(h, 16).ok())
        .or_else(|| t.parse::<u64>().ok())
}

/// Immédiat 32 bits : accepte les deux lectures (`-1` et `0xffffffff`) et rend
/// le motif de bits. MSVC écrit les deux selon le contexte ; la source doit
/// pouvoir relire ce qu'elle a écrit.
fn as_imm32(v: i64) -> Option<i32> {
    if let Ok(x) = i32::try_from(v) {
        return Some(x);
    }
    u32::try_from(v).ok().map(|x| x as i32)
}

fn parse_mem(s: &str) -> Option<Mem> {
    // Préfixe de segment explicite : `gs:[58h]` (TLS Windows x64).
    let t = s.trim();
    let (seg, t) = if let Some(r) = t.strip_prefix("fs:") {
        (Some(Seg::Fs), r)
    } else if let Some(r) = t.strip_prefix("gs:") {
        (Some(Seg::Gs), r)
    } else {
        (None, t)
    };
    if seg.is_some() {
        let mut m = parse_mem(t)?;
        m.seg = seg;
        return Some(m);
    }
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().starts_with("abs") {
        return None; // traité par `mov` : forme accumulateur A0..A3
    }
    if let Some(rest) = inner.trim().strip_prefix("rip") {
        return Some(Mem::rip(parse_u64(rest.trim())?));
    }
    let mut m = Mem::default();
    let mut rest = inner;
    let mut first = true;
    while !rest.is_empty() {
        let (sign, start) = if first {
            (1i32, 0usize)
        } else {
            match rest.as_bytes()[0] {
                b'+' => (1, 1),
                b'-' => (-1, 1),
                _ => return None,
            }
        };
        let body = &rest[start..];
        let stop = body.find(['+', '-']).unwrap_or(body.len());
        let term = &body[..stop];
        rest = &body[stop..];
        first = false;

        if let Some((r, scale)) = term.split_once('*') {
            m.index = Some((reg_of(r)?.0, scale.trim().parse::<u8>().ok()?));
        } else if let Some((reg, _)) = reg_of(term) {
            if m.base.is_none() {
                m.base = Some(reg);
            } else {
                m.index = Some((reg, 1));
            }
        } else {
            let (term, longue) = term.strip_suffix('l').map_or((term, false), |t| (t, true));
            m.disp = i32::try_from(parse_int(term)? * i64::from(sign)).ok()?;
            // Un déplacement nul écrit noir sur blanc demande le `disp8` — ou,
            // avec le suffixe `l`, le `disp32` (`+0x0l`).
            m.disp_explicite = m.disp == 0 && !longue;
            m.disp32 = longue;
        }
    }
    Some(m)
}

/// Vrai si l'opérande est mémoire — crochet, éventuellement précédé d'un
/// préfixe de segment (`gs:[58h]`).
fn starts_mem(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with('[') || t.starts_with("fs:[") || t.starts_with("gs:[")
}

/// Une taille explicite préfixant un opérande mémoire (`dword [rcx]`).
fn split_sized_mem(s: &str) -> Option<(Size, Mem)> {
    let t = s.trim();
    for (name, sz) in SIZES {
        if let Some(rest) = t.strip_prefix(name) {
            return Some((sz, parse_mem(rest)?));
        }
    }
    None
}

/// Table des mnemoniques SSE supportes.
const SSES: [(&str, SseOp); 105] = [
    ("movaps", SseOp::Movaps),
    ("movapd", SseOp::Movapd),
    ("movups", SseOp::Movups),
    ("movupd", SseOp::Movupd),
    ("movss", SseOp::Movss),
    ("movsd", SseOp::Movsd),
    ("movdqa", SseOp::Movdqa),
    ("movdqu", SseOp::Movdqu),
    ("xorps", SseOp::Xorps),
    ("xorpd", SseOp::Xorpd),
    ("andps", SseOp::Andps),
    ("andpd", SseOp::Andpd),
    ("andnps", SseOp::Andnps),
    ("orps", SseOp::Orps),
    ("addps", SseOp::Addps),
    ("addss", SseOp::Addss),
    ("addsd", SseOp::Addsd),
    ("subps", SseOp::Subps),
    ("subss", SseOp::Subss),
    ("subsd", SseOp::Subsd),
    ("mulps", SseOp::Mulps),
    ("mulss", SseOp::Mulss),
    ("mulsd", SseOp::Mulsd),
    ("divps", SseOp::Divps),
    ("divss", SseOp::Divss),
    ("divsd", SseOp::Divsd),
    ("minps", SseOp::Minps),
    ("minss", SseOp::Minss),
    ("maxps", SseOp::Maxps),
    ("maxss", SseOp::Maxss),
    ("sqrtps", SseOp::Sqrtps),
    ("sqrtss", SseOp::Sqrtss),
    ("comiss", SseOp::Comiss),
    ("comisd", SseOp::Comisd),
    ("ucomiss", SseOp::Ucomiss),
    ("ucomisd", SseOp::Ucomisd),
    ("unpcklps", SseOp::Unpcklps),
    ("unpckhps", SseOp::Unpckhps),
    ("cvtss2sd", SseOp::Cvtss2sd),
    ("cvtsd2ss", SseOp::Cvtsd2ss),
    ("rcpss", SseOp::Rcpss),
    ("rsqrtss", SseOp::Rsqrtss),
    ("shufps", SseOp::Shufps),
    ("shufpd", SseOp::Shufpd),
    ("pshufd", SseOp::Pshufd),
    ("movlhps", SseOp::Movlhps),
    ("movhlps", SseOp::Movhlps),
    ("movlps", SseOp::Movlps),
    ("movhps", SseOp::Movhps),
    ("insertps", SseOp::Insertps),
    ("blendps", SseOp::Blendps),
    ("cvtdq2ps", SseOp::Cvtdq2ps),
    ("cvtps2dq", SseOp::Cvtps2dq),
    ("cvttps2dq", SseOp::Cvttps2dq),
    ("cvtps2pd", SseOp::Cvtps2pd),
    ("cvtpd2ps", SseOp::Cvtpd2ps),
    ("cvtdq2pd", SseOp::Cvtdq2pd),
    ("haddps", SseOp::Haddps),
    ("hsubps", SseOp::Hsubps),
    ("cmpps", SseOp::Cmpps),
    ("cmpss", SseOp::Cmpss),
    ("pxor", SseOp::Pxor),
    ("por", SseOp::Por),
    ("pand", SseOp::Pand),
    ("unpcklpd", SseOp::Unpcklpd),
    ("cmppd", SseOp::Cmppd),
    ("cmpsd_x", SseOp::Cmpsd),
    ("rcpps", SseOp::Rcpps),
    ("rsqrtps", SseOp::Rsqrtps),
    ("punpckldq", SseOp::Punpckldq),
    ("punpckhdq", SseOp::Punpckhdq),
    ("paddw", SseOp::Paddw),
    ("paddd", SseOp::Paddd),
    ("psubw", SseOp::Psubw),
    ("psubd", SseOp::Psubd),
    ("pminsw", SseOp::Pminsw),
    ("pmaxsw", SseOp::Pmaxsw),
    ("punpcklqdq", SseOp::Punpcklqdq),
    ("punpckhqdq", SseOp::Punpckhqdq),
    ("psadbw", SseOp::Psadbw),
    ("pmullw", SseOp::Pmullw),
    ("pavgb", SseOp::Pavgb),
    ("pavgw", SseOp::Pavgw),
    ("packuswb", SseOp::Packuswb),
    ("packsswb", SseOp::Packsswb),
    ("packssdw", SseOp::Packssdw),
    ("punpcklbw", SseOp::Punpcklbw),
    ("punpcklwd", SseOp::Punpcklwd),
    ("punpckhbw", SseOp::Punpckhbw),
    ("punpckhwd", SseOp::Punpckhwd),
    ("pcmpeqb", SseOp::Pcmpeqb),
    ("pcmpeqw", SseOp::Pcmpeqw),
    ("pcmpeqd", SseOp::Pcmpeqd),
    ("pcmpgtb", SseOp::Pcmpgtb),
    ("pcmpgtw", SseOp::Pcmpgtw),
    ("pcmpgtd", SseOp::Pcmpgtd),
    ("paddb", SseOp::Paddb),
    ("psubb", SseOp::Psubb),
    ("paddusb", SseOp::Paddusb),
    ("psubusb", SseOp::Psubusb),
    ("pmaddwd", SseOp::Pmaddwd),
    ("pmulhw", SseOp::Pmulhw),
    ("pshuflw", SseOp::Pshuflw),
    ("pshufhw", SseOp::Pshufhw),
    ("pmuludq", SseOp::Pmuludq),
];

/// Table des opérations VEX.
const VEXES: [(&str, VexOp); 17] = [
    ("vmovaps", VexOp::Vmovaps),
    ("vmovaps_st", VexOp::VmovapsStore),
    ("vmovups", VexOp::Vmovups),
    ("vmovups_st", VexOp::VmovupsStore),
    ("vxorps", VexOp::Vxorps),
    ("vaddps", VexOp::Vaddps),
    ("vmulps", VexOp::Vmulps),
    ("vsubps", VexOp::Vsubps),
    ("vpermilps_i", VexOp::VpermilpsImm),
    ("vpermilps", VexOp::Vpermilps),
    ("vfmadd231ps", VexOp::Vfmadd231ps),
    ("vfmadd213ps", VexOp::Vfmadd213ps),
    ("vfmadd132ps", VexOp::Vfmadd132ps),
    ("vmovdqu", VexOp::Vmovdqu),
    ("vmovdqu_st", VexOp::VmovdquStore),
    ("vmovdqa", VexOp::Vmovdqa),
    ("vmovdqa_st", VexOp::VmovdqaStore),
];

fn vex_name(o: VexOp) -> &'static str {
    VEXES
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("vmovaps", |(n, _)| *n)
}

/// Vrai si la forme n'a que deux opérandes visibles (pas de registre `vvvv`).
fn is_two_operand(o: VexOp) -> bool {
    matches!(
        o,
        VexOp::Vmovaps
            | VexOp::VmovapsStore
            | VexOp::Vmovups
            | VexOp::VmovupsStore
            | VexOp::Vmovdqu
            | VexOp::VmovdquStore
            | VexOp::Vmovdqa
            | VexOp::VmovdqaStore
    )
}

/// Nom de l'accumulateur pour une taille (`al`/`ax`/`eax`/`rax`).
fn acc_name(s: Size) -> &'static str {
    match s {
        Size::B => "al",
        Size::W => "ax",
        Size::D => "eax",
        Size::Q => "rax",
    }
}

/// Table des chaînes répétées.
const REPS: [(&str, RepOp, Size); 8] = [
    ("rep_stosb", RepOp::Stos, Size::B),
    ("rep_stosw", RepOp::Stos, Size::W),
    ("rep_stosd", RepOp::Stos, Size::D),
    ("rep_stosq", RepOp::Stos, Size::Q),
    ("rep_movsb", RepOp::Movs, Size::B),
    ("rep_movsw", RepOp::Movs, Size::W),
    ("rep_movsd", RepOp::Movs, Size::D),
    ("rep_movsq", RepOp::Movs, Size::Q),
];

/// Table des opérations sur chaîne **sans** `rep`.
const STRS: [(&str, RepOp, Size); 8] = [
    ("stosb", RepOp::Stos, Size::B),
    ("stosw", RepOp::Stos, Size::W),
    ("stosd", RepOp::Stos, Size::D),
    ("stosq", RepOp::Stos, Size::Q),
    ("movsb", RepOp::Movs, Size::B),
    ("movsw", RepOp::Movs, Size::W),
    ("movsd_str", RepOp::Movs, Size::D),
    ("movsq", RepOp::Movs, Size::Q),
];

/// Table des indications de préchargement.
const PREFETCHES: [(&str, u8); 4] = [
    ("prefetchnta", 0),
    ("prefetcht0", 1),
    ("prefetcht1", 2),
    ("prefetcht2", 3),
];

/// Table des décalages vectoriels à immédiat.
const SSE_SHIFTS: [(&str, SseShiftOp); 10] = [
    ("psrlw", SseShiftOp::Psrlw),
    ("psraw", SseShiftOp::Psraw),
    ("psllw", SseShiftOp::Psllw),
    ("psrld", SseShiftOp::Psrld),
    ("psrad", SseShiftOp::Psrad),
    ("pslld", SseShiftOp::Pslld),
    ("psrlq", SseShiftOp::Psrlq),
    ("psrldq", SseShiftOp::Psrldq),
    ("psllq", SseShiftOp::Psllq),
    ("pslldq", SseShiftOp::Pslldq),
];

fn sse_shift_name(o: SseShiftOp) -> &'static str {
    SSE_SHIFTS
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("psrldq", |(n, _)| *n)
}

/// Table des extractions de masque de signes.
const SSE_MASKS: [(&str, SseMaskOp); 3] = [
    ("movmskps", SseMaskOp::Movmskps),
    ("movmskpd", SseMaskOp::Movmskpd),
    ("pmovmskb", SseMaskOp::Pmovmskb),
];

fn sse_mask_name(o: SseMaskOp) -> &'static str {
    SSE_MASKS
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("movmskps", |(n, _)| *n)
}

/// Mnémonique d'une opération de décalage/rotation.
fn shiftop_name(o: ShiftOp) -> &'static str {
    match o {
        ShiftOp::Shl => "shl",
        ShiftOp::Shr => "shr",
        ShiftOp::Sar => "sar",
        ShiftOp::Rol => "rol",
        ShiftOp::Ror => "ror",
    }
}

fn sse_name(o: SseOp) -> &'static str {
    SSES.iter()
        .find(|(_, x)| *x == o)
        .map_or("movaps", |(n, _)| *n)
}

fn xmm_text(x: Xmm) -> String {
    format!("xmm{}", x.0)
}

fn xmm_of(s: &str) -> Option<Xmm> {
    let n = s.trim().strip_prefix("xmm")?.parse::<u8>().ok()?;
    (n < 16).then_some(Xmm(n))
}

fn xmmrm_text(rm: XmmRm) -> String {
    match rm {
        XmmRm::X(x) => xmm_text(x),
        XmmRm::M(m) => mem_text(m),
    }
}

fn parse_xmmrm(s: &str) -> Option<XmmRm> {
    if s.trim_start().starts_with('[') {
        return Some(XmmRm::M(parse_mem(s)?));
    }
    Some(XmmRm::X(xmm_of(s)?))
}

const CVTS: [(&str, CvtOp); 6] = [
    ("cvtsi2ss", CvtOp::Cvtsi2ss),
    ("cvtsi2sd", CvtOp::Cvtsi2sd),
    ("cvttss2si", CvtOp::Cvttss2si),
    ("cvttsd2si", CvtOp::Cvttsd2si),
    ("cvtss2si", CvtOp::Cvtss2si),
    ("cvtsd2si", CvtOp::Cvtsd2si),
];

fn cvt_name(o: CvtOp) -> &'static str {
    CVTS.iter()
        .find(|(_, x)| *x == o)
        .map_or("cvtsi2ss", |(n, _)| *n)
}

const NOOPS: [(&str, NoOp); 5] = [
    ("cwde", NoOp::Cwde),
    ("cdqe", NoOp::Cdqe),
    ("cdq", NoOp::Cdq),
    ("cqo", NoOp::Cqo),
    ("leave", NoOp::Leave),
];

const BITOPS: [(&str, BitOp); 4] = [
    ("bt", BitOp::Bt),
    ("bts", BitOp::Bts),
    ("btr", BitOp::Btr),
    ("btc", BitOp::Btc),
];

fn noop_name(o: NoOp) -> &'static str {
    NOOPS
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("cdq", |(n, _)| *n)
}

fn bitop_name(o: BitOp) -> &'static str {
    BITOPS
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("bt", |(n, _)| *n)
}

const UNOPS: [(&str, UnOp); 11] = [
    ("inc", UnOp::Inc),
    ("dec", UnOp::Dec),
    ("calli", UnOp::CallInd),
    ("jmpi", UnOp::JmpInd),
    ("pushm", UnOp::PushRm),
    ("not", UnOp::Not),
    ("neg", UnOp::Neg),
    ("mul", UnOp::Mul),
    ("imul1", UnOp::Imul1),
    ("div", UnOp::Div),
    ("idiv", UnOp::Idiv),
];

fn unop_name(o: UnOp) -> &'static str {
    UNOPS
        .iter()
        .find(|(_, x)| *x == o)
        .map_or("inc", |(n, _)| *n)
}

/// Rend un operande `r/m` : registre nu, ou memoire prefixee de sa taille.
fn rm_text(rm: Rm, size: Size) -> String {
    match rm {
        Rm::R(r) => reg_name(r, size).to_string(),
        Rm::M(m) => format!("{} {}", size_name(size), mem_text(m)),
    }
}

/// Analyse un operande `r/m`, en rendant la taille quand elle est explicite.
fn parse_rm(s: &str) -> Option<(Rm, Option<Size>)> {
    if let Some((sz, m)) = split_sized_mem(s) {
        return Some((Rm::M(m), Some(sz)));
    }
    if starts_mem(s) {
        return Some((Rm::M(parse_mem(s)?), None));
    }
    let (r, sz) = reg_of(s)?;
    Some((Rm::R(r), Some(sz)))
}

impl Insn {
    /// Rend l'instruction en syntaxe Intel canonique du dialecte.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn to_text(self) -> String {
        match self {
            Self::Ret => "ret".to_string(),
            Self::RetImm(n) => format!("ret {n:#x}"),
            Self::Int3 => "int3".to_string(),
            Self::Nop(n) => format!("nop {n}"),
            Self::Push(r, x) => {
                format!("push{} {}", if x { ".r" } else { "" }, reg_name(r, Size::Q))
            }
            Self::Pop(r, x) => format!("pop{} {}", if x { ".r" } else { "" }, reg_name(r, Size::Q)),
            Self::MovRegImm8(r, i) => format!("mov {}, {i:#x}", reg_name(r, Size::B)),
            Self::MovRegImm32(r, i) => format!("mov {}, {i:#x}", reg_name(r, Size::D)),
            Self::MovRegImm64(r, i) => format!("movabs {}, {i:#x}", reg_name(r, Size::Q)),
            Self::MovRR(s, a, b) => format!("mov {}, {}", reg_name(a, s), reg_name(b, s)),
            Self::MovRRm(s, a, b) => format!("mov.d {}, {}", reg_name(a, s), reg_name(b, s)),
            Self::Load(s, r, m) => format!("mov {}, {}", reg_name(r, s), mem_text(m)),
            Self::Store(s, m, r) => format!("mov {}, {}", mem_text(m), reg_name(r, s)),
            Self::StoreImm32(s, m, i) => {
                format!("mov {} {}, {:#x}", size_name(s), mem_text(m), i as u32)
            }
            Self::Lea(r, m) => format!("lea {}, {}", reg_name(r, Size::Q), mem_text(m)),
            Self::AluRR(op, s, a, b) => format!(
                "{} {}, {}",
                ALUS[op.digit() as usize].0,
                reg_name(a, s),
                reg_name(b, s)
            ),
            Self::AluRRm(op, s, a, b) => format!(
                "{}.d {}, {}",
                ALUS[op.digit() as usize].0,
                reg_name(a, s),
                reg_name(b, s)
            ),
            Self::AluRM(op, s, r, m) => format!(
                "{} {}, {}",
                ALUS[op.digit() as usize].0,
                reg_name(r, s),
                mem_text(m)
            ),
            Self::AluMR(op, s, m, r) => format!(
                "{} {}, {}",
                ALUS[op.digit() as usize].0,
                mem_text(m),
                reg_name(r, s)
            ),
            Self::AluRI(op, s, r, i, w) => format!(
                "{}{} {}, {:#x}",
                ALUS[op.digit() as usize].0,
                if w { ".w" } else { "" },
                reg_name(r, s),
                i as u32
            ),
            Self::TestRR(s, a, b) => format!("test {}, {}", reg_name(a, s), reg_name(b, s)),
            Self::Shift(op, s, r, i) => {
                let n = shiftop_name(op);
                format!("{n} {}, {i:#x}", reg_name(r, s))
            }
            Self::MovzxR(src, d, s) => {
                format!("movzx {}, {}", reg_name(d, Size::D), reg_name(s, src))
            }
            Self::MovzxM(src, d, m) => format!(
                "movzx {}, {} {}",
                reg_name(d, Size::D),
                size_name(src),
                mem_text(m)
            ),
            Self::Movsxd(d, s) => {
                format!("movsxd {}, {}", reg_name(d, Size::Q), reg_name(s, Size::D))
            }
            Self::Setcc(c, r) => format!("set{} {}", cond_name(c), reg_name(r, Size::B)),
            Self::IncMem32(m) => format!("inc {} {}", size_name(Size::D), mem_text(m)),
            Self::JmpReg(r, x) => {
                format!("jmp{} {}", if x { ".r" } else { "" }, reg_name(r, Size::Q))
            }
            Self::Call(t) => format!("call {t:#x}"),
            Self::Jmp(t, short) => format!("jmp{} {t:#x}", if short { ".s" } else { "" }),
            Self::Jcc(c, t, short) => {
                format!("j{}{} {t:#x}", cond_name(c), if short { ".s" } else { "" })
            }
            Self::AluI(op, s, rm, i, w) => format!(
                "{}{} {}, {:#x}",
                ALUS[op.digit() as usize].0,
                if w { ".w" } else { "" },
                rm_text(rm, s),
                i as u32
            ),
            Self::MovI(s, rm, i) => format!("mov {}, {:#x}", rm_text(rm, s), i as u32),
            Self::Test(s, rm, r) => format!("test {}, {}", rm_text(rm, s), reg_name(r, s)),
            Self::TestI(s, rm, i) => format!("test {}, {:#x}", rm_text(rm, s), i as u32),
            Self::Bswap(s, r) => format!("bswap {}", reg_name(r, s)),
            Self::PushfPopf(pop) => (if pop { "popfq" } else { "pushfq" }).to_string(),
            Self::PushImm(v) => format!("pushi {v:#x}"),
            Self::StringOp(op, s) => {
                let n = match (op, s) {
                    (RepOp::Stos, Size::B) => "stosb",
                    (RepOp::Stos, Size::W) => "stosw",
                    (RepOp::Stos, Size::D) => "stosd",
                    (RepOp::Stos, Size::Q) => "stosq",
                    (RepOp::Movs, Size::B) => "movsb",
                    (RepOp::Movs, Size::W) => "movsw",
                    (RepOp::Movs, Size::D) => "movsd_str",
                    (RepOp::Movs, Size::Q) => "movsq",
                };
                n.to_string()
            }
            Self::LockCmpxchg(s, m, r) => {
                format!(
                    "lock cmpxchg {} {}, {}",
                    size_name(s),
                    mem_text(m),
                    reg_name(r, s)
                )
            }
            Self::BitScan(bsr, s, r, rm) => {
                let n = if bsr { "bsr" } else { "bsf" };
                format!("{n} {}, {}", reg_name(r, s), rm_text(rm, s))
            }
            Self::LockXadd(s, m, r) => {
                format!(
                    "lock xadd {} {}, {}",
                    size_name(s),
                    mem_text(m),
                    reg_name(r, s)
                )
            }
            Self::Vex(op, dst, src1, src2, imm) => {
                let n = vex_name(op);
                let s2 = xmmrm_text(src2);
                match (op.is_store(), imm) {
                    // Forme « store » : la mémoire est la destination.
                    (true, _) => format!("{n} {s2}, {}", xmm_text(dst)),
                    (false, Some(v)) => {
                        format!("{n} {}, {s2}, {v:#x}", xmm_text(dst))
                    }
                    // Sans `vvvv` utile, la forme a deux opérandes visibles.
                    (false, None) if src1.0 == 0 && !op.has_imm() && is_two_operand(op) => {
                        format!("{n} {}, {s2}", xmm_text(dst))
                    }
                    (false, None) => {
                        format!("{n} {}, {}, {s2}", xmm_text(dst), xmm_text(src1))
                    }
                }
            }
            Self::Prefetch(h, m) => {
                let n = match h {
                    0 => "prefetchnta",
                    1 => "prefetcht0",
                    2 => "prefetcht1",
                    _ => "prefetcht2",
                };
                format!("{n} {}", mem_text(m))
            }
            Self::RepString(op, s) => {
                let n = match (op, s) {
                    (RepOp::Stos, Size::B) => "rep_stosb",
                    (RepOp::Stos, Size::W) => "rep_stosw",
                    (RepOp::Stos, Size::D) => "rep_stosd",
                    (RepOp::Stos, Size::Q) => "rep_stosq",
                    (RepOp::Movs, Size::B) => "rep_movsb",
                    (RepOp::Movs, Size::W) => "rep_movsw",
                    (RepOp::Movs, Size::D) => "rep_movsd",
                    (RepOp::Movs, Size::Q) => "rep_movsq",
                };
                n.to_string()
            }
            Self::XchgAcc(s, r) => format!("xchg {}, {}", acc_name(s), reg_name(r, s)),
            Self::XchgMem(s, m, r) => {
                format!("xchg {} {}, {}", size_name(s), mem_text(m), reg_name(r, s))
            }
            Self::SseShift(op, x, i) => {
                format!("{} {}, {i:#x}", sse_shift_name(op), xmm_text(x))
            }
            Self::SseMovmsk(op, r, x) => {
                format!(
                    "{} {}, {}",
                    sse_mask_name(op),
                    reg_name(r, Size::D),
                    xmm_text(x)
                )
            }
            Self::Un(op, s, rm) => format!("{} {}", unop_name(op), rm_text(rm, s)),
            Self::LockUn(op, s, rm) => {
                format!("lock {} {}", unop_name(op), rm_text(rm, s))
            }
            Self::Imul(s, r, rm) => format!("imul {}, {}", reg_name(r, s), rm_text(rm, s)),
            Self::ImulI(s, r, rm, i) => format!(
                "imul {}, {}, {:#x}",
                reg_name(r, s),
                rm_text(rm, s),
                i as u32
            ),
            Self::Movsx(src, dst, r, rm) => {
                format!("movsx {}, {}", reg_name(r, dst), rm_text(rm, src))
            }
            Self::LeaD(r, m) => format!("lea {}, {}", reg_name(r, Size::D), mem_text(m)),
            Self::Sse(op, d, s) => {
                format!("{} {}, {}", sse_name(op), xmm_text(d), xmmrm_text(s))
            }
            Self::MovMoffs(s, a, store) => {
                let acc = reg_name(Reg::Rax, s);
                if store {
                    format!("mov [abs {a:#x}], {acc}")
                } else {
                    format!("mov {acc}, [abs {a:#x}]")
                }
            }
            Self::SseStore(op, m, s) => {
                format!("{} {}, {}", sse_name(op), mem_text(m), xmm_text(s))
            }
            Self::Cmov(c, s, r, rm) => {
                format!(
                    "cmov{} {}, {}",
                    cond_name(c),
                    reg_name(r, s),
                    rm_text(rm, s)
                )
            }
            Self::SseI(op, d, s, i) => format!(
                "{} {}, {}, {i:#x}",
                sse_name(op),
                xmm_text(d),
                xmmrm_text(s)
            ),
            Self::CvtToXmm(op, d, s, sz) => {
                format!("{} {}, {}", cvt_name(op), xmm_text(d), rm_text(s, sz))
            }
            Self::CvtToReg(op, d, s, sz) => {
                format!("{} {}, {}", cvt_name(op), reg_name(d, sz), xmmrm_text(s))
            }
            Self::MovdToXmm(d, s, sz) => format!(
                "{} {}, {}",
                if sz == Size::Q { "movq" } else { "movd" },
                xmm_text(d),
                rm_text(s, sz)
            ),
            Self::MovdToRm(d, s, sz) => format!(
                "{} {}, {}",
                if sz == Size::Q { "movq" } else { "movd" },
                rm_text(d, sz),
                xmm_text(s)
            ),
            Self::MovsxdRm(d, s) => {
                format!("movsxd {}, {}", reg_name(d, Size::Q), rm_text(s, Size::D))
            }
            Self::MovzxRm(src, dst, r, rm) => {
                format!("movzx {}, {}", reg_name(r, dst), rm_text(rm, src))
            }
            Self::MovsxRm(src, dst, r, rm) => {
                format!("movsx {}, {}", reg_name(r, dst), rm_text(rm, src))
            }
            Self::NoOperand(o) => noop_name(o).to_string(),
            Self::SetccRm(c, rm) => format!("set{} {}", cond_name(c), rm_text(rm, Size::B)),
            Self::Shift1(op, s, rm) => {
                let n = shiftop_name(op);
                format!("{n} {}, 1", rm_text(rm, s))
            }
            Self::ShiftCl(op, s, rm) => {
                let n = shiftop_name(op);
                format!("{n} {}, cl", rm_text(rm, s))
            }
            Self::BitRm(op, s, rm, r) => {
                format!("{} {}, {}", bitop_name(op), rm_text(rm, s), reg_name(r, s))
            }
            Self::BitImm(op, s, rm, i) => {
                format!("{} {}, {i:#x}", bitop_name(op), rm_text(rm, s))
            }
        }
    }
}

/// Analyse une instruction du dialecte.
///
/// # Erreurs
/// Retourne une erreur si la ligne n'appartient pas au dialecte supporté.
#[allow(clippy::too_many_lines)]
pub fn parse_insn(line: &str) -> Result<Insn, ParseError> {
    let line = line.trim();
    // `lock` est un préfixe de ligne, pas un mnémonique : on analyse
    // l'instruction qu'il préfixe, puis on l'enveloppe.
    if let Some(rest) = line.strip_prefix("lock ") {
        let rest = rest.trim();
        if let Some(x) = rest.strip_prefix("cmpxchg ") {
            let bad = || ParseError(format!("`lock cmpxchg` mal formé : `{line}`"));
            let (d, sx) = x.split_once(',').ok_or_else(bad)?;
            let (sz, m) = split_sized_mem(d).ok_or_else(bad)?;
            let (r, rsz) = reg_of(sx).ok_or_else(bad)?;
            (sz == rsz).then_some(()).ok_or_else(bad)?;
            return Ok(Insn::LockCmpxchg(sz, m, r));
        }
        if let Some(x) = rest.strip_prefix("xadd ") {
            let (d, sx) = x
                .split_once(',')
                .ok_or_else(|| ParseError(format!("`lock xadd` mal formé : `{line}`")))?;
            let bad = || ParseError(format!("`lock xadd` mal formé : `{line}`"));
            let (sz, m) = split_sized_mem(d).ok_or_else(bad)?;
            let (r, rsz) = reg_of(sx).ok_or_else(bad)?;
            (sz == rsz).then_some(()).ok_or_else(bad)?;
            return Ok(Insn::LockXadd(sz, m, r));
        }
        return match parse_insn(rest)? {
            Insn::Un(op, sz, rm) => Ok(Insn::LockUn(op, sz, rm)),
            _ => Err(ParseError(format!("`lock` non supporté ici : `{line}`"))),
        };
    }
    let (mnem_raw, args) = line.split_once(' ').unwrap_or((line, ""));
    let args = args.trim();
    let err = || ParseError(format!("instruction non supportée : `{line}`"));
    // Deux suffixes distincts, a ne pas confondre :
    //   `.s` = branchement en forme courte (rel8)
    //   `.w` = immediat en forme longue (81 /n id la ou 83 /n ib suffirait)
    let (mnem, short) = mnem_raw
        .strip_suffix(".s")
        .map_or((mnem_raw, false), |m| (m, true));
    let (mnem, wide) = mnem.strip_suffix(".w").map_or((mnem, false), |m| (m, true));
    //   `.r` = préfixe REX nul explicite (`40 53` au lieu de `53`)
    let (mnem, rexp) = mnem.strip_suffix(".r").map_or((mnem, false), |m| (m, true));
    //   `.d` = direction inverse d'une forme registre/registre : `op*8+1`
    //          (r/m ← registre) au lieu de `op*8+3`. Les deux calculent la même
    //          chose ; seuls les octets diffèrent, et c'est l'original qui tranche.
    let (mnem, dir) = mnem.strip_suffix(".d").map_or((mnem, false), |m| (m, true));

    let two = || -> Result<(String, String), ParseError> {
        let (a, b) = args.split_once(',').ok_or_else(err)?;
        Ok((a.trim().to_string(), b.trim().to_string()))
    };

    // Sauts conditionnels : `je`, `jne.s`, …
    if let Some(rest) = mnem.strip_prefix('j')
        && let Some((_, c)) = CONDS.iter().find(|(n, _)| *n == rest)
    {
        return Ok(Insn::Jcc(*c, parse_u64(args).ok_or_else(err)?, short));
    }
    if let Some(rest) = mnem.strip_prefix("set")
        && let Some((_, c)) = CONDS.iter().find(|(n, _)| *n == rest)
    {
        return Ok(Insn::SetccRm(*c, parse_rm(args).ok_or_else(err)?.0));
    }
    if let Some((_, o)) = NOOPS.iter().find(|(n, _)| *n == mnem) {
        return Ok(Insn::NoOperand(*o));
    }
    if let Some((_, o)) = BITOPS.iter().find(|(n, _)| *n == mnem) {
        let (d, s) = two()?;
        let (rm, sz) = parse_rm(&d).ok_or_else(err)?;
        let sz = sz.ok_or_else(err)?;
        if let Some((r, _)) = reg_of(&s) {
            return Ok(Insn::BitRm(*o, sz, rm, r));
        }
        return Ok(Insn::BitImm(
            *o,
            sz,
            rm,
            u8::try_from(parse_int(&s).ok_or_else(err)?).map_err(|_| err())?,
        ));
    }
    if mnem == "movd" || mnem == "movq" {
        let sz = if mnem == "movq" { Size::Q } else { Size::D };
        let (d, s) = two()?;
        if let Some(x) = xmm_of(&d) {
            return Ok(Insn::MovdToXmm(x, parse_rm(&s).ok_or_else(err)?.0, sz));
        }
        return Ok(Insn::MovdToRm(
            parse_rm(&d).ok_or_else(err)?.0,
            xmm_of(&s).ok_or_else(err)?,
            sz,
        ));
    }
    if let Some(rest) = mnem.strip_prefix("cmov")
        && let Some((_, c)) = CONDS.iter().find(|(n, _)| *n == rest)
    {
        let (d, s) = two()?;
        let (r, sz) = reg_of(&d).ok_or_else(err)?;
        let (rm, _) = parse_rm(&s).ok_or_else(err)?;
        return Ok(Insn::Cmov(*c, sz, r, rm));
    }
    if let Some((_, op)) = CVTS.iter().find(|(n, _)| *n == mnem) {
        let (d, s) = two()?;
        if let Some(x) = xmm_of(&d) {
            let (rm, sz) = parse_rm(&s).ok_or_else(err)?;
            return Ok(Insn::CvtToXmm(*op, x, rm, sz.ok_or_else(err)?));
        }
        let (r, sz) = reg_of(&d).ok_or_else(err)?;
        return Ok(Insn::CvtToReg(*op, r, parse_xmmrm(&s).ok_or_else(err)?, sz));
    }
    if let Some((_, op)) = SSES.iter().find(|(n, _)| *n == mnem) {
        let parts: Vec<&str> = args.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            return Ok(Insn::SseI(
                *op,
                xmm_of(parts[0]).ok_or_else(err)?,
                parse_xmmrm(parts[1]).ok_or_else(err)?,
                u8::try_from(parse_int(parts[2]).ok_or_else(err)?).map_err(|_| err())?,
            ));
        }
        let (d, s) = two()?;
        // Destination memoire => forme « memoire <- registre ».
        if d.trim_start().starts_with('[') {
            return Ok(Insn::SseStore(
                *op,
                parse_mem(&d).ok_or_else(err)?,
                xmm_of(&s).ok_or_else(err)?,
            ));
        }
        return Ok(Insn::Sse(
            *op,
            xmm_of(&d).ok_or_else(err)?,
            parse_xmmrm(&s).ok_or_else(err)?,
        ));
    }
    if let Some((_, op)) = VEXES.iter().find(|(n, _)| *n == mnem) {
        let parts: alloc::vec::Vec<&str> = args.split(',').map(str::trim).collect();
        let zero = Xmm(0);
        return match (op.is_store(), op.has_imm(), parts.len()) {
            (true, _, 2) => Ok(Insn::Vex(
                *op,
                xmm_of(parts[1]).ok_or_else(err)?,
                zero,
                XmmRm::M(parse_mem(parts[0]).ok_or_else(err)?),
                None,
            )),
            (false, true, 3) => Ok(Insn::Vex(
                *op,
                xmm_of(parts[0]).ok_or_else(err)?,
                zero,
                parse_xmmrm(parts[1]).ok_or_else(err)?,
                Some(u8::try_from(parse_int(parts[2]).ok_or_else(err)?).map_err(|_| err())?),
            )),
            (false, false, 2) => Ok(Insn::Vex(
                *op,
                xmm_of(parts[0]).ok_or_else(err)?,
                zero,
                parse_xmmrm(parts[1]).ok_or_else(err)?,
                None,
            )),
            (false, false, 3) => Ok(Insn::Vex(
                *op,
                xmm_of(parts[0]).ok_or_else(err)?,
                xmm_of(parts[1]).ok_or_else(err)?,
                parse_xmmrm(parts[2]).ok_or_else(err)?,
                None,
            )),
            _ => Err(err()),
        };
    }
    if let Some((_, op, sz)) = REPS.iter().find(|(n, _, _)| *n == mnem) {
        return Ok(Insn::RepString(*op, *sz));
    }
    if let Some((_, op, sz)) = STRS.iter().find(|(n, _, _)| *n == mnem) {
        return Ok(Insn::StringOp(*op, *sz));
    }
    if mnem == "bswap" {
        let (r, sz) = reg_of(args).ok_or_else(err)?;
        return Ok(Insn::Bswap(sz, r));
    }
    if mnem == "pushfq" {
        return Ok(Insn::PushfPopf(false));
    }
    if mnem == "popfq" {
        return Ok(Insn::PushfPopf(true));
    }
    if mnem == "pushi" {
        return Ok(Insn::PushImm(
            as_imm32(parse_int(args).ok_or_else(err)?).ok_or_else(err)?,
        ));
    }
    if let Some((_, hint)) = PREFETCHES.iter().find(|(n, _)| *n == mnem) {
        return Ok(Insn::Prefetch(*hint, parse_mem(args).ok_or_else(err)?));
    }
    if let Some((_, op)) = SSE_SHIFTS.iter().find(|(n, _)| *n == mnem) {
        let (d, v) = two()?;
        return Ok(Insn::SseShift(
            *op,
            xmm_of(&d).ok_or_else(err)?,
            u8::try_from(parse_int(&v).ok_or_else(err)?).map_err(|_| err())?,
        ));
    }
    if let Some((_, op)) = SSE_MASKS.iter().find(|(n, _)| *n == mnem) {
        let (d, sx) = two()?;
        return Ok(Insn::SseMovmsk(
            *op,
            reg_of(&d).ok_or_else(err)?.0,
            xmm_of(&sx).ok_or_else(err)?,
        ));
    }
    if let Some((_, op)) = UNOPS.iter().find(|(n, _)| *n == mnem) {
        let (rm, sz) = parse_rm(args).ok_or_else(err)?;
        return Ok(Insn::Un(*op, sz.ok_or_else(err)?, rm));
    }
    // Groupe ALU générique. Le suffixe `.w` force la forme longue de l'immédiat.
    if let Some((_, op)) = ALUS.iter().find(|(n, _)| *n == mnem) {
        let (d, s) = two()?;
        if let Some((sz, m)) = split_sized_mem(&d) {
            // `<alu> dword [rcx], 0x5` — memoire avec taille explicite.
            if let Some((r, rsz)) = reg_of(&s) {
                if rsz != sz {
                    return Err(err());
                }
                return Ok(Insn::AluMR(*op, sz, m, r));
            }
            let v = parse_int(&s).ok_or_else(err)?;
            return Ok(Insn::AluI(
                *op,
                sz,
                Rm::M(m),
                as_imm32(v).ok_or_else(err)?,
                wide,
            ));
        }
        if d.starts_with('[') {
            let m = parse_mem(&d).ok_or_else(err)?;
            let (r, sz) = reg_of(&s).ok_or_else(err)?;
            return Ok(Insn::AluMR(*op, sz, m, r));
        }
        let (r, sz) = reg_of(&d).ok_or_else(err)?;
        if s.starts_with('[') {
            return Ok(Insn::AluRM(*op, sz, r, parse_mem(&s).ok_or_else(err)?));
        }
        if let Some((b, sz2)) = reg_of(&s) {
            if sz != sz2 {
                return Err(err());
            }
            return Ok(if dir {
                Insn::AluRRm(*op, sz, r, b)
            } else {
                Insn::AluRR(*op, sz, r, b)
            });
        }
        let v = parse_int(&s).ok_or_else(err)?;
        return Ok(Insn::AluRI(*op, sz, r, as_imm32(v).ok_or_else(err)?, wide));
    }

    match mnem {
        "ret" if args.is_empty() => Ok(Insn::Ret),
        "ret" => Ok(Insn::RetImm(
            u16::try_from(parse_int(args).ok_or_else(err)?).map_err(|_| err())?,
        )),
        "int3" => Ok(Insn::Int3),
        "nop" => Ok(Insn::Nop(
            u8::try_from(parse_int(args).ok_or_else(err)?).map_err(|_| err())?,
        )),
        "push" => Ok(Insn::Push(reg_of(args).ok_or_else(err)?.0, rexp)),
        "pop" => Ok(Insn::Pop(reg_of(args).ok_or_else(err)?.0, rexp)),
        "call" => Ok(Insn::Call(parse_u64(args).ok_or_else(err)?)),
        "jmp" => match reg_of(args) {
            Some((r, Size::Q)) => Ok(Insn::JmpReg(r, rexp)),
            _ => Ok(Insn::Jmp(parse_u64(args).ok_or_else(err)?, short)),
        },
        "test" => {
            let (d, s) = two()?;
            let (rm, dsz) = parse_rm(&d).ok_or_else(err)?;
            if let Some((r, rsz)) = reg_of(&s) {
                if let (Rm::R(a), Some(sz)) = (rm, dsz) {
                    if sz != rsz {
                        return Err(err());
                    }
                    return Ok(Insn::TestRR(sz, a, r));
                }
                return Ok(Insn::Test(rsz, rm, r));
            }
            let v = parse_int(&s).ok_or_else(err)?;
            Ok(Insn::TestI(
                dsz.ok_or_else(err)?,
                rm,
                as_imm32(v).ok_or_else(err)?,
            ))
        }
        "imul" => {
            let parts: Vec<&str> = args.split(',').map(str::trim).collect();
            let (r, sz) = reg_of(parts.first().ok_or_else(err)?).ok_or_else(err)?;
            let (rm, _) = parse_rm(parts.get(1).ok_or_else(err)?).ok_or_else(err)?;
            match parts.get(2) {
                None => Ok(Insn::Imul(sz, r, rm)),
                Some(v) => Ok(Insn::ImulI(
                    sz,
                    r,
                    rm,
                    as_imm32(parse_int(v).ok_or_else(err)?).ok_or_else(err)?,
                )),
            }
        }
        "bsf" | "bsr" => {
            let (d, sx) = two()?;
            let (r, sz) = reg_of(&d).ok_or_else(err)?;
            let (rm, rsz) = parse_rm(&sx).ok_or_else(err)?;
            (rsz.is_none() || rsz == Some(sz))
                .then_some(())
                .ok_or_else(err)?;
            Ok(Insn::BitScan(mnem == "bsr", sz, r, rm))
        }
        "xchg" => {
            let (d, sx) = two()?;
            if let Some((sz, m)) = split_sized_mem(&d) {
                let (r, rsz) = reg_of(&sx).ok_or_else(err)?;
                (sz == rsz).then_some(()).ok_or_else(err)?;
                return Ok(Insn::XchgMem(sz, m, r));
            }
            // Seule la forme accumulateur (`90+r`) est du dialecte : c'est
            // celle que MSVC emploie.
            let (r, sz) = reg_of(&sx).ok_or_else(err)?;
            (acc_name(sz) == d.trim()).then_some(()).ok_or_else(err)?;
            Ok(Insn::XchgAcc(sz, r))
        }
        "shl" | "shr" | "sar" | "rol" | "ror" => {
            let (d, s) = two()?;
            let op = match mnem {
                "shl" => ShiftOp::Shl,
                "shr" => ShiftOp::Shr,
                "rol" => ShiftOp::Rol,
                "ror" => ShiftOp::Ror,
                _ => ShiftOp::Sar,
            };
            let (rm, sz) = parse_rm(&d).ok_or_else(err)?;
            let sz = sz.ok_or_else(err)?;
            if s.trim() == "cl" {
                return Ok(Insn::ShiftCl(op, sz, rm));
            }
            if s.trim() == "1" {
                return Ok(Insn::Shift1(op, sz, rm));
            }
            let imm = u8::try_from(parse_int(&s).ok_or_else(err)?).map_err(|_| err())?;
            match rm {
                Rm::R(r) => Ok(Insn::Shift(op, sz, r, imm)),
                Rm::M(_) => Err(err()),
            }
        }
        "movzx" | "movsx" => {
            let (d, s) = two()?;
            let (dst, dsz) = reg_of(&d).ok_or_else(err)?;
            let (rm, ssz) = parse_rm(&s).ok_or_else(err)?;
            let ssz = ssz.ok_or_else(err)?;
            Ok(if mnem == "movzx" {
                Insn::MovzxRm(ssz, dsz, dst, rm)
            } else {
                Insn::MovsxRm(ssz, dsz, dst, rm)
            })
        }
        "movsxd" => {
            let (d, s) = two()?;
            Ok(Insn::MovsxdRm(
                reg_of(&d).ok_or_else(err)?.0,
                parse_rm(&s).ok_or_else(err)?.0,
            ))
        }
        "movabs" => {
            let (d, s) = two()?;
            Ok(Insn::MovRegImm64(
                reg_of(&d).ok_or_else(err)?.0,
                parse_u64(&s).ok_or_else(err)?,
            ))
        }
        "lea" => {
            let (d, s) = two()?;
            let (r, sz) = reg_of(&d).ok_or_else(err)?;
            let m = parse_mem(&s).ok_or_else(err)?;
            Ok(if sz == Size::D {
                Insn::LeaD(r, m)
            } else {
                Insn::Lea(r, m)
            })
        }
        "mov" => {
            let (d, s) = two()?;
            // Forme accumulateur à adresse absolue (`A0`..`A3`).
            let abs_of = |x: &str| -> Option<u64> {
                parse_u64(
                    x.trim()
                        .strip_prefix('[')?
                        .strip_suffix(']')?
                        .trim()
                        .strip_prefix("abs")?,
                )
            };
            if let Some(a) = abs_of(&d) {
                let (_, sz) = reg_of(&s).ok_or_else(err)?;
                return Ok(Insn::MovMoffs(sz, a, true));
            }
            if let Some(a) = abs_of(&s) {
                let (_, sz) = reg_of(&d).ok_or_else(err)?;
                return Ok(Insn::MovMoffs(sz, a, false));
            }
            if let Some((sz, m)) = split_sized_mem(&d) {
                let v = parse_int(&s).ok_or_else(err)?;
                return Ok(Insn::MovI(sz, Rm::M(m), as_imm32(v).ok_or_else(err)?));
            }
            if starts_mem(&d) {
                let m = parse_mem(&d).ok_or_else(err)?;
                let (r, sz) = reg_of(&s).ok_or_else(err)?;
                return Ok(Insn::Store(sz, m, r));
            }
            let (r, sz) = reg_of(&d).ok_or_else(err)?;
            if starts_mem(&s) {
                return Ok(Insn::Load(sz, r, parse_mem(&s).ok_or_else(err)?));
            }
            if let Some((b, sz2)) = reg_of(&s) {
                if sz != sz2 {
                    return Err(err());
                }
                return Ok(if dir {
                    Insn::MovRRm(sz, r, b)
                } else {
                    Insn::MovRR(sz, r, b)
                });
            }
            let v = parse_int(&s).ok_or_else(err)?;
            match sz {
                Size::B => Ok(Insn::MovRegImm8(r, u8::try_from(v).map_err(|_| err())?)),
                Size::D => Ok(Insn::MovRegImm32(r, as_imm32(v).ok_or_else(err)? as u32)),
                _ => Err(err()),
            }
        }
        _ => Err(err()),
    }
}

/// Rend une suite d'instructions en une ligne (`a ; b ; c`).
#[must_use]
pub fn to_line(insns: &[Insn]) -> String {
    insns
        .iter()
        .map(|i| i.to_text())
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// Analyse une ligne d'instructions séparées par `;`.
///
/// # Erreurs
/// Retourne une erreur si une des instructions n'est pas supportée.
pub fn parse_line(line: &str) -> Result<Vec<Insn>, ParseError> {
    line.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_insn)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_at;
    use alloc::vec;

    #[test]
    fn aller_retour_texte_sur_les_corps_reels() {
        let cases: Vec<Vec<Insn>> = vec![
            vec![Insn::Ret],
            vec![Insn::RetImm(0)],
            vec![Insn::MovRegImm8(Reg::Rax, 1), Insn::Ret],
            vec![
                Insn::AluRR(Alu::Xor, Size::D, Reg::Rax, Reg::Rax),
                Insn::Ret,
            ],
            vec![Insn::MovRR(Size::Q, Reg::Rax, Reg::Rcx), Insn::Ret],
            vec![
                Insn::Store(Size::Q, Mem::base(Reg::Rcx), Reg::Rdx),
                Insn::MovRR(Size::Q, Reg::Rax, Reg::Rcx),
                Insn::Store(Size::Q, Mem::base_disp(Reg::Rcx, 8), Reg::R8),
                Insn::Ret,
            ],
            vec![Insn::Lea(Reg::Rax, Mem::base_disp(Reg::Rcx, 8)), Insn::Ret],
            vec![Insn::MovRegImm32(Reg::Rax, 0xefec_8a0d), Insn::Ret],
            vec![Insn::Load(
                Size::Q,
                Reg::Rax,
                Mem::base_disp(Reg::Rsp, 0x28),
            )],
            vec![Insn::IncMem32(Mem::base(Reg::Rcx))],
            vec![Insn::JmpReg(Reg::Rax, false)],
            vec![Insn::Nop(10)],
            // Nouvelles formes : prologue, appel, saut, rip-relatif.
            vec![
                Insn::Store(Size::Q, Mem::base_disp(Reg::Rsp, 8), Reg::Rbx),
                Insn::Push(Reg::Rdi, false),
                Insn::AluRI(Alu::Sub, Size::Q, Reg::Rsp, 0x20, false),
                Insn::MovRR(Size::Q, Reg::Rbx, Reg::Rcx),
                Insn::Call(0x1_4012_3456),
                Insn::TestRR(Size::Q, Reg::Rax, Reg::Rax),
                Insn::Jcc(Cond::E, 0x1_4000_1050, true),
                Insn::Lea(Reg::Rcx, Mem::rip(0x1_401f_2340)),
                Insn::AluRI(Alu::Add, Size::Q, Reg::Rsp, 0x20, false),
                Insn::Pop(Reg::Rdi, false),
                Insn::Ret,
            ],
            vec![
                Insn::MovzxR(Size::B, Reg::Rax, Reg::Rcx),
                Insn::Setcc(Cond::Ne, Reg::Rax),
                Insn::Movsxd(Reg::Rdx, Reg::Rax),
                Insn::Shift(ShiftOp::Shr, Size::Q, Reg::Rax, 3),
                Insn::MovRegImm64(Reg::R11, 0x1234_5678_9abc_def0),
                Insn::StoreImm32(Size::D, Mem::rip(0x1_4020_0000), 7),
                Insn::MovzxM(Size::W, Reg::Rax, Mem::base(Reg::Rdx)),
                Insn::Jmp(0x1_4000_9000, false),
            ],
        ];
        for c in cases {
            let text = to_line(&c);
            let back = parse_line(&text).unwrap_or_else(|e| panic!("`{text}` : {e}"));
            // Le contrat est l'egalite des OCTETS, pas des variantes : plusieurs
            // formes internes rendent le meme texte et le meme encodage
            // (`IncMem32` et `Un(Inc, …)`, `StoreImm32` et `MovI`). C'est ce que
            // la forge exige, et c'est ce qu'on verifie.
            assert_eq!(
                encode_at(&back, 0x1_4000_1000),
                encode_at(&c, 0x1_4000_1000),
                "aller-retour de `{text}`"
            );
        }
    }

    #[test]
    fn texte_canonique_lisible() {
        assert_eq!(
            to_line(&[Insn::MovRegImm32(Reg::Rax, 0xefec_8a0d), Insn::Ret]),
            "mov eax, 0xefec8a0d ; ret"
        );
        assert_eq!(
            to_line(&[Insn::Store(Size::Q, Mem::base_disp(Reg::Rcx, 8), Reg::R8)]),
            "mov [rcx+0x8], r8"
        );
        assert_eq!(
            to_line(&[Insn::Lea(Reg::Rax, Mem::rip(0x1_401f_2340))]),
            "lea rax, [rip 0x1401f2340]"
        );
        assert_eq!(
            to_line(&[Insn::Jcc(Cond::Ne, 0x1_4000_1050, true)]),
            "jne.s 0x140001050"
        );
        assert_eq!(
            to_line(&[Insn::AluRI(Alu::Sub, Size::Q, Reg::Rsp, 0x20, false)]),
            "sub rsp, 0x20"
        );
        assert_eq!(
            to_line(&[Insn::Load(
                Size::Q,
                Reg::Rax,
                Mem {
                    base: Some(Reg::Rdx),
                    index: Some((Reg::Rcx, 4)),
                    disp: -16,
                    ..Mem::default()
                }
            )]),
            "mov rax, [rdx+rcx*4-0x10]"
        );
    }

    #[test]
    fn instruction_hors_dialecte_est_rejetee() {
        // `vfmadd231ps` faisait partie de ce test jusqu'à ce que le dialecte
        // apprenne l'encodage VEX ; `aesenc` reste hors dialecte.
        assert!(parse_insn("aesenc xmm0, xmm1").is_err());
        assert!(parse_insn("mov rax, ecx").is_err(), "tailles incohérentes");
        assert!(parse_insn("wibble rax").is_err());
    }

    /// Contre-épreuve du test ci-dessus : ce que le dialecte a appris se relit.
    #[test]
    fn les_formes_vex_apprises_se_relisent() {
        for l in [
            "vfmadd231ps xmm0, xmm1, xmm2",
            "vpermilps_i xmm0, xmm4, 0x0",
            "vmovaps xmm1, xmm2",
        ] {
            let i = parse_insn(l).unwrap_or_else(|e| panic!("{l} : {e:?}"));
            assert_eq!(
                parse_insn(&i.to_text()).unwrap(),
                i,
                "aller-retour de `{l}`"
            );
        }
    }
}

#[cfg(test)]
mod tests_prefixes {
    use super::*;
    use alloc::vec;

    /// `lock inc dword [rbx+60h]` : le `F0` fait partie des octets à reproduire.
    #[test]
    fn lock_est_un_prefixe_de_ligne() {
        let i = Insn::LockUn(UnOp::Inc, Size::D, Rm::M(Mem::base_disp(Reg::Rbx, 0x60)));
        assert_eq!(crate::encode(&[i]), vec![0xF0, 0xFF, 0x43, 0x60]);
        // Sans le préfixe, les mêmes octets moins le `F0`.
        let nu = Insn::Un(UnOp::Inc, Size::D, Rm::M(Mem::base_disp(Reg::Rbx, 0x60)));
        assert_eq!(crate::encode(&[nu]), vec![0xFF, 0x43, 0x60]);
        // Aller-retour : le texte rendu se relit à l'identique.
        assert!(i.to_text().starts_with("lock inc "));
        assert_eq!(parse_insn(&i.to_text()).unwrap(), i);
    }

    #[test]
    fn lock_refuse_ce_qui_n_est_pas_unaire() {
        assert!(parse_insn("lock ret").is_err());
    }

    /// `[rbx]` et `[rbx+0x0]` designent le meme acces et n'ont pas les memes
    /// octets : le second garde le `disp8` nul que porte l'original.
    #[test]
    fn le_deplacement_nul_explicite_fait_l_aller_retour() {
        let court = parse_insn("mov ecx, [rbx]").expect("dialecte");
        assert_eq!(crate::encode(&[court]), vec![0x8B, 0x0B]);
        assert_eq!(court.to_text(), "mov ecx, [rbx]");

        let long = parse_insn("mov ecx, [rbx+0x0]").expect("dialecte");
        assert_eq!(crate::encode(&[long]), vec![0x8B, 0x4B, 0x00]);
        assert_eq!(long.to_text(), "mov ecx, [rbx+0x0]");
        assert_ne!(court, long);

        // Un deplacement non nul n'est pas concerne.
        let d = parse_insn("mov ecx, [rbx+0x8]").expect("dialecte");
        assert_eq!(crate::encode(&[d]), vec![0x8B, 0x4B, 0x08]);
    }

    /// Le suffixe `.d` choisit l'autre sens d'encodage d'une forme reg/reg.
    /// Les deux calculent la même chose ; c'est l'original qui tranche.
    #[test]
    fn la_direction_inverse_fait_l_aller_retour_textuel() {
        for (texte, octets) in [
            ("mov.d rbp, rsp", vec![0x48u8, 0x89, 0xE5]),
            ("add.d rcx, rdx", vec![0x48, 0x01, 0xD1]),
        ] {
            let i = parse_insn(texte).expect("dialecte");
            assert_eq!(crate::encode(&[i]), octets, "{texte}");
            assert_eq!(i.to_text(), texte);
        }
        // Sans le suffixe, la forme MSVC — des octets differents.
        assert_eq!(
            crate::encode(&[parse_insn("mov rbp, rsp").expect("dialecte")]),
            vec![0x48, 0x8B, 0xEC]
        );
    }

    /// MSVC émet un REX.W superflu sur `jmp rax` — le suffixe `.r` le demande.
    #[test]
    fn jmp_reg_avec_rex_explicite() {
        let sans = parse_insn("jmp rax").expect("dialecte");
        assert_eq!(crate::encode(&[sans]), vec![0xFF, 0xE0]);
        let avec = parse_insn("jmp.r rax").expect("dialecte");
        assert_eq!(crate::encode(&[avec]), vec![0x48, 0xFF, 0xE0]);
        assert_eq!(avec.to_text(), "jmp.r rax");
        assert_eq!(parse_insn(&avec.to_text()).unwrap(), avec);
    }
}

#[cfg(test)]
mod tests_segment {
    use super::*;
    use alloc::vec;

    /// `mov rax, gs:[58h]` — l'accès TLS de Windows x64, le corps le plus
    /// fréquent du binaire (2 443 unités bloquées avant son support).
    #[test]
    fn gs_absolu_encode_le_prefixe_et_le_sib() {
        let m = Mem {
            seg: Some(Seg::Gs),
            disp: 0x58,
            ..Mem::default()
        };
        let i = Insn::Load(Size::Q, Reg::Rax, m);
        assert_eq!(
            crate::encode(&[i]),
            vec![0x65, 0x48, 0x8B, 0x04, 0x25, 0x58, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn le_segment_fait_l_aller_retour_textuel() {
        let m = Mem {
            seg: Some(Seg::Gs),
            disp: 0x58,
            ..Mem::default()
        };
        let i = Insn::Load(Size::Q, Reg::Rax, m);
        let t = i.to_text();
        assert!(t.contains("gs:["), "rendu : {t}");
        assert_eq!(parse_insn(&t).unwrap(), i);
    }

    /// Sans segment, une adresse absolue reste hors dialecte : elle serait
    /// non relogeable. Avec `gs:`, le déplacement est un offset de segment.
    #[test]
    fn fs_et_gs_ont_des_prefixes_distincts() {
        assert_eq!(Seg::Fs.prefix(), 0x64);
        assert_eq!(Seg::Gs.prefix(), 0x65);
    }
}

#[cfg(test)]
mod tests_vex {
    use super::*;
    use alloc::vec;

    /// `vmovaps [rsp+20h], xmm6` — forme courte `C5` (table `0F`, pas de
    /// registre haut, `W=0`), celle que MSVC emet.
    #[test]
    fn vmovaps_store_utilise_le_vex_court() {
        let m = Mem::base_disp(Reg::Rsp, 0x20);
        let i = Insn::Vex(VexOp::VmovapsStore, Xmm(6), Xmm(0), XmmRm::M(m), None);
        assert_eq!(
            crate::encode(&[i]),
            vec![0xC5, 0xF8, 0x29, 0x74, 0x24, 0x20]
        );
    }

    /// `vpermilps xmm0, xmm4, 0` — table `0F3A`, donc VEX long `C4`.
    #[test]
    fn vpermilps_imm_utilise_le_vex_long() {
        let i = Insn::Vex(
            VexOp::VpermilpsImm,
            Xmm(0),
            Xmm(0),
            XmmRm::X(Xmm(4)),
            Some(0),
        );
        assert_eq!(
            crate::encode(&[i]),
            vec![0xC4, 0xE3, 0x79, 0x04, 0xC4, 0x00]
        );
    }

    /// `vfmadd231ps xmm4, xmm0, xmm3` — table `0F38`, `vvvv` = xmm0.
    #[test]
    fn vfmadd231ps_porte_le_registre_non_destructif() {
        let i = Insn::Vex(VexOp::Vfmadd231ps, Xmm(4), Xmm(0), XmmRm::X(Xmm(3)), None);
        assert_eq!(crate::encode(&[i]), vec![0xC4, 0xE2, 0x79, 0xB8, 0xE3]);
    }
}

#[cfg(test)]
mod tests_octet_haut {
    use super::*;
    use alloc::vec;

    /// `mov dh, 84h` = `B6 84`. Le meme numero avec un REX donnerait `sil`,
    /// et sans les variantes dediees l'encodeur produisait `B2` (`dl`).
    #[test]
    fn les_octets_hauts_s_encodent_sans_rex() {
        let i = Insn::MovRegImm8(Reg::Dh, 0x84);
        assert_eq!(crate::encode(&[i]), vec![0xB6, 0x84]);
        // Contre-epreuve : `sil` porte le meme numero, mais avec REX nul.
        let j = Insn::MovRegImm8(Reg::Rsi, 0x84);
        assert_eq!(crate::encode(&[j]), vec![0x40, 0xB6, 0x84]);
    }

    #[test]
    fn les_octets_hauts_font_l_aller_retour() {
        for (name, r) in [
            ("ah", Reg::Ah),
            ("ch", Reg::Ch),
            ("dh", Reg::Dh),
            ("bh", Reg::Bh),
        ] {
            assert_eq!(reg_of(name), Some((r, Size::B)), "lecture de `{name}`");
            assert_eq!(reg_name(r, Size::B), name, "rendu de `{name}`");
            assert!(r.is_high_byte());
        }
        // `sil` et consorts restent des registres bas malgre le meme numero.
        assert!(!Reg::Rsi.is_high_byte());
        assert_eq!(Reg::Dh.num(), Reg::Rsi.num());
    }
}
