//! `HelpListConfig` — port Rust de la famille `help/*.cfg.bin.json` (Level-5 IEVR).
//!
//! Ces fichiers décrivent les tutoriels / pages d'aide du jeu : les diapositives
//! illustrées (textures `.g4tx`), les entrées d'aide (titre/description via jointure
//! texte) et les schémas d'« operation guide » (légendes de manettes positionnées).
//!
//! ## Vérité terrain
//!
//! Un seul fichier dans `data/common/gamedata/help/` :
//! `help_list_config_1.04.08.00.cfg.bin.json` (format **`entries`**, noeuds positionnels).
//!
//! L'arbre réel contient cinq listes (comptes exacts mesurés sur le dump) :
//!
//! | Noeud | Compte | Vars | Rôle |
//! |-------|--------|------|------|
//! | `HELP_LIST_IMAGE_*` | 322 | 6 | diapositive illustrée (fichier `.g4tx` + hashes) |
//! | `HELP_LIST_INFO_*` | 245 | 15 | entrée d'aide (titre/desc + blob opaque) |
//! | `HELP_LIST_INFO_REF_IMAGE_*` | 245 | 2 | plage (offset,count) dans la liste IMAGE |
//! | `HELP_OPERATION_GUIDE_LAYOUT_*` | 216 | 10 | légende positionnée d'un schéma de manette |
//! | `HELP_OPERATION_GUIDE_INFO_*` | 21 | 2 | en-tête de schéma (id + texte) |
//! | `HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_*` | 21 | 2 | plage (offset,count) dans la liste LAYOUT |
//! | `__SORT_INDEX_*` | 245 | 1 | permutation d'affichage des `HELP_LIST_INFO` |
//!
//! Les noeuds `*_REF_*` apparient **par index** avec leur liste maître : `HELP_LIST_INFO_N`
//! va avec `HELP_LIST_INFO_REF_IMAGE_N` (somme des `count` = 322 = nombre d'images),
//! et `HELP_OPERATION_GUIDE_INFO_N` avec `HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_N`
//! (somme des `count` = 216 = nombre de layouts). Les offsets sont contigus et croissants,
//! confirmant la sémantique « tranche d'une liste globale ».
//!
//! ## Sémantique des positions
//!
//! Aucune source nommée n'existe : le parser inagle (`packages/inagle/src/parsers/help-config.ts`)
//! est purement heuristique (il devine titre/desc parmi les petits entiers qui sont des
//! indices de texte valides). Les positions sont donc documentées d'après l'observation
//! statistique du dump réel, jamais inventées. Les positions toujours nulles sont omises ;
//! les positions opaques sont conservées en `raw` (préservation byte-exacte).
//!
//! La position 12 de `HELP_LIST_INFO` est soit l'entier `0`, soit une **chaîne base64**
//! (blob binaire opaque, ex. `"AAAAAA8FNWum6wYAAQAyAAAAAHg="`) : non décodé, conservé tel
//! quel dans [`HelpInfo::blob`].

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, Var};
use crate::hash::HashId;

// ─── Collecte indexée ───────────────────────────────────────────────────────────

/// Visite récursive de l'arbre `entries`, collectant les noeuds dont le nom est
/// exactement `prefix` + un entier décimal (ex. `prefix="HELP_LIST_INFO_"` matche
/// `HELP_LIST_INFO_0` mais **pas** `HELP_LIST_INFO_REF_IMAGE_0` ni `HELP_LIST_INFO_LIST_BEG_0`).
///
/// Renvoie les noeuds triés par leur suffixe numérique croissant — indispensable pour
/// apparier une liste maître avec sa liste `*_REF_*` (appariement positionnel).
fn collect_indexed<'a>(root: &'a Value, prefix: &str) -> Vec<Node<'a>> {
    let mut out: Vec<(u64, Node<'a>)> = Vec::new();
    let mut stack: Vec<Node<'a>> = Vec::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for e in entries.iter().rev() {
            stack.push(Node::new(e));
        }
    }
    while let Some(node) = stack.pop() {
        let name = node.name();
        if let Some(rest) = name.strip_prefix(prefix)
            && let Ok(idx) = rest.parse::<u64>()
        {
            out.push((idx, node));
        }
        for k in node.children().into_iter().rev() {
            stack.push(k);
        }
    }
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, n)| n).collect()
}

// ─── HelpImage ──────────────────────────────────────────────────────────────────

/// Entrée `HELP_LIST_IMAGE_*` — une diapositive illustrée d'un tutoriel (6 vars).
///
/// Positions observées sur `help_list_config_1.04.08.00.cfg.bin.json` :
///
/// | var | type | présence | observé |
/// |-----|------|----------|---------|
/// | \[0\] | String | 100 % | fichier texture `.g4tx` (219 fichiers distincts) |
/// | \[1\] | Int | 100 % | hash (identifiant de sprite/texture interne) |
/// | \[2\] | Int | ~99 % | numéro d'ordre local dans la séquence (1, 2, 3, 4, …) |
/// | \[3\] | Int | ~7 % | hash optionnel (recoupe `HelpGuideInfo.guide_id`) |
/// | \[4\] | Int | ~7 % | hash optionnel |
/// | \[5\] | Int | 0 % | toujours nul — ignoré |
///
/// Vérité terrain : `HELP_LIST_IMAGE_0` =
/// `["hlp_controls.g4tx", 0x1353F111, 1, 0x2156356E, 0x2764916A, 0]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HelpImage {
    /// var\[0\] — nom de fichier texture (`.g4tx`).
    pub image_file: String,
    /// var\[1\] — hash de sprite/texture (toujours non-nul).
    pub sprite_hash: HashId,
    /// var\[2\] — numéro d'ordre local (1-basé) dans la séquence de diapositives.
    pub sort_no: i64,
    /// var\[3\] — hash optionnel (recoupe `HelpGuideInfo.guide_id` ; ~7 %).
    pub guide_hash: HashId,
    /// var\[4\] — hash optionnel (~7 %).
    pub sub_hash: HashId,
}

impl HelpImage {
    /// Parse un noeud `HELP_LIST_IMAGE_N`. Renvoie `None` si le fichier image est absent.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let image_file = node.string(0);
        if image_file.is_empty() {
            return None;
        }
        Some(Self {
            image_file: String::from(image_file),
            sprite_hash: node.hash(1),
            sort_no: node.int(2),
            guide_hash: node.hash(3),
            sub_hash: node.hash(4),
        })
    }
}

// ─── HelpInfo ───────────────────────────────────────────────────────────────────

/// Entrée `HELP_LIST_INFO_*` — une entrée d'aide / tutoriel (15 vars + plage d'images).
///
/// Appariée par index avec `HELP_LIST_INFO_REF_IMAGE_N`, qui donne la tranche
/// `[image_offset, image_offset+image_count)` de la liste globale [`HelpListConfig::images`].
///
/// Positions observées (sur 245 entrées réelles) :
///
/// | var | présence | observé |
/// |-----|----------|---------|
/// | \[0\] | 100 % | `help_id` — identifiant de l'entrée d'aide |
/// | \[1\] | ~47 % | drapeau/catégorie (petit entier) |
/// | \[2\] | 100 % | `display_no` — numéro d'affichage 1-basé séquentiel |
/// | \[3\]\[4\]\[5\] | ~95 % | hashes (probables ids de texte / condition) |
/// | \[7\]\[8\]\[13\]\[14\] | ~5 % | hashes optionnels |
/// | \[9\]\[10\] | ~4 % | hashes optionnels |
/// | \[12\] | ~35 % | blob base64 opaque (sinon entier 0) — voir [`HelpInfo::blob`] |
/// | \[6\]\[11\] | 0 % | toujours nuls |
///
/// Vérité terrain : `HELP_LIST_INFO_0` a `help_id = 0x2A9CD447`, `display_no = 1`,
/// `raw[7] = 0x0EA50CFC`, `raw[8] = 0x33C5254C`, `raw[13] = 0x17BE3DBD`, `raw[14] = 0x2ADE140D`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HelpInfo {
    /// var\[0\] — identifiant de l'entrée d'aide (toujours non-nul).
    pub help_id: HashId,
    /// var\[1\] — drapeau/catégorie (petit entier, souvent 0).
    pub flag: i64,
    /// var\[2\] — numéro d'affichage 1-basé (séquentiel : 1, 2, 3, …).
    pub display_no: i64,
    /// Les 15 positions brutes (hashes), pour préservation byte-exacte. `raw[12]` vaut
    /// toujours `0` (l'éventuel blob base64 vit dans [`HelpInfo::blob`], pas ici).
    ///
    /// `Vec` plutôt que `[HashId; 15]` (cohérence avec la consigne anti-piège serde).
    pub raw: Vec<HashId>,
    /// var\[12\] — blob binaire opaque encodé base64 (non décodé), ou `None` si la
    /// position est l'entier `0`. Préservé tel quel pour fidélité byte-exacte.
    pub blob: Option<String>,
    /// `HELP_LIST_INFO_REF_IMAGE_N.var[0]` — index de début dans [`HelpListConfig::images`].
    pub image_offset: i64,
    /// `HELP_LIST_INFO_REF_IMAGE_N.var[1]` — nombre d'images de cette entrée.
    pub image_count: i64,
}

impl HelpInfo {
    /// Parse un noeud `HELP_LIST_INFO_N` apparié à son `HELP_LIST_INFO_REF_IMAGE_N`.
    /// Renvoie `None` si `help_id` est nul.
    #[must_use]
    pub fn from_nodes(info: Node<'_>, ref_image: Option<Node<'_>>) -> Option<Self> {
        let help_id = info.hash(0);
        if help_id.is_zero() {
            return None;
        }
        let mut raw = Vec::with_capacity(15);
        let mut blob = None;
        for i in 0..15 {
            match info.var(i) {
                // Position 12 peut être une chaîne base64 : on la met de côté et on
                // pousse 0 dans `raw` (la chaîne n'est pas un hash).
                Some(Var {
                    ty: "String",
                    value,
                    ..
                }) => {
                    blob = Some(String::from(value));
                    raw.push(HashId::ZERO);
                }
                Some(v) => raw.push(v.as_hash()),
                None => raw.push(HashId::ZERO),
            }
        }
        let (image_offset, image_count) = match ref_image {
            Some(r) => (r.int(0), r.int(1)),
            None => (0, 0),
        };
        Some(Self {
            help_id,
            flag: info.int(1),
            display_no: info.int(2),
            raw,
            blob,
            image_offset,
            image_count,
        })
    }
}

// ─── HelpGuideLayout ─────────────────────────────────────────────────────────────

/// Entrée `HELP_OPERATION_GUIDE_LAYOUT_*` — une légende positionnée d'un schéma de
/// manette (10 vars).
///
/// Positions observées (sur 216 entrées réelles) :
///
/// | var | présence | observé |
/// |-----|----------|---------|
/// | \[0\] | 100 % | petit entier (numéro de page/groupe, 1-basé) |
/// | \[1\] | 100 % | petit entier (numéro de ligne dans la page) |
/// | \[2\] | ~17 % | petit drapeau (souvent 0) |
/// | \[3\] | 100 % | hash (id de bouton/texte) |
/// | \[6\] | 100 % | hash secondaire |
/// | \[8\]\[9\] | ~99 % | coordonnées **signées** (ex. -859, 247) |
/// | \[4\]\[5\]\[7\] | ~0 % | quasi toujours nuls |
///
/// Vérité terrain : `HELP_OPERATION_GUIDE_LAYOUT_0` =
/// `[1, 2, 1, 0xF2DD6185, 0, 0, 0xF52C33F5, 0, -859, 247]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HelpGuideLayout {
    /// var\[0\] — numéro de page/groupe (1-basé).
    pub page_no: i64,
    /// var\[1\] — numéro de ligne dans la page.
    pub row_no: i64,
    /// var\[2\] — drapeau (souvent 0).
    pub flag: i64,
    /// var\[3\] — hash (id de bouton/texte associé).
    pub button_hash: HashId,
    /// var\[6\] — hash secondaire.
    pub sub_hash: HashId,
    /// var\[8\] — coordonnée signée (X observé, ex. -859).
    pub coord_x: i64,
    /// var\[9\] — coordonnée signée (Y observé, ex. 247).
    pub coord_y: i64,
}

impl HelpGuideLayout {
    /// Parse un noeud `HELP_OPERATION_GUIDE_LAYOUT_N`. Renvoie `None` si `button_hash`
    /// est nul (toutes les entrées réelles l'ont non-nul).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Option<Self> {
        let button_hash = node.hash(3);
        if button_hash.is_zero() {
            return None;
        }
        Some(Self {
            page_no: node.int(0),
            row_no: node.int(1),
            flag: node.int(2),
            button_hash,
            sub_hash: node.hash(6),
            coord_x: node.int(8),
            coord_y: node.int(9),
        })
    }
}

// ─── HelpGuideInfo ───────────────────────────────────────────────────────────────

/// Entrée `HELP_OPERATION_GUIDE_INFO_*` — en-tête d'un schéma d'« operation guide »
/// (2 vars + plage de layouts).
///
/// Appariée par index avec `HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_N`, qui donne la
/// tranche `[layout_offset, layout_offset+layout_count)` de [`HelpListConfig::guide_layouts`].
///
/// Vérité terrain : `HELP_OPERATION_GUIDE_INFO_0` = `[0x2156356E, 0x74C5F394]`,
/// `HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_0` = `[0, 18]` (18 layouts à partir de l'index 0).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HelpGuideInfo {
    /// var\[0\] — identifiant du schéma (recoupe `HelpImage.guide_hash`).
    pub guide_id: HashId,
    /// var\[1\] — hash de texte/titre du schéma.
    pub text_id: HashId,
    /// `REF_LAYOUT.var[0]` — index de début dans [`HelpListConfig::guide_layouts`].
    pub layout_offset: i64,
    /// `REF_LAYOUT.var[1]` — nombre de layouts de ce schéma.
    pub layout_count: i64,
}

impl HelpGuideInfo {
    /// Parse un noeud `HELP_OPERATION_GUIDE_INFO_N` apparié à son `..._REF_LAYOUT_N`.
    /// Renvoie `None` si `guide_id` est nul.
    #[must_use]
    pub fn from_nodes(info: Node<'_>, ref_layout: Option<Node<'_>>) -> Option<Self> {
        let guide_id = info.hash(0);
        if guide_id.is_zero() {
            return None;
        }
        let (layout_offset, layout_count) = match ref_layout {
            Some(r) => (r.int(0), r.int(1)),
            None => (0, 0),
        };
        Some(Self {
            guide_id,
            text_id: info.hash(1),
            layout_offset,
            layout_count,
        })
    }
}

// ─── HelpListConfig ──────────────────────────────────────────────────────────────

/// Contenu complet d'un `help_list_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_help_list_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HelpListConfig {
    /// `HELP_LIST_IMAGE_*` — 322 diapositives illustrées (liste globale, référencée par tranches).
    pub images: Vec<HelpImage>,
    /// `HELP_LIST_INFO_*` — 245 entrées d'aide (chacune référence une tranche d'`images`).
    pub infos: Vec<HelpInfo>,
    /// `HELP_OPERATION_GUIDE_LAYOUT_*` — 216 légendes positionnées (liste globale).
    pub guide_layouts: Vec<HelpGuideLayout>,
    /// `HELP_OPERATION_GUIDE_INFO_*` — 21 schémas (chacun référence une tranche de `guide_layouts`).
    pub guide_infos: Vec<HelpGuideInfo>,
    /// `__SORT_INDEX_*` — permutation d'affichage des `infos` (245 indices, ordre de tri).
    pub sort_order: Vec<i64>,
}

impl HelpListConfig {
    /// Renvoie la tranche d'[`HelpImage`] référencée par une entrée d'aide.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn images_of(&self, info: &HelpInfo) -> &[HelpImage] {
        let start = info.image_offset.max(0) as usize;
        let end = start.saturating_add(info.image_count.max(0) as usize);
        self.images.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de [`HelpGuideLayout`] référencée par un schéma.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn layouts_of(&self, info: &HelpGuideInfo) -> &[HelpGuideLayout] {
        let start = info.layout_offset.max(0) as usize;
        let end = start.saturating_add(info.layout_count.max(0) as usize);
        self.guide_layouts.get(start..end).unwrap_or(&[])
    }

    /// Recherche une entrée d'aide par `help_id`. `None` si absente.
    #[must_use]
    pub fn find_info(&self, help_id: HashId) -> Option<&HelpInfo> {
        self.infos.iter().find(|i| i.help_id == help_id)
    }
}

// ─── Parseur principal ───────────────────────────────────────────────────────────

/// Parse un `help_list_config_*.cfg.bin.json` complet.
///
/// Apparie chaque `HELP_LIST_INFO_N` à son `HELP_LIST_INFO_REF_IMAGE_N` et chaque
/// `HELP_OPERATION_GUIDE_INFO_N` à son `..._REF_LAYOUT_N` (appariement par index).
///
/// Vérité terrain : `help_list_config_1.04.08.00.cfg.bin.json` →
/// 322 images, 245 infos, 216 layouts, 21 guide_infos, 245 indices de tri.
#[must_use]
pub fn parse_help_list_config(root: &Value) -> HelpListConfig {
    // Listes maîtresses + listes de références (triées par index pour appariement).
    let images: Vec<HelpImage> = collect_indexed(root, "HELP_LIST_IMAGE_")
        .into_iter()
        .filter_map(HelpImage::from_node)
        .collect();

    let info_nodes = collect_indexed(root, "HELP_LIST_INFO_");
    let ref_image_nodes = collect_indexed(root, "HELP_LIST_INFO_REF_IMAGE_");
    let infos: Vec<HelpInfo> = info_nodes
        .into_iter()
        .enumerate()
        .filter_map(|(i, info)| HelpInfo::from_nodes(info, ref_image_nodes.get(i).copied()))
        .collect();

    let guide_layouts: Vec<HelpGuideLayout> = collect_indexed(root, "HELP_OPERATION_GUIDE_LAYOUT_")
        .into_iter()
        .filter_map(HelpGuideLayout::from_node)
        .collect();

    let guide_info_nodes = collect_indexed(root, "HELP_OPERATION_GUIDE_INFO_");
    let ref_layout_nodes = collect_indexed(root, "HELP_OPERATION_GUIDE_INFO_REF_LAYOUT_");
    let guide_infos: Vec<HelpGuideInfo> = guide_info_nodes
        .into_iter()
        .enumerate()
        .filter_map(|(i, info)| HelpGuideInfo::from_nodes(info, ref_layout_nodes.get(i).copied()))
        .collect();

    let sort_order: Vec<i64> = collect_indexed(root, "__SORT_INDEX_")
        .into_iter()
        .map(|n| n.int(0))
        .collect();

    HelpListConfig {
        images,
        infos,
        guide_layouts,
        guide_infos,
        sort_order,
    }
}
