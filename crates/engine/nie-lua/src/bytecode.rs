//! **Décodeur de bytecode Lua 5.2** — lit un `.lua.bin` du jeu et le rend intelligible.
//!
//! Le moteur Level-5 « Lives » livre sa logique de menus/scènes **uniquement compilée** (~616
//! `.lua.bin` sous `data/common/script/lua/`) : aucune source n'est distribuée. [`crate::new_vm`]
//! sait déjà *exécuter* ces chunks, ce qui suffit pour reproduire un comportement, mais pas pour
//! le **comprendre** — il n'y a rien à lire, à commenter, ni à comparer entre deux versions du jeu.
//!
//! Ce module ouvre le conteneur : en-tête, prototypes imbriqués, constantes, instructions,
//! variables locales et upvalues, plus un **désassemblage** textuel lisible.
//!
//! ## Portée honnête : désassembleur, pas décompilateur
//!
//! On produit un listing d'instructions annoté (opérandes résolues, constantes et noms de locales
//! substitués quand ils sont connus) — pas du Lua source reconstruit. Reconstruire du source exige
//! de réassembler le flot de contrôle (boucles, conditions, expressions imbriquées) à partir des
//! sauts : c'est un travail distinct, et prétendre le contraire produirait du code faux d'aspect
//! plausible. Les tables de débogage (`lineinfo`, `locvars`) sont conservées : ce sont elles qui
//! rendent le listing lisible, et elles alimenteraient un décompilateur ultérieur.
//!
//! ## Format
//!
//! PUC-Rio Lua 5.2.4, tel que produit par `luac` et consommé par `lundump.c`. Les tailles
//! (`size_t`, `Instruction`, `lua_Number`) sont lues dans l'en-tête plutôt que supposées : le jeu
//! est 32 bits sur certaines cibles et 64 bits sur d'autres, et un `size_t` mal deviné décale tout
//! le reste du fichier.

use std::fmt::Write as _;

use serde::Serialize;
use thiserror::Error;

/// Erreurs de décodage du conteneur bytecode.
#[derive(Debug, Error)]
pub enum BytecodeError {
    /// Signature `\x1bLua` + version `0x52` absente.
    #[error("pas un bytecode Lua 5.2 (signature/version inattendue)")]
    BadSignature,
    /// En-tête incohérent (tailles non gérées).
    #[error("en-tête bytecode non géré : {0}")]
    UnsupportedHeader(String),
    /// Fin de tampon atteinte au milieu d'un champ.
    #[error("fin de données inattendue à l'offset {0}")]
    UnexpectedEof(usize),
    /// Valeur hors domaine (type de constante inconnu, etc.).
    #[error("donnée invalide à l'offset {offset} : {reason}")]
    Invalid {
        /// Position dans le tampon.
        offset: usize,
        /// Cause lisible.
        reason: String,
    },
}

/// En-tête d'un chunk (`lundump.c`, `LoadHeader`).
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Version encodée (`0x52` pour Lua 5.2).
    pub version: u8,
    /// Format officiel = 0.
    pub format: u8,
    /// `true` si petit-boutiste.
    pub little_endian: bool,
    /// Taille d'un `int` C, en octets.
    pub size_int: u8,
    /// Taille d'un `size_t` C, en octets.
    pub size_size_t: u8,
    /// Taille d'une instruction, en octets (toujours 4).
    pub size_instruction: u8,
    /// Taille d'un `lua_Number`, en octets (8 = double).
    pub size_number: u8,
    /// `true` si les nombres sont entiers (build exotique), `false` = flottants.
    pub number_is_integral: bool,
}

/// Constante du pool d'un prototype.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum Constant {
    /// `nil`.
    Nil,
    /// Booléen.
    Boolean(bool),
    /// Nombre (toujours rendu en `f64`, y compris sur un build entier).
    Number(f64),
    /// Chaîne. Les octets sont conservés tels quels : les scripts du jeu contiennent des libellés
    /// non-UTF-8 (encodage japonais hérité), qu'une conversion lossy corromprait silencieusement.
    String(Vec<u8>),
}

impl Constant {
    /// Rendu court pour le listing.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Nil => "nil".to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Number(n) => {
                // Un entier exact s'affiche sans `.0` — c'est ce que fait `luac -l`, et ça rend
                // les indices de table lisibles.
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Self::String(bytes) => format!("{:?}", String::from_utf8_lossy(bytes)),
        }
    }
}

/// Description d'un upvalue (`instack` + index).
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpvalDesc {
    /// `true` si l'upvalue vient du registre de la fonction englobante, `false` s'il vient de ses
    /// propres upvalues.
    pub in_stack: bool,
    /// Index du registre ou de l'upvalue source.
    pub index: u8,
}

/// Variable locale du bloc de débogage.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct LocVar {
    /// Nom déclaré.
    pub name: String,
    /// Première instruction où la variable est vivante.
    pub start_pc: u32,
    /// Première instruction où elle ne l'est plus.
    pub end_pc: u32,
}

/// Un prototype de fonction — l'unité compilée de Lua.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Prototype {
    /// Ligne de début (`0` pour le chunk principal).
    pub line_defined: u32,
    /// Ligne de fin.
    pub last_line_defined: u32,
    /// Nombre de paramètres fixes.
    pub num_params: u8,
    /// Indicateur `...`.
    pub is_vararg: u8,
    /// Taille de pile requise.
    pub max_stack_size: u8,
    /// Instructions encodées (32 bits).
    pub code: Vec<u32>,
    /// Pool de constantes.
    pub constants: Vec<Constant>,
    /// Fonctions imbriquées.
    pub protos: Vec<Prototype>,
    /// Upvalues.
    pub upvalues: Vec<UpvalDesc>,
    /// Nom de source (débogage), vide si dépouillé.
    pub source: String,
    /// Ligne de chaque instruction (débogage), vide si dépouillé.
    pub line_info: Vec<u32>,
    /// Variables locales (débogage).
    pub loc_vars: Vec<LocVar>,
    /// Noms d'upvalues (débogage).
    pub upvalue_names: Vec<String>,
}

impl Prototype {
    /// Nombre total d'instructions, prototypes imbriqués compris.
    #[must_use]
    pub fn total_instructions(&self) -> usize {
        self.code.len()
            + self
                .protos
                .iter()
                .map(Self::total_instructions)
                .sum::<usize>()
    }

    /// Nombre total de prototypes imbriqués (récursif).
    #[must_use]
    pub fn total_protos(&self) -> usize {
        self.protos.len() + self.protos.iter().map(Self::total_protos).sum::<usize>()
    }
}

/// Un chunk complet : en-tête + prototype principal.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Chunk {
    /// En-tête décodé.
    pub header: Header,
    /// Fonction principale.
    pub main: Prototype,
}

// ─── Opcodes Lua 5.2 ─────────────────────────────────────────────────────────────────────────

/// Noms des 40 opcodes de Lua 5.2, **dans l'ordre de `lopcodes.h`**. L'ordre est significatif :
/// c'est l'index qui identifie l'instruction, pas le nom.
pub const OPCODE_NAMES: [&str; 40] = [
    "MOVE", "LOADK", "LOADKX", "LOADBOOL", "LOADNIL", "GETUPVAL", "GETTABUP", "GETTABLE",
    "SETTABUP", "SETUPVAL", "SETTABLE", "NEWTABLE", "SELF", "ADD", "SUB", "MUL", "DIV", "MOD",
    "POW", "UNM", "NOT", "LEN", "CONCAT", "JMP", "EQ", "LT", "LE", "TEST", "TESTSET", "CALL",
    "TAILCALL", "RETURN", "FORLOOP", "FORPREP", "TFORCALL", "TFORLOOP", "SETLIST", "CLOSURE",
    "VARARG", "EXTRAARG",
];

/// Mode d'encodage des opérandes (`lopcodes.h`).
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpMode {
    /// `A B C`
    ABC,
    /// `A Bx`
    ABx,
    /// `A sBx`
    AsBx,
    /// `Ax`
    Ax,
}

/// Mode d'encodage de chaque opcode, dans le même ordre que [`OPCODE_NAMES`].
const OP_MODES: [OpMode; 40] = [
    OpMode::ABC,  // MOVE
    OpMode::ABx,  // LOADK
    OpMode::ABx,  // LOADKX
    OpMode::ABC,  // LOADBOOL
    OpMode::ABC,  // LOADNIL
    OpMode::ABC,  // GETUPVAL
    OpMode::ABC,  // GETTABUP
    OpMode::ABC,  // GETTABLE
    OpMode::ABC,  // SETTABUP
    OpMode::ABC,  // SETUPVAL
    OpMode::ABC,  // SETTABLE
    OpMode::ABC,  // NEWTABLE
    OpMode::ABC,  // SELF
    OpMode::ABC,  // ADD
    OpMode::ABC,  // SUB
    OpMode::ABC,  // MUL
    OpMode::ABC,  // DIV
    OpMode::ABC,  // MOD
    OpMode::ABC,  // POW
    OpMode::ABC,  // UNM
    OpMode::ABC,  // NOT
    OpMode::ABC,  // LEN
    OpMode::ABC,  // CONCAT
    OpMode::AsBx, // JMP
    OpMode::ABC,  // EQ
    OpMode::ABC,  // LT
    OpMode::ABC,  // LE
    OpMode::ABC,  // TEST
    OpMode::ABC,  // TESTSET
    OpMode::ABC,  // CALL
    OpMode::ABC,  // TAILCALL
    OpMode::ABC,  // RETURN
    OpMode::AsBx, // FORLOOP
    OpMode::AsBx, // FORPREP
    OpMode::ABC,  // TFORCALL
    OpMode::AsBx, // TFORLOOP
    OpMode::ABC,  // SETLIST
    OpMode::ABx,  // CLOSURE
    OpMode::ABC,  // VARARG
    OpMode::Ax,   // EXTRAARG
];

/// Une instruction décodée en champs.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// Mot brut de 32 bits.
    pub raw: u32,
    /// Index d'opcode (0..39).
    pub opcode: u8,
    /// Champ A.
    pub a: u32,
    /// Champ B (mode ABC).
    pub b: u32,
    /// Champ C (mode ABC).
    pub c: u32,
    /// Champ Bx (modes ABx).
    pub bx: u32,
    /// Champ sBx signé (modes AsBx).
    pub sbx: i32,
    /// Champ Ax (mode Ax).
    pub ax: u32,
}

/// Décale/masque conformément à `lopcodes.h` : `OP` sur 6 bits, `A` sur 8, `C` sur 9, `B` sur 9.
#[must_use]
pub fn decode_instruction(raw: u32) -> Instruction {
    let opcode = (raw & 0x3F) as u8;
    let a = (raw >> 6) & 0xFF;
    let c = (raw >> 14) & 0x1FF;
    let b = (raw >> 23) & 0x1FF;
    let bx = (raw >> 14) & 0x3FFFF;
    // `sBx` est un `Bx` biaisé de `MAXARG_sBx` = (2^18-1)/2.
    let sbx = bx as i32 - 131_071;
    let ax = (raw >> 6) & 0x3FF_FFFF;
    Instruction {
        raw,
        opcode,
        a,
        b,
        c,
        bx,
        sbx,
        ax,
    }
}

impl Instruction {
    /// Nom lisible de l'opcode, ou `OP_<n>` si hors table (bytecode corrompu ou dialecte modifié).
    #[must_use]
    pub fn name(&self) -> String {
        OPCODE_NAMES
            .get(self.opcode as usize)
            .map_or_else(|| format!("OP_{}", self.opcode), |s| (*s).to_string())
    }

    /// Mode d'encodage, `ABC` par défaut pour un opcode inconnu.
    #[must_use]
    pub fn mode(&self) -> OpMode {
        OP_MODES
            .get(self.opcode as usize)
            .copied()
            .unwrap_or(OpMode::ABC)
    }
}

/// Un opérande B/C en mode ABC peut désigner un registre ou une constante : le bit 8 (`BITRK`)
/// distingue les deux. `luac -l` note les constantes `-n` ; on est plus explicite.
fn rk(operand: u32, constants: &[Constant]) -> String {
    const BITRK: u32 = 1 << 8;
    if operand & BITRK != 0 {
        let idx = (operand & !BITRK) as usize;
        constants.get(idx).map_or_else(
            || format!("K{idx}?"),
            |k| format!("K{idx}({})", k.display()),
        )
    } else {
        format!("R{operand}")
    }
}

// ─── Lecture ─────────────────────────────────────────────────────────────────────────────────

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    little_endian: bool,
    size_size_t: usize,
    size_int: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BytecodeError> {
        if self.pos + n > self.data.len() {
            return Err(BytecodeError::UnexpectedEof(self.pos));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, BytecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, BytecodeError> {
        let b = self.take(4)?;
        let arr = [b[0], b[1], b[2], b[3]];
        Ok(if self.little_endian {
            u32::from_le_bytes(arr)
        } else {
            u32::from_be_bytes(arr)
        })
    }

    /// Lit un `int` C dont la taille vient de l'en-tête.
    fn int(&mut self) -> Result<u32, BytecodeError> {
        match self.size_int {
            4 => self.u32(),
            8 => {
                let v = self.size_t()?;
                Ok(v as u32)
            }
            n => Err(BytecodeError::UnsupportedHeader(format!(
                "sizeof(int) = {n}"
            ))),
        }
    }

    fn size_t(&mut self) -> Result<u64, BytecodeError> {
        let n = self.size_size_t;
        let b = self.take(n)?;
        let mut v: u64 = 0;
        if self.little_endian {
            for (i, byte) in b.iter().enumerate() {
                v |= u64::from(*byte) << (8 * i);
            }
        } else {
            for byte in b {
                v = (v << 8) | u64::from(*byte);
            }
        }
        Ok(v)
    }

    fn number(&mut self) -> Result<f64, BytecodeError> {
        let b = self.take(8)?;
        let arr = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        Ok(if self.little_endian {
            f64::from_le_bytes(arr)
        } else {
            f64::from_be_bytes(arr)
        })
    }

    /// Chaîne Lua : `size_t` de longueur (0 = NULL), octets **terminaison `\0` comprise**.
    fn string_bytes(&mut self) -> Result<Vec<u8>, BytecodeError> {
        let size = self.size_t()?;
        if size == 0 {
            return Ok(Vec::new());
        }
        let raw = self.take(size as usize)?;
        // Le `\0` final fait partie de la longueur écrite mais pas de la chaîne.
        Ok(raw[..raw.len().saturating_sub(1)].to_vec())
    }

    fn string(&mut self) -> Result<String, BytecodeError> {
        Ok(String::from_utf8_lossy(&self.string_bytes()?).into_owned())
    }
}

/// Décode un chunk `.lua.bin` complet.
///
/// # Errors
/// [`BytecodeError`] si la signature manque, si l'en-tête décrit un format non géré, ou si le
/// tampon est tronqué.
pub fn parse(data: &[u8]) -> Result<Chunk, BytecodeError> {
    if data.len() < 18 || data[..4] != [0x1B, 0x4C, 0x75, 0x61] || data[4] != 0x52 {
        return Err(BytecodeError::BadSignature);
    }

    let header = Header {
        version: data[4],
        format: data[5],
        little_endian: data[6] == 1,
        size_int: data[7],
        size_size_t: data[8],
        size_instruction: data[9],
        size_number: data[10],
        number_is_integral: data[11] != 0,
    };

    if header.format != 0 {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "format = {} (attendu 0)",
            header.format
        )));
    }
    if data[6] > 1 {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "endianness = {} (attendu 0 ou 1)",
            data[6]
        )));
    }
    if !matches!(header.size_int, 4 | 8) {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "sizeof(int) = {}",
            header.size_int
        )));
    }
    if !matches!(header.size_size_t, 4 | 8) {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "sizeof(size_t) = {}",
            header.size_size_t
        )));
    }
    if header.size_instruction != 4 {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "sizeof(Instruction) = {}",
            header.size_instruction
        )));
    }
    if header.size_number != 8 || header.number_is_integral {
        return Err(BytecodeError::UnsupportedHeader(format!(
            "lua_Number : taille {} intégral {}",
            header.size_number, header.number_is_integral
        )));
    }

    // `LUAC_TAIL` (6 octets) suit l'en-tête depuis 5.2 : il détecte les transferts qui abîment les
    // fins de ligne. On le saute sans l'exiger — un chunk réencodé par un outil tiers peut l'omettre.
    let mut pos = 12;
    const LUAC_TAIL: [u8; 6] = [0x19, 0x93, b'\r', b'\n', 0x1A, b'\n'];
    if data.len() >= pos + 6 && data[pos..pos + 6] == LUAC_TAIL {
        pos += 6;
    }

    let mut reader = Reader {
        data,
        pos,
        little_endian: header.little_endian,
        size_size_t: header.size_size_t as usize,
        size_int: header.size_int as usize,
    };

    let main = read_prototype(&mut reader)?;
    Ok(Chunk { header, main })
}

fn read_prototype(r: &mut Reader<'_>) -> Result<Prototype, BytecodeError> {
    let line_defined = r.int()?;
    let last_line_defined = r.int()?;
    let num_params = r.u8()?;
    let is_vararg = r.u8()?;
    let max_stack_size = r.u8()?;

    let code_len = r.int()? as usize;
    let mut code = Vec::with_capacity(code_len.min(1 << 20));
    for _ in 0..code_len {
        code.push(r.u32()?);
    }

    let const_len = r.int()? as usize;
    let mut constants = Vec::with_capacity(const_len.min(1 << 20));
    for _ in 0..const_len {
        let tag = r.u8()?;
        constants.push(match tag {
            0 => Constant::Nil,
            1 => Constant::Boolean(r.u8()? != 0),
            3 => Constant::Number(r.number()?),
            4 => Constant::String(r.string_bytes()?),
            other => {
                return Err(BytecodeError::Invalid {
                    offset: r.pos,
                    reason: format!("type de constante inconnu {other}"),
                });
            }
        });
    }

    let proto_len = r.int()? as usize;
    let mut protos = Vec::with_capacity(proto_len.min(1 << 16));
    for _ in 0..proto_len {
        protos.push(read_prototype(r)?);
    }

    let upval_len = r.int()? as usize;
    let mut upvalues = Vec::with_capacity(upval_len.min(1 << 16));
    for _ in 0..upval_len {
        let in_stack = r.u8()? != 0;
        let index = r.u8()?;
        upvalues.push(UpvalDesc { in_stack, index });
    }

    // Bloc de débogage — vide si le chunk a été dépouillé (`luac -s`).
    let source = r.string()?;
    let line_count = r.int()? as usize;
    let mut line_info = Vec::with_capacity(line_count.min(1 << 20));
    for _ in 0..line_count {
        line_info.push(r.int()?);
    }

    let locvar_count = r.int()? as usize;
    let mut loc_vars = Vec::with_capacity(locvar_count.min(1 << 16));
    for _ in 0..locvar_count {
        let name = r.string()?;
        let start_pc = r.int()?;
        let end_pc = r.int()?;
        loc_vars.push(LocVar {
            name,
            start_pc,
            end_pc,
        });
    }

    let upname_count = r.int()? as usize;
    let mut upvalue_names = Vec::with_capacity(upname_count.min(1 << 16));
    for _ in 0..upname_count {
        upvalue_names.push(r.string()?);
    }

    Ok(Prototype {
        line_defined,
        last_line_defined,
        num_params,
        is_vararg,
        max_stack_size,
        code,
        constants,
        protos,
        upvalues,
        source,
        line_info,
        loc_vars,
        upvalue_names,
    })
}

// ─── Désassemblage ───────────────────────────────────────────────────────────────────────────

/// Produit un listing lisible du chunk, prototypes imbriqués compris.
///
/// Format proche de `luac -l -l`, enrichi : les opérandes constantes sont résolues en valeur, et
/// la ligne source est rappelée quand la table de débogage est présente.
#[must_use]
pub fn disassemble(chunk: &Chunk) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "; Lua {:#04x} — {}-boutiste, int {} o, size_t {} o, number {} o",
        chunk.header.version,
        if chunk.header.little_endian {
            "petit"
        } else {
            "gros"
        },
        chunk.header.size_int,
        chunk.header.size_size_t,
        chunk.header.size_number
    );
    disassemble_proto(&chunk.main, "main", 0, &mut out);
    out
}

fn disassemble_proto(p: &Prototype, label: &str, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let _ = writeln!(
        out,
        "\n{pad}function {label} ({} params, {} upvalues, {} slots, lignes {}-{})",
        p.num_params,
        p.upvalues.len(),
        p.max_stack_size,
        p.line_defined,
        p.last_line_defined
    );
    if !p.source.is_empty() {
        let _ = writeln!(out, "{pad}; source : {}", p.source);
    }

    for (pc, raw) in p.code.iter().enumerate() {
        let ins = decode_instruction(*raw);
        let line = p
            .line_info
            .get(pc)
            .map_or_else(String::new, |l| format!("[{l}]"));
        let operands = format_operands(&ins, p, pc);
        let _ = writeln!(
            out,
            "{pad}  {pc:>4} {line:>7} {:<10} {operands}",
            ins.name()
        );
    }

    if !p.loc_vars.is_empty() {
        let _ = writeln!(
            out,
            "{pad}  ; locales : {}",
            p.loc_vars
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !p.upvalue_names.is_empty() {
        let _ = writeln!(out, "{pad}  ; upvalues : {}", p.upvalue_names.join(", "));
    }

    for (i, sub) in p.protos.iter().enumerate() {
        disassemble_proto(sub, &format!("{label}:{i}"), depth + 1, out);
    }
}

/// Formate les opérandes selon le mode de l'instruction, en résolvant ce qui peut l'être.
fn format_operands(ins: &Instruction, p: &Prototype, pc: usize) -> String {
    let k = &p.constants;
    match ins.mode() {
        OpMode::ABx => {
            // LOADK/LOADKX/CLOSURE : Bx indexe directement les constantes (ou les prototypes).
            let name = ins.name();
            if name == "CLOSURE" {
                format!("R{} := closure #{}", ins.a, ins.bx)
            } else {
                let value = k
                    .get(ins.bx as usize)
                    .map_or_else(|| "?".to_string(), Constant::display);
                format!("R{} := K{}({value})", ins.a, ins.bx)
            }
        }
        OpMode::AsBx => {
            // Les sauts Lua sont relatifs à l'instruction suivante, pas au début du prototype.
            // Garder le PC dans le listing évite de fabriquer un graphe de contrôle faux.
            let target = (pc as i64) + 1 + i64::from(ins.sbx);
            format!("A={} sBx={} (→ pc {})", ins.a, ins.sbx, target)
        }
        OpMode::Ax => format!("Ax={}", ins.ax),
        OpMode::ABC => {
            let name = ins.name();
            match name.as_str() {
                // Les accès de table par upvalue portent le nom du global dans C : c'est la forme
                // la plus fréquente et la plus parlante d'un script de menu (`GETTABUP 0 _ENV "X"`).
                "GETTABUP" => format!("R{} := Up{}[{}]", ins.a, ins.b, rk(ins.c, k)),
                "SETTABUP" => format!("Up{}[{}] := {}", ins.a, rk(ins.b, k), rk(ins.c, k)),
                "GETTABLE" => format!("R{} := R{}[{}]", ins.a, ins.b, rk(ins.c, k)),
                "SETTABLE" => format!("R{}[{}] := {}", ins.a, rk(ins.b, k), rk(ins.c, k)),
                "CALL" | "TAILCALL" => {
                    let args = if ins.b == 0 {
                        "vararg".to_string()
                    } else {
                        (ins.b - 1).to_string()
                    };
                    let returns = if ins.c == 0 {
                        "multret".to_string()
                    } else {
                        (ins.c - 1).to_string()
                    };
                    format!("R{}(...) — {} args, {} retours", ins.a, args, returns)
                }
                "RETURN" => {
                    let returns = if ins.b == 0 {
                        "multret".to_string()
                    } else {
                        format!("{} valeur(s)", ins.b - 1)
                    };
                    format!("retourne R{}..{} ({returns})", ins.a, ins.b)
                }
                "MOVE" => format!("R{} := R{}", ins.a, ins.b),
                _ => format!("A={} B={} C={}", ins.a, ins.b, ins.c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Décode un chunk produit par la VM elle-même : `mlua` compile une source connue, on relit le
    /// bytecode et on vérifie qu'on retrouve la structure attendue.
    ///
    /// Ce test ne dépend d'aucun asset du jeu — il vérifie le décodeur contre la MÊME
    /// implémentation Lua que celle du moteur (PUC-Rio 5.2.4 vendored), donc contre la vérité.
    #[test]
    fn decode_un_chunk_produit_par_la_vm() {
        let lua = crate::new_vm();
        let dumped = lua
            .load("local function f(a, b) return a + b end return f(1, 2)")
            .set_name("essai")
            .into_function()
            .expect("compilation")
            .dump(false);

        let chunk = parse(&dumped).expect("décodage du bytecode");
        assert_eq!(chunk.header.version, 0x52);
        assert_eq!(chunk.header.size_instruction, 4);

        // Une fonction imbriquée (`f`) doit apparaître dans les prototypes du chunk principal.
        assert_eq!(
            chunk.main.protos.len(),
            1,
            "la fonction locale doit être un prototype imbriqué"
        );
        let f = &chunk.main.protos[0];
        assert_eq!(f.num_params, 2, "f prend deux paramètres");

        // Le corps de `f` doit contenir une addition puis un retour.
        let ops: Vec<String> = f
            .code
            .iter()
            .map(|c| decode_instruction(*c).name())
            .collect();
        assert!(ops.contains(&"ADD".to_string()), "opcodes de f : {ops:?}");
        assert!(
            ops.contains(&"RETURN".to_string()),
            "opcodes de f : {ops:?}"
        );

        // Le listing doit être non vide et nommer les opcodes.
        let listing = disassemble(&chunk);
        assert!(listing.contains("ADD"), "listing :\n{listing}");
        assert!(listing.contains("function main"), "listing :\n{listing}");
    }

    #[test]
    fn listing_resout_les_sauts_depuis_le_pc_et_les_arites_lua() {
        let lua = crate::new_vm();
        let dumped = lua
            .load("local x = ...; if x then return x, 2 end return 0")
            .set_name("controle")
            .into_function()
            .expect("compilation")
            .dump(false);
        let listing = disassemble(&parse(&dumped).expect("décodage"));
        assert!(
            listing.contains("→ pc "),
            "cible de saut absente :\n{listing}"
        );
        assert!(
            listing.contains("retourne") && listing.contains("valeur(s)"),
            "arité Lua absente :\n{listing}"
        );
    }

    #[test]
    fn rejette_un_tampon_qui_nest_pas_du_bytecode() {
        assert!(matches!(
            parse(b"-- juste du texte Lua\nreturn 1\n"),
            Err(BytecodeError::BadSignature)
        ));
    }

    #[test]
    fn rejette_un_en_tete_lua_incoherent_sans_paniquer() {
        let lua = crate::new_vm();
        let dumped = lua
            .load("return 1")
            .into_function()
            .expect("compilation")
            .dump(false);

        let mut bad_format = dumped.clone();
        bad_format[5] = 1;
        assert!(matches!(
            parse(&bad_format),
            Err(BytecodeError::UnsupportedHeader(message)) if message.contains("format")
        ));

        let mut bad_size = dumped;
        bad_size[8] = 3;
        assert!(matches!(
            parse(&bad_size),
            Err(BytecodeError::UnsupportedHeader(message)) if message.contains("size_t")
        ));
    }

    /// Décode les VRAIS `.lua.bin` du jeu présents localement (`data/lua_scripts/`).
    ///
    /// `#[ignore]` : dépend du dépôt d'assets, absent d'une CI. Lancer avec
    /// `cargo test -p nie-lua -- --ignored`.
    #[test]
    #[ignore = "nécessite les assets du jeu (data/lua_scripts)"]
    fn decode_les_vrais_scripts_du_jeu() {
        let dir = nie_formats::vfs::resolve_game_dir().join("data/lua_scripts");
        if !dir.is_dir() {
            eprintln!(
                "skip : {} introuvable (définir NIE_GAME_DIR)",
                dir.display()
            );
            return;
        }
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("lecture de data/lua_scripts")
            .filter_map(Result::ok)
            .filter(|e| e.path().to_string_lossy().ends_with(".lua.bin"))
            .collect();
        assert!(!entries.is_empty(), "aucun .lua.bin trouvé");

        let mut ok = 0usize;
        let mut failures: Vec<String> = Vec::new();
        let mut total_instructions = 0usize;

        for entry in &entries {
            let data = std::fs::read(entry.path()).expect("lecture");
            match parse(&data) {
                Ok(chunk) => {
                    total_instructions += chunk.main.total_instructions();
                    // Un désassemblage ne doit jamais paniquer, quel que soit le script.
                    let listing = disassemble(&chunk);
                    assert!(!listing.is_empty());
                    ok += 1;
                }
                Err(e) => failures.push(format!("{} : {e}", entry.path().display())),
            }
        }

        eprintln!(
            "décodés {ok}/{} scripts réels, {total_instructions} instructions au total",
            entries.len()
        );
        for f in failures.iter().take(10) {
            eprintln!("  échec {f}");
        }
        assert!(failures.is_empty(), "{} échecs de décodage", failures.len());
    }
}
