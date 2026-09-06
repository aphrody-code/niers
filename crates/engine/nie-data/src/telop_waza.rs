//! Famille `telop_waza` — port Rust du `skill_telop_info_config` (« télops de technique »).
//!
//! Un *télop* de technique est le bandeau de nom affiché à l'écran quand un personnage
//! déclenche un skill. Ce fichier décrit, par skill, les **marges** (« blanks ») gauche
//! et droite à réserver autour du texte, et cela **par langue** (la longueur du nom varie
//! d'une langue à l'autre, donc le cadrage aussi).
//!
//! ## Vérité terrain
//!
//! Fichier unique : `data/common/gamedata/skill/skill_telop_info_config_0.00.00.cfg.bin`
//! (RDBN, format `lists`). Deux listes parallèles de 926 entrées :
//!
//! | Liste | `typeName` | Rôle |
//! |-------|------------|------|
//! | `m_skillTelopInfoList` | `SkillTelopInfo` | 1 entrée par skill : `id`, paire `blankSizeInfo = [gauche, droite]`, `eldoradoId` |
//! | `m_blankSizeInfoList` | `BlankSizeInfo` | table des marges (pixels) gauche/droite par langue |
//!
//! Chaque `SkillTelopInfo` ne stocke pas ses marges directement : `blankSizeInfo` est un
//! couple d'**index** dans `m_blankSizeInfoList` (index `[0]` = marge gauche, index `[1]`
//! = marge droite). Le parseur résout ces index vers les vraies valeurs par langue.
//!
//! ## Port 1:1 d'inagle
//!
//! Référence : `packages/inagle/src/parsers/telop-waza.ts` (`parseTelopWazaContent`,
//! l.101-135). Comportements préservés à l'identique :
//! - `eldoradoId == 0x00000000` → `None` (constante `ZERO_HASH` côté TS) ;
//! - index `blankSizeInfo` **hors borne** → marge nulle (« jamais inventer une valeur »,
//!   commentaire d'inagle l.116) : un côté hors borne donne des marges toutes à zéro,
//!   indépendamment de l'autre côté.
//!
//! Langues, dans l'ordre d'inagle (`TELOP_LANGS`, l.30-40) :
//! `ja, en, pt, fr, it, de, es, zh_hant, zh_hans`.

use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Les 9 langues du jeu, dans l'ordre exact d'inagle (`TELOP_LANGS`).
pub const TELOP_LANGS: [&str; 9] = [
    "ja", "en", "pt", "fr", "it", "de", "es", "zh_hant", "zh_hans",
];

/// Marges (en pixels) d'**un côté** du télop, résolues pour les 9 langues.
///
/// Obtenue en projetant une [`BlankSizeInfo`] sur sa moitié gauche ([`BlankSizeInfo::left`])
/// ou droite ([`BlankSizeInfo::right`]) — équivalent du `pickSide` d'inagle (l.93-99).
/// `Default` = toutes les marges à zéro (cas « index hors borne »).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LangMargins {
    /// Japonais (`ja`).
    pub ja: i64,
    /// Anglais (`en`).
    pub en: i64,
    /// Portugais (`pt`).
    pub pt: i64,
    /// Français (`fr`).
    pub fr: i64,
    /// Italien (`it`).
    pub it: i64,
    /// Allemand (`de`).
    pub de: i64,
    /// Espagnol (`es`).
    pub es: i64,
    /// Chinois traditionnel (`zh_hant`).
    pub zh_hant: i64,
    /// Chinois simplifié (`zh_hans`).
    pub zh_hans: i64,
}

impl LangMargins {
    /// Marge pour un code langue (`"ja"`, `"en"`, …), `None` si le code est inconnu.
    #[must_use]
    pub fn get(&self, lang: &str) -> Option<i64> {
        Some(match lang {
            "ja" => self.ja,
            "en" => self.en,
            "pt" => self.pt,
            "fr" => self.fr,
            "it" => self.it,
            "de" => self.de,
            "es" => self.es,
            "zh_hant" => self.zh_hant,
            "zh_hans" => self.zh_hans,
            _ => return None,
        })
    }
}

/// Entrée brute de `m_blankSizeInfoList` (`typeName` `BlankSizeInfo`) : marge gauche et
/// droite, en pixels, pour chacune des 9 langues.
///
/// Vérité terrain : `m_blankSizeInfoList[0]` : `ja = (246, 246)`, `en = (324, 327)`,
/// `fr = (6, 28)`, `de = (307, 306)` (chaque couple = `(left, right)`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlankSizeInfo {
    /// `ja_left` / `ja_right`.
    pub ja_left: i64,
    pub ja_right: i64,
    /// `en_left` / `en_right`.
    pub en_left: i64,
    pub en_right: i64,
    /// `pt_left` / `pt_right`.
    pub pt_left: i64,
    pub pt_right: i64,
    /// `fr_left` / `fr_right`.
    pub fr_left: i64,
    pub fr_right: i64,
    /// `it_left` / `it_right`.
    pub it_left: i64,
    pub it_right: i64,
    /// `de_left` / `de_right`.
    pub de_left: i64,
    pub de_right: i64,
    /// `es_left` / `es_right`.
    pub es_left: i64,
    pub es_right: i64,
    /// `zh_hant_left` / `zh_hant_right`.
    pub zh_hant_left: i64,
    pub zh_hant_right: i64,
    /// `zh_hans_left` / `zh_hans_right`.
    pub zh_hans_left: i64,
    pub zh_hans_right: i64,
}

impl BlankSizeInfo {
    /// Parse une entrée `BlankSizeInfo` (les marges sont des `Short` RDBN).
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        let g = |k: &str| field_i64(v, k).unwrap_or(0);
        Self {
            ja_left: g("ja_left"),
            ja_right: g("ja_right"),
            en_left: g("en_left"),
            en_right: g("en_right"),
            pt_left: g("pt_left"),
            pt_right: g("pt_right"),
            fr_left: g("fr_left"),
            fr_right: g("fr_right"),
            it_left: g("it_left"),
            it_right: g("it_right"),
            de_left: g("de_left"),
            de_right: g("de_right"),
            es_left: g("es_left"),
            es_right: g("es_right"),
            zh_hant_left: g("zh_hant_left"),
            zh_hant_right: g("zh_hant_right"),
            zh_hans_left: g("zh_hans_left"),
            zh_hans_right: g("zh_hans_right"),
        }
    }

    /// Projette sur les marges **gauche** (`pickSide(_, "left")` d'inagle).
    #[must_use]
    pub fn left(&self) -> LangMargins {
        LangMargins {
            ja: self.ja_left,
            en: self.en_left,
            pt: self.pt_left,
            fr: self.fr_left,
            it: self.it_left,
            de: self.de_left,
            es: self.es_left,
            zh_hant: self.zh_hant_left,
            zh_hans: self.zh_hans_left,
        }
    }

    /// Projette sur les marges **droite** (`pickSide(_, "right")` d'inagle).
    #[must_use]
    pub fn right(&self) -> LangMargins {
        LangMargins {
            ja: self.ja_right,
            en: self.en_right,
            pt: self.pt_right,
            fr: self.fr_right,
            it: self.it_right,
            de: self.de_right,
            es: self.es_right,
            zh_hant: self.zh_hant_right,
            zh_hans: self.zh_hans_right,
        }
    }
}

/// Ligne aplatie : 1 télop de skill, avec ses marges gauche/droite **résolues** par langue.
///
/// Port 1:1 de `ParsedTelopWaza` d'inagle (`telop-waza.ts` l.66-80).
///
/// Vérité terrain : `m_skillTelopInfoList[0]` : `id = 0x5B6A04E9`, `blankSizeInfo = [0, 1]`,
/// `eldoradoId = 0x00000000` (→ `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedTelopWaza {
    /// `id` — hash du skill auquel ce télop est rattaché (clé stable).
    pub skill_id: HashId,
    /// `blankSizeInfo[0]` — index de la marge gauche dans `m_blankSizeInfoList`.
    pub blank_left_index: i64,
    /// `blankSizeInfo[1]` — index de la marge droite dans `m_blankSizeInfoList`.
    pub blank_right_index: i64,
    /// `eldoradoId` — hash du skill Eldorado lié (`None` si `0x00000000`).
    pub eldorado_id: Option<HashId>,
    /// Marges **gauche** par langue, résolues depuis `blank_left_index`
    /// (toutes nulles si l'index est hors borne).
    pub left: LangMargins,
    /// Marges **droite** par langue, résolues depuis `blank_right_index`
    /// (toutes nulles si l'index est hors borne).
    pub right: LangMargins,
}

/// Hash nul : `eldoradoId` valant `0x00000000` signifie « aucun skill Eldorado lié ».
const ZERO_HASH: HashId = HashId::ZERO;

/// Lit un champ paire `[gauche, droite]` (`(0, 0)` si absent ou mal formé).
///
/// `blankSizeInfo` est un `ShortTuple` RDBN, sérialisé en tableau JSON `[i16, i16]`.
fn pair(v: &Value, key: &str) -> (i64, i64) {
    let arr = v.get(key).and_then(Value::as_array);
    let left = arr
        .and_then(|a| a.first())
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let right = arr
        .and_then(|a| a.get(1))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (left, right)
}

/// Résout un index dans la table des marges vers le côté demandé.
///
/// Index hors borne (ou négatif) → [`LangMargins::default`] (toutes les marges à zéro),
/// exactement comme inagle qui retourne `{ ...empty }` quand `blanks[index]` est `undefined`.
fn resolve(blanks: &[BlankSizeInfo], index: i64, side: Side) -> LangMargins {
    let Ok(idx) = usize::try_from(index) else {
        return LangMargins::default();
    };
    match blanks.get(idx) {
        Some(b) => match side {
            Side::Left => b.left(),
            Side::Right => b.right(),
        },
        None => LangMargins::default(),
    }
}

/// Côté du télop à résoudre.
#[derive(Debug, Clone, Copy)]
enum Side {
    Left,
    Right,
}

/// Parse le `skill_telop_info_config_*.cfg.bin.json` complet : une [`ParsedTelopWaza`] par
/// entrée de `m_skillTelopInfoList`, marges résolues depuis `m_blankSizeInfoList`.
///
/// Port 1:1 de `parseTelopWazaContent` (`telop-waza.ts` l.101-135). Aucune entrée n'est
/// sautée : le compte du `Vec` retourné égale `m_skillTelopInfoList` (926 sur le dump réel).
///
/// Vérité terrain : `skill_telop_info_config_0.00.00.cfg.bin` →
/// `m_skillTelopInfoList` = 926, `m_blankSizeInfoList` = 926.
#[must_use]
pub fn parse_telop_waza_config(root: &Value) -> Vec<ParsedTelopWaza> {
    let blanks: Vec<BlankSizeInfo> = list_values(root, "m_blankSizeInfoList")
        .map(|vs| vs.iter().map(BlankSizeInfo::from_value).collect())
        .unwrap_or_default();

    let Some(telops) = list_values(root, "m_skillTelopInfoList") else {
        return Vec::new();
    };

    let mut out = Vec::with_capacity(telops.len());
    for val in telops {
        let (blank_left_index, blank_right_index) = pair(val, "blankSizeInfo");
        let eldo = field_hash(val, "eldoradoId");
        out.push(ParsedTelopWaza {
            skill_id: field_hash(val, "id"),
            blank_left_index,
            blank_right_index,
            eldorado_id: if eldo == ZERO_HASH { None } else { Some(eldo) },
            left: resolve(&blanks, blank_left_index, Side::Left),
            right: resolve(&blanks, blank_right_index, Side::Right),
        });
    }
    out
}

/// Recherche un télop par hash de skill (équivalent du `byId` d'inagle).
#[must_use]
pub fn find_by_skill_id(telops: &[ParsedTelopWaza], skill_id: HashId) -> Option<&ParsedTelopWaza> {
    telops.iter().find(|t| t.skill_id == skill_id)
}
