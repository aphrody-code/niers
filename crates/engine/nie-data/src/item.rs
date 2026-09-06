//! `ItemInfo` + `ItemCategory` — port des noeuds `ITEM_*_INFO_*` de l'`item_config`.
//!
//! ## Vérité terrain
//!
//! - Parser TS : `packages/inagle/src/parsers/item-config.ts` (`CATEGORY_MAP` l.51-72,
//!   `traverse` l.155-241).
//! - Données : `data/common/gamedata/item/item_config_7.00.25.00.cfg.bin.json`.
//!
//! Mapping de variables (port tel quel) : var0 = `itemId`, var2 = `nameId`, var3 = `descId`,
//! var4 = `price`, var5/var6 = stats (shoes/misanga/accessory/special/fashion), var11 =
//! `internalCode` (asset ID), dernière var = `uniformId` (fashion).
//!
//! Noeud vérifié `ITEM_SHOES_INFO_0` :
//! `[1834815904, 0, 1853054332, 0, 1401, 30, 31, 999, 0, 0, 0, "eq_sh110001", 1, 0, 0, 224, 0, 0, 961180446]`
//! → itemId = `0x6D5D11A0`, nameId = `0x6E735D7C`, price = 1401, stat1 = 30, stat2 = 31,
//!   internalCode = `eq_sh110001`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned, walk_named};
use crate::hash::HashId;

/// Catégorie d'objet (20 variantes). Source : `CATEGORY_MAP` (item-config.ts l.51-72).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ItemCategory {
    Consume,
    Shoes,
    Misanga,
    Accessory,
    Special,
    Formation,
    SpecialTactics,
    SuperTactics,
    SpecialSkill,
    Title,
    Fashion,
    Costume,
    Emblem,
    Unique,
    CraftObj,
    Animal,
    KizunaLink,
    NamePlate,
    Performance,
    Important,
}

impl ItemCategory {
    /// Identifiant string stable (= valeur du `CATEGORY_MAP` TS).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Consume => "consume",
            Self::Shoes => "shoes",
            Self::Misanga => "misanga",
            Self::Accessory => "accessory",
            Self::Special => "special",
            Self::Formation => "formation",
            Self::SpecialTactics => "special_tactics",
            Self::SuperTactics => "super_tactics",
            Self::SpecialSkill => "special_skill",
            Self::Title => "title",
            Self::Fashion => "fashion",
            Self::Costume => "costume",
            Self::Emblem => "emblem",
            Self::Unique => "unique",
            Self::CraftObj => "craft_obj",
            Self::Animal => "animal",
            Self::KizunaLink => "kizuna_link",
            Self::NamePlate => "name_plate",
            Self::Performance => "performance",
            Self::Important => "important",
        }
    }

    /// `true` si l'objet est un équipement portant des stats (var5/var6). Source : `traverse`.
    #[must_use]
    pub fn has_equipment_stats(self) -> bool {
        matches!(
            self,
            Self::Shoes | Self::Misanga | Self::Accessory | Self::Special | Self::Fashion
        )
    }
}

/// Table préfixe de noeud → catégorie (clé du `CATEGORY_MAP`). Ordre préservé du TS.
const CATEGORY_MAP: &[(&str, ItemCategory)] = &[
    ("ITEM_CONSUME_INFO", ItemCategory::Consume),
    ("ITEM_SHOES_INFO", ItemCategory::Shoes),
    ("ITEM_MISANGA_INFO", ItemCategory::Misanga),
    ("ITEM_ACCESSORY_INFO", ItemCategory::Accessory),
    ("ITEM_SPECIAL_INFO", ItemCategory::Special),
    ("ITEM_FORMATION_INFO", ItemCategory::Formation),
    ("ITEM_SPECIAL_TACTICS_INFO", ItemCategory::SpecialTactics),
    ("ITEM_SUPER_TACTICS_INFO", ItemCategory::SuperTactics),
    ("ITEM_SPECIAL_SKILL_INFO", ItemCategory::SpecialSkill),
    ("ITEM_TITLE_INFO", ItemCategory::Title),
    ("ITEM_FASHION_INFO", ItemCategory::Fashion),
    ("ITEM_COSTUME_INFO", ItemCategory::Costume),
    ("ITEM_EMBLEM_INFO", ItemCategory::Emblem),
    ("ITEM_UNIQUE_INFO", ItemCategory::Unique),
    ("ITEM_CRAFT_OBJ_INFO", ItemCategory::CraftObj),
    ("ITEM_ANIMAL_INFO", ItemCategory::Animal),
    ("ITEM_KIZUNA_LINK_INFO", ItemCategory::KizunaLink),
    ("ITEM_NAME_PLATE_INFO", ItemCategory::NamePlate),
    ("ITEM_PERFORMANCE_INFO", ItemCategory::Performance),
    ("ITEM_IMPORTANT_INFO", ItemCategory::Important),
];

/// Stats d'équipement (var5, var6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemStats {
    pub stat1: i64,
    pub stat2: i64,
}

/// Un objet `ITEM_*_INFO_*` porté.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemInfo {
    /// var0 — `itemId`.
    pub item_id: HashId,
    /// Catégorie (dérivée du préfixe de noeud).
    pub category: ItemCategory,
    /// var2 — `nameId`.
    pub name_id: HashId,
    /// var3 — `descId`.
    pub desc_id: HashId,
    /// var4 — prix d'achat (None si ≤ 0, comme le TS qui n'ajoute le prix que si > 0).
    pub price: Option<i64>,
    /// var5/var6 — stats d'équipement (présentes seulement pour les catégories équipement).
    pub stats: Option<ItemStats>,
    /// var11 — `internalCode` (asset ID, ex. `eq_sh110001`).
    pub internal_code: Option<String>,
    /// dernière var — `uniformId` (fashion uniquement).
    pub uniform_id: Option<HashId>,
}

impl ItemInfo {
    /// Parse un noeud d'objet pour une catégorie donnée. `None` si aucune variable.
    #[must_use]
    fn from_node(node: Node<'_>, category: ItemCategory) -> Option<Self> {
        let n = node.var_count();
        if n == 0 {
            return None;
        }
        let item_id = node.hash(0);
        let name_id = if n > 2 { node.hash(2) } else { HashId::ZERO };
        let desc_id = if n > 3 { node.hash(3) } else { HashId::ZERO };

        let price = if n > 4 {
            let p = node.int(4);
            if p > 0 { Some(p) } else { None }
        } else {
            None
        };

        // Stats : seulement pour les catégories équipement, et si var6 existe.
        let stats = if category.has_equipment_stats() && n > 6 {
            Some(ItemStats {
                stat1: node.int(5),
                stat2: node.int(6),
            })
        } else {
            None
        };

        // internalCode : var11 (asset ID).
        let internal_code = if n > 11 {
            let s = node.string(11);
            if s.is_empty() { None } else { Some(owned(s)) }
        } else {
            None
        };

        // uniformId : dernière var, fashion uniquement.
        let uniform_id = if category == ItemCategory::Fashion && n > 0 {
            let h = node.hash(n - 1);
            Some(h)
        } else {
            None
        };

        Some(ItemInfo {
            item_id,
            category,
            name_id,
            desc_id,
            price,
            stats,
            internal_code,
            uniform_id,
        })
    }
}

/// Parse tous les objets d'un `item_config_*.cfg.bin.json` désérialisé.
///
/// Reproduit le `traverse` d'inagle : pour chaque préfixe de `CATEGORY_MAP`, on prend les noeuds
/// dont le nom commence par le préfixe **et** ne contient pas `_LIST_BEG_`.
#[must_use]
pub fn parse_all_items(root: &Value) -> Vec<ItemInfo> {
    let mut out = Vec::new();
    // Un seul parcours global, on classe chaque noeud par préfixe (premier match du CATEGORY_MAP).
    walk_named(root, "ITEM_", |node| {
        let name = node.name();
        if name.contains("_LIST_BEG_") {
            return;
        }
        for (prefix, category) in CATEGORY_MAP {
            if name.starts_with(prefix) {
                if let Some(item) = ItemInfo::from_node(node, *category) {
                    out.push(item);
                }
                break;
            }
        }
    });
    out
}

/// Résout le **nom localisé** d'un objet via une liste `item_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Port de la jointure de `buildItemDatabase` (`item-config.ts` : `textMap.get(nameIdHex)`) :
/// le nom est résolu par `name_id` (var2) dans la table `item_text`. `None` si absent.
#[must_use]
pub fn resolve_name<'a>(item: &ItemInfo, item_text: &'a [(HashId, String)]) -> Option<&'a str> {
    crate::text::find_text(item_text, item.name_id)
}

/// Résout la **description localisée** d'un objet via la **même** table `item_text`.
///
/// Port fidèle : *« il n'existe PAS de fichier `item_explain_text` — les descriptions vivent DANS
/// `item_text`, résolues via le `descId` (var3) »* (`item-config.ts`). `None` si absent.
#[must_use]
pub fn resolve_description<'a>(
    item: &ItemInfo,
    item_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(item_text, item.desc_id)
}
