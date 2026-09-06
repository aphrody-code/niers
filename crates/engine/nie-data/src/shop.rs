//! `ShopConfig` — port Rust de `shop/shop_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/shop/shop_config_3.00.22.cfg.bin` (VFS IEVR).
//! - Référence de portage 1:1 : `packages/inagle/src/parsers/shop-config.ts`.
//! - Format : T2B (`entries`), arbre imbriqué.
//!
//! ## Structure des noeuds
//!
//! Côté **inagle** (son convertisseur `config-parser.ts`), l'item-list est un *enfant* du
//! shop : `SHOP_INFO_LIST_BEG_x > SHOP_INFO_x > SHOP_INFO_ITEM_LIST_BEG_x > SHOP_INFO_ITEM_y`.
//!
//! Côté **niers** (`cfgbin_to_t2b_iecode_root`, qui imbrique sur les marqueurs `_BEG`/`_END`),
//! le noeud `SHOP_INFO_x` n'a **pas** de marqueur `_BEG` : il est donc une feuille, et son
//! item-list `SHOP_INFO_ITEM_LIST_BEG_x` est un **frère** placé juste après lui :
//!
//! ```text
//! SHOP_INFO_LIST_BEG_0
//!   SHOP_INFO_0                 (feuille — var[0]=shopId, var[1]=nameHash)
//!   SHOP_INFO_ITEM_LIST_BEG_0
//!     SHOP_INFO_ITEM_0          (var[2]=itemId)
//!     SHOP_INFO_ITEM_1
//!     ...
//!   SHOP_INFO_1
//!   SHOP_INFO_ITEM_LIST_BEG_1
//!     ...
//! ```
//!
//! Le parseur gère **les deux dispositions** : l'item-list est rattaché au shop courant
//! qu'il soit un frère (niers) ou un enfant (inagle). Dans les deux cas, on extrait
//! exactement les mêmes données qu'inagle (shopId, nameHash, ensemble d'itemId).
//!
//! ## Variables positionnelles
//!
//! | Noeud                       | Variables                                                |
//! |-----------------------------|----------------------------------------------------------|
//! | `SHOP_INFO_x`               | \[0\] shopId (`Int`→u32), \[1\] nameHash (`Int`→u32)      |
//! | `SHOP_INFO_ITEM_y`          | \[2\] itemId (`Int`→u32) — réf. vers `item_config`        |
//!
//! ## Sémantique d'ensemble (`Set`)
//!
//! inagle collecte les items dans un `Set<number>` : les doublons sont **fusionnés** et
//! l'ordre d'insertion préservé (sémantique `Set` JS). On reproduit cela avec un
//! `Vec<HashId>` dédupliqué à l'insertion (premier passage conservé). Exemple réel :
//! `SHOP_INFO_2` a 88 noeuds item mais **87** items uniques (l'item `-1937608713`
//! apparaît deux fois de suite).

use alloc::vec::Vec;
use serde_json::Value;

use alloc::string::String;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

// ─── ShopInfo ─────────────────────────────────────────────────────────────────

/// Une boutique (`SHOP_INFO_*`) et son inventaire.
///
/// Port de `ShopInfo` (inagle `shop-config.ts`). `items` a la sémantique d'un `Set` JS :
/// dédupliqué, ordre de première insertion conservé.
///
/// Vérité terrain — `shop_config_3.00.22.cfg.bin` :
/// - `SHOP_INFO_0` : `shop_id = 0x61245112` (1629770002), `name_hash = 0x3505C205`
///   (889569797), 74 items uniques.
/// - `SHOP_INFO_1` : `shop_id = 0x1CF22DA7` (485633447), `name_hash = 0x00000000`, 282 items.
/// - `SHOP_INFO_2` : `shop_id = 0xB13B2BD5` (-1321522219), `name_hash = 0xB5B2EA44`
///   (-1246565820), 87 items uniques (88 noeuds, 1 doublon).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShopInfo {
    /// var\[0\] — identifiant de la boutique (`Int` réinterprété en u32, comme `>>> 0`).
    pub shop_id: HashId,
    /// var\[1\] — hash du nom de la boutique (réf. vers `shop_text`). `0` = sans nom.
    pub name_hash: HashId,
    /// Inventaire : identifiants d'items uniques (réf. vers `item_config`), ordre d'insertion.
    pub items: Vec<HashId>,
}

impl ShopInfo {
    /// `true` si l'item donné est en vente dans cette boutique.
    #[must_use]
    pub fn has_item(&self, item_id: HashId) -> bool {
        self.items.contains(&item_id)
    }

    /// Nombre d'items uniques en vente.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────────

/// Parse un `shop_config_*.cfg.bin.json` complet en liste de [`ShopInfo`].
///
/// Port 1:1 de `loadShopConfig` (inagle `shop-config.ts`), adapté aux deux dispositions
/// d'item-list (frère niers ou enfant inagle, cf. doc du module).
///
/// Règles (identiques à inagle) :
/// - un shop n'est retenu que si `var[0]` et `var[1]` existent et sont de type `Int` ;
/// - chaque `SHOP_INFO_ITEM_y` n'ajoute son `var[2]` que s'il est de type `Int` ;
/// - les itemId sont collectés en ensemble (doublons fusionnés, ordre conservé).
///
/// Comptes réels (`shop_config_3.00.22.cfg.bin`) : 16 boutiques, 2504 noeuds item au total.
#[must_use]
pub fn parse_shop_config(root: &Value) -> Vec<ShopInfo> {
    let mut shops: Vec<ShopInfo> = Vec::new();

    walk_named(root, "SHOP_INFO_LIST_BEG_", |container| {
        // Parcours séquentiel des frères : un shop, puis (éventuellement) son item-list.
        let mut current: Option<usize> = None;
        for child in container.children() {
            let name = child.name();
            if is_shop_node(name) {
                if let Some(shop) = parse_shop_node(child) {
                    shops.push(shop);
                    current = Some(shops.len() - 1);
                    // Disposition inagle : l'item-list peut être un enfant direct du shop.
                    if let Some(idx) = current {
                        collect_nested_item_lists(child, &mut shops[idx].items);
                    }
                } else {
                    current = None;
                }
            } else if is_item_list_node(name) {
                // Disposition niers : l'item-list est un frère rattaché au shop courant.
                if let Some(idx) = current {
                    collect_items_from_list(child, &mut shops[idx].items);
                }
            }
        }
    });

    shops
}

/// Résout le **nom localisé** d'une boutique via une liste `shop_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Le modèle de données est documenté par inagle (`shop-config.ts` : *« \[1\] = Shop Name Hash ->
/// links to shop_text »*) ; `name_hash` est la clé. `None` si absent.
#[must_use]
pub fn resolve_name<'a>(shop: &ShopInfo, shop_text: &'a [(HashId, String)]) -> Option<&'a str> {
    crate::text::find_text(shop_text, shop.name_hash)
}

// ─── Helpers internes ────────────────────────────────────────────────────────────

/// `true` pour un noeud boutique `SHOP_INFO_x` (et non un item ou un conteneur de liste).
fn is_shop_node(name: &str) -> bool {
    name.starts_with("SHOP_INFO_") && !name.contains("_ITEM") && !name.contains("_LIST_")
}

/// `true` pour un conteneur d'inventaire `SHOP_INFO_ITEM_LIST_BEG_x`.
fn is_item_list_node(name: &str) -> bool {
    name.starts_with("SHOP_INFO_ITEM_LIST_BEG_")
}

/// `true` pour un noeud item `SHOP_INFO_ITEM_y` (et non son conteneur de liste).
fn is_item_node(name: &str) -> bool {
    name.starts_with("SHOP_INFO_ITEM_") && !name.contains("_LIST_")
}

/// Lit shopId/nameHash d'un noeud `SHOP_INFO_x`. `None` si `var[0]`/`var[1]` absents ou
/// non-`Int` (inagle renvoie `null` dans ce cas).
fn parse_shop_node(node: Node<'_>) -> Option<ShopInfo> {
    let shop_var = node.var(0)?;
    if shop_var.ty != "Int" {
        return None;
    }
    let name_var = node.var(1)?;
    if name_var.ty != "Int" {
        return None;
    }
    Some(ShopInfo {
        shop_id: shop_var.as_hash(),
        name_hash: name_var.as_hash(),
        items: Vec::new(),
    })
}

/// Ajoute les items d'un conteneur `SHOP_INFO_ITEM_LIST_BEG_x` à `items` (dédup, ordre).
fn collect_items_from_list(list_node: Node<'_>, items: &mut Vec<HashId>) {
    for item in list_node.children() {
        if !is_item_node(item.name()) {
            continue;
        }
        // itemId à l'index 2, uniquement si présent et de type Int (comme inagle).
        match item.var(2) {
            Some(v) if v.ty == "Int" => push_unique(items, v.as_hash()),
            _ => {}
        }
    }
}

/// Disposition inagle : scanne les enfants d'un shop à la recherche d'item-lists imbriquées.
fn collect_nested_item_lists(shop_node: Node<'_>, items: &mut Vec<HashId>) {
    for child in shop_node.children() {
        if is_item_list_node(child.name()) {
            collect_items_from_list(child, items);
        }
    }
}

/// Insère `id` seulement s'il est absent (sémantique `Set` JS : dédup + ordre d'insertion).
fn push_unique(items: &mut Vec<HashId>, id: HashId) {
    if !items.contains(&id) {
        items.push(id);
    }
}

// ─── Tests internes (valeurs réelles, dump shop_config_3.00.22) ──────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use serde_json::json;

    /// Construit un noeud item `SHOP_INFO_ITEM_y` avec `item_id` à l'index 2 (vars 0/1 à 0).
    fn item_node(idx: usize, item_id: i64) -> Value {
        json!({
            "name": alloc::format!("SHOP_INFO_ITEM_{idx}"),
            "variables": [
                { "type": "Int", "value": "0" },
                { "type": "Int", "value": "0" },
                { "type": "Int", "value": item_id.to_string() },
            ],
            "children": []
        })
    }

    /// Fixture niers (item-list = frère du shop), valeurs réelles de `SHOP_INFO_2`
    /// (avec son doublon `-1937608713`).
    #[test]
    fn shop2_dedup_set_comme_inagle() {
        // Sous-ensemble réel : SHOP_INFO_2 avec le doublon consécutif (-1937608713 x2).
        let items_raw: &[i64] = &[1385048577, -1937608713, -1937608713, 825536693];
        let item_children: Vec<Value> = items_raw
            .iter()
            .enumerate()
            .map(|(i, v)| item_node(i, *v))
            .collect();
        let root = json!({
            "entries": [{
                "name": "SHOP_INFO_LIST_BEG_0",
                "variables": [{ "type": "Int", "value": "0" }],
                "children": [
                    { "name": "SHOP_INFO_2",
                      "variables": [
                          { "type": "Int", "value": "-1321522219" },
                          { "type": "Int", "value": "-1246565820" },
                      ],
                      "children": [] },
                    { "name": "SHOP_INFO_ITEM_LIST_BEG_0",
                      "variables": [{ "type": "Int", "value": "0" }],
                      "children": item_children },
                ]
            }]
        });
        let shops = parse_shop_config(&root);
        assert_eq!(shops.len(), 1);
        let s = &shops[0];
        assert_eq!(s.shop_id, HashId(0xB13B_2BD5));
        assert_eq!(s.name_hash, HashId(0xB5B2_EA44));
        // 4 noeuds → 3 items uniques (doublon fusionné), ordre conservé.
        assert_eq!(
            s.items,
            vec![
                HashId::from_signed(1385048577),
                HashId::from_signed(-1937608713),
                HashId::from_signed(825536693),
            ]
        );
        assert!(s.has_item(HashId::from_signed(-1937608713)));
        assert_eq!(s.item_count(), 3);
    }

    /// Fixture inagle (item-list = enfant du shop) — même résultat (deux dispositions gérées).
    #[test]
    fn disposition_inagle_item_list_enfant() {
        let root = json!({
            "entries": [{
                "name": "SHOP_INFO_LIST_BEG_0",
                "variables": [],
                "children": [{
                    "name": "SHOP_INFO_0",
                    "variables": [
                        { "type": "Int", "value": "1629770002" },
                        { "type": "Int", "value": "889569797" },
                    ],
                    "children": [{
                        "name": "SHOP_INFO_ITEM_LIST_BEG_0",
                        "variables": [],
                        "children": [ item_node(0, 1834815904), item_node(1, 1534657310) ],
                    }],
                }]
            }]
        });
        let shops = parse_shop_config(&root);
        assert_eq!(shops.len(), 1);
        assert_eq!(shops[0].shop_id, HashId(0x6124_5112));
        assert_eq!(shops[0].name_hash, HashId(0x3505_C205));
        assert_eq!(
            shops[0].items,
            vec![HashId(0x6D5D_11A0), HashId(0x5B79_031E)]
        );
    }

    /// Un shop dont `var[1]` manque est ignoré (comme inagle qui renvoie `null`).
    #[test]
    fn shop_sans_name_hash_ignore() {
        let root = json!({
            "entries": [{
                "name": "SHOP_INFO_LIST_BEG_0",
                "variables": [],
                "children": [{
                    "name": "SHOP_INFO_0",
                    "variables": [{ "type": "Int", "value": "1629770002" }],
                    "children": []
                }]
            }]
        });
        assert!(parse_shop_config(&root).is_empty());
    }
}
