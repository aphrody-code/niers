//! `nie-asm` — encodeur x86-64 **dialecte MSVC**, pur Rust.
//!
//! ## Pourquoi ce crate existe
//!
//! `nie.exe` est produit par MSVC 14.44. Deux compilateurs qui émettent la même
//! instruction ne choisissent pas le même encodage : `mov rax, rcx` s'écrit
//! `48 8b c1` chez MSVC (opcode `8B`, direction registre←r/m) et `48 89 c8` chez
//! LLVM. Attendre de `rustc` qu'il reproduise les octets de MSVC est donc
//! structurellement vain — c'était le plafond invisible du projet.
//!
//! La forge le contourne en **assemblant elle-même** : le dépôt commite une
//! source symbolique (`push rbx ; sub rsp, 0x20 ; call 0x140123456`), et ce crate
//! la traduit en octets selon les conventions d'encodage de MSVC. Le binaire
//! n'est plus recopié : il est **produit**, depuis une source lisible.
//!
//! ## Encodage conscient de l'adresse
//!
//! Les branchements et les opérandes relatifs au pointeur d'instruction sont
//! écrits en **adresse absolue** dans la source (`call 0x140123456`,
//! `lea rax, [rip 0x1401f2340]`) ; [`encode_at`] calcule le déplacement depuis
//! l'adresse de l'instruction courante — le travail normal d'un assembleur. La
//! source reste donc lisible et vérifiable, sans dépendre d'un état de linker.
//!
//! ## Falsifiabilité
//!
//! L'encodeur ne « colle » pas aux octets d'origine : il applique des règles
//! canoniques. Si MSVC a choisi une autre forme, le résultat diffère et la forge
//! refuse l'unité. Aucun faux positif possible : la comparaison est byte-à-byte
//! contre le binaire réel.
//!
//! ```
//! use nie_asm::{Insn, Reg, encode};
//! // mov al, 1 ; ret  — les gestionnaires « return true » de nie.exe
//! assert_eq!(encode(&[Insn::MovRegImm8(Reg::Rax, 1), Insn::Ret]), vec![0xb0, 0x01, 0xc3]);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![no_std]

extern crate alloc;

pub mod text;

use alloc::vec::Vec;
pub use text::{ParseError, parse_insn, parse_line, to_line};

/// Registre général 64 bits (les formes 8/16/32 bits partagent l'encodage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Reg {
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    /// `ah` — octet **haut** de `rax`, numéro 4 et **sans** préfixe REX.
    Ah,
    /// `ch` — octet haut de `rcx`, numéro 5.
    Ch,
    /// `dh` — octet haut de `rdx`, numéro 6.
    Dh,
    /// `bh` — octet haut de `rbx`, numéro 7.
    Bh,
}

impl Reg {
    /// Numéro d'encodage 0..15.
    ///
    /// Les octets hauts (`ah`/`ch`/`dh`/`bh`) portent les numéros 4 à 7 — les
    /// mêmes que `spl`/`bpl`/`sil`/`dil`, dont seule l'absence de préfixe REX
    /// les distingue.
    #[must_use]
    pub fn num(self) -> u8 {
        match self {
            Self::Ah => 4,
            Self::Ch => 5,
            Self::Dh => 6,
            Self::Bh => 7,
            _ => self as u8,
        }
    }

    /// Vrai pour `ah`/`ch`/`dh`/`bh`, qui **interdisent** le préfixe REX.
    #[must_use]
    pub fn is_high_byte(self) -> bool {
        matches!(self, Self::Ah | Self::Ch | Self::Dh | Self::Bh)
    }

    /// Bit haut (bit 3), porté par REX.
    #[must_use]
    pub fn hi(self) -> u8 {
        self.num() >> 3
    }

    /// 3 bits bas, portés par ModRM/SIB.
    #[must_use]
    pub fn lo(self) -> u8 {
        self.num() & 7
    }
}

/// Taille d'opérande.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Size {
    /// 8 bits.
    B,
    /// 16 bits (préfixe `66`).
    W,
    /// 32 bits.
    D,
    /// 64 bits (REX.W).
    Q,
}

impl Size {
    /// Largeur en octets.
    #[must_use]
    pub fn bytes(self) -> u8 {
        match self {
            Self::B => 1,
            Self::W => 2,
            Self::D => 4,
            Self::Q => 8,
        }
    }

    /// Vrai si l'opérande impose REX.W.
    #[must_use]
    pub fn rex_w(self) -> bool {
        self == Self::Q
    }
}

/// Opération arithmétique/logique du groupe 1 (`/n` du ModRM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Alu {
    Add,
    Or,
    Adc,
    Sbb,
    And,
    Sub,
    Xor,
    Cmp,
}

impl Alu {
    /// Champ `/n` (aussi la base d'opcode : `op*8`).
    #[must_use]
    pub fn digit(self) -> u8 {
        self as u8
    }
}

/// Condition d'un `jcc` / `setcc` (numérotation architecturale `tttn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Cond {
    O,
    No,
    B,
    Ae,
    E,
    Ne,
    Be,
    A,
    S,
    Ns,
    P,
    Np,
    L,
    Ge,
    Le,
    G,
}

impl Cond {
    /// Code `tttn` 0..15.
    #[must_use]
    pub fn code(self) -> u8 {
        self as u8
    }
}

/// Décalage du groupe 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum ShiftOp {
    Shl,
    Shr,
    Sar,
    Rol,
    Ror,
}

impl ShiftOp {
    /// Champ `/n`.
    #[must_use]
    pub fn digit(self) -> u8 {
        match self {
            Self::Rol => 0,
            Self::Ror => 1,
            Self::Shl => 4,
            Self::Shr => 5,
            Self::Sar => 7,
        }
    }
}

/// Opérande mémoire `[base + index*scale + disp]`, ou `[rip → cible absolue]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mem {
    /// Préfixe de segment explicite (`fs:` / `gs:`).
    ///
    /// Windows x64 accède au TLS par `gs:[58h]` : le préfixe `65` fait partie
    /// des octets, il ne se déduit d'aucun autre champ.
    pub seg: Option<Seg>,
    /// Registre de base.
    pub base: Option<Reg>,
    /// Registre d'index et facteur d'échelle (1, 2, 4 ou 8).
    pub index: Option<(Reg, u8)>,
    /// Déplacement signé.
    pub disp: i32,
    /// Déplacement **nul encodé explicitement** (`mod=01`, `disp8 = 0`).
    ///
    /// `[rbx]` s'encode normalement `8B 0B` (`mod=00`) ; une partie de
    /// `nie.exe` écrit `8B 4B 00`, un octet de plus pour le même accès. Le
    /// choix appartient au binaire d'origine, comme la forme longue d'un
    /// immédiat dans [`Insn::AluRI`] — sans ce champ, 1 732 corps se
    /// ré-encodaient un octet trop court et étaient rejetés.
    ///
    /// Sans effet quand `mod=01` est de toute façon imposé (base `rbp`/`r13`)
    /// ou impossible (adresse absolue, `rip`-relatif).
    pub disp_explicite: bool,
    /// Cible **absolue** d'un adressage relatif au pointeur d'instruction.
    ///
    /// Quand ce champ est renseigné, `base`/`index`/`disp` sont ignorés et
    /// l'encodeur calcule `cible - adresse_de_l_instruction_suivante`.
    pub rip: Option<u64>,
}

/// Segment adressé explicitement par un préfixe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Seg {
    /// `fs:` — préfixe `64`.
    Fs,
    /// `gs:` — préfixe `65` (TLS Windows x64).
    Gs,
}

impl Seg {
    /// Octet de préfixe correspondant.
    #[must_use]
    pub const fn prefix(self) -> u8 {
        match self {
            Self::Fs => 0x64,
            Self::Gs => 0x65,
        }
    }
}

/// Émet le préfixe de segment d'un opérande mémoire, s'il en porte un.
///
/// Doit précéder `66`, le REX et l'opcode.
fn seg_prefix(out: &mut Vec<u8>, m: Mem) {
    if let Some(sg) = m.seg {
        out.push(sg.prefix());
    }
}

impl Mem {
    /// `[base]`.
    #[must_use]
    pub fn base(base: Reg) -> Self {
        Self {
            base: Some(base),
            ..Self::default()
        }
    }

    /// `[base + disp]`.
    #[must_use]
    pub fn base_disp(base: Reg, disp: i32) -> Self {
        Self {
            base: Some(base),
            disp,
            ..Self::default()
        }
    }

    /// `[rip → cible]`.
    #[must_use]
    pub fn rip(target: u64) -> Self {
        Self {
            rip: Some(target),
            ..Self::default()
        }
    }
}

/// Opérande « registre ou mémoire » (le `r/m` de l'encodage x86).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Rm {
    R(Reg),
    M(Mem),
}

/// Opération unaire du groupe `FF` / `F7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum UnOp {
    /// `inc` (`FF /0`)
    Inc,
    /// `dec` (`FF /1`)
    Dec,
    /// `call r/m` (`FF /2`) — appel indirect (imports, vtables)
    CallInd,
    /// `jmp r/m` (`FF /4`)
    JmpInd,
    /// `push r/m` (`FF /6`)
    PushRm,
    /// `not` (`F7 /2`)
    Not,
    /// `neg` (`F7 /3`)
    Neg,
    /// `mul r/m` (`F7 /4`)
    Mul,
    /// `imul r/m` — forme à un opérande (`F7 /5`)
    Imul1,
    /// `div r/m` (`F7 /6`)
    Div,
    /// `idiv r/m` (`F7 /7`)
    Idiv,
}

impl UnOp {
    /// Champ `/n`.
    #[must_use]
    pub fn digit(self) -> u8 {
        match self {
            Self::Inc => 0,
            Self::Dec => 1,
            Self::CallInd | Self::Not => 2,
            Self::Neg => 3,
            Self::JmpInd | Self::Mul => 4,
            Self::PushRm | Self::Div => 6,
            Self::Imul1 => 5,
            Self::Idiv => 7,
        }
    }

    /// Vrai si l'opération appartient au groupe `F7` (sinon `FF`).
    #[must_use]
    pub fn is_f7(self) -> bool {
        matches!(
            self,
            Self::Not | Self::Neg | Self::Mul | Self::Imul1 | Self::Div | Self::Idiv
        )
    }
}

/// Registre vectoriel 128 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Xmm(pub u8);

impl Xmm {
    /// Bit haut (bit 3), porté par REX.
    #[must_use]
    pub fn hi(self) -> u8 {
        self.0 >> 3
    }

    /// 3 bits bas, portés par ModRM.
    #[must_use]
    pub fn lo(self) -> u8 {
        self.0 & 7
    }
}

/// Opérande vectoriel « registre ou mémoire ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum XmmRm {
    X(Xmm),
    M(Mem),
}

/// Opération SSE à opérandes `xmm, xmm/m`.
///
/// Le jeu couvre ce qui bloque réellement le relevé de `nie.exe` : les
/// mouvements vectoriels (`movaps`/`movups`/`movss`/`movsd`/`movdq*`), les
/// logiques, l'arithmétique scalaire et paquetée, et les comparaisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum SseOp {
    Movaps,
    Movups,
    Movss,
    Movsd,
    Movdqa,
    Movdqu,
    Movapd,
    Movupd,
    Xorps,
    Xorpd,
    Andps,
    Andpd,
    Andnps,
    Orps,
    Addps,
    Addss,
    Addsd,
    Subps,
    Subss,
    Subsd,
    Mulps,
    Mulss,
    Mulsd,
    Divps,
    Divss,
    Divsd,
    Minss,
    Minps,
    Maxss,
    Maxps,
    Sqrtss,
    Sqrtps,
    Comiss,
    Comisd,
    Ucomiss,
    Ucomisd,
    Unpcklps,
    Unpckhps,
    Cvtss2sd,
    Cvtsd2ss,
    Rcpss,
    Rsqrtss,
    Shufps,
    Shufpd,
    Pshufd,
    Movlhps,
    Movhlps,
    Movlps,
    Movhps,
    Insertps,
    Blendps,
    Cvtdq2ps,
    Cvtps2dq,
    Cvttps2dq,
    Cvtps2pd,
    Cvtpd2ps,
    Cvtdq2pd,
    Haddps,
    Hsubps,
    Cmpps,
    Cmpss,
    Pxor,
    Por,
    Pand,
    Unpcklpd,
    Cmppd,
    Cmpsd,
    Rcpps,
    Rsqrtps,
    Punpckldq,
    Punpckhdq,
    Paddw,
    Paddd,
    Psubw,
    Psubd,
    Pminsw,
    Pmaxsw,
    Punpcklqdq,
    Punpckhqdq,
    Psadbw,
    Pmullw,
    Pavgb,
    Pavgw,
    Packuswb,
    Packsswb,
    Packssdw,
    Punpcklbw,
    Punpcklwd,
    Punpckhbw,
    Punpckhwd,
    Pcmpeqb,
    Pcmpeqw,
    Pcmpeqd,
    Pcmpgtb,
    Pcmpgtw,
    Pcmpgtd,
    Paddb,
    Psubb,
    Paddusb,
    Psubusb,
    Pmaddwd,
    Pmulhw,
    Pshuflw,
    Pshufhw,
    Pmuludq,
}

/// Table d'opcode d'une instruction VEX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VexMap {
    /// `0F`
    M0F,
    /// `0F 38`
    M0F38,
    /// `0F 3A`
    M0F3A,
}

impl VexMap {
    /// Champ `mmmmm` du préfixe VEX à trois octets.
    const fn mm(self) -> u8 {
        match self {
            Self::M0F => 1,
            Self::M0F38 => 2,
            Self::M0F3A => 3,
        }
    }
}

/// Opération AVX encodée en VEX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VexOp {
    /// `vmovaps xmm, xmm/m` (`VEX.128.0F.WIG 28 /r`)
    Vmovaps,
    /// `vmovaps xmm/m, xmm` (`VEX.128.0F.WIG 29 /r`)
    VmovapsStore,
    /// `vmovups xmm, xmm/m` (`VEX.128.0F.WIG 10 /r`)
    Vmovups,
    /// `vmovups xmm/m, xmm` (`VEX.128.0F.WIG 11 /r`)
    VmovupsStore,
    /// `vxorps` (`VEX.128.0F.WIG 57 /r`)
    Vxorps,
    /// `vaddps` (`VEX.128.0F.WIG 58 /r`)
    Vaddps,
    /// `vmulps` (`VEX.128.0F.WIG 59 /r`)
    Vmulps,
    /// `vsubps` (`VEX.128.0F.WIG 5C /r`)
    Vsubps,
    /// `vpermilps xmm, xmm, imm8` (`VEX.128.66.0F3A.W0 04 /r ib`)
    VpermilpsImm,
    /// `vpermilps xmm, xmm, xmm` (`VEX.128.66.0F38.W0 0C /r`)
    Vpermilps,
    /// `vfmadd231ps` (`VEX.128.66.0F38.W0 B8 /r`)
    Vfmadd231ps,
    /// `vfmadd213ps` (`VEX.128.66.0F38.W0 A8 /r`)
    Vfmadd213ps,
    /// `vfmadd132ps` (`VEX.128.66.0F38.W0 98 /r`)
    Vfmadd132ps,
    /// `vmovdqu xmm, xmm/m` (`VEX.128.F3.0F.WIG 6F /r`)
    Vmovdqu,
    /// `vmovdqu xmm/m, xmm` (`VEX.128.F3.0F.WIG 7F /r`)
    VmovdquStore,
    /// `vmovdqa xmm, xmm/m` (`VEX.128.66.0F.WIG 6F /r`)
    Vmovdqa,
    /// `vmovdqa xmm/m, xmm` (`VEX.128.66.0F.WIG 7F /r`)
    VmovdqaStore,
}

impl VexOp {
    /// `(table, pp, opcode, W, l'opérande mémoire est-il la destination)`.
    const fn encoding(self) -> (VexMap, u8, u8, bool, bool) {
        match self {
            Self::Vmovaps => (VexMap::M0F, 0, 0x28, false, false),
            Self::VmovapsStore => (VexMap::M0F, 0, 0x29, false, true),
            Self::Vmovups => (VexMap::M0F, 0, 0x10, false, false),
            Self::VmovupsStore => (VexMap::M0F, 0, 0x11, false, true),
            Self::Vxorps => (VexMap::M0F, 0, 0x57, false, false),
            Self::Vaddps => (VexMap::M0F, 0, 0x58, false, false),
            Self::Vmulps => (VexMap::M0F, 0, 0x59, false, false),
            Self::Vsubps => (VexMap::M0F, 0, 0x5C, false, false),
            Self::VpermilpsImm => (VexMap::M0F3A, 1, 0x04, false, false),
            Self::Vpermilps => (VexMap::M0F38, 1, 0x0C, false, false),
            Self::Vfmadd231ps => (VexMap::M0F38, 1, 0xB8, false, false),
            Self::Vfmadd213ps => (VexMap::M0F38, 1, 0xA8, false, false),
            Self::Vfmadd132ps => (VexMap::M0F38, 1, 0x98, false, false),
            Self::Vmovdqu => (VexMap::M0F, 2, 0x6F, false, false),
            Self::VmovdquStore => (VexMap::M0F, 2, 0x7F, false, true),
            Self::Vmovdqa => (VexMap::M0F, 1, 0x6F, false, false),
            Self::VmovdqaStore => (VexMap::M0F, 1, 0x7F, false, true),
        }
    }

    /// Vrai si la forme prend un immédiat 8 bits.
    #[must_use]
    pub const fn has_imm(self) -> bool {
        matches!(self, Self::VpermilpsImm)
    }

    /// Vrai si l'opérande mémoire est la **destination** (forme « store »).
    #[must_use]
    pub const fn is_store(self) -> bool {
        matches!(
            self,
            Self::VmovapsStore | Self::VmovupsStore | Self::VmovdquStore | Self::VmovdqaStore
        )
    }
}

/// Opération sur chaîne, préfixée par `rep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepOp {
    /// `stos` — remplit `[rdi]` depuis `al`/`eax`/`rax` (`AA`/`AB`).
    Stos,
    /// `movs` — copie `[rsi]` vers `[rdi]` (`A4`/`A5`).
    Movs,
}

impl RepOp {
    /// Opcode pour la taille d'opérande (`B` = forme octet).
    #[must_use]
    pub const fn opcode(self, size: Size) -> u8 {
        let byte = matches!(size, Size::B);
        match (self, byte) {
            (Self::Stos, true) => 0xAA,
            (Self::Stos, false) => 0xAB,
            (Self::Movs, true) => 0xA4,
            (Self::Movs, false) => 0xA5,
        }
    }
}

/// Décalage vectoriel à immédiat (groupe `0F 71`/`72`/`73`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SseShiftOp {
    /// `psrlw` (`66 0F 71 /2`)
    Psrlw,
    /// `psraw` (`66 0F 71 /4`)
    Psraw,
    /// `psllw` (`66 0F 71 /6`)
    Psllw,
    /// `psrld` (`66 0F 72 /2`)
    Psrld,
    /// `psrad` (`66 0F 72 /4`)
    Psrad,
    /// `pslld` (`66 0F 72 /6`)
    Pslld,
    /// `psrlq` (`66 0F 73 /2`)
    Psrlq,
    /// `psrldq` (`66 0F 73 /3`)
    Psrldq,
    /// `psllq` (`66 0F 73 /6`)
    Psllq,
    /// `pslldq` (`66 0F 73 /7`)
    Pslldq,
}

impl SseShiftOp {
    /// `(opcode, digit)` du groupe.
    #[must_use]
    pub const fn encoding(self) -> (u8, u8) {
        match self {
            Self::Psrlw => (0x71, 2),
            Self::Psraw => (0x71, 4),
            Self::Psllw => (0x71, 6),
            Self::Psrld => (0x72, 2),
            Self::Psrad => (0x72, 4),
            Self::Pslld => (0x72, 6),
            Self::Psrlq => (0x73, 2),
            Self::Psrldq => (0x73, 3),
            Self::Psllq => (0x73, 6),
            Self::Pslldq => (0x73, 7),
        }
    }
}

/// Extraction de masque de signes vers un registre général.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SseMaskOp {
    /// `movmskps` (`0F 50 /r`)
    Movmskps,
    /// `movmskpd` (`66 0F 50 /r`)
    Movmskpd,
    /// `pmovmskb` (`66 0F D7 /r`)
    Pmovmskb,
}

impl SseMaskOp {
    /// `(préfixe 66 requis, opcode)`.
    #[must_use]
    pub const fn encoding(self) -> (bool, u8) {
        match self {
            Self::Movmskps => (false, 0x50),
            Self::Movmskpd => (true, 0x50),
            Self::Pmovmskb => (true, 0xD7),
        }
    }
}

/// Préfixe obligatoire d'une opération SSE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SsePrefix {
    None,
    P66,
    F2,
    F3,
}

impl SseOp {
    /// Vrai si l'opcode est de la famille `0F 3A xx` (trois octets).
    fn three_byte(self) -> bool {
        matches!(self, Self::Insertps | Self::Blendps)
    }

    /// `(préfixe, opcode « registre ← r/m », opcode de la forme « mémoire ← registre »)`.
    fn encoding(self) -> (SsePrefix, u8, Option<u8>) {
        use SsePrefix::{F2, F3, None as N, P66};
        match self {
            Self::Movaps => (N, 0x28, Some(0x29)),
            Self::Movapd => (P66, 0x28, Some(0x29)),
            Self::Movups => (N, 0x10, Some(0x11)),
            Self::Movupd => (P66, 0x10, Some(0x11)),
            Self::Movss => (F3, 0x10, Some(0x11)),
            Self::Movsd => (F2, 0x10, Some(0x11)),
            Self::Movdqa => (P66, 0x6F, Some(0x7F)),
            Self::Movdqu => (F3, 0x6F, Some(0x7F)),
            Self::Xorps => (N, 0x57, None),
            Self::Xorpd => (P66, 0x57, None),
            Self::Andps => (N, 0x54, None),
            Self::Andpd => (P66, 0x54, None),
            Self::Andnps => (N, 0x55, None),
            Self::Orps => (N, 0x56, None),
            Self::Addps => (N, 0x58, None),
            Self::Addss => (F3, 0x58, None),
            Self::Addsd => (F2, 0x58, None),
            Self::Subps => (N, 0x5C, None),
            Self::Subss => (F3, 0x5C, None),
            Self::Subsd => (F2, 0x5C, None),
            Self::Mulps => (N, 0x59, None),
            Self::Mulss => (F3, 0x59, None),
            Self::Mulsd => (F2, 0x59, None),
            Self::Divps => (N, 0x5E, None),
            Self::Divss => (F3, 0x5E, None),
            Self::Divsd => (F2, 0x5E, None),
            Self::Minps => (N, 0x5D, None),
            Self::Minss => (F3, 0x5D, None),
            Self::Maxps => (N, 0x5F, None),
            Self::Maxss => (F3, 0x5F, None),
            Self::Sqrtps => (N, 0x51, None),
            Self::Sqrtss => (F3, 0x51, None),
            Self::Comiss => (N, 0x2F, None),
            Self::Comisd => (P66, 0x2F, None),
            Self::Ucomiss => (N, 0x2E, None),
            Self::Ucomisd => (P66, 0x2E, None),
            Self::Unpcklps => (N, 0x14, None),
            Self::Unpckhps => (N, 0x15, None),
            Self::Cvtss2sd => (F3, 0x5A, None),
            Self::Cvtsd2ss => (F2, 0x5A, None),
            Self::Rcpss => (F3, 0x53, None),
            Self::Rsqrtss => (F3, 0x52, None),
            Self::Shufps => (N, 0xC6, None),
            Self::Shufpd => (P66, 0xC6, None),
            Self::Pshufd => (P66, 0x70, None),
            Self::Movlhps => (N, 0x16, None),
            Self::Movhlps => (N, 0x12, None),
            Self::Movlps => (N, 0x12, Some(0x13)),
            Self::Movhps => (N, 0x16, Some(0x17)),
            // Opcodes a trois octets `0F 3A xx` : le second octet est porte par
            // `three_byte()`, l'opcode final reste ici.
            Self::Insertps => (P66, 0x21, None),
            Self::Blendps => (P66, 0x0C, None),
            Self::Cvtdq2ps => (N, 0x5B, None),
            Self::Cvtps2dq => (P66, 0x5B, None),
            Self::Cvttps2dq => (F3, 0x5B, None),
            Self::Cvtps2pd => (N, 0x5A, None),
            Self::Cvtpd2ps => (P66, 0x5A, None),
            Self::Cvtdq2pd => (F3, 0xE6, None),
            Self::Haddps => (F2, 0x7C, None),
            Self::Hsubps => (F2, 0x7D, None),
            Self::Cmpps => (N, 0xC2, None),
            Self::Cmpss => (F3, 0xC2, None),
            Self::Pxor => (P66, 0xEF, None),
            Self::Por => (P66, 0xEB, None),
            Self::Pand => (P66, 0xDB, None),
            Self::Unpcklpd => (P66, 0x14, None),
            Self::Cmppd => (P66, 0xC2, None),
            Self::Cmpsd => (F2, 0xC2, None),
            Self::Rcpps => (N, 0x53, None),
            Self::Rsqrtps => (N, 0x52, None),
            Self::Punpckldq => (P66, 0x62, None),
            Self::Punpckhdq => (P66, 0x6A, None),
            Self::Paddw => (P66, 0xFD, None),
            Self::Paddd => (P66, 0xFE, None),
            Self::Psubw => (P66, 0xF9, None),
            Self::Psubd => (P66, 0xFA, None),
            Self::Pminsw => (P66, 0xEA, None),
            Self::Pmaxsw => (P66, 0xEE, None),
            Self::Punpcklqdq => (P66, 0x6C, None),
            Self::Punpckhqdq => (P66, 0x6D, None),
            Self::Psadbw => (P66, 0xF6, None),
            Self::Pmullw => (P66, 0xD5, None),
            Self::Pavgb => (P66, 0xE0, None),
            Self::Pavgw => (P66, 0xE3, None),
            Self::Packuswb => (P66, 0x67, None),
            Self::Packsswb => (P66, 0x63, None),
            Self::Packssdw => (P66, 0x6B, None),
            Self::Punpcklbw => (P66, 0x60, None),
            Self::Punpcklwd => (P66, 0x61, None),
            Self::Punpckhbw => (P66, 0x68, None),
            Self::Punpckhwd => (P66, 0x69, None),
            Self::Pcmpeqb => (P66, 0x74, None),
            Self::Pcmpeqw => (P66, 0x75, None),
            Self::Pcmpeqd => (P66, 0x76, None),
            Self::Pcmpgtb => (P66, 0x64, None),
            Self::Pcmpgtw => (P66, 0x65, None),
            Self::Pcmpgtd => (P66, 0x66, None),
            Self::Paddb => (P66, 0xFC, None),
            Self::Psubb => (P66, 0xF8, None),
            Self::Paddusb => (P66, 0xDC, None),
            Self::Psubusb => (P66, 0xD8, None),
            Self::Pmaddwd => (P66, 0xF5, None),
            Self::Pmulhw => (P66, 0xE5, None),
            Self::Pshuflw => (F2, 0x70, None),
            Self::Pshufhw => (F3, 0x70, None),
            Self::Pmuludq => (P66, 0xF4, None),
        }
    }
}

/// Instruction supportée par l'encodeur.
///
/// Le jeu est restreint aux formes réellement présentes dans `nie.exe` : chaque
/// ajout est justifié par les unités qu'il fait basculer, et validé contre leurs
/// octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Insn {
    /// `ret`
    Ret,
    /// `ret imm16`
    RetImm(u16),
    /// `int3`
    Int3,
    /// `nop` multi-octets de longueur `n` (1..=15), forme canonique.
    Nop(u8),
    /// `push r64` ; le booléen force un préfixe REX nul (`40`).
    ///
    /// MSVC émet `40 53` là où `53` suffirait. Ce préfixe ne change pas la
    /// sémantique mais fait partie des octets à reproduire — 4,1 Mo de `.text`
    /// en dépendent.
    Push(Reg, bool),
    /// `pop r64` ; idem.
    Pop(Reg, bool),
    /// `mov r8, imm8` (opcode `B0+rb`)
    MovRegImm8(Reg, u8),
    /// `mov r32, imm32` (opcode `B8+rd`)
    MovRegImm32(Reg, u32),
    /// `mov r64, imm64` (REX.W + `B8+rd`)
    MovRegImm64(Reg, u64),
    /// `mov r, r` — MSVC encode `8B /r`
    MovRR(Size, Reg, Reg),
    /// `mov r, r` **encodé dans l'autre sens** : `89 /r` (`88 /r` en 8 bits).
    ///
    /// Même remarque que pour [`Insn::AluRRm`] : `mov rbp, rsp` s'écrit
    /// `48 89 E5` dans une partie de `nie.exe`, `48 8B EC` sous MSVC.
    MovRRm(Size, Reg, Reg),
    /// `mov r, [mem]` (`8B /r`)
    Load(Size, Reg, Mem),
    /// `mov [mem], r` (`89 /r`)
    Store(Size, Mem, Reg),
    /// `mov dword/qword [mem], imm32` (`C7 /0`)
    StoreImm32(Size, Mem, i32),
    /// `lea r64, [mem]` (`48 8D /r`)
    Lea(Reg, Mem),
    /// `<alu> r, r` — MSVC encode `op*8+3` (registre ← r/m)
    AluRR(Alu, Size, Reg, Reg),
    /// `<alu> r, r` **encodé dans l'autre sens** : `op*8+1` (r/m ← registre).
    ///
    /// Les deux formes calculent la même chose et le désassembleur les rend
    /// identiques ; seuls les octets diffèrent. `nie.exe` porte les deux —
    /// `add rcx, rdx` y apparaît en `48 01 D1` là où MSVC écrit `48 03 CA`,
    /// signature du code lié statiquement qui n'a pas été compilé par MSVC.
    /// Le choix appartient au binaire d'origine, comme pour [`Insn::AluRI`].
    AluRRm(Alu, Size, Reg, Reg),
    /// `<alu> r, [mem]` (`op*8+3`)
    AluRM(Alu, Size, Reg, Mem),
    /// `<alu> [mem], r` (`op*8+1`)
    AluMR(Alu, Size, Mem, Reg),
    /// `<alu> r, imm` — le booléen force la forme **longue** (`81 /n id`).
    ///
    /// MSVC n'encode pas toujours au plus court : `and rdx, -0x10` s'écrit
    /// tantôt `48 83 E2 F0`, tantôt `48 81 E2 F0 FF FF FF`. Le choix appartient
    /// au binaire d'origine, pas à l'encodeur — la source le conserve donc.
    AluRI(Alu, Size, Reg, i32, bool),
    /// `test r, r` (`85 /r`, `84 /r` en 8 bits)
    TestRR(Size, Reg, Reg),
    /// `<shift> r, imm8` (`C1 /n ib`)
    Shift(ShiftOp, Size, Reg, u8),
    /// `movzx r32, r/m8|16` (`0F B6` / `0F B7`)
    MovzxR(Size, Reg, Reg),
    /// `movzx r32, [mem]` en 8 ou 16 bits source
    MovzxM(Size, Reg, Mem),
    /// `movsxd r64, r32` (`63 /r`)
    Movsxd(Reg, Reg),
    /// `setcc r8` (`0F 90+cc`)
    Setcc(Cond, Reg),
    /// `inc dword [mem]` (`FF /0`)
    IncMem32(Mem),
    /// Opération unaire précédée du préfixe `lock` (`F0`).
    ///
    /// MSVC l'émet pour les compteurs de références (`lock inc`/`lock dec` sur
    /// un `dword` en mémoire). L'opération encodée est identique à [`Insn::Un`] :
    /// seul le préfixe change, mais il fait partie des octets à reproduire.
    LockUn(UnOp, Size, Rm),
    /// `bswap r32/r64` (`0F C8+r`) — inversion de l'ordre des octets.
    Bswap(Size, Reg),
    /// `pushfq` (`9C`) / `popfq` (`9D`).
    PushfPopf(bool),
    /// `push imm32` (`68 id`).
    PushImm(i32),
    /// Opération sur chaîne **sans** préfixe `rep` (`stosb` seul).
    StringOp(RepOp, Size),
    /// `lock cmpxchg [mem], reg` (`F0 0F B0/B1 /r`) — comparaison-échange atomique.
    LockCmpxchg(Size, Mem, Reg),
    /// `bsf`/`bsr` : indice du premier/dernier bit à 1 (`0F BC`/`0F BD`).
    BitScan(bool, Size, Reg, Rm),
    /// `lock xadd [mem], reg` (`F0 0F C1 /r`) — échange-et-ajoute atomique.
    LockXadd(Size, Mem, Reg),
    /// Instruction encodée **VEX** (AVX) à trois opérandes.
    ///
    /// `vpermilps xmm0, xmm4, 0` ; `vfmadd231ps xmm4, xmm0, xmm3` ;
    /// `vmovaps [rsp+20h], xmm6`. `src1` alimente le champ `vvvv` du préfixe
    /// (registre non destructif) ; il vaut `xmm0` quand la forme n'en a pas.
    Vex(VexOp, Xmm, Xmm, XmmRm, Option<u8>),
    /// `prefetch<hint> [mem]` (`0F 18 /n`) — indication de préchargement.
    ///
    /// Le niveau de cache est porté par le champ `reg` du ModRM : `nta` = 0,
    /// `t0` = 1, `t1` = 2, `t2` = 3.
    Prefetch(u8, Mem),
    /// Chaîne répétée : `rep stosb`/`stosd`/`stosq`, `rep movsb`… (`F3` + opcode).
    RepString(RepOp, Size),
    /// `xchg` registre↔registre — forme courte `90+r` avec `rax`.
    XchgAcc(Size, Reg),
    /// `xchg [mem], reg` (`87 /r`) — échange atomique avec la mémoire.
    ///
    /// L'échange avec un opérande mémoire est implicitement verrouillé, sans
    /// préfixe `lock` : MSVC s'en sert pour les compteurs atomiques.
    XchgMem(Size, Mem, Reg),
    /// Décalage vectoriel à immédiat : `psrldq xmm1, 1` (`66 0F 73 /3 ib`).
    ///
    /// Le champ `reg` du ModRM porte le **digit** de l'opération, l'opérande
    /// vectoriel est en `rm` : une forme de groupe, distincte de `SseI`.
    SseShift(SseShiftOp, Xmm, u8),
    /// Extraction de masque de signes : `movmskps eax, xmm1` (`0F 50 /r`).
    ///
    /// Destination = registre **général**, source = registre vectoriel.
    SseMovmsk(SseMaskOp, Reg, Xmm),
    /// `jmp r64` (`FF /4`).
    ///
    /// Le booléen demande un préfixe **REX.W explicite** (`48 FF E0` au lieu de
    /// `FF E0`). `jmp r/m64` est déjà 64 bits en mode long, ce REX est donc
    /// superflu — mais MSVC l'émet, et la forge exige l'octet exact.
    JmpReg(Reg, bool),
    /// `call <cible absolue>` (`E8 rel32`)
    Call(u64),
    /// `jmp <cible absolue>` ; `short` choisit `EB rel8` plutôt que `E9 rel32`
    Jmp(u64, bool),
    /// `jcc <cible absolue>` ; `short` choisit `7x rel8` plutôt que `0F 8x rel32`
    Jcc(Cond, u64, bool),
    /// `<alu> r/m, imm` (`80/81/83 /n`) ; le booléen force la forme longue.
    AluI(Alu, Size, Rm, i32, bool),
    /// `mov r/m, imm` (`C6 /0` en 8 bits, `C7 /0` sinon)
    MovI(Size, Rm, i32),
    /// `test r/m, r`
    Test(Size, Rm, Reg),
    /// `test r/m, imm` (`F6 /0` / `F7 /0`)
    TestI(Size, Rm, i32),
    /// `<unop> r/m` (groupes `FF` et `F7`)
    Un(UnOp, Size, Rm),
    /// `imul r, r/m` (`0F AF /r`)
    Imul(Size, Reg, Rm),
    /// `imul r, r/m, imm` (`69 /r id` ou `6B /r ib`)
    ImulI(Size, Reg, Rm, i32),
    /// `movsx r32/r64, r/m8|16` (`0F BE` / `0F BF`)
    Movsx(Size, Size, Reg, Rm),
    /// `lea r32, [mem]` (sans REX.W)
    LeaD(Reg, Mem),
    /// SSE, direction « registre ← xmm/mémoire » (`movss xmm0, [rcx]`)
    Sse(SseOp, Xmm, XmmRm),
    /// SSE, direction « mémoire ← registre » (`movaps [rcx], xmm0`)
    SseStore(SseOp, Mem, Xmm),
    /// `mov al/eax/rax, [adresse absolue 64 bits]` et sa réciproque (`A0`..`A3`).
    ///
    /// Forme réservée à l'accumulateur, qui porte son adresse en clair sur 8
    /// octets plutôt qu'en déplacement rip-relatif.
    MovMoffs(Size, u64, bool),
    /// `cmovcc r, r/m` (`0F 40+cc /r`)
    Cmov(Cond, Size, Reg, Rm),
    /// SSE à immédiat : `shufps xmm0, xmm1, 0x4e` (`0F C6 /r ib`)
    SseI(SseOp, Xmm, XmmRm, u8),
    /// Conversion `xmm ← r/m entier` (`cvtsi2ss`/`cvtsi2sd`)
    CvtToXmm(CvtOp, Xmm, Rm, Size),
    /// Conversion `r entier ← xmm/m` (`cvttss2si`, `cvtsd2si`…)
    CvtToReg(CvtOp, Reg, XmmRm, Size),
    /// `movd/movq xmm, r/m` (`66 0F 6E /r`, REX.W pour 64 bits)
    MovdToXmm(Xmm, Rm, Size),
    /// `movd/movq r/m, xmm` (`66 0F 7E /r`)
    MovdToRm(Rm, Xmm, Size),
    /// `movsxd r64, r/m32` (`63 /r`) — forme générale, mémoire comprise
    MovsxdRm(Reg, Rm),
    /// `movzx r, r/m8|16` — forme générale avec destination 32 ou 64 bits
    MovzxRm(Size, Size, Reg, Rm),
    /// `movsx r, r/m8|16` — idem
    MovsxRm(Size, Size, Reg, Rm),
    /// Instruction sans opérande (`cdqe`, `cdq`, `cqo`, `cwde`, `leave`)
    NoOperand(NoOp),
    /// `setcc r/m8` — forme générale, mémoire comprise
    SetccRm(Cond, Rm),
    /// `<shift> r/m, cl` (`D2`/`D3 /n`)
    ShiftCl(ShiftOp, Size, Rm),
    /// `<shift> r/m, 1` (`D0`/`D1 /n`) — forme dédiée, plus courte que `C1 /n 01`
    Shift1(ShiftOp, Size, Rm),
    /// `bt/bts/btr/btc r/m, r` (`0F A3/AB/B3/BB /r`)
    BitRm(BitOp, Size, Rm, Reg),
    /// `bt/bts/btr/btc r/m, imm8` (`0F BA /n ib`)
    BitImm(BitOp, Size, Rm, u8),
}

/// Instruction sans opérande.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum NoOp {
    /// `cwde` (`98`)
    Cwde,
    /// `cdqe` (`48 98`)
    Cdqe,
    /// `cdq` (`99`)
    Cdq,
    /// `cqo` (`48 99`)
    Cqo,
    /// `leave` (`C9`)
    Leave,
}

/// Opération sur bit (`bt` et dérivées).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum BitOp {
    Bt,
    Bts,
    Btr,
    Btc,
}

impl BitOp {
    /// Opcode de la forme « r/m, registre ».
    fn opcode(self) -> u8 {
        match self {
            Self::Bt => 0xA3,
            Self::Bts => 0xAB,
            Self::Btr => 0xB3,
            Self::Btc => 0xBB,
        }
    }

    /// Champ `/n` de la forme à immédiat (`0F BA`).
    fn digit(self) -> u8 {
        match self {
            Self::Bt => 4,
            Self::Bts => 5,
            Self::Btr => 6,
            Self::Btc => 7,
        }
    }
}

/// Conversion SSE ↔ entier / flottant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum CvtOp {
    Cvtsi2ss,
    Cvtsi2sd,
    Cvttss2si,
    Cvttsd2si,
    Cvtss2si,
    Cvtsd2si,
}

impl CvtOp {
    /// `(préfixe, opcode)`.
    fn encoding(self) -> (u8, u8) {
        match self {
            Self::Cvtsi2ss => (0xF3, 0x2A),
            Self::Cvtsi2sd => (0xF2, 0x2A),
            Self::Cvttss2si => (0xF3, 0x2C),
            Self::Cvttsd2si => (0xF2, 0x2C),
            Self::Cvtss2si => (0xF3, 0x2D),
            Self::Cvtsd2si => (0xF2, 0x2D),
        }
    }
}

/// Encode une suite d'instructions à l'adresse `0` (formes sans adresse).
#[must_use]
pub fn encode(insns: &[Insn]) -> Vec<u8> {
    encode_at(insns, 0)
}

/// Encode une suite d'instructions placée à l'adresse virtuelle `va`.
///
/// Les branchements et les opérandes `[rip …]` sont résolus par rapport à
/// l'adresse réelle de chaque instruction.
#[must_use]
pub fn encode_at(insns: &[Insn], va: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for i in insns {
        let here = va.wrapping_add(out.len() as u64);
        encode_one(*i, here, &mut out);
    }
    out
}

/// Préfixe REX si nécessaire (`w`, `r`, `x`, `b`).
fn rex(out: &mut Vec<u8>, w: bool, r: u8, x: u8, b: u8) {
    rex_forced(out, w, r, x, b, false);
}

/// Variante forçant l'émission d'un REX nul.
///
/// Indispensable en 8 bits : sans REX, les numéros 4..7 désignent `ah/ch/dh/bh` ;
/// avec un REX même vide, ils désignent `spl/bpl/sil/dil`. MSVC émet donc un
/// `40` apparemment inutile — l'omettre change l'instruction.
fn rex_forced(out: &mut Vec<u8>, w: bool, r: u8, x: u8, b: u8, force: bool) {
    let v = 0x40 | (u8::from(w) << 3) | ((r & 1) << 2) | ((x & 1) << 1) | (b & 1);
    if v != 0x40 || force {
        out.push(v);
    }
}

/// Vrai si le registre exige un REX en contexte 8 bits (`spl`/`bpl`/`sil`/`dil`).
fn needs_rex8(size: Size, r: Reg) -> bool {
    // `ah`/`ch`/`dh`/`bh` portent aussi les numéros 4-7, mais exigent
    // exactement l'inverse : sans REX, ce sont eux ; avec, ce sont
    // `spl`/`bpl`/`sil`/`dil`.
    size == Size::B && !r.is_high_byte() && (4..=7).contains(&r.num())
}

/// Préfixe de taille d'opérande 16 bits.
fn opsize(out: &mut Vec<u8>, size: Size) {
    if size == Size::W {
        out.push(0x66);
    }
}

/// Émet ModRM (+ SIB + déplacement) pour un opérande mémoire.
///
/// `at` (adresse de l'instruction), `base` (sa position dans `out`) et `imm_len`
/// servent au cas `[rip …]`, dont le déplacement est relatif à la **fin** de
/// l'instruction, immédiat compris.
///
/// Piège corrigé ici : `out` est le tampon de **tout le corps**, pas de la seule
/// instruction — sans `base`, le déplacement se calculerait depuis le début du
/// corps et serait faux dès la deuxième instruction.
fn modrm_mem(out: &mut Vec<u8>, reg: u8, m: Mem, at: u64, base: usize, imm_len: usize) {
    if let Some(target) = m.rip {
        out.push((reg & 7) << 3 | 0b101); // mod=00, rm=101 → rip-relatif
        let emitted = (out.len() - base) as u64;
        let end = at.wrapping_add(emitted + 4 + imm_len as u64);
        let rel = target.wrapping_sub(end) as i32;
        out.extend_from_slice(&rel.to_le_bytes());
        return;
    }
    let base = m.base;
    // Sans base ni index (adresse absolue, `gs:[58h]`), `rm` vaut 100 — ce qui
    // *exige* un octet SIB (base=101, index=100) suivi du disp32. Omettre ce
    // SIB décalait tout l'opérande d'un octet.
    let need_sib = m.index.is_some() || base.is_none() || base.is_some_and(|b| b.lo() == 4);
    // rbp/r13 en mod=00 signifierait « rip-relatif » : forcer un disp8 nul.
    // `disp_explicite` demande le même octet, mais parce que l'original le
    // porte et non parce que l'encodage l'exige.
    let force_disp8 =
        base.is_some() && m.disp == 0 && (base.is_some_and(|b| b.lo() == 5) || m.disp_explicite);
    let mode = if base.is_none() || (m.disp == 0 && !force_disp8) {
        0b00
    } else if i8::try_from(m.disp).is_ok() {
        0b01
    } else {
        0b10
    };
    let rm = if need_sib { 4 } else { base.map_or(4, Reg::lo) };
    out.push((mode << 6) | ((reg & 7) << 3) | rm);
    if need_sib {
        let (idx, scale) = m.index.map_or((4, 0u8), |(r, s)| {
            (
                r.lo(),
                match s {
                    2 => 1,
                    4 => 2,
                    8 => 3,
                    _ => 0,
                },
            )
        });
        out.push((scale << 6) | (idx << 3) | base.map_or(5, Reg::lo));
    }
    match mode {
        0b01 => out.push(m.disp as u8),
        0b10 => out.extend_from_slice(&m.disp.to_le_bytes()),
        _ if base.is_none() => out.extend_from_slice(&m.disp.to_le_bytes()),
        _ => {}
    }
}

/// Bits REX portés par un opérande mémoire.
fn mem_rex(m: Mem) -> (u8, u8) {
    (
        m.index.map_or(0, |(r, _)| r.hi()),
        m.base.map_or(0, Reg::hi),
    )
}

/// Instruction registre↔mémoire.
fn mem_form(out: &mut Vec<u8>, size: Size, opcode: u8, reg: Reg, m: Mem, at: u64, imm: usize) {
    let base = out.len();
    seg_prefix(out, m);
    opsize(out, size);
    let (x, b) = mem_rex(m);
    rex_forced(out, size.rex_w(), reg.hi(), x, b, needs_rex8(size, reg));
    out.push(opcode);
    modrm_mem(out, reg.lo(), m, at, base, imm);
}

/// Instruction registre↔registre (`mod=11`).
fn reg_form(out: &mut Vec<u8>, size: Size, opcode: u8, reg: Reg, rm: Reg) {
    opsize(out, size);
    let force = needs_rex8(size, reg) || needs_rex8(size, rm);
    rex_forced(out, size.rex_w(), reg.hi(), 0, rm.hi(), force);
    out.push(opcode);
    out.push(0xC0 | (reg.lo() << 3) | rm.lo());
}

/// Émet une instruction à opérande `r/m` : préfixes, opcode(s), ModRM, immédiat.
///
/// `reg` est le champ `/r` (numéro de registre ou extension d'opcode `/n`).
/// `imm` est écrit après le ModRM, sa largeur servant aussi au calcul du
/// déplacement `[rip …]`.
#[allow(clippy::too_many_arguments)] // encodage x86 : chaque champ est un champ du format
fn rm_form(
    out: &mut Vec<u8>,
    size: Size,
    opcodes: &[u8],
    reg: u8,
    reg_hi: u8,
    rm: Rm,
    at: u64,
    imm: &[u8],
) {
    let base = out.len();
    if let Rm::M(m) = rm {
        seg_prefix(out, m);
    }
    opsize(out, size);
    match rm {
        Rm::R(r) => {
            rex_forced(out, size.rex_w(), reg_hi, 0, r.hi(), needs_rex8(size, r));
            out.extend_from_slice(opcodes);
            out.push(0xC0 | ((reg & 7) << 3) | r.lo());
        }
        Rm::M(m) => {
            let (x, b) = mem_rex(m);
            rex(out, size.rex_w(), reg_hi, x, b);
            out.extend_from_slice(opcodes);
            modrm_mem(out, reg, m, at, base, imm.len());
        }
    }
    out.extend_from_slice(imm);
}

/// Immédiat d'une opération ALU/`mov` selon la taille d'opérande.
/// Immédiat **à la largeur de l'opérande** : 2 octets en 16 bits, 4 sinon.
///
/// `or si, 1D6h` s'encode `66 81 CE D6 01` — cinq octets. Émettre l'immédiat
/// sur 4 octets en ajoutait deux et changeait l'instruction.
fn imm_sized(out: &mut Vec<u8>, size: Size, v: i32) {
    if size == Size::W {
        out.extend_from_slice(&v.to_le_bytes()[..2]);
    } else {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn imm_bytes(size: Size, v: i32, force_wide: bool) -> (Vec<u8>, bool) {
    if size == Size::B {
        return (alloc::vec![v as u8], false);
    }
    if !force_wide && i8::try_from(v).is_ok() {
        return (alloc::vec![v as u8], true); // forme courte `83 /n ib`
    }
    match size {
        Size::W => (v.to_le_bytes()[..2].to_vec(), false),
        _ => (v.to_le_bytes().to_vec(), false),
    }
}

/// Opcode « registre ← r/m » du groupe ALU (`03`, `0B`, `23`, `2B`, `33`, `3B`…).
fn alu_rm_op(op: Alu, size: Size) -> u8 {
    op.digit() * 8 + if size == Size::B { 2 } else { 3 }
}

/// Opcode « r/m ← registre » du groupe ALU (`01`, `09`, `21`, `29`, `31`, `39`…).
fn alu_mr_op(op: Alu, size: Size) -> u8 {
    op.digit() * 8 + u8::from(size != Size::B)
}

fn encode_one(i: Insn, at: u64, out: &mut Vec<u8>) {
    match i {
        Insn::Ret => out.push(0xC3),
        Insn::RetImm(n) => {
            out.push(0xC2);
            out.extend_from_slice(&n.to_le_bytes());
        }
        Insn::Int3 => out.push(0xCC),
        Insn::Nop(n) => nop(out, n),
        Insn::Push(r, force) => {
            rex_forced(out, false, 0, 0, r.hi(), force);
            out.push(0x50 + r.lo());
        }
        Insn::Pop(r, force) => {
            rex_forced(out, false, 0, 0, r.hi(), force);
            out.push(0x58 + r.lo());
        }
        Insn::MovRegImm8(r, imm) => {
            rex_forced(out, false, 0, 0, r.hi(), needs_rex8(Size::B, r));
            out.push(0xB0 + r.lo());
            out.push(imm);
        }
        Insn::MovRegImm32(r, imm) => {
            rex(out, false, 0, 0, r.hi());
            out.push(0xB8 + r.lo());
            out.extend_from_slice(&imm.to_le_bytes());
        }
        Insn::MovRegImm64(r, imm) => {
            rex(out, true, 0, 0, r.hi());
            out.push(0xB8 + r.lo());
            out.extend_from_slice(&imm.to_le_bytes());
        }
        Insn::MovRR(size, dst, src) => {
            reg_form(
                out,
                size,
                if size == Size::B { 0x8A } else { 0x8B },
                dst,
                src,
            );
        }
        Insn::MovRRm(size, dst, src) => {
            reg_form(
                out,
                size,
                if size == Size::B { 0x88 } else { 0x89 },
                src,
                dst,
            );
        }
        Insn::Load(size, r, m) => {
            mem_form(
                out,
                size,
                if size == Size::B { 0x8A } else { 0x8B },
                r,
                m,
                at,
                0,
            );
        }
        Insn::Store(size, m, r) => {
            mem_form(
                out,
                size,
                if size == Size::B { 0x88 } else { 0x89 },
                r,
                m,
                at,
                0,
            );
        }
        Insn::StoreImm32(size, m, imm) => {
            let base = out.len();
            opsize(out, size);
            let (x, b) = mem_rex(m);
            rex(out, size.rex_w(), 0, x, b);
            out.push(0xC7);
            modrm_mem(out, 0, m, at, base, if size == Size::W { 2 } else { 4 });
            imm_sized(out, size, imm);
        }
        Insn::Lea(r, m) => mem_form(out, Size::Q, 0x8D, r, m, at, 0),
        Insn::AluRR(op, size, dst, src) => reg_form(out, size, alu_rm_op(op, size), dst, src),
        // Sens inverse : le registre source occupe le champ `/r`, la destination
        // le champ `r/m`.
        Insn::AluRRm(op, size, dst, src) => reg_form(out, size, alu_mr_op(op, size), src, dst),
        Insn::AluRM(op, size, dst, m) => mem_form(out, size, alu_rm_op(op, size), dst, m, at, 0),
        Insn::AluMR(op, size, m, src) => mem_form(out, size, alu_mr_op(op, size), src, m, at, 0),
        Insn::AluRI(op, size, r, imm, wide) => {
            let short = !wide && i8::try_from(imm).is_ok();
            // Forme accumulateur 8 bits : `cmp al, 0Ah` s'encode `3C 0A`,
            // pas `80 F8 0A`.
            if r == Reg::Rax && size == Size::B {
                out.push(op.digit() * 8 + 4);
                out.push(imm as u8);
                return;
            }
            // Forme accumulateur large : `and eax, imm32` = `25 id`.
            if r == Reg::Rax && size != Size::B && !short {
                opsize(out, size);
                rex(out, size.rex_w(), 0, 0, 0);
                out.push(op.digit() * 8 + 5);
                imm_sized(out, size, imm);
                return;
            }
            opsize(out, size);
            // `cmp sil, 1Ah` = `40 80 FE 1A` : REX nul requis pour spl/bpl/sil/dil.
            rex_forced(out, size.rex_w(), 0, 0, r.hi(), needs_rex8(size, r));
            if short {
                out.push(if size == Size::B { 0x80 } else { 0x83 });
                out.push(0xC0 | (op.digit() << 3) | r.lo());
                out.push(imm as u8);
            } else if size == Size::B {
                out.push(0x80);
                out.push(0xC0 | (op.digit() << 3) | r.lo());
                out.push(imm as u8);
            } else {
                out.push(0x81);
                out.push(0xC0 | (op.digit() << 3) | r.lo());
                imm_sized(out, size, imm);
            }
        }
        Insn::TestRR(size, a, b) => {
            reg_form(out, size, if size == Size::B { 0x84 } else { 0x85 }, b, a);
        }
        Insn::Shift(op, size, r, imm) => {
            opsize(out, size);
            // `shr bpl, 5` exige un REX nul (`40`) : sans lui, le numéro 5
            // désignerait `ch` et non `bpl`.
            rex_forced(out, size.rex_w(), 0, 0, r.hi(), needs_rex8(size, r));
            out.push(if size == Size::B { 0xC0 } else { 0xC1 });
            out.push(0xC0 | (op.digit() << 3) | r.lo());
            out.push(imm);
        }
        Insn::MovzxR(src_size, dst, src) => {
            rex(out, false, dst.hi(), 0, src.hi());
            out.push(0x0F);
            out.push(if src_size == Size::B { 0xB6 } else { 0xB7 });
            out.push(0xC0 | (dst.lo() << 3) | src.lo());
        }
        Insn::MovzxM(src_size, dst, m) => {
            let base = out.len();
            let (x, b) = mem_rex(m);
            rex(out, false, dst.hi(), x, b);
            out.push(0x0F);
            out.push(if src_size == Size::B { 0xB6 } else { 0xB7 });
            modrm_mem(out, dst.lo(), m, at, base, 0);
        }
        Insn::Movsxd(dst, src) => reg_form(out, Size::Q, 0x63, dst, src),
        Insn::Setcc(c, r) => {
            rex(out, false, 0, 0, r.hi());
            out.push(0x0F);
            out.push(0x90 + c.code());
            out.push(0xC0 | r.lo());
        }
        Insn::IncMem32(m) => {
            let base = out.len();
            let (x, b) = mem_rex(m);
            rex(out, false, 0, x, b);
            out.push(0xFF);
            modrm_mem(out, 0, m, at, base, 0);
        }
        Insn::JmpReg(r, w) => {
            rex_forced(out, w, 0, 0, r.hi(), w);
            out.push(0xFF);
            out.push(0xE0 | r.lo());
        }
        Insn::Call(target) => {
            out.push(0xE8);
            let rel = target.wrapping_sub(at.wrapping_add(5)) as i32;
            out.extend_from_slice(&rel.to_le_bytes());
        }
        Insn::Jmp(target, short) => {
            if short {
                out.push(0xEB);
                out.push(target.wrapping_sub(at.wrapping_add(2)) as u8);
            } else {
                out.push(0xE9);
                let rel = target.wrapping_sub(at.wrapping_add(5)) as i32;
                out.extend_from_slice(&rel.to_le_bytes());
            }
        }
        Insn::Jcc(c, target, short) => {
            if short {
                out.push(0x70 + c.code());
                out.push(target.wrapping_sub(at.wrapping_add(2)) as u8);
            } else {
                out.push(0x0F);
                out.push(0x80 + c.code());
                let rel = target.wrapping_sub(at.wrapping_add(6)) as i32;
                out.extend_from_slice(&rel.to_le_bytes());
            }
        }
        Insn::AluI(op, size, rm, v, wide) => {
            let (imm, short) = imm_bytes(size, v, wide);
            // Forme accumulateur : MSVC préfère `3D id` (cmp eax, imm32) à
            // `81 F8 id`, un octet de moins. L'ignorer ferait échouer la
            // comparaison sur une grande part du `.text`.
            if !short
                && let Rm::R(r) = rm
                && r == Reg::Rax
            {
                opsize(out, size);
                rex(out, size.rex_w(), 0, 0, 0);
                out.push(op.digit() * 8 + if size == Size::B { 4 } else { 5 });
                out.extend_from_slice(&imm);
                return;
            }
            let opcode = if size == Size::B {
                0x80
            } else if short {
                0x83
            } else {
                0x81
            };
            rm_form(out, size, &[opcode], op.digit(), 0, rm, at, &imm);
        }
        Insn::MovI(size, rm, v) => {
            let (imm, _) = imm_bytes(size, v, true);
            let opcode = if size == Size::B { 0xC6 } else { 0xC7 };
            rm_form(out, size, &[opcode], 0, 0, rm, at, &imm);
        }
        Insn::Test(size, rm, r) => {
            let opcode = if size == Size::B { 0x84 } else { 0x85 };
            rm_form(out, size, &[opcode], r.lo(), r.hi(), rm, at, &[]);
        }
        Insn::TestI(size, rm, v) => {
            let (imm, _) = imm_bytes(size, v, true);
            // `test al, imm8` = A8, `test eax, imm32` = A9 : même idiome.
            if let Rm::R(r) = rm
                && r == Reg::Rax
            {
                opsize(out, size);
                rex(out, size.rex_w(), 0, 0, 0);
                out.push(if size == Size::B { 0xA8 } else { 0xA9 });
                out.extend_from_slice(&imm);
                return;
            }
            let opcode = if size == Size::B { 0xF6 } else { 0xF7 };
            rm_form(out, size, &[opcode], 0, 0, rm, at, &imm);
        }
        Insn::LockUn(op, size, rm) => {
            out.push(0xF0);
            encode_one(Insn::Un(op, size, rm), at + 1, out);
        }
        Insn::Un(op, size, rm) => {
            let opcode = if op.is_f7() {
                if size == Size::B { 0xF6 } else { 0xF7 }
            } else if size == Size::B {
                0xFE
            } else {
                0xFF
            };
            // `call`/`jmp`/`push` indirects sont 64 bits implicites : `Size::D`
            // signifie ici « pas de REX.W », l'opérande faisant déjà la taille
            // d'un pointeur. `Size::Q` demande le REX.W **explicite** que MSVC
            // émet parfois sans nécessité — la forge doit reproduire l'octet.
            let sz = match op {
                UnOp::CallInd | UnOp::JmpInd | UnOp::PushRm if size != Size::Q => Size::D,
                _ => size,
            };
            rm_form(out, sz, &[opcode], op.digit(), 0, rm, at, &[]);
        }
        Insn::Imul(size, r, rm) => rm_form(out, size, &[0x0F, 0xAF], r.lo(), r.hi(), rm, at, &[]),
        Insn::ImulI(size, r, rm, v) => {
            let (imm, short) = imm_bytes(size, v, false);
            let opcode = if short { 0x6B } else { 0x69 };
            rm_form(out, size, &[opcode], r.lo(), r.hi(), rm, at, &imm);
        }
        Insn::Movsx(src, dst_size, r, rm) => {
            let opcode = if src == Size::B { 0xBE } else { 0xBF };
            rm_form(out, dst_size, &[0x0F, opcode], r.lo(), r.hi(), rm, at, &[]);
        }
        Insn::LeaD(r, m) => mem_form(out, Size::D, 0x8D, r, m, at, 0),
        Insn::MovMoffs(size, addr, store) => {
            opsize(out, size);
            rex(out, size.rex_w(), 0, 0, 0);
            out.push(match (size == Size::B, store) {
                (true, false) => 0xA0,
                (false, false) => 0xA1,
                (true, true) => 0xA2,
                (false, true) => 0xA3,
            });
            out.extend_from_slice(&addr.to_le_bytes());
        }
        Insn::Sse(op, dst, src) => {
            let (prefix, opcode, _) = op.encoding();
            sse_form_full(out, prefix, opcode, dst, src, at, None, op.three_byte());
        }
        Insn::Cmov(c, size, r, rm) => {
            rm_form(
                out,
                size,
                &[0x0F, 0x40 + c.code()],
                r.lo(),
                r.hi(),
                rm,
                at,
                &[],
            );
        }
        Insn::Bswap(size, r) => {
            rex(out, size.rex_w(), 0, 0, r.hi());
            out.push(0x0F);
            out.push(0xC8 | r.lo());
        }
        Insn::PushfPopf(pop) => out.push(if pop { 0x9D } else { 0x9C }),
        Insn::PushImm(v) => {
            out.push(0x68);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Insn::StringOp(op, size) => {
            opsize(out, size);
            rex(out, size.rex_w(), 0, 0, 0);
            out.push(op.opcode(size));
        }
        Insn::LockCmpxchg(size, m, r) => {
            out.push(0xF0);
            let base = out.len() - 1;
            let (x, b) = mem_rex(m);
            opsize(out, size);
            rex(out, size.rex_w(), r.hi(), x, b);
            out.push(0x0F);
            out.push(if size == Size::B { 0xB0 } else { 0xB1 });
            modrm_mem(out, r.lo(), m, at, base, 0);
        }
        Insn::BitScan(bsr, size, r, rm) => {
            rm_form(
                out,
                size,
                &[0x0F, if bsr { 0xBD } else { 0xBC }],
                r.lo(),
                r.hi(),
                rm,
                at,
                &[],
            );
        }
        Insn::LockXadd(size, m, r) => {
            out.push(0xF0);
            let base = out.len() - 1;
            let (x, b) = mem_rex(m);
            opsize(out, size);
            rex(out, size.rex_w(), r.hi(), x, b);
            out.push(0x0F);
            out.push(if size == Size::B { 0xC0 } else { 0xC1 });
            modrm_mem(out, r.lo(), m, at, base, 0);
        }
        Insn::Vex(op, dst, src1, src2, imm) => {
            let base = out.len();
            let (map, pp, opcode, w, _) = op.encoding();
            // Bits `R`/`X`/`B` : ils désignent les registres hauts et sont
            // stockés **inversés** dans le préfixe.
            let (x, b) = match src2 {
                XmmRm::X(r) => (0, r.hi()),
                XmmRm::M(m) => mem_rex(m),
            };
            let r = dst.hi();
            // La forme courte `C5` n'existe que pour la table `0F`, sans
            // `X`/`B` hauts et sans `W` : MSVC la choisit dès qu'elle
            // s'applique, et la forge doit rendre les mêmes octets.
            if map == VexMap::M0F && x == 0 && b == 0 && !w {
                out.push(0xC5);
                out.push(((1 - r) << 7) | ((!src1.0 & 0x0F) << 3) | pp);
            } else {
                out.push(0xC4);
                out.push(((1 - r) << 7) | ((1 - x) << 6) | ((1 - b) << 5) | map.mm());
                out.push((u8::from(w) << 7) | ((!src1.0 & 0x0F) << 3) | pp);
            }
            out.push(opcode);
            match src2 {
                XmmRm::X(rr) => out.push(0xC0 | (dst.lo() << 3) | rr.lo()),
                XmmRm::M(m) => {
                    modrm_mem(out, dst.lo(), m, at, base, usize::from(imm.is_some()));
                }
            }
            if let Some(v) = imm {
                out.push(v);
            }
        }
        Insn::Prefetch(hint, m) => {
            let base = out.len();
            let (x, b) = mem_rex(m);
            rex(out, false, 0, x, b);
            out.push(0x0F);
            out.push(0x18);
            modrm_mem(out, hint, m, at, base, 0);
        }
        Insn::RepString(op, size) => {
            out.push(0xF3);
            opsize(out, size);
            rex(out, size.rex_w(), 0, 0, 0);
            out.push(op.opcode(size));
        }
        Insn::XchgMem(size, m, r) => {
            mem_form(
                out,
                size,
                if size == Size::B { 0x86 } else { 0x87 },
                r,
                m,
                at,
                0,
            );
        }
        Insn::XchgAcc(size, r) => {
            opsize(out, size);
            rex(out, size.rex_w(), 0, 0, r.hi());
            out.push(0x90 | r.lo());
        }
        Insn::SseShift(op, x, imm) => {
            let (opcode, digit) = op.encoding();
            out.push(0x66);
            rex(out, false, 0, 0, x.hi());
            out.push(0x0F);
            out.push(opcode);
            out.push(0xC0 | (digit << 3) | x.lo());
            out.push(imm);
        }
        Insn::SseMovmsk(op, r, x) => {
            let (p66, opcode) = op.encoding();
            if p66 {
                out.push(0x66);
            }
            rex(out, false, r.hi(), 0, x.hi());
            out.push(0x0F);
            out.push(opcode);
            out.push(0xC0 | (r.lo() << 3) | x.lo());
        }
        Insn::SseI(op, dst, src, imm) => {
            let (prefix, opcode, _) = op.encoding();
            sse_form_full(
                out,
                prefix,
                opcode,
                dst,
                src,
                at,
                Some(imm),
                op.three_byte(),
            );
        }
        Insn::CvtToXmm(op, dst, src, size) => {
            let (prefix, opcode) = op.encoding();
            let base = out.len();
            out.push(prefix);
            match src {
                Rm::R(r) => {
                    rex(out, size.rex_w(), dst.hi(), 0, r.hi());
                    out.push(0x0F);
                    out.push(opcode);
                    out.push(0xC0 | (dst.lo() << 3) | r.lo());
                }
                Rm::M(m) => {
                    let (x, b) = mem_rex(m);
                    rex(out, size.rex_w(), dst.hi(), x, b);
                    out.push(0x0F);
                    out.push(opcode);
                    modrm_mem(out, dst.lo(), m, at, base, 0);
                }
            }
        }
        Insn::MovdToXmm(dst, src, size) => {
            let base = out.len();
            // `movq xmm, m64` a sa forme dediee `F3 0F 7E /r`, plus courte que
            // `66 REX.W 0F 6E /r` (qui charge depuis un registre *general*).
            // MSVC emploie la premiere : ce sont ses octets qu'il faut rendre.
            if size == Size::Q
                && let Rm::M(m) = src
            {
                out.push(0xF3);
                let (x, b) = mem_rex(m);
                rex(out, false, dst.hi(), x, b);
                out.push(0x0F);
                out.push(0x7E);
                modrm_mem(out, dst.lo(), m, at, base, 0);
                return;
            }
            out.push(0x66);
            match src {
                Rm::R(r) => {
                    rex(out, size.rex_w(), dst.hi(), 0, r.hi());
                    out.push(0x0F);
                    out.push(0x6E);
                    out.push(0xC0 | (dst.lo() << 3) | r.lo());
                }
                Rm::M(m) => {
                    let (x, b) = mem_rex(m);
                    rex(out, size.rex_w(), dst.hi(), x, b);
                    out.push(0x0F);
                    out.push(0x6E);
                    modrm_mem(out, dst.lo(), m, at, base, 0);
                }
            }
        }
        Insn::MovdToRm(dst, src, size) => {
            let base = out.len();
            // Symetrique : `movq m64, xmm` s'encode `66 0F D6 /r`.
            if size == Size::Q
                && let Rm::M(m) = dst
            {
                out.push(0x66);
                let (x, b) = mem_rex(m);
                rex(out, false, src.hi(), x, b);
                out.push(0x0F);
                out.push(0xD6);
                modrm_mem(out, src.lo(), m, at, base, 0);
                return;
            }
            out.push(0x66);
            match dst {
                Rm::R(r) => {
                    rex(out, size.rex_w(), src.hi(), 0, r.hi());
                    out.push(0x0F);
                    out.push(0x7E);
                    out.push(0xC0 | (src.lo() << 3) | r.lo());
                }
                Rm::M(m) => {
                    let (x, b) = mem_rex(m);
                    rex(out, size.rex_w(), src.hi(), x, b);
                    out.push(0x0F);
                    out.push(0x7E);
                    modrm_mem(out, src.lo(), m, at, base, 0);
                }
            }
        }
        Insn::NoOperand(op) => match op {
            NoOp::Cwde => out.push(0x98),
            NoOp::Cdqe => out.extend_from_slice(&[0x48, 0x98]),
            NoOp::Cdq => out.push(0x99),
            NoOp::Cqo => out.extend_from_slice(&[0x48, 0x99]),
            NoOp::Leave => out.push(0xC9),
        },
        Insn::SetccRm(c, rm) => {
            rm_form(out, Size::B, &[0x0F, 0x90 + c.code()], 0, 0, rm, at, &[]);
        }
        Insn::Shift1(op, size, rm) => {
            let opcode = if size == Size::B { 0xD0 } else { 0xD1 };
            rm_form(out, size, &[opcode], op.digit(), 0, rm, at, &[]);
        }
        Insn::ShiftCl(op, size, rm) => {
            let opcode = if size == Size::B { 0xD2 } else { 0xD3 };
            rm_form(out, size, &[opcode], op.digit(), 0, rm, at, &[]);
        }
        Insn::BitRm(op, size, rm, r) => {
            rm_form(out, size, &[0x0F, op.opcode()], r.lo(), r.hi(), rm, at, &[]);
        }
        Insn::BitImm(op, size, rm, imm) => {
            rm_form(out, size, &[0x0F, 0xBA], op.digit(), 0, rm, at, &[imm]);
        }
        Insn::MovsxdRm(dst, src) => {
            rm_form(out, Size::Q, &[0x63], dst.lo(), dst.hi(), src, at, &[]);
        }
        Insn::MovzxRm(src_size, dst_size, r, rm) => {
            let opcode = if src_size == Size::B { 0xB6 } else { 0xB7 };
            movx_form(out, src_size, dst_size, opcode, r, rm, at);
        }
        Insn::MovsxRm(src_size, dst_size, r, rm) => {
            let opcode = if src_size == Size::B { 0xBE } else { 0xBF };
            movx_form(out, src_size, dst_size, opcode, r, rm, at);
        }
        Insn::CvtToReg(op, dst, src, size) => {
            let (prefix, opcode) = op.encoding();
            let base = out.len();
            out.push(prefix);
            match src {
                XmmRm::X(x) => {
                    rex(out, size.rex_w(), dst.hi(), 0, x.hi());
                    out.push(0x0F);
                    out.push(opcode);
                    out.push(0xC0 | (dst.lo() << 3) | x.lo());
                }
                XmmRm::M(m) => {
                    let (xr, b) = mem_rex(m);
                    rex(out, size.rex_w(), dst.hi(), xr, b);
                    out.push(0x0F);
                    out.push(opcode);
                    modrm_mem(out, dst.lo(), m, at, base, 0);
                }
            }
        }
        Insn::SseStore(op, m, src) => {
            let (prefix, load, store) = op.encoding();
            // Sans forme « mémoire ← registre », l'opération n'est pas stockable :
            // on retombe sur l'opcode de chargement, et la comparaison byte-à-byte
            // rejettera l'unité — jamais de silence.
            sse_form(out, prefix, store.unwrap_or(load), src, XmmRm::M(m), at);
        }
    }
}

/// `movzx`/`movsx` : la largeur REX vient de la **destination**, mais l'exigence
/// d'un REX nul vient de la **source** (`movzx edx, dil` s'écrit `40 0F B6 D7`).
fn movx_form(
    out: &mut Vec<u8>,
    src_size: Size,
    dst_size: Size,
    opcode: u8,
    r: Reg,
    rm: Rm,
    at: u64,
) {
    let force = matches!(rm, Rm::R(x) if needs_rex8(src_size, x));
    let base = out.len();
    match rm {
        Rm::R(x) => {
            rex_forced(out, dst_size.rex_w(), r.hi(), 0, x.hi(), force);
            out.extend_from_slice(&[0x0F, opcode]);
            out.push(0xC0 | (r.lo() << 3) | x.lo());
        }
        Rm::M(m) => {
            let (ix, b) = mem_rex(m);
            rex(out, dst_size.rex_w(), r.hi(), ix, b);
            out.extend_from_slice(&[0x0F, opcode]);
            modrm_mem(out, r.lo(), m, at, base, 0);
        }
    }
}

/// Émet une instruction SSE `0F <opcode>` avec son préfixe obligatoire.
///
/// Ordre imposé par l'architecture : préfixe hérité (`66`/`F2`/`F3`), puis REX,
/// puis `0F`, puis l'opcode. Inverser REX et le préfixe change l'instruction.
fn sse_form(out: &mut Vec<u8>, prefix: SsePrefix, opcode: u8, reg: Xmm, rm: XmmRm, at: u64) {
    sse_form_imm(out, prefix, opcode, reg, rm, at, None);
}

/// Variante avec immédiat 8 bits (`shufps`, `cmpss`…).
fn sse_form_imm(
    out: &mut Vec<u8>,
    prefix: SsePrefix,
    opcode: u8,
    reg: Xmm,
    rm: XmmRm,
    at: u64,
    imm: Option<u8>,
) {
    sse_form_full(out, prefix, opcode, reg, rm, at, imm, false);
}

/// Forme complète, avec le drapeau « opcode à trois octets `0F 3A xx` ».
#[allow(clippy::too_many_arguments)] // encodage x86 : chaque champ est un champ du format
fn sse_form_full(
    out: &mut Vec<u8>,
    prefix: SsePrefix,
    opcode: u8,
    reg: Xmm,
    rm: XmmRm,
    at: u64,
    imm: Option<u8>,
    three: bool,
) {
    let base = out.len();
    match prefix {
        SsePrefix::None => {}
        SsePrefix::P66 => out.push(0x66),
        SsePrefix::F2 => out.push(0xF2),
        SsePrefix::F3 => out.push(0xF3),
    }
    match rm {
        XmmRm::X(x) => {
            rex(out, false, reg.hi(), 0, x.hi());
            out.push(0x0F);
            if three {
                out.push(0x3A);
            }
            out.push(opcode);
            out.push(0xC0 | (reg.lo() << 3) | x.lo());
        }
        XmmRm::M(m) => {
            let (x, b) = mem_rex(m);
            rex(out, false, reg.hi(), x, b);
            out.push(0x0F);
            if three {
                out.push(0x3A);
            }
            out.push(opcode);
            modrm_mem(out, reg.lo(), m, at, base, usize::from(imm.is_some()));
        }
    }
    if let Some(v) = imm {
        out.push(v);
    }
}

/// Formes canoniques du `nop` multi-octets (identiques chez MSVC et Intel).
fn nop(out: &mut Vec<u8>, n: u8) {
    const FORMS: [&[u8]; 16] = [
        &[],
        &[0x90],
        &[0x66, 0x90],
        &[0x0F, 0x1F, 0x00],
        &[0x0F, 0x1F, 0x40, 0x00],
        &[0x0F, 0x1F, 0x44, 0x00, 0x00],
        &[0x66, 0x0F, 0x1F, 0x44, 0x00, 0x00],
        &[0x0F, 0x1F, 0x80, 0x00, 0x00, 0x00, 0x00],
        &[0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[
            0x66, 0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        &[
            0x66, 0x66, 0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        &[
            0x66, 0x66, 0x66, 0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        &[
            0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        &[
            0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00,
            0x00,
        ],
    ];
    if let Some(f) = FORMS.get(n as usize) {
        out.extend_from_slice(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Chaque vecteur vient d'un **corps de fonction réel** de `nie.exe`
    /// (recensé par `nie-forge candidates`) : l'encodeur est validé contre le
    /// binaire, pas contre lui-même.
    #[test]
    fn encode_les_corps_reels_de_nie_exe() {
        assert_eq!(encode(&[Insn::Ret]), vec![0xC3]);
        assert_eq!(encode(&[Insn::RetImm(0)]), vec![0xC2, 0x00, 0x00]);
        // `mov al, 1 ; ret` — 320 unités (gestionnaires « return true »)
        assert_eq!(
            encode(&[Insn::MovRegImm8(Reg::Rax, 1), Insn::Ret]),
            vec![0xB0, 0x01, 0xC3]
        );
        // `xor eax, eax ; ret` — 163 unités (dialecte MSVC : 33, pas 31)
        assert_eq!(
            encode(&[
                Insn::AluRR(Alu::Xor, Size::D, Reg::Rax, Reg::Rax),
                Insn::Ret
            ]),
            vec![0x33, 0xC0, 0xC3]
        );
        // `xor al, al ; ret` — 163 unités
        assert_eq!(
            encode(&[
                Insn::AluRR(Alu::Xor, Size::B, Reg::Rax, Reg::Rax),
                Insn::Ret
            ]),
            vec![0x32, 0xC0, 0xC3]
        );
        // `mov rax, rcx ; ret` — 178 unités (MSVC : 8B, pas 89)
        assert_eq!(
            encode(&[Insn::MovRR(Size::Q, Reg::Rax, Reg::Rcx), Insn::Ret]),
            vec![0x48, 0x8B, 0xC1, 0xC3]
        );
        // Les mêmes opérations dans l'autre sens d'encodage : `nie.exe` porte
        // les deux formes, celle-ci venant du code lié statiquement hors MSVC.
        assert_eq!(
            encode(&[Insn::MovRRm(Size::Q, Reg::Rbp, Reg::Rsp)]),
            vec![0x48, 0x89, 0xE5],
            "mov rbp, rsp en 89, pas 8B"
        );
        assert_eq!(
            encode(&[Insn::AluRRm(Alu::Add, Size::Q, Reg::Rcx, Reg::Rdx)]),
            vec![0x48, 0x01, 0xD1],
            "add rcx, rdx en 01, pas 03"
        );
        assert_eq!(
            encode(&[Insn::AluRRm(Alu::Xor, Size::B, Reg::Rax, Reg::Rax)]),
            vec![0x30, 0xC0],
            "forme 8 bits : op*8+0"
        );
        // `mov [rcx], rdx ; mov rax, rcx ; mov [rcx+8], r8 ; ret` — 125 unités
        assert_eq!(
            encode(&[
                Insn::Store(Size::Q, Mem::base(Reg::Rcx), Reg::Rdx),
                Insn::MovRR(Size::Q, Reg::Rax, Reg::Rcx),
                Insn::Store(Size::Q, Mem::base_disp(Reg::Rcx, 8), Reg::R8),
                Insn::Ret
            ]),
            vec![
                0x48, 0x89, 0x11, 0x48, 0x8B, 0xC1, 0x4C, 0x89, 0x41, 0x08, 0xC3
            ]
        );
        // `lea rax, [rcx+8] ; ret` — 265 unités
        assert_eq!(
            encode(&[Insn::Lea(Reg::Rax, Mem::base_disp(Reg::Rcx, 8)), Insn::Ret]),
            vec![0x48, 0x8D, 0x41, 0x08, 0xC3]
        );
        // `mov eax, 0xefec8a0d ; ret` — 200 unités (accesseurs de hash/type-id)
        assert_eq!(
            encode(&[Insn::MovRegImm32(Reg::Rax, 0xefec_8a0d), Insn::Ret]),
            vec![0xB8, 0x0D, 0x8A, 0xEC, 0xEF, 0xC3]
        );
        // `mov eax, [rdx] ; mov [rcx], eax ; mov rax, rcx ; ret` — 55 unités
        assert_eq!(
            encode(&[
                Insn::Load(Size::D, Reg::Rax, Mem::base(Reg::Rdx)),
                Insn::Store(Size::D, Mem::base(Reg::Rcx), Reg::Rax),
                Insn::MovRR(Size::Q, Reg::Rax, Reg::Rcx),
                Insn::Ret
            ]),
            vec![0x8B, 0x02, 0x89, 0x01, 0x48, 0x8B, 0xC1, 0xC3]
        );
        // Forme SIB imposée par rsp.
        assert_eq!(
            encode(&[Insn::Load(
                Size::Q,
                Reg::Rax,
                Mem::base_disp(Reg::Rsp, 0x28)
            )]),
            vec![0x48, 0x8B, 0x44, 0x24, 0x28]
        );
        assert_eq!(
            encode(&[Insn::IncMem32(Mem::base(Reg::Rcx))]),
            vec![0xFF, 0x01]
        );
        // `and rax, -1 ; shl rdx, 0x20 ; or rax, rdx ; ret`
        assert_eq!(
            encode(&[
                Insn::AluRI(Alu::And, Size::Q, Reg::Rax, -1, false),
                Insn::Shift(ShiftOp::Shl, Size::Q, Reg::Rdx, 0x20),
                Insn::AluRR(Alu::Or, Size::Q, Reg::Rax, Reg::Rdx),
                Insn::Ret
            ]),
            vec![
                0x48, 0x83, 0xE0, 0xFF, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x0B, 0xC2, 0xC3
            ]
        );
        assert_eq!(encode(&[Insn::JmpReg(Reg::Rax, false)]), vec![0xFF, 0xE0]);
        // MSVC emet un REX.W superflu sur `jmp rax` : la forge exige l'octet.
        assert_eq!(
            encode(&[Insn::JmpReg(Reg::Rax, true)]),
            vec![0x48, 0xFF, 0xE0]
        );
        // Registre haut : REX.B seul, puis REX.W|B.
        assert_eq!(
            encode(&[Insn::JmpReg(Reg::R8, false)]),
            vec![0x41, 0xFF, 0xE0]
        );
        assert_eq!(
            encode(&[Insn::JmpReg(Reg::R8, true)]),
            vec![0x49, 0xFF, 0xE0]
        );
        assert_eq!(
            encode(&[Insn::Nop(10)]),
            vec![0x66, 0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn prologue_et_epilogue_msvc() {
        // Séquence d'ouverture typique : mov [rsp+8],rbx ; push rdi ; sub rsp,0x20
        assert_eq!(
            encode(&[
                Insn::Store(Size::Q, Mem::base_disp(Reg::Rsp, 8), Reg::Rbx),
                Insn::Push(Reg::Rdi, false),
                Insn::AluRI(Alu::Sub, Size::Q, Reg::Rsp, 0x20, false),
            ]),
            vec![0x48, 0x89, 0x5C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x20]
        );
        // Fermeture : add rsp,0x20 ; pop rdi ; ret
        assert_eq!(
            encode(&[
                Insn::AluRI(Alu::Add, Size::Q, Reg::Rsp, 0x20, false),
                Insn::Pop(Reg::Rdi, false),
                Insn::Ret
            ]),
            vec![0x48, 0x83, 0xC4, 0x20, 0x5F, 0xC3]
        );
        // Registres étendus : push r14 / pop r14 portent REX.B
        assert_eq!(encode(&[Insn::Push(Reg::R14, false)]), vec![0x41, 0x56]);
        assert_eq!(encode(&[Insn::Pop(Reg::R14, false)]), vec![0x41, 0x5E]);
        // sub rsp, 0x108 → immédiat 32 bits
        assert_eq!(
            encode(&[Insn::AluRI(Alu::Sub, Size::Q, Reg::Rsp, 0x108, false)]),
            vec![0x48, 0x81, 0xEC, 0x08, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn branchements_resolus_depuis_l_adresse_courante() {
        // `call 0x140001000` placé en 0x140000000 → rel32 = 0x1000 - 5
        assert_eq!(
            encode_at(&[Insn::Call(0x1_4000_1000)], 0x1_4000_0000),
            vec![0xE8, 0xFB, 0x0F, 0x00, 0x00]
        );
        // saut arrière court
        assert_eq!(
            encode_at(&[Insn::Jmp(0x1_4000_0000, true)], 0x1_4000_0010),
            vec![0xEB, 0xEE]
        );
        // jcc near
        assert_eq!(
            encode_at(&[Insn::Jcc(Cond::E, 0x1_4000_0100, false)], 0x1_4000_0000),
            vec![0x0F, 0x84, 0xFA, 0x00, 0x00, 0x00]
        );
        // jcc court
        assert_eq!(
            encode_at(&[Insn::Jcc(Cond::Ne, 0x1_4000_0020, true)], 0x1_4000_0000),
            vec![0x75, 0x1E]
        );
        // `lea rax, [rip → 0x140002000]` depuis 0x140001000
        assert_eq!(
            encode_at(
                &[Insn::Lea(Reg::Rax, Mem::rip(0x1_4000_2000))],
                0x1_4000_1000
            ),
            vec![0x48, 0x8D, 0x05, 0xF9, 0x0F, 0x00, 0x00]
        );
        // `mov rax, [rip → cible]` : le déplacement part de la fin de l'instruction
        assert_eq!(
            encode_at(
                &[Insn::Load(Size::Q, Reg::Rax, Mem::rip(0x1_4000_2000))],
                0x1_4000_1000
            ),
            vec![0x48, 0x8B, 0x05, 0xF9, 0x0F, 0x00, 0x00]
        );
        // Immédiat après un opérande rip : le rel32 doit en tenir compte.
        assert_eq!(
            encode_at(
                &[Insn::StoreImm32(Size::D, Mem::rip(0x1_4000_2000), 7)],
                0x1_4000_1000
            ),
            vec![0xC7, 0x05, 0xF6, 0x0F, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn formes_de_comparaison_et_conversion() {
        assert_eq!(
            encode(&[Insn::TestRR(Size::Q, Reg::Rax, Reg::Rax)]),
            vec![0x48, 0x85, 0xC0]
        );
        assert_eq!(
            encode(&[Insn::TestRR(Size::B, Reg::Rax, Reg::Rax)]),
            vec![0x84, 0xC0]
        );
        assert_eq!(
            encode(&[Insn::AluRI(Alu::Cmp, Size::D, Reg::Rax, 5, false)]),
            vec![0x83, 0xF8, 0x05]
        );
        assert_eq!(
            encode(&[Insn::Setcc(Cond::E, Reg::Rax)]),
            vec![0x0F, 0x94, 0xC0]
        );
        assert_eq!(
            encode(&[Insn::MovzxR(Size::B, Reg::Rax, Reg::Rcx)]),
            vec![0x0F, 0xB6, 0xC1]
        );
        assert_eq!(
            encode(&[Insn::Movsxd(Reg::Rax, Reg::Rcx)]),
            vec![0x48, 0x63, 0xC1]
        );
        assert_eq!(
            encode(&[Insn::MovRegImm64(Reg::Rax, 0x1234_5678_9abc_def0)]),
            vec![0x48, 0xB8, 0xF0, 0xDE, 0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12]
        );
        assert_eq!(encode(&[Insn::Int3]), vec![0xCC]);
    }

    #[test]
    fn registres_etendus_portent_rex() {
        assert_eq!(
            encode(&[Insn::MovRR(Size::Q, Reg::R8, Reg::R9)]),
            vec![0x4D, 0x8B, 0xC1]
        );
        assert_eq!(
            encode(&[Insn::Store(Size::Q, Mem::base(Reg::R12), Reg::Rax)]),
            vec![0x49, 0x89, 0x04, 0x24]
        );
        assert_eq!(
            encode(&[Insn::Load(Size::Q, Reg::Rax, Mem::base(Reg::Rbp))]),
            vec![0x48, 0x8B, 0x45, 0x00]
        );
    }

    #[test]
    fn deplacement_32_bits_quand_disp8_ne_suffit_pas() {
        assert_eq!(
            encode(&[Insn::Load(
                Size::Q,
                Reg::Rax,
                Mem::base_disp(Reg::Rcx, 0x1234)
            )]),
            vec![0x48, 0x8B, 0x81, 0x34, 0x12, 0x00, 0x00]
        );
    }
}
