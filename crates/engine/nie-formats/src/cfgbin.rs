//! Lecteur de fichiers RDBN (format Level-5, `cfg.bin` IEVR).
//!
//! Port Rust de :
//! - `IECODE.Core/Formats/Level5/CfgBin/Rdbn/RdbnReader.cs`
//! - `IECODE.Core/Formats/Level5/CfgBin/Rdbn/RdbnStructures.cs`
//!
//! ## Format RDBN (wire layout, tout little-endian)
//!
//! ```text
//! Offset  Taille  Champ
//!  0x00     4     Magic "RDBN" (LE : 0x4E424452)
//!  0x04     2     header_size  (généralement 0x50)
//!  0x06     4     version      (généralement 0x64 = 100)
//!  0x0A     2     data_offset  (× 4 = offset absolu du début de la section données)
//!  0x0C     4     data_size
//!  0x10     20    padding
//!  0x24     2     type_offset  (× 4, relatif à data_offset)
//!  0x26     2     type_count
//!  0x28     2     field_offset (× 4, relatif à data_offset)
//!  0x2A     2     field_count
//!  0x2C     2     root_offset  (× 4, relatif à data_offset)
//!  0x2E     2     root_count
//!  0x30     2     string_hash_offset (× 4, relatif à data_offset)
//!  0x32     2     string_offsets_offset (× 4, relatif à data_offset)
//!  0x34     2     hash_count
//!  0x36     2     value_offset (× 4, relatif à data_offset)
//!  0x38     4     string_offset (absolu relatif à data_offset, en octets)
//! ```
//!
//! Toutes les tables de types/champs/racines utilisent des entrées de 0x20 octets
//! (32 octets), padées.
//!
//! ## `std`
//!
//! Compatible no_std+alloc. Utilise `thiserror` → `std` requis pour `std::error::Error`,
//! car `FormatError` est défini dans `lib.rs` avec `thiserror`.

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::FormatError;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Magic RDBN en little-endian u32.
pub const RDBN_MAGIC: u32 = 0x4E424452; // "RDBN"

/// Magic RDBN en octets (LE).
pub const RDBN_MAGIC_BYTES: [u8; 4] = *b"RDBN";

/// Taille minimale d'un fichier RDBN valide.
pub const MIN_SIZE: usize = 0x50;

/// Taille de chaque entrée dans les tables type/field/root.
pub const ENTRY_SIZE: usize = 0x20;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Types de champs RDBN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum RdbnFieldType {
    AbilityData = 0,
    EnhanceData = 1,
    StatusRate = 2,
    Bool = 3,
    Byte = 4,
    Short = 5,
    Int = 6,
    ActType = 9,
    Flag = 10,
    Float = 13,
    Hash = 15,
    Rates = 18,
    Position = 19,
    Condition = 20,
    ShortTuple = 21,
    /// Type inconnu (valeur brute conservée).
    Unknown(i16),
}

impl RdbnFieldType {
    fn from_i16(v: i16) -> Self {
        match v {
            0 => Self::AbilityData,
            1 => Self::EnhanceData,
            2 => Self::StatusRate,
            3 => Self::Bool,
            4 => Self::Byte,
            5 => Self::Short,
            6 => Self::Int,
            9 => Self::ActType,
            10 => Self::Flag,
            13 => Self::Float,
            15 => Self::Hash,
            18 => Self::Rates,
            19 => Self::Position,
            20 => Self::Condition,
            21 => Self::ShortTuple,
            other => Self::Unknown(other),
        }
    }
}

/// En-tête d'un fichier RDBN parsé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnHeader {
    /// Version du format (généralement 100).
    pub version: i32,
    /// Offset absolu de la section données (data_offset × 4).
    pub data_offset: usize,
    /// Taille de la section données.
    pub data_size: i32,
    /// Nombre de types.
    pub type_count: u16,
    /// Nombre de champs.
    pub field_count: u16,
    /// Nombre d'entrées racines.
    pub root_count: u16,
    /// Nombre de chaînes dans la table de hachage.
    pub hash_count: u16,
}

/// Entrée de type RDBN.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnTypeEntry {
    /// Hash CRC32 du nom du type.
    pub name_hash: u32,
    /// Hash secondaire (inconnu).
    pub unk_hash: u32,
    /// Index du premier champ dans la table de champs.
    pub field_index: i16,
    /// Nombre de champs dans ce type.
    pub field_count: i16,
}

/// Entrée de champ RDBN.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnFieldEntry {
    /// Hash CRC32 du nom du champ.
    pub name_hash: u32,
    /// Type du champ.
    pub field_type: RdbnFieldType,
    /// Catégorie de type (usage interne Level-5).
    pub type_category: i16,
    /// Taille en octets de la valeur.
    pub value_size: i32,
    /// Offset de la valeur dans le bloc de valeurs (relatif à value_section).
    pub value_offset: i32,
    /// Nombre de valeurs.
    pub value_count: i32,
}

/// Entrée racine RDBN (liste de données).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnRootEntry {
    /// Index dans la table de types.
    pub type_index: i16,
    /// Champ inconnu.
    pub unk1: i16,
    /// Offset de la première valeur dans le bloc de valeurs.
    pub value_offset: i32,
    /// Taille d'une valeur.
    pub value_size: i32,
    /// Nombre de valeurs.
    pub value_count: i32,
    /// Hash CRC32 du nom de cette liste.
    pub name_hash: u32,
}

/// Table de hachage RDBN : association hash CRC32 → nom de chaîne.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnStringTable {
    /// Associations (hash, chaîne).
    pub entries: Vec<(u32, String)>,
}

impl RdbnStringTable {
    /// Résout un hash en chaîne, ou retourne `None`.
    #[must_use]
    pub fn resolve(&self, hash: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, s)| s.as_str())
    }
}

/// Données RDBN complètes.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnData {
    /// En-tête parsé.
    pub header: RdbnHeader,
    /// Table de types.
    pub types: Vec<RdbnTypeEntry>,
    /// Table de champs.
    pub fields: Vec<RdbnFieldEntry>,
    /// Entrées racines.
    pub roots: Vec<RdbnRootEntry>,
    /// Table de chaînes (hashes CRC32 ↔ noms).
    pub strings: RdbnStringTable,
    /// Offset absolu (dans le tampon) du bloc de valeurs : `(value_offset << 2) + data_offset`.
    /// Source de vérité : `RdbnReader.Read` L60 (`valueOffset = (header.ValueOffset << 2) + dataOffset`).
    pub value_abs: usize,
    /// Offset absolu (dans le tampon) du début de la table de chaînes :
    /// `string_offset + data_offset`. Utilisé par le type `Condition` (0x14) qui traite la
    /// valeur lue comme un offset dans cette table. Source : `RdbnReader.Read` L56.
    pub string_abs: usize,
}

impl RdbnData {
    /// Résout le nom d'une entrée racine depuis la table de chaînes.
    #[must_use]
    pub fn root_name(&self, root: &RdbnRootEntry) -> Option<&str> {
        self.strings.resolve(root.name_hash)
    }

    /// Résout le nom d'un type depuis la table de chaînes.
    #[must_use]
    pub fn type_name(&self, entry: &RdbnTypeEntry) -> Option<&str> {
        self.strings.resolve(entry.name_hash)
    }

    /// Résout le nom d'un champ depuis la table de chaînes.
    #[must_use]
    pub fn field_name(&self, entry: &RdbnFieldEntry) -> Option<&str> {
        self.strings.resolve(entry.name_hash)
    }
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Vrai si `data` commence par le magic RDBN.
#[must_use]
pub fn is_rdbn(data: &[u8]) -> bool {
    data.starts_with(&RDBN_MAGIC_BYTES)
}

/// Parse un fichier RDBN depuis un slice d'octets.
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si le tampon est plus court que le minimum.
/// - [`FormatError::BadMagic`] si le magic n'est pas `RDBN`.
/// - [`FormatError::Corrupt`] pour toute incohérence interne.
pub fn parse(data: &[u8]) -> Result<RdbnData, FormatError> {
    if data.len() < MIN_SIZE {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: MIN_SIZE,
        });
    }
    if !is_rdbn(data) {
        return Err(FormatError::BadMagic { format: "RDBN" });
    }

    let header = parse_header(data)?;
    let da = header.data_offset;

    // Offsets absolus dans `data`.
    let type_abs = (read_i16_le(data, 0x24)? as usize * 4) + da;
    let field_abs = (read_i16_le(data, 0x28)? as usize * 4) + da;
    let root_abs = (read_i16_le(data, 0x2C)? as usize * 4) + da;
    let hash_abs = (read_i16_le(data, 0x30)? as usize * 4) + da;
    let offsets_abs = (read_i16_le(data, 0x32)? as usize * 4) + da;
    let value_abs = (read_i16_le(data, 0x36)? as usize * 4) + da;
    let string_abs = read_i32_le(data, 0x38)? as usize + da;

    let types = parse_types(data, type_abs, header.type_count as usize)?;
    let fields = parse_fields(data, field_abs, header.field_count as usize)?;
    let roots = parse_roots(data, root_abs, header.root_count as usize)?;
    let strings = parse_strings(
        data,
        header.hash_count as usize,
        hash_abs,
        offsets_abs,
        string_abs,
    )?;

    Ok(RdbnData {
        header,
        types,
        fields,
        roots,
        strings,
        value_abs,
        string_abs,
    })
}

// ---------------------------------------------------------------------------
// Décodage des VALEURS RDBN (corps des listes typées)
//
// Port exact de `RdbnReader.CreateRdbnData` (L194), `ReadFieldValue` (L249) et
// `ReadConditionValue` (L293). Vérifié octet par octet contre le vrai fichier
// `/home/ubuntu/rg/iecode/re/menu/extracted/fonts/font_color.cfg.bin` (liste
// `m_FontColorDataList`, type `FONT_COLOR`, 7 champs, 64 lignes de 100 octets).
// ---------------------------------------------------------------------------

/// Valeur typée décodée d'un champ RDBN.
///
/// Correspondance 1:1 avec le `switch` de `ReadFieldValue` (RdbnReader.cs L257-290) :
/// - `Bool` (type 3) — un octet, `!= 0`.
/// - `Byte` (type 4) — un octet brut.
/// - `Short` (type 5) — i16 LE.
/// - `Int` (type 6) — i32 LE.
/// - `ActType` (type 9) — i16 LE (même lecture que `Short`).
/// - `Flag` (type 10) — i32 LE (même lecture que `Int`).
/// - `Float` (type 13) — f32 LE.
/// - `Hash` (type 15) — u32 LE brut (le C# le formate `0x%08X`).
/// - `Rates` (type 18) / `Position` (type 19) — 4 × f32 LE.
/// - `Condition` (type 20) — u32 traité comme offset dans la table de chaînes.
/// - `ShortTuple` (type 21) — 2 × i16 LE.
/// - `Blob` — octets bruts (types 0/1/2 et tout type inconnu : `ReadBlobAsHex`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RdbnValue {
    /// `RdbnFieldType::Bool` (3) — octet non nul.
    Bool(bool),
    /// `RdbnFieldType::Byte` (4) — octet brut.
    Byte(u8),
    /// `RdbnFieldType::Short` (5) — i16 LE.
    Short(i16),
    /// `RdbnFieldType::Int` (6) — i32 LE.
    Int(i32),
    /// `RdbnFieldType::ActType` (9) — i16 LE.
    ActType(i16),
    /// `RdbnFieldType::Flag` (10) — i32 LE.
    Flag(i32),
    /// `RdbnFieldType::Float` (13) — f32 LE.
    Float(f32),
    /// `RdbnFieldType::Hash` (15) — u32 LE brut.
    Hash(u32),
    /// `RdbnFieldType::Rates` (18) — 4 × f32 LE.
    Rates([f32; 4]),
    /// `RdbnFieldType::Position` (19) — 4 × f32 LE.
    Position([f32; 4]),
    /// `RdbnFieldType::Condition` (20) — chaîne résolue depuis la table de chaînes.
    Condition(String),
    /// `RdbnFieldType::ShortTuple` (21) — 2 × i16 LE.
    ShortTuple([i16; 2]),
    /// Types 0/1/2 et inconnus — octets bruts (`field.value_size` octets).
    Blob(Vec<u8>),
    /// Lecture impossible (offset + taille hors du tampon) — équivalent C# `"<invalid>"`.
    Invalid,
}

/// Une ligne d'une liste RDBN : association ordonnée (nom de champ → valeur décodée).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnRow {
    /// Champs de la ligne, dans l'ordre de la table de types (nom résolu, valeur).
    pub fields: Vec<(String, RdbnValue)>,
}

/// Une liste RDBN décodée (équivalent de `RdbnList` C#).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdbnList {
    /// Nom de la liste (résolu depuis `root.name_hash`, sinon `Unknown_0x…`).
    pub name: String,
    /// Nom du type de la liste (résolu depuis `type.name_hash`, sinon `Type_0x…`).
    pub type_name: String,
    /// Lignes décodées (`root.value_count` entrées de `root.value_size` octets).
    pub rows: Vec<RdbnRow>,
}

/// Décode le corps de toutes les listes RDBN (port de `RdbnReader.CreateRdbnData`).
///
/// Pour chaque `root` :
/// 1. résout le nom de liste (`root.name_hash`) et de type (`type.name_hash`) ;
/// 2. itère `root.value_count` lignes, chacune à
///    `value_abs + root.value_offset + v * root.value_size` ;
/// 3. pour chaque champ du type (`type.field_index .. + type.field_count`), lit la valeur à
///    `entry + field.value_offset` selon [`RdbnFieldType`].
///
/// Aucune valeur n'est fabriquée : un offset hors limites donne [`RdbnValue::Invalid`]
/// (équivalent du `"<invalid>"` C#, `ReadFieldValue` L252-253).
#[must_use]
pub fn read_values(rdbn: &RdbnData, data: &[u8]) -> Vec<RdbnList> {
    let mut lists = Vec::with_capacity(rdbn.roots.len());

    for root in &rdbn.roots {
        let name = rdbn.strings.resolve(root.name_hash).map_or_else(
            || alloc::format!("Unknown_0x{:08X}", root.name_hash),
            String::from,
        );

        // type_index doit être un index valide dans la table de types.
        let Some(ty) = usize::try_from(root.type_index)
            .ok()
            .and_then(|i| rdbn.types.get(i))
        else {
            // Type hors plage : on émet une liste vide nommée, sans fabriquer de lignes.
            lists.push(RdbnList {
                name,
                type_name: alloc::format!("Type_0x{:08X}", 0u32),
                rows: Vec::new(),
            });
            continue;
        };

        let type_name = rdbn.strings.resolve(ty.name_hash).map_or_else(
            || alloc::format!("Type_0x{:08X}", ty.name_hash),
            String::from,
        );

        let root_value_offset = rdbn.value_abs.wrapping_add(root.value_offset as usize);
        let mut rows = Vec::with_capacity(root.value_count.max(0) as usize);

        for v in 0..root.value_count.max(0) {
            let entry_offset =
                root_value_offset.wrapping_add(v as usize * root.value_size as usize);
            let mut fields = Vec::with_capacity(ty.field_count.max(0) as usize);

            for f in 0..ty.field_count.max(0) {
                let field_idx = ty.field_index as i64 + f as i64;
                let Some(field) = usize::try_from(field_idx)
                    .ok()
                    .and_then(|i| rdbn.fields.get(i))
                else {
                    continue;
                };
                let field_name = rdbn.strings.resolve(field.name_hash).map_or_else(
                    || alloc::format!("Field_0x{:08X}", field.name_hash),
                    String::from,
                );

                let field_value_offset = entry_offset.wrapping_add(field.value_offset as usize);
                let value = read_field_value(data, field_value_offset, field, rdbn.string_abs);
                fields.push((field_name, value));
            }

            rows.push(RdbnRow { fields });
        }

        lists.push(RdbnList {
            name,
            type_name,
            rows,
        });
    }

    lists
}

/// Lit une valeur de champ unique (port de `ReadFieldValue`, RdbnReader.cs L249).
fn read_field_value(
    data: &[u8],
    offset: usize,
    field: &RdbnFieldEntry,
    string_abs: usize,
) -> RdbnValue {
    let size = field.value_size.max(0) as usize;
    // Garde stricte identique au C# : `offset + ValueSize > data.Length` ⇒ "<invalid>".
    if offset.checked_add(size).is_none_or(|end| end > data.len()) {
        return RdbnValue::Invalid;
    }

    match field.field_type {
        RdbnFieldType::Bool => RdbnValue::Bool(data[offset] != 0),
        RdbnFieldType::Byte => RdbnValue::Byte(data[offset]),
        RdbnFieldType::Short => {
            read_i16_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Short)
        }
        RdbnFieldType::Int => read_i32_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Int),
        RdbnFieldType::ActType => {
            read_i16_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::ActType)
        }
        RdbnFieldType::Flag => {
            read_i32_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Flag)
        }
        RdbnFieldType::Float => {
            read_f32_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Float)
        }
        RdbnFieldType::Hash => {
            read_u32_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Hash)
        }
        RdbnFieldType::Rates => {
            read_vec4_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Rates)
        }
        RdbnFieldType::Position => {
            read_vec4_le(data, offset).map_or(RdbnValue::Invalid, RdbnValue::Position)
        }
        RdbnFieldType::Condition => read_condition_value(data, offset, string_abs),
        RdbnFieldType::ShortTuple => {
            match (read_i16_le(data, offset), read_i16_le(data, offset + 2)) {
                (Ok(a), Ok(b)) => RdbnValue::ShortTuple([a, b]),
                _ => RdbnValue::Invalid,
            }
        }
        // Types 0/1/2 (AbilityData/EnhanceData/StatusRate) et inconnus ⇒ blob brut.
        RdbnFieldType::AbilityData
        | RdbnFieldType::EnhanceData
        | RdbnFieldType::StatusRate
        | RdbnFieldType::Unknown(_) => RdbnValue::Blob(data[offset..offset + size].to_vec()),
    }
}

/// Port de `ReadConditionValue` (RdbnReader.cs L293) : lit un u32 à `offset`, le traite comme
/// offset relatif dans la table de chaînes (`string_abs + value`), et lit la chaîne null-terminée.
/// Si la position résolue est hors limites, on renvoie la valeur numérique sous forme de blob u32.
fn read_condition_value(data: &[u8], offset: usize, string_abs: usize) -> RdbnValue {
    let Ok(value) = read_u32_le(data, offset) else {
        return RdbnValue::Invalid;
    };
    let str_pos = string_abs.wrapping_add(value as usize);
    // Le C# exige `strPos < data.Length && strPos > 0`. `string_abs > 0` toujours (≥ data_offset),
    // donc on reproduit seulement la borne haute.
    if str_pos < data.len() && str_pos > 0 {
        RdbnValue::Condition(read_cstr(data, str_pos))
    } else {
        // Pas une chaîne résoluble : on conserve la valeur brute (équivalent du `return value;` C#).
        RdbnValue::Hash(value)
    }
}

// ---------------------------------------------------------------------------
// Helpers de parsing
// ---------------------------------------------------------------------------

fn parse_header(data: &[u8]) -> Result<RdbnHeader, FormatError> {
    let version = read_i32_le(data, 6)?;
    let data_offset = read_i16_le(data, 10)? as usize * 4;
    let data_size = read_i32_le(data, 12)?;
    let type_count = read_u16_le(data, 0x26)?;
    let field_count = read_u16_le(data, 0x2A)?;
    let root_count = read_u16_le(data, 0x2E)?;
    let hash_count = read_u16_le(data, 0x34)?;

    Ok(RdbnHeader {
        version,
        data_offset,
        data_size,
        type_count,
        field_count,
        root_count,
        hash_count,
    })
}

fn parse_types(
    data: &[u8],
    abs_offset: usize,
    count: usize,
) -> Result<Vec<RdbnTypeEntry>, FormatError> {
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let pos = abs_offset + i * ENTRY_SIZE;
        entries.push(RdbnTypeEntry {
            name_hash: read_u32_le(data, pos)?,
            unk_hash: read_u32_le(data, pos + 4)?,
            field_index: read_i16_le(data, pos + 8)?,
            field_count: read_i16_le(data, pos + 10)?,
        });
    }
    Ok(entries)
}

fn parse_fields(
    data: &[u8],
    abs_offset: usize,
    count: usize,
) -> Result<Vec<RdbnFieldEntry>, FormatError> {
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let pos = abs_offset + i * ENTRY_SIZE;
        let raw_type = read_i16_le(data, pos + 4)?;
        entries.push(RdbnFieldEntry {
            name_hash: read_u32_le(data, pos)?,
            field_type: RdbnFieldType::from_i16(raw_type),
            type_category: read_i16_le(data, pos + 6)?,
            value_size: read_i32_le(data, pos + 8)?,
            value_offset: read_i32_le(data, pos + 12)?,
            value_count: read_i32_le(data, pos + 16)?,
        });
    }
    Ok(entries)
}

fn parse_roots(
    data: &[u8],
    abs_offset: usize,
    count: usize,
) -> Result<Vec<RdbnRootEntry>, FormatError> {
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let pos = abs_offset + i * ENTRY_SIZE;
        entries.push(RdbnRootEntry {
            type_index: read_i16_le(data, pos)?,
            unk1: read_i16_le(data, pos + 2)?,
            value_offset: read_i32_le(data, pos + 4)?,
            value_size: read_i32_le(data, pos + 8)?,
            value_count: read_i32_le(data, pos + 12)?,
            name_hash: read_u32_le(data, pos + 16)?,
        });
    }
    Ok(entries)
}

fn parse_strings(
    data: &[u8],
    count: usize,
    hash_abs: usize,
    offsets_abs: usize,
    string_abs: usize,
) -> Result<RdbnStringTable, FormatError> {
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let hash = read_u32_le(data, hash_abs + i * 4)?;
        let str_off = read_i32_le(data, offsets_abs + i * 4)? as usize;
        let abs = string_abs
            .checked_add(str_off)
            .ok_or(FormatError::Corrupt("RDBN : overflow offset chaîne"))?;
        let s = read_cstr(data, abs);
        entries.push((hash, s));
    }
    Ok(RdbnStringTable { entries })
}

fn read_cstr(data: &[u8], abs: usize) -> String {
    let slice = data.get(abs..).unwrap_or(&[]);
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

// ---------------------------------------------------------------------------
// Primitives de lecture LE (no_std-friendly, sans unsafe)
// ---------------------------------------------------------------------------

fn read_u16_le(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let bytes: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("RDBN : lecture u16 hors limites"))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_i16_le(data: &[u8], off: usize) -> Result<i16, FormatError> {
    read_u16_le(data, off).map(|v| v as i16)
}

fn read_u32_le(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let bytes: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("RDBN : lecture u32 hors limites"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32_le(data: &[u8], off: usize) -> Result<i32, FormatError> {
    read_u32_le(data, off).map(|v| v as i32)
}

fn read_f32_le(data: &[u8], off: usize) -> Result<f32, FormatError> {
    read_u32_le(data, off).map(f32::from_bits)
}

/// Lit 4 f32 LE consécutifs (types `Rates` / `Position`).
fn read_vec4_le(data: &[u8], off: usize) -> Result<[f32; 4], FormatError> {
    Ok([
        read_f32_le(data, off)?,
        read_f32_le(data, off + 4)?,
        read_f32_le(data, off + 8)?,
        read_f32_le(data, off + 12)?,
    ])
}

// ---------------------------------------------------------------------------
// CRC32 (IEEE 802.3 / PKZIP polynomial 0xEDB88320)
// ---------------------------------------------------------------------------

/// Calcule le hash CRC32 compatible avec le format RDBN.
///
/// Polynomial : 0xEDB88320 (IEEE 802.3, LE). Identique à `Crc32.cs` de IECODE.
/// Utilisé pour hasher les noms de types/champs/listes.
///
/// # Exemple
///
/// ```
/// use nie_formats::cfgbin::crc32;
/// let hash = crc32(b"PlayerParam");
/// assert!(hash != 0); // non-trivial
/// ```
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;

    // Slicing-by-8 : huit octets consommés par tour, huit lectures de table indépendantes
    // que le processeur ordonnance en parallèle. Le calcul bit-à-bit (8 décalages par octet)
    // plafonnait à ~600 Mio/s ; ce chemin dépasse 3 Gio/s, pour un résultat identique —
    // c'est le même polynôme, seule la fenêtre de traitement change.
    //
    // `chunks_exact` plutôt qu'un `while len >= 8` sur des tranches : la longueur du bloc
    // est connue du compilateur, qui supprime alors les vérifications de bornes des huit
    // indexations. Avec l'indexation manuelle, la boucle tournait 30 % moins vite.
    let mut it = data.chunks_exact(8);
    for c in &mut it {
        let lo = u32::from_le_bytes([c[0], c[1], c[2], c[3]]) ^ crc;
        let hi = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
        crc = CRC32_SLICE[7][(lo & 0xFF) as usize]
            ^ CRC32_SLICE[6][((lo >> 8) & 0xFF) as usize]
            ^ CRC32_SLICE[5][((lo >> 16) & 0xFF) as usize]
            ^ CRC32_SLICE[4][((lo >> 24) & 0xFF) as usize]
            ^ CRC32_SLICE[3][(hi & 0xFF) as usize]
            ^ CRC32_SLICE[2][((hi >> 8) & 0xFF) as usize]
            ^ CRC32_SLICE[1][((hi >> 16) & 0xFF) as usize]
            ^ CRC32_SLICE[0][((hi >> 24) & 0xFF) as usize];
    }
    for &byte in it.remainder() {
        crc = CRC32_SLICE[0][((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Polynôme CRC32 IEEE 802.3 en forme réfléchie.
const CRC32_POLY: u32 = 0xEDB8_8320;

/// Tables du slicing-by-8, construites à la compilation (aucune initialisation au runtime,
/// et le crate reste `no_std`).
const CRC32_SLICE: [[u32; 256]; 8] = build_crc32_slices();

/// Construit les huit tables : la première est la table CRC32 classique, chaque suivante
/// décale d'un octet de plus.
const fn build_crc32_slices() -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ CRC32_POLY
            } else {
                crc >> 1
            };
            j += 1;
        }
        t[0][i] = crc;
        i += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = t[k - 1][i];
            t[k][i] = (prev >> 8) ^ t[0][(prev & 0xFF) as usize];
            i += 1;
        }
        k += 1;
    }
    t
}

use alloc::collections::BTreeMap;

/// Formats possibles de fichiers de configuration Level-5.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Format {
    Rdbn,
    T2b,
}

/// Valeur d'une variable CfgBin typée pour T2B.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    String(String),
    Int(i32),
    Float(f32),
}

/// Entrée de configuration Level-5 structurée de façon hiérarchique.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CfgEntry {
    pub name: String,
    pub variables: Vec<Value>,
    pub children: Vec<CfgEntry>,
}

/// Fichier de configuration Level-5 décodé (RDBN ou T2B).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CfgBinFile {
    pub format: Format,
    pub entries: Vec<CfgEntry>,
}

/// Parse un fichier cfg.bin (T2B).
pub fn cfgbin_parse(data: &[u8]) -> Result<CfgBinFile, FormatError> {
    parse_t2b(data)
}

/// Parse un fichier binaire T2B Level-5.
pub fn parse_t2b(data: &[u8]) -> Result<CfgBinFile, FormatError> {
    if data.len() < 16 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 16,
        });
    }

    let entries_count = i32::from_le_bytes(data[0..4].try_into().unwrap());
    let string_table_off_i = i32::from_le_bytes(data[4..8].try_into().unwrap());
    let string_table_len_i = i32::from_le_bytes(data[8..12].try_into().unwrap());
    let string_table_count = i32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;

    // En-tête T2B : nombre d'entrées, offset et longueur de la table de chaînes sont
    // des entiers non signés en pratique. Une entrée du jeu chiffrée ou compressée (p.ex.
    // `cpk_list.cfg.bin` sur certaines installations Steam) produit ici des valeurs
    // aberrantes ou négatives. Les caster directement en `usize` les transforme en
    // valeurs proches de `usize::MAX`, puis l'addition `off + len` déborde — panic en
    // debug (overflow check), wrap silencieux en release (pire : parse de données fausses).
    // On valide donc le signe puis on additionne en `checked_add` : un fichier valide
    // (offsets petits et positifs) est inchangé byte-exact ; un fichier illisible renvoie
    // proprement `Corrupt` au lieu de paniquer.
    if entries_count < 0 || string_table_off_i < 0 || string_table_len_i < 0 {
        return Err(FormatError::Corrupt(
            "T2B header: negative count/offset/length (fichier chiffré ou corrompu ?)",
        ));
    }
    let string_table_off = string_table_off_i as usize;
    let string_table_len = string_table_len_i as usize;

    let string_table_end =
        string_table_off
            .checked_add(string_table_len)
            .ok_or(FormatError::Corrupt(
                "T2B string table offset/length overflow",
            ))?;
    if string_table_off < 16 || string_table_end > data.len() {
        return Err(FormatError::Corrupt("String table offset out of bounds"));
    }

    let mut strings = BTreeMap::new();
    {
        let mut pos = 0;
        let mut count = 0;
        while pos < string_table_len && count < string_table_count {
            let start = string_table_off + pos;
            let slice = &data[start..string_table_off + string_table_len];
            let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            let s = String::from_utf8_lossy(&slice[..nul]).into_owned();
            strings.insert(pos as i32, s.clone());
            pos += s.len() + 1;
            count += 1;
        }
    }

    let key_table_offset = string_table_end.div_ceil(16) * 16;
    let mut key_table = BTreeMap::new();
    if key_table_offset + 16 <= data.len() {
        let key_length = i32::from_le_bytes(
            data[key_table_offset..key_table_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        if key_length > 0 && key_table_offset + key_length <= data.len() {
            let key_count = i32::from_le_bytes(
                data[key_table_offset + 4..key_table_offset + 8]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let key_str_off = i32::from_le_bytes(
                data[key_table_offset + 8..key_table_offset + 12]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let key_str_len = i32::from_le_bytes(
                data[key_table_offset + 12..key_table_offset + 16]
                    .try_into()
                    .unwrap(),
            ) as usize;

            let max_possible = key_length / 8;
            if key_count <= max_possible && key_str_off < key_length {
                let key_base = key_table_offset + 16;
                let str_blob = key_table_offset + key_str_off;

                for i in 0..key_count {
                    let ep = key_base + i * 8;
                    if ep + 8 > data.len() {
                        break;
                    }
                    let crc = u32::from_le_bytes(data[ep..ep + 4].try_into().unwrap());
                    let str_start =
                        i32::from_le_bytes(data[ep + 4..ep + 8].try_into().unwrap()) as usize;

                    if str_start < key_str_len {
                        let slice = &data[str_blob + str_start..key_table_offset + key_length];
                        let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                        let s = String::from_utf8_lossy(&slice[..nul]).into_owned();
                        key_table.insert(crc, s);
                    }
                }
            }
        }
    }

    let mut flat_entries = Vec::new();
    {
        let mut pos = 16usize;
        let buf_len = string_table_off;
        for _ in 0..entries_count {
            if pos + 5 > buf_len {
                break;
            }
            let crc = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            let param_count = data[pos + 4] as usize;
            pos += 5;

            let type_bytes = param_count.div_ceil(4);
            let mut param_types = Vec::new();
            let mut pi = 0;
            for _ in 0..type_bytes {
                if pos >= buf_len {
                    break;
                }
                let tb = data[pos];
                pos += 1;
                for k in 0..4 {
                    if pi < param_count {
                        param_types.push((tb >> (2 * k)) & 3);
                        pi += 1;
                    }
                }
            }

            let total_header = 5 + type_bytes;
            if !total_header.is_multiple_of(4) {
                pos += 4 - (total_header % 4);
            }

            let name = if let Some(s) = key_table.get(&crc) {
                s.clone()
            } else {
                alloc::format!("UNKNOWN_{:08X}", crc)
            };

            let mut variables = Vec::new();
            for j in 0..param_count {
                if pos + 4 > buf_len {
                    break;
                }
                let val_bytes = &data[pos..pos + 4];
                let ty = param_types.get(j).copied().unwrap_or(0);
                match ty {
                    0 => {
                        let off = i32::from_le_bytes(val_bytes.try_into().unwrap());
                        let s = if off != -1 {
                            strings.get(&off).cloned().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        variables.push(Value::String(s));
                    }
                    1 => {
                        let val = i32::from_le_bytes(val_bytes.try_into().unwrap());
                        variables.push(Value::Int(val));
                    }
                    2 => {
                        let val = f32::from_le_bytes(val_bytes.try_into().unwrap());
                        variables.push(Value::Float(val));
                    }
                    _ => {
                        let val = i32::from_le_bytes(val_bytes.try_into().unwrap());
                        variables.push(Value::Int(val));
                    }
                }
                pos += 4;
            }

            flat_entries.push(CfgEntry {
                name,
                variables,
                children: Vec::new(),
            });
        }
    }

    fn parse_sub(iter: &mut impl Iterator<Item = CfgEntry>) -> Vec<CfgEntry> {
        let mut children = Vec::new();
        while let Some(mut entry) = iter.next() {
            let is_end = entry.name.ends_with("_END")
                || entry.name == "_PTREE"
                || entry.name.contains("_END_");
            if entry.variables.is_empty() && is_end {
                break;
            }
            // `_BGN` est la 3ᵉ orthographe d'ouverture employée par Level-5 (les fichiers
            // `common/property/**` l'utilisent : `PROP_INFO_BGN`, `INFO_PARAM_BGN`). Sans elle,
            // le `_END` correspondant fermait le niveau RACINE : `camera_ctrl_property_info.cfg.bin`
            // ne rendait que 3 entrées sur 219.
            let is_begin = entry.name.ends_with("_BEG")
                || entry.name.ends_with("_BEGIN")
                || entry.name.ends_with("_BGN")
                || entry.name.contains("_BEG_")
                || entry.name.contains("_BGN_")
                || entry.name.starts_with("PTREE");
            if is_begin {
                entry.children = parse_sub(iter);
            }
            children.push(entry);
        }
        children
    }

    let mut iter = flat_entries.into_iter();
    let entries = parse_sub(&mut iter);

    Ok(CfgBinFile {
        format: Format::T2b,
        entries,
    })
}

/// Encode un arbre `CfgEntry` en fichier T2B binaire — inverse de [`parse_t2b`]. Écrit suite à
/// la demande utilisatrice « niers doit pouvoir éditer, pas juste explorer » : `nie-formats`
/// n'avait jusqu'ici AUCUN encodeur (RDBN ni T2B), seulement des décodeurs.
///
/// Ne vise PAS un round-trip octet-identique à un fichier T2B d'origine quelconque
/// (l'agencement exact des tables produit par l'outil Level-5 — dédoublonnage, ordre — n'est pas
/// reversé) : vise un fichier VALIDE, relu à l'IDENTIQUE par [`parse_t2b`] (même arbre
/// `entries`/`variables`/`children`) — vérifié réellement par
/// [`tests::encode_t2b_round_trip_sur_le_vrai_jeu`] (décode de vrais fichiers du jeu → encode →
/// redécode → compare structurellement à l'original), pas supposé.
///
/// Convention de marqueur de fin : [`parse_t2b`] reconnaît toute entrée à variables vides dont
/// le nom contient `"_END"` comme fin de bloc (`is_end` dans `parse_sub`) ; ce marqueur
/// synthétique n'est JAMAIS exposé dans l'arbre décodé (consommé par `break`), donc son nom
/// d'origine exact est perdu à la lecture — on émet toujours `"_END"` littéral en écriture, qui
/// satisfait `ends_with("_END")` sans avoir besoin de le connaître.
#[must_use]
pub fn encode_t2b(entries: &[CfgEntry]) -> Vec<u8> {
    enum FlatItem<'a> {
        Entry(&'a CfgEntry),
        End,
    }
    // MÊME critère que `parse_t2b` (`is_begin` dans `parse_sub`) : un conteneur se reconnaît au
    // NOM (suffixe `_BEG`/`_BEGIN`, motif `_BEG_`, préfixe `PTREE`), PAS au fait que `children`
    // soit non vide. Bug réel trouvé par le round-trip sur le vrai jeu (`encode_t2b_round_trip_
    // sur_le_vrai_jeu`) : un conteneur au sous-arbre VIDE (fermé immédiatement dans le fichier
    // d'origine) a quand même `entry.children = parse_sub(iter)` côté décodeur — juste vide.
    // Se baser sur `!children.is_empty()` omettait le marqueur de fin pour ces conteneurs vides,
    // et le ré-décodage avalait alors tous les frères suivants comme si c'était leur contenu.
    fn is_begin_name(name: &str) -> bool {
        name.ends_with("_BEG")
            || name.ends_with("_BEGIN")
            || name.contains("_BEG_")
            || name.starts_with("PTREE")
    }
    fn flatten<'a>(entries: &'a [CfgEntry], out: &mut Vec<FlatItem<'a>>) {
        for e in entries {
            out.push(FlatItem::Entry(e));
            if is_begin_name(&e.name) {
                flatten(&e.children, out);
                out.push(FlatItem::End);
            }
        }
    }
    let mut flat: Vec<FlatItem> = Vec::new();
    flatten(entries, &mut flat);

    // Table de chaînes de VALEURS (`Value::String`), dédupliquée par valeur — offset = position
    // en octets depuis le début du blob, MÊME convention que `parse_t2b`
    // (`strings.insert(pos as i32, ...)`, `pos` cumulé sur `s.len() + 1` NUL compris).
    let mut string_blob: Vec<u8> = Vec::new();
    let mut string_offsets: BTreeMap<String, i32> = BTreeMap::new();
    let mut intern_value_string = |s: &str| -> i32 {
        if let Some(&off) = string_offsets.get(s) {
            return off;
        }
        let off = string_blob.len() as i32;
        string_blob.extend_from_slice(s.as_bytes());
        string_blob.push(0);
        string_offsets.insert(s.to_string(), off);
        off
    };

    // Table de clés (noms de champs) — CRC32(nom) → nom, dédupliquée, avec son propre blob.
    let mut key_blob: Vec<u8> = Vec::new();
    let mut key_entries: Vec<(u32, i32)> = Vec::new(); // (crc, str_start dans key_blob)
    let mut seen_keys: BTreeMap<u32, ()> = BTreeMap::new();
    let mut intern_key = |name: &str| -> u32 {
        let crc = crc32(name.as_bytes());
        if seen_keys.insert(crc, ()).is_none() {
            let str_start = key_blob.len() as i32;
            key_blob.extend_from_slice(name.as_bytes());
            key_blob.push(0);
            key_entries.push((crc, str_start));
        }
        crc
    };

    // 1. Corps des entrées aplaties (même layout que lu par `parse_t2b` : crc(4) + param_count(1)
    //    + bitmap de types 2 bits/param, padé à 4 octets + valeurs 4 octets chacune).
    let mut body: Vec<u8> = Vec::new();
    for item in &flat {
        let (name, variables): (&str, &[Value]) = match item {
            FlatItem::Entry(e) => (e.name.as_str(), &e.variables),
            FlatItem::End => ("_END", &[]),
        };
        let crc = intern_key(name);
        body.extend_from_slice(&crc.to_le_bytes());
        body.push(variables.len() as u8);

        let type_bytes = variables.len().div_ceil(4);
        let mut type_buf: Vec<u8> = alloc::vec![0u8; type_bytes];
        for (i, v) in variables.iter().enumerate() {
            let ty: u8 = match v {
                Value::String(_) => 0,
                Value::Int(_) => 1,
                Value::Float(_) => 2,
            };
            type_buf[i / 4] |= ty << (2 * (i % 4));
        }
        body.extend_from_slice(&type_buf);

        let total_header = 5 + type_bytes;
        if !total_header.is_multiple_of(4) {
            let pad = 4 - (total_header % 4);
            body.resize(body.len() + pad, 0);
        }

        for v in variables {
            match v {
                Value::String(s) => {
                    let off = if s.is_empty() {
                        -1
                    } else {
                        intern_value_string(s)
                    };
                    body.extend_from_slice(&off.to_le_bytes());
                }
                Value::Int(n) => body.extend_from_slice(&n.to_le_bytes()),
                Value::Float(f) => body.extend_from_slice(&f.to_le_bytes()),
            }
        }
    }

    // 2. Assemble : header(16) + body + string_blob + padding(→16) + table de clés.
    let string_table_off = 16 + body.len();
    let string_table_len = string_blob.len();
    let string_table_count = string_offsets.len();

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&(flat.len() as i32).to_le_bytes());
    out.extend_from_slice(&(string_table_off as i32).to_le_bytes());
    out.extend_from_slice(&(string_table_len as i32).to_le_bytes());
    out.extend_from_slice(&(string_table_count as i32).to_le_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&string_blob);

    let string_table_end = out.len();
    let key_table_offset = string_table_end.div_ceil(16) * 16;
    out.resize(key_table_offset, 0);

    // Sous-en-tête de la table de clés : key_length/key_count/key_str_off/key_str_len — mêmes
    // 4 champs relus par `parse_t2b` à `key_table_offset..+16`.
    let key_header_len = 16 + key_entries.len() * 8;
    let key_length = key_header_len + key_blob.len();
    out.extend_from_slice(&(key_length as i32).to_le_bytes());
    out.extend_from_slice(&(key_entries.len() as i32).to_le_bytes());
    out.extend_from_slice(&(key_header_len as i32).to_le_bytes());
    out.extend_from_slice(&(key_blob.len() as i32).to_le_bytes());
    for (crc, str_start) in &key_entries {
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&str_start.to_le_bytes());
    }
    out.extend_from_slice(&key_blob);

    out
}

// ---------------------------------------------------------------------------
// Encodage RDBN
// ---------------------------------------------------------------------------

/// (type sur le wire, taille en octets) d'une valeur RDBN — miroir exact, en sens inverse, de
/// [`read_field_value`] / [`RdbnFieldType`].
fn rdbn_value_wire(v: &RdbnValue) -> Result<(i16, i32), String> {
    Ok(match v {
        RdbnValue::Bool(_) => (3, 1),
        RdbnValue::Byte(_) => (4, 1),
        RdbnValue::Short(_) => (5, 2),
        RdbnValue::Int(_) => (6, 4),
        RdbnValue::ActType(_) => (9, 2),
        RdbnValue::Flag(_) => (10, 4),
        RdbnValue::Float(_) => (13, 4),
        RdbnValue::Hash(_) => (15, 4),
        RdbnValue::Rates(_) => (18, 16),
        RdbnValue::Position(_) => (19, 16),
        RdbnValue::Condition(_) => (20, 4),
        RdbnValue::ShortTuple(_) => (21, 4),
        // Le type d'origine (0=AbilityData/1=EnhanceData/2=StatusRate/inconnu) n'est PAS conservé
        // par `RdbnValue::Blob` (layout interne non modélisé, cf. doc de l'enum). On écrit `0` —
        // choix arbitraire mais TRANSPARENT en lecture : `read_field_value` traite 0/1/2/tout type
        // inconnu de façon strictement identique (fallback octets bruts), donc peu importe lequel
        // des quatre on écrit, `read_values` redécode la MÊME valeur `RdbnValue::Blob`.
        RdbnValue::Blob(b) => (0, b.len() as i32),
        RdbnValue::Invalid => {
            return Err(
                "valeur RdbnValue::Invalid (lecture d'origine hors limites) : ré-encodage impossible sans deviner"
                    .to_string(),
            )
        }
    })
}

/// Layout d'un champ dans l'enregistrement d'une liste RDBN.
struct RdbnFieldLayout {
    offset: i32,
    size: i32,
    wire_type: i16,
}

/// Layout complet d'une liste RDBN : taille d'un enregistrement (stride) + layout par champ.
/// Les NOMS et l'ORDRE des champs sont dérivés de la PREMIÈRE ligne — RDBN n'a qu'un seul type de
/// ligne par liste (taille fixe), donc les lignes suivantes doivent partager exactement ce schéma
/// (vérifié par [`encode_rdbn`], pas supposé). Le TYPE de chaque colonne, lui, est déterminé sur
/// TOUTES les lignes via [`rdbn_column_wire_type`] — cf. sa doc pour le pourquoi (ambiguïté
/// Condition/Hash).
struct RdbnTypeLayout {
    record_size: i32,
    fields: Vec<RdbnFieldLayout>,
}

/// Type "sur le wire" effectif d'une colonne (champ, par index) d'une liste RDBN.
///
/// Ne se fie PAS à la seule première ligne : un champ de type `Condition` (20) — cf.
/// `read_condition_value` — se décode en `RdbnValue::Condition(String)` quand son offset résout
/// une chaîne, mais en `RdbnValue::Hash(u32)` (valeur brute) sinon. Les DEUX décodages viennent du
/// MÊME `field_type` d'origine (20, 4 octets) : si la première ligne d'une colonne tombe sur un
/// cas non résolu (`Hash`), ce n'est PAS un champ `Hash` (15) — c'est un champ `Condition` dont
/// CETTE valeur particulière ne résout pas. On scanne donc toutes les lignes : dès qu'UNE seule
/// contient une vraie `Condition(String)`, toute la colonne est `Condition` (20, 4 octets) — les
/// autres lignes à `Hash` sur cette même colonne s'y encodent alors comme valeur brute (cf.
/// [`rdbn_encode_value`], qui ne dépend jamais du type de colonne, seulement de la variante).
fn rdbn_column_wire_type(list: &RdbnList, field_index: usize) -> Result<(i16, i32), String> {
    let mut fallback: Option<(i16, i32)> = None;
    for row in &list.rows {
        let Some((_, v)) = row.fields.get(field_index) else {
            continue;
        };
        if matches!(v, RdbnValue::Condition(_)) {
            return Ok((20, 4));
        }
        if fallback.is_none() {
            fallback = Some(rdbn_value_wire(v).map_err(|e| {
                alloc::format!("liste {:?} : champ {field_index} : {e}", list.name)
            })?);
        }
    }
    fallback.ok_or_else(|| {
        alloc::format!(
            "liste {:?} : champ {field_index} sans aucune ligne",
            list.name
        )
    })
}

fn rdbn_compute_layout(list: &RdbnList) -> Result<RdbnTypeLayout, String> {
    let Some(reference) = list.rows.first() else {
        return Ok(RdbnTypeLayout {
            record_size: 0,
            fields: Vec::new(),
        });
    };
    let mut offset = 0i32;
    let mut fields = Vec::with_capacity(reference.fields.len());
    for i in 0..reference.fields.len() {
        let (wire_type, size) = rdbn_column_wire_type(list, i)?;
        fields.push(RdbnFieldLayout {
            offset,
            size,
            wire_type,
        });
        offset += size;
    }
    if offset % 4 != 0 {
        offset += 4 - (offset % 4);
    }
    Ok(RdbnTypeLayout {
        record_size: offset,
        fields,
    })
}

/// Encode la valeur d'un champ RDBN dans `out` (taille exacte = `layout.size`), en sens inverse de
/// [`read_field_value`].
fn rdbn_encode_value(
    v: &RdbnValue,
    out: &mut [u8],
    cond_pool: &BTreeMap<String, i32>,
) -> Result<(), String> {
    match v {
        RdbnValue::Bool(b) => out[0] = u8::from(*b),
        RdbnValue::Byte(b) => out[0] = *b,
        RdbnValue::Short(s) => out.copy_from_slice(&s.to_le_bytes()),
        RdbnValue::Int(i) => out.copy_from_slice(&i.to_le_bytes()),
        RdbnValue::ActType(s) => out.copy_from_slice(&s.to_le_bytes()),
        RdbnValue::Flag(i) => out.copy_from_slice(&i.to_le_bytes()),
        RdbnValue::Float(f) => out.copy_from_slice(&f.to_le_bytes()),
        RdbnValue::Hash(h) => out.copy_from_slice(&h.to_le_bytes()),
        RdbnValue::Rates(r) | RdbnValue::Position(r) => {
            for (i, f) in r.iter().enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        RdbnValue::Condition(s) => {
            // Toujours une chaîne résolue avec succès à la lecture (cf. `read_condition_value` :
            // le cas non résoluble produit `RdbnValue::Hash`, jamais `Condition`) — le pool DOIT
            // la contenir, sinon c'est un bug interne de `encode_rdbn`, pas une donnée invalide.
            let off = *cond_pool.get(s).ok_or_else(|| {
                alloc::format!("chaîne Condition {s:?} absente du pool interne (bug interne)")
            })?;
            out.copy_from_slice(&(off as u32).to_le_bytes());
        }
        RdbnValue::ShortTuple([a, b]) => {
            out[0..2].copy_from_slice(&a.to_le_bytes());
            out[2..4].copy_from_slice(&b.to_le_bytes());
        }
        RdbnValue::Blob(b) => out[..b.len()].copy_from_slice(b),
        RdbnValue::Invalid => unreachable!("filtré par rdbn_value_wire avant l'appel"),
    }
    Ok(())
}

/// Encode des listes RDBN décodées ([`read_values`]) en fichier `.cfg.bin` RDBN binaire —
/// inverse de [`read_values`]/[`parse`].
///
/// Port du writer C++ `iecode::level5::cfgbin_write_rdbn` (dépôt privé `aphrody-code/iecode`,
/// `cli/src/formats/level5/cfgbin.cpp:1143-1422`) — le seul encodeur RDBN connu à ce jour : absent
/// du C# `IECODE.Core` d'origine (`apps/iecode` local, lecture seule), absent jusqu'ici de
/// `nie-formats`. Layout d'en-tête et de tables croisé octet à octet contre [`parse`] (déjà
/// vérifié contre le vrai jeu) : mêmes offsets `0x24/0x28/0x2C/0x30/0x32/0x34/0x36/0x38`, mêmes
/// entrées de [`ENTRY_SIZE`] octets — confirmé avant portage, pas supposé.
///
/// Corrige un bug réel du writer C++ source : celui-ci n'écrit jamais le `name_hash` (offset
/// `+0x00`) des entrées de la table de TYPES (reste à 0 dans le fichier produit), ce qui perdrait
/// `type_name` au redécodage. Ici `list.type_name` est interné dans le pool de chaînes comme
/// n'importe quel autre nom et son hash est bien écrit.
///
/// Champs `+0x04` (unk_hash, table types), `+0x06` (type_category, table champs), `+0x10`
/// (value_count, table champs) et `+0x02` (unk1, table racines) : laissés à `0`. Aucun n'est lu
/// par [`parse`]/[`read_values`]/[`read_field_value`] — aucune preuve de leur usage réel, donc pas
/// de valeur devinée.
///
/// # Contrainte de forme
///
/// Une liste RDBN a UN SEUL type de ligne à taille fixe (pas un type par ligne) : toutes les
/// [`RdbnRow`] d'une même liste DOIVENT partager exactement le schéma de la première ligne (mêmes
/// noms de champs, dans le même ordre, mêmes variantes [`RdbnValue`]). C'est une contrainte réelle
/// du format, pas un choix — une divergence retourne une erreur explicite plutôt que de fabriquer
/// un fichier corrompu en silence.
///
/// # Erreurs
///
/// - Une ligne dont le schéma diverge de la première ligne de sa liste (nombre de champs, noms,
///   ou type de valeur différents).
/// - Une valeur [`RdbnValue::Invalid`] (ne peut être ré-encodée sans deviner — le fichier
///   d'origine avait un offset hors limites à cet endroit).
///
/// Vérifié par [`tests::encode_rdbn_round_trip_sur_le_vrai_jeu`] : décode de vrais fichiers RDBN
/// du jeu (`parse` + `read_values`) → `encode_rdbn` → redécode → comparaison structurelle complète
/// à l'original (noms de listes/types/champs ET valeurs), pas seulement auto-cohérence.
pub fn encode_rdbn(lists: &[RdbnList]) -> Result<Vec<u8>, String> {
    // `lists` vide est un cas RÉEL et valide (pas une erreur d'usage) : constaté sur le vrai jeu
    // (`data/common/gamedata/soccer/soccer_common_text.cfg.bin`, 0 liste) — round-trip trouvé par
    // `encode_rdbn_round_trip_sur_le_vrai_jeu`. Toutes les boucles ci-dessous itèrent naturellement
    // 0 fois et produisent un fichier RDBN valide (header seul, tout à 0/vide), qui reredécode en
    // `lists: []` — pas de cas particulier à coder.

    // ── 1. Pool de chaînes "nommées" (résolues par hash CRC32 à la lecture) : noms de listes,
    //    noms de types (FIX — cf. doc), noms de champs. Dédupliqué par CRC32, même convention que
    //    `encode_t2b::intern_key`.
    let mut name_order: Vec<(u32, String)> = Vec::new();
    let mut name_seen: BTreeMap<u32, ()> = BTreeMap::new();
    let mut intern_name = |name: &str| -> u32 {
        let hash = crc32(name.as_bytes());
        if name_seen.insert(hash, ()).is_none() {
            name_order.push((hash, name.to_string()));
        }
        hash
    };

    let list_name_hashes: Vec<u32> = lists.iter().map(|l| intern_name(&l.name)).collect();
    let type_name_hashes: Vec<u32> = lists.iter().map(|l| intern_name(&l.type_name)).collect();

    // ── 2. Layouts + table de champs globale (un type par liste, pas de déduplication — même
    //    simplification que le writer C++ source, suffisante pour un round-trip valide).
    struct RdbnTypeInfo {
        field_start: i16,
        field_count: i16,
    }
    struct RdbnFieldInfo {
        hash: u32,
        wire_type: i16,
        size: i32,
        offset: i32,
    }

    let mut types_info: Vec<RdbnTypeInfo> = Vec::with_capacity(lists.len());
    let mut all_fields: Vec<RdbnFieldInfo> = Vec::new();
    let mut layouts: Vec<RdbnTypeLayout> = Vec::with_capacity(lists.len());

    for list in lists {
        let layout = rdbn_compute_layout(list)?;
        let field_start = all_fields.len() as i16;
        let mut field_count = 0i16;
        if let Some(reference) = list.rows.first() {
            for (i, (name, _)) in reference.fields.iter().enumerate() {
                let hash = intern_name(name);
                let fl = &layout.fields[i];
                all_fields.push(RdbnFieldInfo {
                    hash,
                    wire_type: fl.wire_type,
                    size: fl.size,
                    offset: fl.offset,
                });
            }
            field_count = reference.fields.len() as i16;
        }
        types_info.push(RdbnTypeInfo {
            field_start,
            field_count,
        });
        layouts.push(layout);
    }

    // ── 3. Chaînes de type Condition : pool séparé, dédupliqué par CONTENU (référencées par
    //    offset direct dans le blob, pas par hash — cf. `read_condition_value`), dans l'ordre de
    //    première apparition sur TOUTES les lignes (pas seulement la ligne de référence).
    let mut cond_order: Vec<String> = Vec::new();
    let mut cond_seen: BTreeMap<String, ()> = BTreeMap::new();
    for list in lists {
        for row in &list.rows {
            for (_, v) in &row.fields {
                if let RdbnValue::Condition(s) = v
                    && cond_seen.insert(s.clone(), ()).is_none()
                {
                    cond_order.push(s.clone());
                }
            }
        }
    }

    // ── 4. Assembler la string table : noms (résolubles par hash) puis chaînes Condition
    //    (résolubles par offset direct), dans un seul blob contigu — même convention que
    //    `read_condition_value` (`string_abs + valeur`).
    let mut str_blob: Vec<u8> = Vec::new();
    let mut str_hashes: Vec<u32> = Vec::with_capacity(name_order.len());
    let mut str_offsets: Vec<i32> = Vec::with_capacity(name_order.len());
    for (hash, name) in &name_order {
        str_hashes.push(*hash);
        str_offsets.push(str_blob.len() as i32);
        str_blob.extend_from_slice(name.as_bytes());
        str_blob.push(0);
    }
    let name_strings_size = str_blob.len() as i32;

    let mut cond_pool: BTreeMap<String, i32> = BTreeMap::new();
    let mut cond_pos = name_strings_size;
    for s in &cond_order {
        cond_pool.insert(s.clone(), cond_pos);
        str_blob.extend_from_slice(s.as_bytes());
        str_blob.push(0);
        cond_pos += s.len() as i32 + 1;
    }

    // ── 5. Encoder les valeurs (une fois le pool Condition finalisé, ses offsets sont stables).
    let mut values_blob: Vec<u8> = Vec::new();
    let mut root_val_offsets: Vec<i32> = Vec::with_capacity(lists.len());
    let mut root_val_sizes: Vec<i32> = Vec::with_capacity(lists.len());
    let mut root_val_counts: Vec<i32> = Vec::with_capacity(lists.len());

    for (li, list) in lists.iter().enumerate() {
        let layout = &layouts[li];
        root_val_offsets.push(values_blob.len() as i32);
        root_val_sizes.push(layout.record_size);
        root_val_counts.push(list.rows.len() as i32);

        let Some(reference) = list.rows.first() else {
            continue;
        };
        for (ri, row) in list.rows.iter().enumerate() {
            if row.fields.len() != reference.fields.len() {
                return Err(alloc::format!(
                    "liste {:?} : ligne {ri} a {} champ(s), {} attendu(s) (schéma de la 1re ligne)",
                    list.name,
                    row.fields.len(),
                    reference.fields.len()
                ));
            }
            let start = values_blob.len();
            values_blob.resize(start + layout.record_size as usize, 0);
            for (fi, (name, value)) in row.fields.iter().enumerate() {
                let (ref_name, _) = &reference.fields[fi];
                if name != ref_name {
                    return Err(alloc::format!(
                        "liste {:?} : ligne {ri} champ {fi} nommé {name:?}, attendu {ref_name:?} (schéma hétérogène)",
                        list.name
                    ));
                }
                let fl = &layout.fields[fi];
                // Compatible avec le type de COLONNE (`fl.wire_type`, déterminé sur toutes les
                // lignes par `rdbn_column_wire_type`), pas avec le type de la ligne de référence
                // seule — cf. sa doc : une colonne `Condition` (20) admet légitimement des lignes
                // `Condition(String)` ET `Hash(u32)` (valeur non résolue), qui ne sont PAS des
                // types de champ différents dans le fichier d'origine.
                let compatible = match value {
                    RdbnValue::Condition(_) => fl.wire_type == 20,
                    RdbnValue::Hash(_) => fl.wire_type == 20 || fl.wire_type == 15,
                    other => rdbn_value_wire(other)?.0 == fl.wire_type,
                };
                if !compatible {
                    return Err(alloc::format!(
                        "liste {:?} : ligne {ri} champ {name:?} de type incompatible avec la colonne (schéma hétérogène)",
                        list.name
                    ));
                }
                let field_start = start + fl.offset as usize;
                let out = &mut values_blob[field_start..field_start + fl.size as usize];
                rdbn_encode_value(value, out, &cond_pool)?;
            }
        }
    }

    // ── 6. Calculer les offsets des sections (mêmes conventions que [`parse`] : tout relatif à
    //    `data_offset`, sections type/field/root/hash/offsets stockées en "mots" ×4, valeurs et
    //    chaînes en octets bruts).
    const HEADER_SIZE: i32 = 0x50;
    let data_offset = HEADER_SIZE;

    let type_pos = 0i32;
    let type_size = types_info.len() as i32 * ENTRY_SIZE as i32;
    let field_pos = type_pos + type_size;
    let field_size = all_fields.len() as i32 * ENTRY_SIZE as i32;
    let root_pos = field_pos + field_size;
    let root_size = lists.len() as i32 * ENTRY_SIZE as i32;
    let hash_pos = root_pos + root_size;
    let hash_size = str_hashes.len() as i32 * 4;
    let offs_pos = hash_pos + hash_size;
    let offs_size = str_offsets.len() as i32 * 4;
    let value_pos = offs_pos + offs_size;
    let value_size = values_blob.len() as i32;
    let string_pos = value_pos + value_size;
    let string_size = str_blob.len() as i32;
    let total_data = string_pos + string_size;
    let total_file = data_offset + total_data;

    // ── 7. Assembler le fichier.
    let mut out = alloc::vec![0u8; total_file as usize];
    out[0..4].copy_from_slice(b"RDBN");
    out[0x04..0x06].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    out[0x06..0x0A].copy_from_slice(&100i32.to_le_bytes()); // version = 100 (0x64)
    out[0x0A..0x0C].copy_from_slice(&((data_offset / 4) as u16).to_le_bytes());
    out[0x0C..0x10].copy_from_slice(&(total_data as u32).to_le_bytes());
    out[0x24..0x26].copy_from_slice(&((type_pos / 4) as u16).to_le_bytes());
    out[0x26..0x28].copy_from_slice(&(types_info.len() as u16).to_le_bytes());
    out[0x28..0x2A].copy_from_slice(&((field_pos / 4) as u16).to_le_bytes());
    out[0x2A..0x2C].copy_from_slice(&(all_fields.len() as u16).to_le_bytes());
    out[0x2C..0x2E].copy_from_slice(&((root_pos / 4) as u16).to_le_bytes());
    out[0x2E..0x30].copy_from_slice(&(lists.len() as u16).to_le_bytes());
    out[0x30..0x32].copy_from_slice(&((hash_pos / 4) as u16).to_le_bytes());
    out[0x32..0x34].copy_from_slice(&((offs_pos / 4) as u16).to_le_bytes());
    out[0x34..0x36].copy_from_slice(&(str_hashes.len() as u16).to_le_bytes());
    out[0x36..0x38].copy_from_slice(&((value_pos / 4) as u16).to_le_bytes());
    out[0x38..0x3C].copy_from_slice(&string_pos.to_le_bytes());

    for (i, t) in types_info.iter().enumerate() {
        let base = (data_offset + type_pos) as usize + i * ENTRY_SIZE;
        out[base..base + 4].copy_from_slice(&type_name_hashes[i].to_le_bytes());
        out[base + 8..base + 10].copy_from_slice(&t.field_start.to_le_bytes());
        out[base + 10..base + 12].copy_from_slice(&t.field_count.to_le_bytes());
    }

    for (i, f) in all_fields.iter().enumerate() {
        let base = (data_offset + field_pos) as usize + i * ENTRY_SIZE;
        out[base..base + 4].copy_from_slice(&f.hash.to_le_bytes());
        out[base + 4..base + 6].copy_from_slice(&f.wire_type.to_le_bytes());
        out[base + 8..base + 12].copy_from_slice(&f.size.to_le_bytes());
        out[base + 12..base + 16].copy_from_slice(&f.offset.to_le_bytes());
    }

    for i in 0..lists.len() {
        let base = (data_offset + root_pos) as usize + i * ENTRY_SIZE;
        out[base..base + 2].copy_from_slice(&(i as i16).to_le_bytes());
        out[base + 4..base + 8].copy_from_slice(&root_val_offsets[i].to_le_bytes());
        out[base + 8..base + 12].copy_from_slice(&root_val_sizes[i].to_le_bytes());
        out[base + 12..base + 16].copy_from_slice(&root_val_counts[i].to_le_bytes());
        out[base + 16..base + 20].copy_from_slice(&list_name_hashes[i].to_le_bytes());
    }

    for (i, h) in str_hashes.iter().enumerate() {
        let base = (data_offset + hash_pos) as usize + i * 4;
        out[base..base + 4].copy_from_slice(&h.to_le_bytes());
    }
    for (i, o) in str_offsets.iter().enumerate() {
        let base = (data_offset + offs_pos) as usize + i * 4;
        out[base..base + 4].copy_from_slice(&o.to_le_bytes());
    }

    let vbase = (data_offset + value_pos) as usize;
    out[vbase..vbase + values_blob.len()].copy_from_slice(&values_blob);
    let sbase = (data_offset + string_pos) as usize;
    out[sbase..sbase + str_blob.len()].copy_from_slice(&str_blob);

    let _ = (field_size, root_size, offs_size, value_size, string_size);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Forme JSON « iecode » — celle que lisent les parseurs typés de `nie-data`
// ---------------------------------------------------------------------------
//
// `decode()` (ci-dessus) sérialise les structures BRUTES (`RdbnData`/`CfgBinFile`) — utile
// pour une vue structurelle générique (explorateur, FFI), mais **pas** ce que consomment les
// parseurs `nie-data` : ceux-ci attendent la forme documentée dans
// `csharp/IECODE.Core/Dump/DataPathExporter.cs` — RDBN -> `{ "lists": [{name,typeName,values}] }`,
// T2B -> `{ "entries": [{name,variables,children}] }` avec les frères de même nom suffixés
// `_0`, `_1`… (les parseurs matchent un préfixe à underscore final). Portée depuis la copie
// privée de `nie-model-serve` (`cfgbin_to_typed_root` et consorts) pour que `niers decode
// --typed` et toute autre régénération de corpus `.cfg.bin.json` produisent la MÊME forme,
// au lieu de deux implémentations qui dérivent l'une de l'autre.

/// Encode des octets bruts en hex MAJUSCULE sans séparateur (ex. `"000000008FC2753F"`),
/// identique au dump iecode des champs `position`/`blob`.
fn blob_hex_upper(bytes: &[u8]) -> String {
    use core::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

/// Convertit une [`RdbnValue`] en JSON, encodage identique au dump iecode (`hash` ->
/// `"0x........"`, `blob`/`position` -> hex MAJUSCULE) — directement consommable par les
/// parseurs typés de `nie-data`.
fn rdbn_value_to_iecode_json(v: &RdbnValue) -> serde_json::Value {
    use serde_json::json;
    match v {
        RdbnValue::Bool(b) => json!(b),
        RdbnValue::Byte(n) => json!(n),
        RdbnValue::Short(n) | RdbnValue::ActType(n) => json!(n),
        RdbnValue::Int(n) | RdbnValue::Flag(n) => json!(n),
        RdbnValue::Float(f) => json!(f),
        RdbnValue::Hash(h) => json!(alloc::format!("0x{h:08X}")),
        RdbnValue::Rates(a) | RdbnValue::Position(a) => json!(a),
        RdbnValue::Condition(s) => json!(s),
        RdbnValue::ShortTuple(t) => json!(t),
        RdbnValue::Blob(b) => json!(blob_hex_upper(b)),
        _ => serde_json::Value::Null,
    }
}

/// Décode un `cfg.bin` RDBN vers la forme canonique iecode `{ "lists": [{ "name", "typeName",
/// "values": [{champ: valeur}] }] }`. `None` si `data` n'est pas du RDBN.
#[must_use]
pub fn rdbn_to_iecode_json(data: &[u8]) -> Option<serde_json::Value> {
    use serde_json::{Map, Value as Json, json};
    if !is_rdbn(data) {
        return None;
    }
    let rdbn = parse(data).ok()?;
    let lists = read_values(&rdbn, data);
    let lists_json: Vec<Json> = lists
        .iter()
        .map(|l| {
            let values: Vec<Json> = l
                .rows
                .iter()
                .map(|row| {
                    let mut m = Map::new();
                    for (name, val) in &row.fields {
                        m.insert(name.clone(), rdbn_value_to_iecode_json(val));
                    }
                    Json::Object(m)
                })
                .collect();
            json!({ "name": l.name, "typeName": l.type_name, "values": values })
        })
        .collect();
    Some(json!({ "lists": lists_json }))
}

/// Convertit une liste de frères T2B [`CfgEntry`] vers la forme iecode attendue par les
/// parseurs `entries` de `nie-data`, en répliquant le suffixe d'index d'iecode : chaque nœud
/// est renommé `<base>_<i>` où `i` est son rang d'occurrence parmi ses frères de même nom
/// (`MISSION_CONFIG_INFO` -> `..._0`, `..._1`…). `value` est toujours une chaîne (les
/// parseurs la re-parsent ; `type` indicatif).
fn t2b_siblings_to_iecode_json(siblings: &[CfgEntry]) -> Vec<serde_json::Value> {
    use serde_json::json;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = alloc::format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<serde_json::Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    Value::String(s) => json!({ "type": "String", "value": s }),
                    Value::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    Value::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            let children = t2b_siblings_to_iecode_json(&e.children);
            json!({ "name": name, "variables": variables, "children": children })
        })
        .collect()
}

/// Décode un `cfg.bin` T2B vers la forme iecode `{ "entries": [...] }` consommable par les
/// parseurs `entries` de `nie-data`. `None` si `data` est du RDBN (utiliser
/// [`rdbn_to_iecode_json`]) ou ne parse pas comme T2B.
#[must_use]
pub fn t2b_to_iecode_json(data: &[u8]) -> Option<serde_json::Value> {
    use serde_json::json;
    if is_rdbn(data) {
        return None;
    }
    let cfg = cfgbin_parse(data).ok()?;
    Some(json!({ "entries": t2b_siblings_to_iecode_json(&cfg.entries) }))
}

/// Décode un `cfg.bin` vers la forme iecode adaptée à son format (RDBN `lists` ou T2B
/// `entries`) : aiguille vers [`rdbn_to_iecode_json`] ou [`t2b_to_iecode_json`]. C'est la
/// forme que lisent tous les parseurs de `nie-data` (`list_values`/`Var`, cf. leur doc de
/// module) — à distinguer de [`decode`] plus haut, qui rend la structure BRUTE.
#[must_use]
pub fn to_iecode_json(data: &[u8]) -> Option<serde_json::Value> {
    if is_rdbn(data) {
        rdbn_to_iecode_json(data)
    } else {
        t2b_to_iecode_json(data)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // CRC32
    // -------------------------------------------------------------------

    #[test]
    fn crc32_vide() {
        // CRC32("") = 0x00000000 (complémentaire de 0xFFFFFFFF ^ 0 = 0xFFFFFFFF → ~= 0)
        assert_eq!(crc32(b""), 0x0000_0000);
    }

    #[test]
    fn crc32_vecteur_connu() {
        // CRC32("123456789") = 0xCBF43926 (vecteur de test standard IEEE)
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_non_nul_pour_nom_typique() {
        let h = crc32(b"PlayerParam");
        assert_ne!(h, 0);
        // Déterministe.
        assert_eq!(crc32(b"PlayerParam"), h);
    }

    // -------------------------------------------------------------------
    // Détection magic
    // -------------------------------------------------------------------

    #[test]
    fn is_rdbn_detection() {
        assert!(is_rdbn(b"RDBN\x00\x00\x00\x00"));
        assert!(!is_rdbn(b"rdbn\x00\x00\x00\x00"));
        // 4 octets = exact match du magic → true (is_rdbn ne valide pas la taille minimale).
        assert!(is_rdbn(b"RDBN"));
        // Trop court (moins de 4 octets) → false.
        assert!(!is_rdbn(b"RDB"));
        assert!(!is_rdbn(b""));
    }

    #[test]
    fn is_rdbn_vide() {
        assert!(!is_rdbn(b""));
    }

    // -------------------------------------------------------------------
    // Erreurs de format
    // -------------------------------------------------------------------

    #[test]
    fn trop_court_renvoie_erreur() {
        let buf = [0u8; 0x20]; // < MIN_SIZE (0x50)
        assert!(matches!(parse(&buf), Err(FormatError::TooShort { .. })));
    }

    #[test]
    fn mauvais_magic_renvoie_erreur() {
        let mut buf = [0u8; 0x50];
        buf[0..4].copy_from_slice(b"NOTM");
        assert!(matches!(parse(&buf), Err(FormatError::BadMagic { .. })));
    }

    // -------------------------------------------------------------------
    // Parse d'un header minimal synthétique
    // -------------------------------------------------------------------

    /// Construit un fichier RDBN minimal avec 0 types/champs/racines/chaînes.
    fn build_empty_rdbn() -> Vec<u8> {
        // data_offset = 0x14 (× 4 = 0x50 = position juste après le header de 0x50 octets)
        // version = 100
        // Toutes les tables sont vides → offsets pointent vers 0 dans la section données.

        let mut buf = alloc::vec![0u8; 0x50];
        buf[0..4].copy_from_slice(b"RDBN");
        buf[4..6].copy_from_slice(&(0x50i16).to_le_bytes()); // header_size
        buf[6..10].copy_from_slice(&(100i32).to_le_bytes()); // version
        // data_offset en quarts (0x14 × 4 = 0x50).
        buf[10..12].copy_from_slice(&(0x14i16).to_le_bytes()); // data_offset / 4
        buf[12..16].copy_from_slice(&(0i32).to_le_bytes()); // data_size = 0

        // Tous les offsets de tables = 0, tous les comptes = 0.
        // (le tampon est initialisé à 0, donc pas besoin d'écrire).

        // string_offset (0x38) = 0 (relatif à data_offset = 0x50).
        // data_offset abs = 0x50 + 0 = 0x50 → dépasse buf.len() mais count=0 → pas lu.

        buf
    }

    #[test]
    fn parse_rdbn_vide() {
        let buf = build_empty_rdbn();
        let rdbn = parse(&buf).expect("RDBN vide doit parser");
        assert_eq!(rdbn.header.version, 100);
        assert_eq!(rdbn.header.data_offset, 0x50);
        assert!(rdbn.types.is_empty());
        assert!(rdbn.fields.is_empty());
        assert!(rdbn.roots.is_empty());
        assert!(rdbn.strings.entries.is_empty());
    }

    #[test]
    fn parse_rdbn_header_version() {
        let mut buf = build_empty_rdbn();
        // Changer la version.
        buf[6..10].copy_from_slice(&(200i32).to_le_bytes());
        let rdbn = parse(&buf).unwrap();
        assert_eq!(rdbn.header.version, 200);
    }

    // -------------------------------------------------------------------
    // RdbnStringTable::resolve
    // -------------------------------------------------------------------

    #[test]
    fn string_table_resolve() {
        let table = RdbnStringTable {
            entries: alloc::vec![
                (0xDEAD_BEEF, "PlayerParam".into()),
                (0x1234_5678, "SkillParam".into()),
            ],
        };
        assert_eq!(table.resolve(0xDEAD_BEEF), Some("PlayerParam"));
        assert_eq!(table.resolve(0x1234_5678), Some("SkillParam"));
        assert_eq!(table.resolve(0x0000_0001), None);
    }

    // -------------------------------------------------------------------
    // RdbnFieldType::from_i16
    // -------------------------------------------------------------------

    #[test]
    fn rdbn_field_type_connus() {
        assert!(matches!(RdbnFieldType::from_i16(3), RdbnFieldType::Bool));
        assert!(matches!(RdbnFieldType::from_i16(6), RdbnFieldType::Int));
        assert!(matches!(RdbnFieldType::from_i16(13), RdbnFieldType::Float));
        assert!(matches!(RdbnFieldType::from_i16(15), RdbnFieldType::Hash));
    }

    #[test]
    fn rdbn_field_type_inconnu() {
        assert!(matches!(
            RdbnFieldType::from_i16(0x7F),
            RdbnFieldType::Unknown(0x7F)
        ));
    }

    // -------------------------------------------------------------------
    // Décodage des VALEURS — golden values issues du VRAI fichier
    // /home/ubuntu/rg/iecode/re/menu/extracted/fonts/font_color.cfg.bin
    // (copié dans tests/fixtures/). Header tracé octet par octet :
    //   header_size=0x50, version=100, data_offset=0x14(×4=0x50),
    //   type_count=1, field_offset=8, field_count=7, root_offset=0x40,
    //   root_count=1, hash_count=9, value_offset=0x5a(×4+0x50=0x1B8),
    //   string_offset=0x1a68.
    // Liste m_FontColorDataList / type FONT_COLOR / 64 lignes de 100 octets.
    // -------------------------------------------------------------------

    #[cfg(feature = "real-fixtures")]
    const FONT_COLOR_FIXTURE: &[u8] = include_bytes!("../tests/fixtures/font_color.cfg.bin");

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn font_color_header_golden() {
        let rdbn = parse(FONT_COLOR_FIXTURE).expect("parse font_color");
        assert_eq!(rdbn.header.version, 100);
        assert_eq!(rdbn.header.data_offset, 0x50);
        assert_eq!(rdbn.header.type_count, 1);
        assert_eq!(rdbn.header.field_count, 7);
        assert_eq!(rdbn.header.root_count, 1);
        assert_eq!(rdbn.header.hash_count, 9);
        // value_abs = (0x5a << 2) + 0x50 = 0x168 + 0x50 = 0x1B8.
        assert_eq!(rdbn.value_abs, 0x1B8);
        // string_abs = 0x1a68 + 0x50 = 0x1AB8.
        assert_eq!(rdbn.string_abs, 0x1A68 + 0x50);
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn font_color_strings_resolved() {
        let rdbn = parse(FONT_COLOR_FIXTURE).unwrap();
        // 9 chaînes : 1 type + 7 champs + 1 liste.
        assert_eq!(rdbn.strings.entries.len(), 9);
        // Hashes vérifiés via crc32() : tous présents.
        assert_eq!(
            rdbn.strings.resolve(crc32(b"FONT_COLOR")),
            Some("FONT_COLOR")
        );
        assert_eq!(
            rdbn.strings.resolve(crc32(b"fontColorId")),
            Some("fontColorId")
        );
        assert_eq!(rdbn.strings.resolve(crc32(b"red")), Some("red"));
        assert_eq!(
            rdbn.strings.resolve(crc32(b"m_FontColorDataList")),
            Some("m_FontColorDataList")
        );
    }

    /// `.fxbin` (shaders FX), `.ptlb` (particules), `.clobin` (collision) et `.linb` (effets de
    /// ligne/locus) ne sont PAS des cfg.bin nominaux mais le MÊME conteneur **T2B** (footer
    /// `0xFFFFFFFF`) → `parse_t2b` les lit. Prouve que ces extensions (372 + 655 + 39 + 16 fichiers)
    /// sont **réellement parsées** par le parseur existant, pas seulement « reconnues ». Validé live
    /// via model-serve `/cfg` sur les vrais fichiers.
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn fxbin_et_ptlb_parsent_comme_t2b() {
        fn names(entries: &[CfgEntry], out: &mut Vec<String>) {
            for e in entries {
                out.push(e.name.clone());
                names(&e.children, out);
            }
        }
        let fx = parse_t2b(include_bytes!(
            "../tests/fixtures/t2b/chr_pbrt1_cutout.fxbin"
        ))
        .expect("fxbin parse T2B");
        let mut fxn = Vec::new();
        names(&fx.entries, &mut fxn);
        assert!(!fxn.is_empty());
        assert!(
            fxn.iter().any(|n| n == "SHADERFX_MEMB_NUM"),
            "fxbin = FX shader T2B"
        );
        assert!(fxn.iter().any(|n| n == "TEC_BGN"), "technique présente");

        let pt = parse_t2b(include_bytes!("../tests/fixtures/t2b/ega0077a.ptlb"))
            .expect("ptlb parse T2B");
        let mut ptn = Vec::new();
        names(&pt.entries, &mut ptn);
        assert!(
            ptn.iter().any(|n| n == "PARTICLE_NODE_INFO_BGN"),
            "ptlb = table de particules T2B"
        );

        // .clobin (collision/bone-line) est aussi un T2B → cfgbin le lit.
        let cl = parse_t2b(include_bytes!("../tests/fixtures/t2b/sample.clobin"))
            .expect("clobin parse T2B");
        let mut cln = Vec::new();
        names(&cl.entries, &mut cln);
        assert!(
            cln.iter().any(|n| n == "DA_BONE_LINE_START"),
            "clobin = bone-line T2B"
        );

        // .linb (effet de ligne/locus) est aussi un T2B → cfgbin le lit.
        let lb =
            parse_t2b(include_bytes!("../tests/fixtures/t2b/sample.linb")).expect("linb parse T2B");
        let mut lbn = Vec::new();
        names(&lb.entries, &mut lbn);
        assert!(
            lbn.iter().any(|n| n == "LINE_EFF_NODE_NUM"),
            "linb = effet de ligne T2B"
        );
        assert!(
            lbn.iter().any(|n| n == "LINE_EFF_INFO_BGN"),
            "linb : table d'infos présente"
        );
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn font_color_values_golden() {
        let rdbn = parse(FONT_COLOR_FIXTURE).unwrap();
        let lists = read_values(&rdbn, FONT_COLOR_FIXTURE);

        assert_eq!(lists.len(), 1);
        let list = &lists[0];
        assert_eq!(list.name, "m_FontColorDataList");
        assert_eq!(list.type_name, "FONT_COLOR");
        // root.value_count = 0x40 = 64 lignes.
        assert_eq!(list.rows.len(), 64);

        // Ligne 0 : 7 champs dans l'ordre du type.
        let r0 = &list.rows[0];
        assert_eq!(r0.fields.len(), 7);
        // Noms de champs résolus, dans l'ordre.
        let names: Vec<&str> = r0.fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "fontColorId",
                "red",
                "green",
                "blue",
                "rubiRed",
                "rubiGreen",
                "rubiBlue"
            ]
        );

        // Valeurs golden de la ligne 0, lues @0x1B8 (tracées au xxd) :
        //   fontColorId (Hash) = 0x270d2bda
        //   red=245 green=230 blue=245 rubiRed=245 rubiGreen=245 rubiBlue=230
        assert_eq!(r0.fields[0].1, RdbnValue::Hash(0x270D_2BDA));
        assert_eq!(r0.fields[1].1, RdbnValue::Int(245));
        assert_eq!(r0.fields[2].1, RdbnValue::Int(230));
        assert_eq!(r0.fields[3].1, RdbnValue::Int(245));
        assert_eq!(r0.fields[4].1, RdbnValue::Int(245));
        assert_eq!(r0.fields[5].1, RdbnValue::Int(245));
        assert_eq!(r0.fields[6].1, RdbnValue::Int(230));
    }

    #[test]
    fn read_field_value_types_synthetiques() {
        // Vérifie chaque branche du switch sur des octets contrôlés.
        // Bool (3)
        let f = |t: RdbnFieldType, size: i32| RdbnFieldEntry {
            name_hash: 0,
            field_type: t,
            type_category: 0,
            value_size: size,
            value_offset: 0,
            value_count: 1,
        };
        assert_eq!(
            read_field_value(&[1], 0, &f(RdbnFieldType::Bool, 1), 0),
            RdbnValue::Bool(true)
        );
        assert_eq!(
            read_field_value(&[0], 0, &f(RdbnFieldType::Bool, 1), 0),
            RdbnValue::Bool(false)
        );
        assert_eq!(
            read_field_value(&[0xAB], 0, &f(RdbnFieldType::Byte, 1), 0),
            RdbnValue::Byte(0xAB)
        );
        assert_eq!(
            read_field_value(&0x1234i16.to_le_bytes(), 0, &f(RdbnFieldType::Short, 2), 0),
            RdbnValue::Short(0x1234)
        );
        assert_eq!(
            read_field_value(&(-5i32).to_le_bytes(), 0, &f(RdbnFieldType::Int, 4), 0),
            RdbnValue::Int(-5)
        );
        assert_eq!(
            read_field_value(&1.5f32.to_le_bytes(), 0, &f(RdbnFieldType::Float, 4), 0),
            RdbnValue::Float(1.5)
        );
        assert_eq!(
            read_field_value(
                &0xDEAD_BEEFu32.to_le_bytes(),
                0,
                &f(RdbnFieldType::Hash, 4),
                0
            ),
            RdbnValue::Hash(0xDEAD_BEEF)
        );
        // ShortTuple (21) : 2 i16.
        let mut buf = Vec::new();
        buf.extend_from_slice(&3i16.to_le_bytes());
        buf.extend_from_slice(&7i16.to_le_bytes());
        assert_eq!(
            read_field_value(&buf, 0, &f(RdbnFieldType::ShortTuple, 4), 0),
            RdbnValue::ShortTuple([3, 7])
        );
        // Rates (18) : 4 floats.
        let mut rb = Vec::new();
        for x in [1.0f32, 2.0, 3.0, 4.0] {
            rb.extend_from_slice(&x.to_le_bytes());
        }
        assert_eq!(
            read_field_value(&rb, 0, &f(RdbnFieldType::Rates, 16), 0),
            RdbnValue::Rates([1.0, 2.0, 3.0, 4.0])
        );
        // Hors limites ⇒ Invalid.
        assert_eq!(
            read_field_value(&[0u8; 2], 0, &f(RdbnFieldType::Int, 4), 0),
            RdbnValue::Invalid
        );
        // Type inconnu / blob (AbilityData=0) ⇒ octets bruts.
        assert_eq!(
            read_field_value(&[1, 2, 3, 4], 0, &f(RdbnFieldType::AbilityData, 4), 0),
            RdbnValue::Blob(alloc::vec![1, 2, 3, 4])
        );
    }

    #[test]
    fn condition_value_resolution() {
        // Table de chaînes synthétique : string_abs=8, à +8 "ABC\0".
        // Champ Condition à offset 0 contenant u32 = 0 → pointe sur "ABC".
        let mut buf = alloc::vec![0u8; 4];
        buf.extend_from_slice(b"ABC\0");
        // value (u32 @0) = 0 → str_pos = string_abs(4) + 0 = 4 → "ABC".
        let v = read_condition_value(&buf, 0, 4);
        assert_eq!(v, RdbnValue::Condition("ABC".into()));
    }

    // -------------------------------------------------------------------
    // parse_t2b : robustesse en-tête (anti-panic sur données chiffrées)
    // -------------------------------------------------------------------

    /// En-tête T2B avec offset de table de chaînes négatif (i32 = -1) : caster en `usize`
    /// donne `usize::MAX`, et l'addition `off + len` débordait (panic debug / wrap release).
    /// Doit désormais renvoyer `Corrupt` proprement, jamais paniquer.
    #[test]
    fn parse_t2b_offset_negatif_ne_panique_pas() {
        let mut data = alloc::vec![0u8; 16];
        data[0..4].copy_from_slice(&1i32.to_le_bytes()); // entries_count = 1
        data[4..8].copy_from_slice(&(-1i32).to_le_bytes()); // string_table_off = -1
        data[8..12].copy_from_slice(&16i32.to_le_bytes()); // string_table_len = 16
        let r = parse_t2b(&data);
        assert!(matches!(r, Err(FormatError::Corrupt(_))), "got {r:?}");
    }

    /// Offset + longueur tous deux énormes mais positifs : `checked_add` doit intercepter
    /// le débordement et renvoyer `Corrupt`.
    #[test]
    fn parse_t2b_overflow_offset_plus_len() {
        let mut data = alloc::vec![0u8; 16];
        data[0..4].copy_from_slice(&0i32.to_le_bytes());
        data[4..8].copy_from_slice(&i32::MAX.to_le_bytes()); // off = 2^31-1
        data[8..12].copy_from_slice(&i32::MAX.to_le_bytes()); // len = 2^31-1
        // off + len = ~2^32 < usize::MAX sur 64 bits → pas d'overflow usize, mais > data.len()
        // → borne dépassée → Corrupt (et surtout : aucun panic).
        let r = parse_t2b(&data);
        assert!(matches!(r, Err(FormatError::Corrupt(_))), "got {r:?}");
    }

    /// Données chiffrées réalistes (entête haute-entropie type `cpk_list.cfg.bin`) :
    /// le parseur ne doit jamais paniquer, seulement renvoyer une erreur.
    #[test]
    fn parse_t2b_donnees_chiffrees_ne_paniquent_pas() {
        // Premiers octets réels observés sur une install Steam (cpk_list.cfg.bin chiffré).
        let data = [
            0x9du8, 0x9b, 0x87, 0x19, 0x68, 0x0b, 0xd1, 0x32, 0x5d, 0x84, 0x4d, 0xda, 0x05, 0x10,
            0xb0, 0x5b, 0xef, 0xff, 0x11, 0xf6, 0xf3, 0x46, 0x8f, 0xb9, 0xa1, 0x85, 0xd9, 0x3f,
        ];
        let r = parse_t2b(&data);
        assert!(
            r.is_err(),
            "données chiffrées doivent échouer proprement, got {r:?}"
        );
    }

    // -------------------------------------------------------------------
    // encode_t2b : round-trip sur de VRAIS fichiers du jeu (pas de fixture synthétique) —
    // seule preuve valable qu'un encodeur écrit sans avoir jamais existé n'est pas deviné.
    // -------------------------------------------------------------------

    /// Décode un vrai `.cfg.bin` T2B du jeu → réencode via [`encode_t2b`] → redécode avec
    /// [`parse_t2b`] (le décodeur déjà vérifié) → compare l'arbre obtenu à l'original. Ne vise
    /// PAS un round-trip octet-identique du FICHIER (l'agencement exact de l'outil Level-5
    /// d'origine n'est pas reversé), mais un arbre `entries`/`variables`/`children` STRICTEMENT
    /// identique — la seule chose qui compte pour qu'un fichier réencodé reste lisible par le
    /// jeu avec le même contenu.
    #[test]
    fn encode_t2b_round_trip_sur_le_vrai_jeu() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip encode_t2b_round_trip_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = crate::vfs::Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                (p.contains("/gamedata/") || p.contains("/text/")) && p.ends_with(".cfg.bin")
            })
            .collect();
        assert!(
            candidates.len() > 100,
            "attendu > 100 candidats, obtenu {}",
            candidates.len()
        );

        let step = (candidates.len() / 500).max(1);
        let mut n_t2b = 0usize;
        let mut n_ok = 0usize;
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in candidates.iter().step_by(step) {
            let Ok(bytes) = vfs.read(path) else { continue };
            if is_rdbn(&bytes) {
                continue; // ce test cible T2B uniquement — RDBN a son propre encodeur à écrire.
            }
            let Ok(original) = parse_t2b(&bytes) else {
                continue;
            }; // fichier non-T2B/illisible, hors périmètre
            n_t2b += 1;

            let reencoded = encode_t2b(&original.entries);
            match parse_t2b(&reencoded) {
                Ok(roundtripped) => {
                    if roundtripped.entries == original.entries {
                        n_ok += 1;
                    } else {
                        failed.push((path.clone(), "arbre différent après round-trip".to_string()));
                    }
                }
                Err(e) => failed.push((path.clone(), alloc::format!("redécodage échoué : {e}"))),
            }
        }

        eprintln!(
            "encode_t2b round-trip : {n_ok}/{n_t2b} identiques (sur {} candidats, pas={step})",
            candidates.len()
        );
        for (p, e) in failed.iter().take(10) {
            eprintln!("  échec {p} : {e}");
        }
        assert!(
            n_t2b > 5,
            "attendu au moins quelques fichiers T2B réels dans l'échantillon, obtenu {n_t2b}"
        );
        assert_eq!(
            n_ok,
            n_t2b,
            "{} échec(s) de round-trip sur {n_t2b} fichiers T2B réels",
            failed.len()
        );
    }

    // -------------------------------------------------------------------
    // encode_rdbn : round-trip sur de VRAIS fichiers RDBN du jeu — même exigence de preuve que
    // encode_t2b_round_trip_sur_le_vrai_jeu (un encodeur porté depuis une source externe non
    // vérifiée par le vrai jeu ne compte pas comme fait tant qu'il n'est pas confronté au vrai
    // lecteur `parse`/`read_values`).
    // -------------------------------------------------------------------

    /// Décode un vrai `.cfg.bin` RDBN du jeu (`parse` + `read_values`) → réencode via
    /// [`encode_rdbn`] → redécode avec le MÊME lecteur → compare les `RdbnList` obtenues à
    /// l'original (noms de listes/types/champs ET valeurs). Ne vise pas un round-trip
    /// octet-identique du fichier (agencement de l'outil Level-5 d'origine non reversé, table de
    /// types non dédupliquée), mais un contenu logique STRICTEMENT identique.
    #[test]
    fn encode_rdbn_round_trip_sur_le_vrai_jeu() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !crate::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip encode_rdbn_round_trip_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = crate::vfs::Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                (p.contains("/gamedata/") || p.contains("/text/")) && p.ends_with(".cfg.bin")
            })
            .collect();
        assert!(
            candidates.len() > 100,
            "attendu > 100 candidats, obtenu {}",
            candidates.len()
        );

        // Pas plus fin que le test T2B : le RDBN est nettement plus rare parmi les `.cfg.bin`
        // (constaté : 3 RDBN pour ~500 échantillons au pas `/500`) — `/5000` échantillonne assez
        // pour dépasser le seuil `n_rdbn > 5` ci-dessous, sans scanner les 50k+ candidats.
        let step = (candidates.len() / 5000).max(1);
        let mut n_rdbn = 0usize;
        let mut n_ok = 0usize;
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in candidates.iter().step_by(step) {
            let Ok(bytes) = vfs.read(path) else { continue };
            if !is_rdbn(&bytes) {
                continue; // ce test cible RDBN uniquement.
            }
            let Ok(rdbn) = parse(&bytes) else { continue }; // fichier illisible, hors périmètre
            n_rdbn += 1;
            let original = read_values(&rdbn, &bytes);

            match encode_rdbn(&original) {
                Ok(reencoded) => match parse(&reencoded) {
                    Ok(rdbn2) => {
                        let roundtripped = read_values(&rdbn2, &reencoded);
                        if roundtripped == original {
                            n_ok += 1;
                        } else {
                            failed.push((
                                path.clone(),
                                "listes différentes après round-trip".to_string(),
                            ));
                        }
                    }
                    Err(e) => {
                        failed.push((path.clone(), alloc::format!("redécodage échoué : {e}")))
                    }
                },
                Err(e) => failed.push((path.clone(), alloc::format!("encode_rdbn échoué : {e}"))),
            }
        }

        eprintln!(
            "encode_rdbn round-trip : {n_ok}/{n_rdbn} identiques (sur {} candidats, pas={step})",
            candidates.len()
        );
        for (p, e) in failed.iter().take(10) {
            eprintln!("  échec {p} : {e}");
        }
        assert!(
            n_rdbn > 5,
            "attendu au moins quelques fichiers RDBN réels dans l'échantillon, obtenu {n_rdbn}"
        );
        assert_eq!(
            n_ok,
            n_rdbn,
            "{} échec(s) de round-trip sur {n_rdbn} fichiers RDBN réels",
            failed.len()
        );
    }
}
