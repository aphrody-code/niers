//! Catalogue **dérivé du dump mémoire** : table de localisateurs nommés pour l'éditeur live
//! ([`crate::bin`] `nie-edit`).
//!
//! Chaque entrée mappe un *concept* d'`nie.exe` (tension, rang, cooldown, niveau…) vers la façon de
//! le **retrouver dans le process vivant** : une signature AOB (à scanner), la RVA validée au dump
//! où ce site est tombé, et — selon le concept — un offset de champ ou une chaîne de pointeurs.
//!
//! ## Provenance
//!
//! Les signatures et offsets sont des **faits sur `nie.exe`** établis en exploitant un dump
//! full-memory du jeu (cf. `docs/game-data/dump-exploitation.md`) : census RTTI, carte AOB→RVA,
//! décodage d'opérandes. Ex. la tension : l'AOB `8B 80 58 10 00 00` décode littéralement
//! `mov eax,[rax+0x1058]` → la valeur vit à `entity+0x1058`.
//!
//! ## Discipline
//!
//! - `rva = None` quand l'AOB **n'a pas été retrouvé** au dump (hors-match, ou motif trop générique) :
//!   on ne devine pas une adresse. Le scan live peut tout de même la résoudre en contexte.
//! - [`Kind::Toggle`] = site de **code** que le trainer d'origine patche (comportement on/off) ; ce
//!   n'est pas un scalaire éditable directement — `nie-edit` le *résout* et le *vérifie*, il ne
//!   réimplémente pas l'injection de code propriétaire.
//! - [`Kind::Value`] / [`Kind::StructField`] = scalaire lisible/inscriptible une fois l'adresse
//!   résolue (VA directe, base+RVA, ou base d'objet + offset/chaîne).

use crate::aob::{AobError, Pattern};

/// Base image statique d'`nie.exe` sous r2 (`static = NIE_IMAGE_BASE + rva`).
pub const NIE_IMAGE_BASE: u64 = 0x1_4000_0000;
/// Borne haute prudente de l'image mappée d'`nie.exe` (~40,8 Mo, sections jusqu'à `.reloc`).
pub const NIE_IMAGE_SIZE: u64 = 0x0270_0000;

/// Type scalaire d'une valeur éditable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    /// Octet non signé.
    U8,
    /// Entier 16 bits non signé.
    U16,
    /// Entier 32 bits non signé.
    U32,
    /// Entier 32 bits signé.
    I32,
    /// Entier 64 bits non signé.
    U64,
    /// Flottant 32 bits (jauges, cooldowns, temps).
    F32,
}

impl Ty {
    /// Taille en octets.
    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Ty::U8 => 1,
            Ty::U16 => 2,
            Ty::U32 | Ty::I32 | Ty::F32 => 4,
            Ty::U64 => 8,
        }
    }

    /// Nom court (`u32`, `f32`…), tel qu'accepté en suffixe `:TYPE`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Ty::U8 => "u8",
            Ty::U16 => "u16",
            Ty::U32 => "u32",
            Ty::I32 => "i32",
            Ty::U64 => "u64",
            Ty::F32 => "f32",
        }
    }

    /// Parse un suffixe `:TYPE` (`u8`/`u16`/`u32`/`i32`/`u64`/`f32`, casse insensible).
    #[must_use]
    pub fn from_tag(s: &str) -> Option<Ty> {
        match s.trim_start_matches(':').to_ascii_lowercase().as_str() {
            "u8" => Some(Ty::U8),
            "u16" => Some(Ty::U16),
            "u32" => Some(Ty::U32),
            "i32" => Some(Ty::I32),
            "u64" => Some(Ty::U64),
            "f32" => Some(Ty::F32),
            _ => None,
        }
    }

    /// Encode `s` (décimal, ou `0x…` hex pour les entiers) en octets little-endian de la bonne taille.
    ///
    /// # Errors
    /// Message lisible si `s` n'est pas parsable dans le type.
    pub fn encode(self, s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim();
        let parse_u = |max_bits: u32| -> Result<u64, String> {
            let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                u64::from_str_radix(h, 16).map_err(|_| format!("hex invalide: {s}"))?
            } else {
                s.parse::<u64>()
                    .map_err(|_| format!("entier invalide: {s}"))?
            };
            if max_bits < 64 && v >= (1u64 << max_bits) {
                return Err(format!("{s} déborde {} bits", max_bits));
            }
            Ok(v)
        };
        Ok(match self {
            Ty::U8 => vec![u8::try_from(parse_u(8)?).map_err(|_| "déborde u8")?],
            Ty::U16 => (parse_u(16)? as u16).to_le_bytes().to_vec(),
            Ty::U32 => (parse_u(32)? as u32).to_le_bytes().to_vec(),
            Ty::U64 => parse_u(64)?.to_le_bytes().to_vec(),
            Ty::I32 => {
                let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    i64::from_str_radix(h, 16).map_err(|_| format!("hex invalide: {s}"))?
                } else {
                    s.parse::<i64>()
                        .map_err(|_| format!("entier signé invalide: {s}"))?
                };
                i32::try_from(v)
                    .map_err(|_| format!("{s} déborde i32"))?
                    .to_le_bytes()
                    .to_vec()
            }
            Ty::F32 => s
                .parse::<f32>()
                .map_err(|_| format!("flottant invalide: {s}"))?
                .to_le_bytes()
                .to_vec(),
        })
    }

    /// Décode `b` (au moins [`Ty::size`] octets) en chaîne lisible (`42`, `3.5`, `0x…`).
    #[must_use]
    pub fn decode(self, b: &[u8]) -> String {
        if b.len() < self.size() {
            return "<court>".to_owned();
        }
        match self {
            Ty::U8 => b[0].to_string(),
            Ty::U16 => u16::from_le_bytes([b[0], b[1]]).to_string(),
            Ty::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
            Ty::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
            Ty::U64 => {
                u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]).to_string()
            }
            Ty::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
        }
    }
}

/// Nature du localisateur — détermine ce que `nie-edit` peut faire de l'entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Site de **code** patché par le trainer d'origine (comportement on/off). `nie-edit` le résout
    /// et le vérifie, sans réimplémenter l'injection.
    Toggle,
    /// Scalaire directement lisible/inscriptible à `base + rva` une fois résolu.
    Value,
    /// Champ dans un objet : nécessite la **base de l'objet** au runtime (offset [`Entry::field`]
    /// ou chaîne [`Entry::chain`]).
    StructField,
}

/// Catégorie d'affichage (regroupe la table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Joueur / personnage (niveau, capacités, enhance).
    Player,
    /// Match en cours (tension, temps, cooldowns, gel, rang).
    Match,
    /// Boutique / économie (achats gratuits, taux de drop).
    Shop,
    /// Esprits / cartes esprit.
    Spirit,
    /// Passifs / effets.
    Passive,
}

impl Category {
    /// Étiquette courte.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Category::Player => "player",
            Category::Match => "match",
            Category::Shop => "shop",
            Category::Spirit => "spirit",
            Category::Passive => "passive",
        }
    }
}

/// Une entrée du catalogue : concept nommé + comment le localiser dans le process live.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Identifiant kebab-case utilisé en CLI (`tension`, `match-time`…).
    pub id: &'static str,
    /// Nom de la fonctionnalité d'origine (référence RE).
    pub feature: &'static str,
    /// Catégorie d'affichage.
    pub category: Category,
    /// Nature du localisateur.
    pub kind: Kind,
    /// Type scalaire de la valeur associée.
    pub ty: Ty,
    /// Signature AOB (masque Cheat-Engine) du site, si connue.
    pub aob: Option<&'static str>,
    /// RVA `nie.exe` validée au dump (None = AOB non retrouvé au dump → résoudre en live).
    pub rva: Option<u64>,
    /// Offset de champ décodé depuis l'instruction (ex. tension `0x1058`), si pertinent.
    pub field: Option<i64>,
    /// Chaîne de pointeurs depuis une base d'objet (ex. rang `[+0x69A0]+0x5C`), si pertinent.
    pub chain: Option<&'static [i64]>,
    /// Description courte (sémantique RE).
    pub doc: &'static str,
}

impl Entry {
    /// Adresse statique r2 du site (`NIE_IMAGE_BASE + rva`), si la RVA est connue.
    #[must_use]
    pub fn static_addr(&self) -> Option<u64> {
        self.rva.map(|r| NIE_IMAGE_BASE + r)
    }

    /// Compile la signature AOB de l'entrée, si présente.
    ///
    /// # Errors
    /// [`AobError`] si le motif est malformé (ne devrait pas arriver pour le catalogue figé — testé).
    pub fn pattern(&self) -> Option<Result<Pattern, AobError>> {
        self.aob.map(Pattern::parse)
    }
}

/// Cherche une entrée par son `id` (kebab-case).
#[must_use]
pub fn find(id: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.id == id)
}

/// Chaîne du rang, **décodée depuis l'instruction** au site ré-ancré `rva 0xE6AA57` :
/// `mov rax,[rcx+0x69C8]` puis `mov edx,[rax+0x5C]`. L'offset du singleton est passé de
/// `0x69A0` à `0x69C8` entre builds ; le `+0x5C` final, lui, n'a pas bougé.
const RANK_CHAIN: &[i64] = &[0x69C8, 0x5C];

/// Table figée des localisateurs dérivés du dump (cf. `aob_rva_map.json` + `findings.json`).
pub static CATALOG: &[Entry] = &[
    // ── Player ─────────────────────────────────────────────────────────────────────
    Entry {
        id: "max-abilities",
        feature: "MemoryEditor::InjectMaxAbilities",
        category: Category::Player,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("44 8B 6F 10 8B 47 04"),
        rva: Some(0xD8FF75),
        field: Some(0x10),
        chain: None,
        doc: "force les capacités au max — site mov r13d,[rdi+0x10] (hits_other=1, vérifier le hit)",
    },
    Entry {
        id: "enhance",
        feature: "MemoryEditor::InjectEnhance",
        category: Category::Player,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("44 8B 81 ?? ?? ?? ?? 48 ?? ?? 45 ?? ?? 0F ?? ?? ?? ?? ?? 66"),
        rva: Some(0x1194FB6),
        field: None,
        chain: None,
        doc: "edition de l'enhancement (renforcement d'objet)",
    },
    Entry {
        id: "player-level",
        feature: "PlayerLevel::EnablePlayerLevel",
        category: Category::Player,
        kind: Kind::Value,
        ty: Ty::U32,
        aob: Some("FF ?? 48 ?? ?? ?? 40 88 7D ?? 49 8B CF"),
        rva: Some(0xC72400),
        field: None,
        chain: None,
        doc: "niveau du joueur (hook d'écriture)",
    },
    // ── Match ──────────────────────────────────────────────────────────────────────
    Entry {
        id: "tension",
        feature: "MemoryEditor::InjectTensionMatch",
        category: Category::Match,
        kind: Kind::StructField,
        ty: Ty::U32,
        aob: Some("8B 80 58 10 00 00 EB ?? 8B ?? 80"),
        rva: Some(0xEB5D8D),
        field: Some(0x1058),
        chain: None,
        doc: "tension/jauge live à entity+0x1058 — mov eax,[rax+0x1058] (offset décodé)",
    },
    Entry {
        id: "rank",
        feature: "MemoryEditor::InjectRankCapture",
        category: Category::Match,
        kind: Kind::StructField,
        ty: Ty::I32,
        // Les deux offsets sont masqués : ce sont eux qui bougent d'un build à l'autre, pas la
        // forme du site. Figés, l'AOB ne donnait aucun hit ; libérés, il en donne exactement un.
        aob: Some("48 8B 81 ?? ?? 00 00 4C 8B 89 ?? ?? 00 00 8B 50 5C 33 C0"),
        rva: Some(0xE6AA57),
        field: None,
        chain: Some(RANK_CHAIN),
        doc: "rang de match : [singleton+0x69C8]+0x5C — offsets décodés au site ré-ancré",
    },
    Entry {
        id: "match-time",
        feature: "MemoryEditor::InjectMatchTime",
        category: Category::Match,
        kind: Kind::StructField,
        ty: Ty::F32,
        aob: Some("F3 0F 59 C6 F3 0F 58 ?? ?? ?? ?? ?? F3 0F 11 ?? ?? ?? ?? ?? EB"),
        rva: Some(0x16831B9),
        field: Some(0x2208),
        chain: None,
        doc: "chrono de match à entity+0x2208 — site `addss xmm0,[rdi+0x2208]` \
              (accumulation du delta), offset décodé depuis l'instruction",
    },
    Entry {
        id: "special-moves-cooldown",
        feature: "MemoryEditor::InjectSpecialMovesCooldown",
        category: Category::Match,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("F3 0F 5C ?? 0F ?? ?? F3 ?? ?? ?? 77 ?? 89"),
        rva: Some(0x15B51DE),
        field: None,
        chain: None,
        doc: "cooldown des techniques (f32)",
    },
    Entry {
        id: "special-tactics-cooldown",
        feature: "MemoryEditor::InjectSpecialTacticsCooldown",
        category: Category::Match,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("76 ?? F3 0F 5C ?? 0F 2F C6 F3 0F 11 84"),
        rva: Some(0x16CB9EC),
        field: None,
        chain: None,
        doc: "cooldown des tactiques (f32)",
    },
    Entry {
        id: "freeze-time-ok-to-pass",
        feature: "MemoryEditor::InjectFreezeTimeOkToPass",
        category: Category::Match,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("0F ?? ?? ?? ?? ?? F3 ?? ?? ?? B9 ?? ?? ?? ?? F3 0F 58 83 ?? ?? ?? ?? F3"),
        rva: Some(0x14ED6DE),
        field: None,
        chain: None,
        doc: "gel du timer « ok to pass »",
    },
    Entry {
        id: "freeze-time-special-move",
        feature: "MemoryEditor::InjectFreezeTimeSpecialMove",
        category: Category::Match,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("F3 0F 10 83 ?? ?? ?? ?? F3 41 0F 58 ?? ?? F3 0F 5D ?? F3"),
        rva: Some(0x14EE985),
        field: None,
        chain: None,
        doc: "gel du timer de technique",
    },
    Entry {
        id: "freeze-time-goal-keeper",
        feature: "MemoryEditor::InjectFreezeTimeGoalKeeper",
        category: Category::Match,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("75 ?? F3 ?? ?? ?? F3 41 0F 5C ?? F3 ?? ?? ?? F3"),
        rva: Some(0x152322E),
        field: None,
        chain: None,
        doc: "gel du timer gardien",
    },
    // ── Shop ───────────────────────────────────────────────────────────────────────
    Entry {
        id: "free-buy-shop",
        feature: "MemoryEditor::InjectFreeBuyShop",
        category: Category::Shop,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("4C 8B 7C 24 ?? 8B ?? 4C 8B 64"),
        rva: None,
        field: None,
        chain: None,
        doc: "achats gratuits boutique — AMBIGU (hits_nie=2, other=24 ; alt rva 0x163D037)",
    },
    Entry {
        id: "free-buy-spirit-market",
        feature: "MemoryEditor::InjectFreeBuySpiritMarket",
        category: Category::Shop,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("0F B7 C6 41 89 06 0F"),
        rva: Some(0xE62D1D),
        field: None,
        chain: None,
        doc: "achats gratuits marché aux esprits (DRIFT : resolve live a trouvé 0xE3EDCD ≠ dump 0xC38C0E — à investiguer)",
    },
    Entry {
        id: "drop-rate",
        feature: "MemoryEditor::InjectDropRate",
        category: Category::Shop,
        kind: Kind::StructField,
        ty: Ty::F32,
        aob: Some("F3 0F 10 B0 ?? 00 00 00 F3 0F 58 70 1C"),
        rva: Some(0xC27CF4),
        field: Some(0x1C),
        chain: None,
        doc: "taux de drop — addss xmm6,[rax+0x1C] (offset décodé)",
    },
    // ── Spirit ─────────────────────────────────────────────────────────────────────
    Entry {
        id: "spirit-increment",
        feature: "MemoryEditor::InjectSpiritIncrement",
        category: Category::Spirit,
        kind: Kind::Toggle,
        ty: Ty::U16,
        aob: Some("66 89 70 10 EB 41 3C 05"),
        rva: Some(0xDC0B25),
        field: Some(0x10),
        chain: None,
        doc: "incrément d'esprit — mov [rax+0x10],si",
    },
    Entry {
        id: "elite-spirit-increment",
        feature: "MemoryEditor::InjectEliteSpiritIncrement",
        category: Category::Spirit,
        kind: Kind::Toggle,
        ty: Ty::U16,
        aob: Some("66 41 89 74 42 14 4C 8B 74 24 40"),
        rva: Some(0xDC0B66),
        field: Some(0x14),
        chain: None,
        doc: "incrément d'esprit élite — mov [r10+rax*2+0x14],si",
    },
    Entry {
        id: "spirit-card",
        feature: "MemoryEditor::EnableSpiritCardInjection",
        category: Category::Spirit,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("49 8B 07 49 8B CF FF 50 30 ?? ?? ?? ?? 44 8B"),
        rva: Some(0xDBFB29),
        field: None,
        chain: None,
        doc: "injection de carte esprit",
    },
    Entry {
        id: "add-spirit",
        feature: "PlayerSpirits::EnableAddSpiritHook",
        category: Category::Spirit,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("4D 8B 7C ?? 08 4D ?? ?? 0F 84 ?? ?? ?? ?? 49"),
        rva: Some(0xDBFB1B),
        field: None,
        chain: None,
        doc: "hook d'ajout d'esprit (hits_other=1)",
    },
    Entry {
        id: "spirit-list",
        feature: "PlayerSpirits::EnableGetListHook",
        category: Category::Spirit,
        kind: Kind::StructField,
        ty: Ty::U32,
        aob: Some("48 69 E9 28 01 00 00 49 03 2E 74 ?? 39 ?? ?? 74"),
        rva: Some(0xE9271A),
        field: Some(0x128),
        chain: None,
        doc: "liste d'esprits — imul rbp,rcx,0x128 (stride d'enregistrement décodé)",
    },
    Entry {
        id: "spirit-id",
        feature: "PlayerSpirits::EnableGetSpiritIdHook",
        category: Category::Spirit,
        kind: Kind::StructField,
        ty: Ty::U32,
        aob: Some("49 8B ?? 48 8B 51 18 49 8B ?? FF D2 39 30"),
        rva: Some(0xC9DC1F),
        field: Some(0x18),
        chain: None,
        doc: "id d'esprit — mov rdx,[rcx+0x18]",
    },
    Entry {
        id: "unlimited-spirits",
        feature: "UnlimitedSpirits::EnableUnlimitedSpirits",
        category: Category::Spirit,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("75 03 8B 58 10 49"),
        rva: None,
        field: Some(0x10),
        chain: None,
        doc: "esprits illimités — site mov ebx,[rax+0x10] (AOB non retrouvé au dump)",
    },
    // ── Passive ────────────────────────────────────────────────────────────────────
    Entry {
        id: "passive-value",
        feature: "MemoryEditor::InjectPassiveValueEditing",
        category: Category::Passive,
        kind: Kind::Toggle,
        ty: Ty::F32,
        aob: Some("48 8B 0F 0F 57 C9 F3 ?? ?? C8 E8 ?? ?? ?? ?? EB"),
        rva: Some(0xC14385),
        field: None,
        chain: None,
        doc: "edition des valeurs de passif (RVA retrouvée LIVE — absente du dump hors-match)",
    },
    Entry {
        id: "special-move-type",
        feature: "MemoryEditor::InjectSpecialMoveTypeEditing",
        category: Category::Passive,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("E8 ?? ?? ?? ?? 48 85 C0 74 3F 8B 58 04"),
        rva: None,
        field: Some(0x04),
        chain: None,
        doc: "edition du type de technique — mov ebx,[rax+0x04] (RVA retrouvée LIVE, absente du dump)",
    },
    Entry {
        id: "custom-passives",
        feature: "CustomPassives::EnableHookInternal",
        category: Category::Passive,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("75 03 45 8B 3E 45 ?? ?? 74 ?? 48 ?? ?? ?? ?? ?? ?? 48 ?? ?? 74 ?? 4C"),
        rva: Some(0x165D4DA),
        field: None,
        chain: None,
        doc: "passifs personnalisés (hook interne)",
    },
    Entry {
        id: "custom-passives-summon",
        feature: "CustomPassivesSummon::EnableHookInternal",
        category: Category::Passive,
        kind: Kind::Toggle,
        ty: Ty::U32,
        aob: Some("42 8B BC AB ?? ?? 00 00"),
        rva: Some(0xE7A070),
        field: None,
        chain: None,
        doc: "passifs d'invocation — mov edi,[rbx+r13*4+disp32] (REX.X via 0x42)",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_parses() {
        // Toutes les entrées du catalogue figé ont un AOB valide (si une entrée future met aob:None,
        // ce test le signale → mettre à jour).
        for e in CATALOG {
            let p = e
                .pattern()
                .expect("entrée sans AOB")
                .unwrap_or_else(|err| panic!("{}: {err}", e.id));
            assert!(!p.is_empty(), "{}: motif vide", e.id);
        }
    }

    #[test]
    fn pattern_none_when_no_aob() {
        let e = Entry {
            id: "x",
            feature: "x",
            category: Category::Player,
            kind: Kind::Value,
            ty: Ty::U8,
            aob: None,
            rva: None,
            field: None,
            chain: None,
            doc: "",
        };
        assert!(e.pattern().is_none());
    }

    #[test]
    fn ids_unique_and_kebab() {
        let mut seen = std::collections::HashSet::new();
        for e in CATALOG {
            assert!(seen.insert(e.id), "id dupliqué: {}", e.id);
            assert!(
                e.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id non kebab: {}",
                e.id
            );
            assert!(find(e.id).is_some());
        }
    }

    #[test]
    fn rvas_within_image() {
        for e in CATALOG {
            if let Some(r) = e.rva {
                assert!(
                    (0x1000..NIE_IMAGE_SIZE).contains(&r),
                    "{}: rva 0x{r:x} hors image",
                    e.id
                );
                assert_eq!(e.static_addr(), Some(NIE_IMAGE_BASE + r));
            }
        }
    }

    #[test]
    fn struct_fields_have_a_locator() {
        // Un StructField doit fournir de quoi se résoudre : offset de champ OU chaîne.
        for e in CATALOG {
            if e.kind == Kind::StructField {
                assert!(
                    e.field.is_some() || e.chain.is_some(),
                    "{}: StructField sans field ni chain",
                    e.id
                );
            }
        }
    }

    #[test]
    fn ty_roundtrip() {
        assert_eq!(Ty::U32.encode("0x1000").unwrap(), 4096u32.to_le_bytes());
        assert_eq!(Ty::U32.decode(&4096u32.to_le_bytes()), "4096");
        assert_eq!(Ty::I32.encode("-5").unwrap(), (-5i32).to_le_bytes());
        assert_eq!(Ty::I32.decode(&(-5i32).to_le_bytes()), "-5");
        assert_eq!(Ty::F32.decode(&1.5f32.to_le_bytes()), "1.5");
        assert!(Ty::U8.encode("300").is_err());
        assert_eq!(Ty::from_tag(":f32"), Some(Ty::F32));
        assert_eq!(Ty::from_tag("u16"), Some(Ty::U16));
        assert_eq!(Ty::from_tag("bogus"), None);
    }

    const ALL_TY: [Ty; 6] = [Ty::U8, Ty::U16, Ty::U32, Ty::I32, Ty::U64, Ty::F32];

    #[test]
    fn ty_every_variant_size_name_roundtrip() {
        for ty in ALL_TY {
            assert_eq!(
                ty.size(),
                ty.name()
                    .trim_start_matches(['u', 'i', 'f'])
                    .parse::<usize>()
                    .unwrap()
                    / 8
            );
            assert_eq!(Ty::from_tag(ty.name()), Some(ty));
            assert_eq!(
                Ty::from_tag(&format!(":{}", ty.name().to_uppercase())),
                Some(ty)
            );
            // décodage d'un tampon trop court.
            assert_eq!(ty.decode(&[]), "<court>");
            // encode d'une valeur nominale ("1" parse pour chaque type) puis vérifie la taille.
            let enc = ty.encode("1").unwrap();
            assert_eq!(enc.len(), ty.size());
        }
    }

    #[test]
    fn ty_encode_decode_each_kind() {
        assert_eq!(Ty::U8.encode("255").unwrap(), vec![0xFF]);
        assert_eq!(Ty::U8.decode(&[0xFF]), "255");
        assert_eq!(Ty::U16.encode("0xBEEF").unwrap(), 0xBEEFu16.to_le_bytes());
        assert_eq!(
            Ty::U64.encode("0x0102030405060708").unwrap(),
            0x0102_0304_0506_0708u64.to_le_bytes()
        );
        assert_eq!(
            Ty::U64.decode(&0x1122_3344_5566_7788u64.to_le_bytes()),
            "1234605616436508552"
        );
        assert_eq!(
            Ty::I32.encode("0x7FFFFFFF").unwrap(),
            0x7FFF_FFFFi32.to_le_bytes()
        );
        assert_eq!(Ty::F32.encode("2.5").unwrap(), 2.5f32.to_le_bytes());
        assert_eq!(Ty::U16.decode(&0x1234u16.to_le_bytes()), "4660");
    }

    #[test]
    fn ty_encode_error_paths() {
        assert!(Ty::U16.encode("70000").is_err()); // déborde 16 bits
        assert!(Ty::U32.encode("0xZZ").is_err()); // hex invalide
        assert!(Ty::U32.encode("abc").is_err()); // entier invalide
        assert!(Ty::U64.encode("0xFFFFFFFFFFFFFFFFF").is_err()); // déborde 64 bits
        assert!(Ty::I32.encode("0x1_2").is_err()); // hex i32 invalide
        assert!(Ty::I32.encode("not").is_err()); // i32 invalide
        assert!(Ty::I32.encode("5000000000").is_err()); // déborde i32
        assert!(Ty::F32.encode("x.y").is_err()); // flottant invalide
        assert!(Ty::U8.encode("0x1FF").is_err()); // déborde u8 (hex)
    }

    #[test]
    fn category_labels_all() {
        for cat in [
            Category::Player,
            Category::Match,
            Category::Shop,
            Category::Spirit,
            Category::Passive,
        ] {
            assert!(!cat.label().is_empty());
            // chaque catégorie a au moins une entrée.
            assert!(CATALOG.iter().any(|e| e.category == cat));
        }
    }

    #[test]
    fn entry_methods() {
        // static_addr présent quand rva connu, absent sinon.
        // Comparé à la rva du catalogue, pas à une constante : un ré-ancrage sur un
        // nouveau build déplace les rva sans invalider la relation testée ici.
        let with_rva = find("tension").unwrap();
        assert_eq!(
            with_rva.static_addr(),
            Some(NIE_IMAGE_BASE + with_rva.rva.unwrap())
        );
        assert!(with_rva.pattern().unwrap().is_ok());
        // `rank` porte désormais une rva ré-ancrée *et* une chaîne de pointeurs.
        assert!(find("rank").unwrap().chain.is_some());
        // Un AOB à hits multiples reste `rva: None` — on ne devine pas une adresse.
        let no_rva = find("free-buy-shop").unwrap();
        assert_eq!(no_rva.static_addr(), None);
        // find d'un id absent.
        assert!(find("does-not-exist").is_none());
    }
}
