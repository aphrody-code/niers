//! `CraftObjConfig` + `CraftThemeConfig` — port Rust de la famille `craft/*.cfg.bin.json`
//! (Level-5 IEVR, artisanat / « Craft »).
//!
//! ## Vérité terrain
//!
//! Deux fichiers dans `data/common/gamedata/craft/`, tous deux au format **`entries`**
//! (noeuds nommés, variables positionnelles — aucun nom officiel sans source TS).
//! La seule référence TS connue est `packages/inagle/src/parsers/gameplay-config.ts`
//! (`buildCraftDatabase`), qui n'extrait que `CRAFT_OBJ_INFO_*.var[0]` (= `craft_id`) et
//! laisse explicitement le reste « Placeholder until we map fields ». Les positions
//! au-delà de `var[0]` sont donc documentées par observation, jamais par invention.
//!
//! ### `craft_obj_config_1.04.10.00.cfg.bin.json` (6 listes)
//!
//! | Liste (préfixe `CRAFT_OBJ_…`) | Entrées | Vars | Modèle |
//! |-------------------------------|---------|------|--------|
//! | `INTERREST_LOTTERY_UNIQUE_CHARA_INFO_` | 1005 | 2 | [`LotteryUniqueCharaInfo`] |
//! | `INTERREST_SOCKET_INFO_`               | 87   | 4 | [`SocketInfo`] |
//! | `NPC_STICK_POINT_INFO_`                | 105  | 6 | [`NpcStickPointInfo`] |
//! | `VISUAL_INFO_`                         | 187  | 3 | [`VisualInfo`] |
//! | `VISUAL_GROUP_INFO_` (+ `_REF_VISUAL_INFO_`) | 167 | 1 (+2) | [`VisualGroupInfo`] |
//! | `INFO_` (+ 4 `_REF_…` + `CATEGORY_INFO_`) | 157 | 28 | [`CraftObjInfo`] |
//!
//! Les noeuds `CRAFT_OBJ_INFO_N` sont chacun suivis (en frères plats sous le même
//! `…_LIST_BEG_`) de 4 noeuds `CRAFT_OBJ_INFO_REF_…_N` portant chacun `[offset, count]`
//! pointant une plage dans la sous-liste correspondante. Ils sont ré-appariés par
//! position (ordre du fichier conservé par le parcours profondeur-d'abord).
//!
//! ### `craft_theme_config_0.00.00.cfg.bin.json` (2 listes)
//!
//! - `CRAFT_THEME_TYPE_INFO_` — 9 entrées (6 vars) → [`CraftThemeTypeInfo`].
//! - `CRAFT_THEME_INFO_` — 3 entrées (3 vars), chacune suivie d'un
//!   `CRAFT_THEME_INFO_REF_TYPE_INFO_` `[offset, count]` pointant la plage de types
//!   du thème → [`CraftThemeInfo`] (offsets 0/3/6, count 3 ; 3 × 3 = 9 types).
//!
//! ## Conservation des flottants
//!
//! Plusieurs positions mêlent `Int` et `Float` selon les entrées (coordonnées NPC,
//! taux de socket, coefficients d'objet). Pour rester fidèle au dump, ces positions
//! sont stockées en `f64` (les valeurs hash signées y tiennent exactement, `|x| < 2^31`).
//! Les positions purement hash/entier conservent `HashId`/`i64`.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Lit la variable `i` d'un noeud comme `f64` (0.0 si absente).
///
/// Conserve les `Float` (ex. `"1,5"` → 1.5) comme les `Int` (ex. `"5"` → 5.0).
#[must_use]
fn node_f64(node: Node<'_>, i: usize) -> f64 {
    node.var(i).map_or(0.0, |v| v.as_f64())
}

// ─── RefRange ───────────────────────────────────────────────────────────────────

/// Plage `[offset, count]` d'un noeud de référence (`…_REF_…`).
///
/// Référence une tranche `entries[offset .. offset + count]` d'une sous-liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RefRange {
    /// var\[0\] — index de début (0-basé) dans la sous-liste référencée.
    pub offset: i64,
    /// var\[1\] — nombre d'entrées de la plage.
    pub count: i64,
}

impl RefRange {
    /// Parse un noeud de référence (`[offset, count]`).
    #[must_use]
    fn from_node(node: Node<'_>) -> Self {
        Self {
            offset: node.int(0),
            count: node.int(1),
        }
    }
}

// ─── LotteryUniqueCharaInfo ───────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_*` (2 vars).
///
/// Table de tirage des personnages uniques « d'intérêt » liés à un objet de craft.
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, entrée 0 :
/// `[-1717452464, 2]` → `chara_id = 0x99A1C150`, `weight = 2.0`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LotteryUniqueCharaInfo {
    /// var\[0\] — hash du personnage unique (toujours non-nul).
    pub chara_id: HashId,
    /// var\[1\] — poids/quantité de tirage observé ∈ {1.0, 2.0, 1.5}.
    ///
    /// `Float` une seule fois sur 1005 entrées (entrée 688 : `1,5`) ; sinon `Int`.
    /// Sémantique exacte inconnue sans source TS.
    pub weight: f64,
}

impl LotteryUniqueCharaInfo {
    /// Parse un noeud. `None` si `chara_id` est nul.
    #[must_use]
    fn from_node(node: Node<'_>) -> Option<Self> {
        let chara_id = node.hash(0);
        if chara_id.is_zero() {
            return None;
        }
        Some(Self {
            chara_id,
            weight: node_f64(node, 1),
        })
    }
}

// ─── SocketInfo ───────────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_INTERREST_SOCKET_INFO_*` (4 vars).
///
/// 4 variables positionnelles brutes. Observé : var\[0\] = 1, var\[1\] = 0, var\[3\] = 0
/// constants ; var\[2\] varie (entier ou flottant, ex. `0,5`, `1,5`, `3`, `5`) — probable
/// taux/coefficient. Sémantique exacte inconnue sans source TS, d'où le stockage brut.
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, entrée 6 :
/// `[1, 0, 0.5, 0]` → `raw = [1.0, 0.0, 0.5, 0.0]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SocketInfo {
    /// var\[0\..=3\] — variables positionnelles brutes (var\[2\] porte le flottant variable).
    pub raw: [f64; 4],
}

impl SocketInfo {
    /// Parse un noeud (4 variables, jamais filtré).
    #[must_use]
    fn from_node(node: Node<'_>) -> Self {
        Self {
            raw: [
                node_f64(node, 0),
                node_f64(node, 1),
                node_f64(node, 2),
                node_f64(node, 3),
            ],
        }
    }
}

// ─── NpcStickPointInfo ────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_NPC_STICK_POINT_INFO_*` (6 vars).
///
/// Point d'accroche d'un PNJ sur l'objet de craft — données géométriques (var\[0\] et
/// var\[2\] souvent flottantes : offsets/coordonnées ; var\[3\] = angle entier ∈ {-1, 0,
/// 45, 90, 180, 270, 310…}). Sémantique précise inconnue sans source TS.
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, entrée 1 :
/// `[1.7, 0, 1.5, -1, 0, 0]` → `raw = [1.7, 0.0, 1.5, -1.0, 0.0, 0.0]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NpcStickPointInfo {
    /// var\[0\..=5\] — variables positionnelles brutes (var\[0\], var\[2\] flottantes).
    pub raw: [f64; 6],
}

impl NpcStickPointInfo {
    /// Parse un noeud (6 variables, jamais filtré).
    #[must_use]
    fn from_node(node: Node<'_>) -> Self {
        let mut raw = [0.0_f64; 6];
        for (i, slot) in raw.iter_mut().enumerate() {
            *slot = node_f64(node, i);
        }
        Self { raw }
    }
}

// ─── VisualInfo ───────────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_VISUAL_INFO_*` (3 vars).
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, entrée 0 :
/// `[757857389, 0, 1]` → `visual_id = 0x2D2BFC6D`, `secondary = 0x00000000`, `count = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualInfo {
    /// var\[0\] — hash du visuel/modèle (toujours non-nul).
    pub visual_id: HashId,
    /// var\[1\] — hash secondaire (souvent nul ; un seul autre hash observé).
    pub secondary: HashId,
    /// var\[2\] — petit entier ∈ {1, 5, 15, 30, 50} (probable poids/durée).
    pub count: i64,
}

impl VisualInfo {
    /// Parse un noeud. `None` si `visual_id` est nul.
    #[must_use]
    fn from_node(node: Node<'_>) -> Option<Self> {
        let visual_id = node.hash(0);
        if visual_id.is_zero() {
            return None;
        }
        Some(Self {
            visual_id,
            secondary: node.hash(1),
            count: node.int(2),
        })
    }
}

// ─── VisualGroupInfo ──────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_VISUAL_GROUP_INFO_*` (1 var) + son `_REF_VISUAL_INFO_` apparié.
///
/// Chaque groupe (hash `group_id`) est suivi, en frère plat, d'un unique
/// `CRAFT_OBJ_VISUAL_GROUP_INFO_REF_VISUAL_INFO_N` portant `[visual_index, count]`
/// (plage dans la liste `VisualInfo`). Le `group_id` se répète entre groupes (collections).
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, groupe 0 :
/// `group_id = 0xE35E00DF`, ref `[0, 1]` → `visual_index = 0`, `count = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualGroupInfo {
    /// var\[0\] du noeud `…_GROUP_INFO_N` — hash du groupe (peut se répéter).
    pub group_id: HashId,
    /// var\[0\] du `…_REF_VISUAL_INFO_N` — index de début dans la liste `VisualInfo`.
    pub visual_index: i64,
    /// var\[1\] du `…_REF_VISUAL_INFO_N` — nombre de visuels du groupe.
    pub count: i64,
}

// ─── CraftCategoryInfo ────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_CATEGORY_INFO_*` (2 vars) — sous-liste de `CRAFT_OBJ_INFO_LIST_BEG_0`.
///
/// 5 catégories. Vérité terrain : `[1, 80]`, `[2, 50]`, `[3, 20]`, `[4, 100]`, `[5, 2]`
/// → probable plafond/quota par catégorie. Sémantique exacte inconnue sans source TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftCategoryInfo {
    /// var\[0\] — identifiant de catégorie (1..=5).
    pub category: i64,
    /// var\[1\] — valeur associée (plafond/quota observé : 80, 50, 20, 100, 2).
    pub value: i64,
}

impl CraftCategoryInfo {
    /// Parse un noeud (2 variables).
    #[must_use]
    fn from_node(node: Node<'_>) -> Self {
        Self {
            category: node.int(0),
            value: node.int(1),
        }
    }
}

// ─── CraftObjInfo ─────────────────────────────────────────────────────────────────

/// Entrée `CRAFT_OBJ_INFO_*` (28 vars) — définition principale d'un objet de craft.
///
/// var\[0\] est le `craft_id` (extrait par inagle). Les 27 variables suivantes sont
/// positionnelles, de sémantique non documentée (inagle : « Placeholder until we map
/// fields ») et mêlent entiers, hashes et flottants ; elles sont conservées brutes
/// dans `raw` (f64, sans perte pour les valeurs observées).
///
/// Les 4 [`RefRange`] pointent les sous-listes associées (loterie, sockets, points
/// d'accroche PNJ, groupe de visuels), ré-appariés par position de fichier.
///
/// Vérité terrain : `craft_obj_config_1.04.10.00`, objet 0 :
/// `craft_id = 0x46B34E49` ; ref_visual_group = `[0, 1]` ; les 3 autres refs = `[0, 0]`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftObjInfo {
    /// var\[0\] — identifiant de l'objet de craft (toujours non-nul).
    pub craft_id: HashId,
    /// var\[1\..=27\] — 27 variables positionnelles brutes (entiers/hashes/flottants).
    ///
    /// Positions hash observées : `raw[12]` (var\[13\]) et `raw[14]` (var\[15\]) portent
    /// parfois un hash signé. Sémantique exacte inconnue sans source TS.
    pub raw: Vec<f64>,
    /// `CRAFT_OBJ_INFO_REF_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_N` — plage loterie.
    pub ref_lottery: RefRange,
    /// `CRAFT_OBJ_INFO_REF_INTERREST_SOCKET_INFO_N` — plage sockets.
    pub ref_socket: RefRange,
    /// `CRAFT_OBJ_INFO_REF_NPC_STICK_POINT_INFO_N` — plage points d'accroche PNJ.
    pub ref_npc_stick: RefRange,
    /// `CRAFT_OBJ_INFO_REF_VISUAL_GROUP_INFO_N` — plage groupes de visuels.
    pub ref_visual_group: RefRange,
}

// ─── CraftThemeTypeInfo ───────────────────────────────────────────────────────────

/// Entrée `CRAFT_THEME_TYPE_INFO_*` (6 vars) — type/variante d'un thème de craft.
///
/// Vérité terrain : `craft_theme_config_0.00.00`, type 0 :
/// `[-1537270480, -1537270480, -406116050, 895208218, Int "0", 0]`
/// → `type_id = 0xA45F1D30` (= `type_id_dup`), `sub_hash = 0xE7CB292E`,
///   `variant_hash = 0x355BCB1A`, `data_b64 = ""`, `extra_hash = 0x00000000`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftThemeTypeInfo {
    /// var\[0\] — hash du type (3 valeurs distinctes, se répètent).
    pub type_id: HashId,
    /// var\[1\] — doublon exact de var\[0\] dans toutes les entrées réelles.
    pub type_id_dup: HashId,
    /// var\[2\] — hash secondaire (souvent nul).
    pub sub_hash: HashId,
    /// var\[3\] — hash de variante (unique par entrée).
    pub variant_hash: HashId,
    /// var\[4\] — données encodées base64 (chaîne vide si `Int "0"`, cas de la 1re entrée).
    pub data_b64: String,
    /// var\[5\] — hash additionnel (nul pour la 1re entrée).
    pub extra_hash: HashId,
}

impl CraftThemeTypeInfo {
    /// Parse un noeud. `None` si `type_id` est nul.
    #[must_use]
    fn from_node(node: Node<'_>) -> Option<Self> {
        let type_id = node.hash(0);
        if type_id.is_zero() {
            return None;
        }
        // var[4] : String → base64 présent ; Int "0" → absent (chaîne vide).
        let data_b64 = match node.var(4) {
            Some(var) if var.ty == "String" => String::from(var.value),
            _ => String::new(),
        };
        Some(Self {
            type_id,
            type_id_dup: node.hash(1),
            sub_hash: node.hash(2),
            variant_hash: node.hash(3),
            data_b64,
            extra_hash: node.hash(5),
        })
    }
}

// ─── CraftThemeInfo ───────────────────────────────────────────────────────────────

/// Entrée `CRAFT_THEME_INFO_*` (3 vars) + son `_REF_TYPE_INFO_` apparié.
///
/// Chaque thème référence une plage de [`CraftThemeTypeInfo`] via le noeud frère
/// `CRAFT_THEME_INFO_REF_TYPE_INFO_N` (`[offset, count]`).
///
/// Vérité terrain : `craft_theme_config_0.00.00`, thème 0 :
/// `[-1917049535, 489051273, 0]` → `theme_id = 0x8DBC2541`, `hash1 = 0x1D265489`,
///   `hash2 = 0x00000000` ; ref_types = `[0, 3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftThemeInfo {
    /// var\[0\] — hash du thème (toujours non-nul).
    pub theme_id: HashId,
    /// var\[1\] — hash secondaire.
    pub hash1: HashId,
    /// var\[2\] — hash tertiaire (nul pour le 1er thème).
    pub hash2: HashId,
    /// `…_REF_TYPE_INFO_N` — plage de `CraftThemeTypeInfo` du thème.
    pub ref_types: RefRange,
}

// ─── CraftObjConfig ───────────────────────────────────────────────────────────────

/// Contenu complet de `craft_obj_config_*.cfg.bin.json` (6 listes + catégories).
///
/// Utiliser [`parse_craft_obj_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftObjConfig {
    /// `CRAFT_OBJ_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_*` — 1005 entrées.
    pub lottery_unique_charas: Vec<LotteryUniqueCharaInfo>,
    /// `CRAFT_OBJ_INTERREST_SOCKET_INFO_*` — 87 entrées.
    pub sockets: Vec<SocketInfo>,
    /// `CRAFT_OBJ_NPC_STICK_POINT_INFO_*` — 105 entrées.
    pub npc_stick_points: Vec<NpcStickPointInfo>,
    /// `CRAFT_OBJ_VISUAL_INFO_*` — 187 entrées.
    pub visual_infos: Vec<VisualInfo>,
    /// `CRAFT_OBJ_VISUAL_GROUP_INFO_*` (+ refs) — 167 entrées.
    pub visual_groups: Vec<VisualGroupInfo>,
    /// `CRAFT_OBJ_INFO_*` (+ 4 refs) — 157 objets de craft.
    pub objs: Vec<CraftObjInfo>,
    /// `CRAFT_OBJ_CATEGORY_INFO_*` — 5 catégories.
    pub categories: Vec<CraftCategoryInfo>,
}

impl CraftObjConfig {
    /// Recherche un objet de craft par son `craft_id`. `None` si absent.
    #[must_use]
    pub fn find_obj(&self, craft_id: HashId) -> Option<&CraftObjInfo> {
        self.objs.iter().find(|o| o.craft_id == craft_id)
    }

    /// Renvoie la tranche de [`VisualInfo`] référencée par un groupe de visuels.
    ///
    /// Retourne `&[]` si la plage dépasse la liste.
    #[must_use]
    pub fn visuals_of_group(&self, group: &VisualGroupInfo) -> &[VisualInfo] {
        let start = group.visual_index.max(0) as usize;
        let end = start.saturating_add(group.count.max(0) as usize);
        self.visual_infos.get(start..end).unwrap_or(&[])
    }
}

// ─── CraftThemeConfig ─────────────────────────────────────────────────────────────

/// Contenu complet de `craft_theme_config_*.cfg.bin.json` (2 listes).
///
/// Utiliser [`parse_craft_theme_config`] pour construire depuis un JSON désérialisé.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CraftThemeConfig {
    /// `CRAFT_THEME_TYPE_INFO_*` — 9 types.
    pub theme_types: Vec<CraftThemeTypeInfo>,
    /// `CRAFT_THEME_INFO_*` (+ refs) — 3 thèmes.
    pub themes: Vec<CraftThemeInfo>,
}

impl CraftThemeConfig {
    /// Renvoie la tranche de [`CraftThemeTypeInfo`] référencée par un thème.
    ///
    /// Retourne `&[]` si la plage dépasse la liste.
    #[must_use]
    pub fn types_of_theme(&self, theme: &CraftThemeInfo) -> &[CraftThemeTypeInfo] {
        let start = theme.ref_types.offset.max(0) as usize;
        let end = start.saturating_add(theme.ref_types.count.max(0) as usize);
        self.theme_types.get(start..end).unwrap_or(&[])
    }
}

// ─── Parseurs ─────────────────────────────────────────────────────────────────────

/// Collecte tous les noeuds dont le nom commence par `prefix`, en excluant les
/// conteneurs `_LIST_` et, si `skip_ref`, les noeuds de référence `_REF_`.
fn collect<'a>(root: &'a Value, prefix: &str, skip_ref: bool) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    walk_named(root, prefix, |node| {
        let name = node.name();
        if name.contains("_LIST_") {
            return;
        }
        if skip_ref && name.contains("_REF_") {
            return;
        }
        out.push(node);
    });
    out
}

/// Parse un `craft_obj_config_*.cfg.bin.json` complet.
///
/// Comptes réels (`craft_obj_config_1.04.10.00`) : 1005 loteries, 87 sockets,
/// 105 points d'accroche, 187 visuels, 167 groupes, 157 objets, 5 catégories.
#[must_use]
pub fn parse_craft_obj_config(root: &Value) -> CraftObjConfig {
    let lottery_unique_charas = collect(
        root,
        "CRAFT_OBJ_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_",
        false,
    )
    .into_iter()
    .filter_map(LotteryUniqueCharaInfo::from_node)
    .collect();

    let sockets = collect(root, "CRAFT_OBJ_INTERREST_SOCKET_INFO_", false)
        .into_iter()
        .map(SocketInfo::from_node)
        .collect();

    let npc_stick_points = collect(root, "CRAFT_OBJ_NPC_STICK_POINT_INFO_", false)
        .into_iter()
        .map(NpcStickPointInfo::from_node)
        .collect();

    let visual_infos = collect(root, "CRAFT_OBJ_VISUAL_INFO_", false)
        .into_iter()
        .filter_map(VisualInfo::from_node)
        .collect();

    // Groupes de visuels : noeuds `…_GROUP_INFO_N` (1 var) appariés 1:1 par position
    // avec leurs `…_GROUP_INFO_REF_VISUAL_INFO_N` ([visual_index, count]).
    let group_nodes = collect(root, "CRAFT_OBJ_VISUAL_GROUP_INFO_", true);
    let group_refs = collect(root, "CRAFT_OBJ_VISUAL_GROUP_INFO_REF_VISUAL_INFO_", false);
    let visual_groups = group_nodes
        .iter()
        .zip(group_refs.iter())
        .map(|(g, r)| {
            let rng = RefRange::from_node(*r);
            VisualGroupInfo {
                group_id: g.hash(0),
                visual_index: rng.offset,
                count: rng.count,
            }
        })
        .collect();

    // Objets de craft : noeud principal `CRAFT_OBJ_INFO_N` + 4 refs appariés par position.
    let obj_nodes = collect_obj_main(root);
    let ref_lottery = collect(
        root,
        "CRAFT_OBJ_INFO_REF_INTERREST_LOTTERY_UNIQUE_CHARA_INFO_",
        false,
    );
    let ref_socket = collect(root, "CRAFT_OBJ_INFO_REF_INTERREST_SOCKET_INFO_", false);
    let ref_npc = collect(root, "CRAFT_OBJ_INFO_REF_NPC_STICK_POINT_INFO_", false);
    let ref_visual = collect(root, "CRAFT_OBJ_INFO_REF_VISUAL_GROUP_INFO_", false);
    let objs = obj_nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| {
            let craft_id = node.hash(0);
            if craft_id.is_zero() {
                return None;
            }
            let raw = (1..node.var_count()).map(|j| node_f64(*node, j)).collect();
            Some(CraftObjInfo {
                craft_id,
                raw,
                ref_lottery: ref_lottery
                    .get(i)
                    .map(|n| RefRange::from_node(*n))
                    .unwrap_or_default(),
                ref_socket: ref_socket
                    .get(i)
                    .map(|n| RefRange::from_node(*n))
                    .unwrap_or_default(),
                ref_npc_stick: ref_npc
                    .get(i)
                    .map(|n| RefRange::from_node(*n))
                    .unwrap_or_default(),
                ref_visual_group: ref_visual
                    .get(i)
                    .map(|n| RefRange::from_node(*n))
                    .unwrap_or_default(),
            })
        })
        .collect();

    let categories = collect(root, "CRAFT_OBJ_CATEGORY_INFO_", false)
        .into_iter()
        .map(CraftCategoryInfo::from_node)
        .collect();

    CraftObjConfig {
        lottery_unique_charas,
        sockets,
        npc_stick_points,
        visual_infos,
        visual_groups,
        objs,
        categories,
    }
}

/// Collecte les noeuds principaux `CRAFT_OBJ_INFO_N` (hors `_REF_`, `_LIST_`, `_CATEGORY_`).
///
/// `_CATEGORY_` ne commence pas par `CRAFT_OBJ_INFO_` mais on l'exclut par prudence.
fn collect_obj_main(root: &Value) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    walk_named(root, "CRAFT_OBJ_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_") || name.contains("_CATEGORY_") {
            return;
        }
        out.push(node);
    });
    out
}

/// Parse un `craft_theme_config_*.cfg.bin.json` complet.
///
/// Comptes réels (`craft_theme_config_0.00.00`) : 9 types, 3 thèmes.
#[must_use]
pub fn parse_craft_theme_config(root: &Value) -> CraftThemeConfig {
    let theme_types = collect(root, "CRAFT_THEME_TYPE_INFO_", false)
        .into_iter()
        .filter_map(CraftThemeTypeInfo::from_node)
        .collect();

    // Thèmes : `CRAFT_THEME_INFO_N` (hors TYPE, hors REF) + ref `…_REF_TYPE_INFO_N`.
    let theme_nodes = collect_theme_main(root);
    let theme_refs = collect(root, "CRAFT_THEME_INFO_REF_TYPE_INFO_", false);
    let themes = theme_nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| {
            let theme_id = node.hash(0);
            if theme_id.is_zero() {
                return None;
            }
            Some(CraftThemeInfo {
                theme_id,
                hash1: node.hash(1),
                hash2: node.hash(2),
                ref_types: theme_refs
                    .get(i)
                    .map(|n| RefRange::from_node(*n))
                    .unwrap_or_default(),
            })
        })
        .collect();

    CraftThemeConfig {
        theme_types,
        themes,
    }
}

/// Collecte les noeuds principaux `CRAFT_THEME_INFO_N` (hors `_TYPE_`, `_REF_`, `_LIST_`).
fn collect_theme_main(root: &Value) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    walk_named(root, "CRAFT_THEME_INFO_", |node| {
        let name = node.name();
        if name.contains("_LIST_") || name.contains("_REF_") || name.contains("_TYPE_") {
            return;
        }
        out.push(node);
    });
    out
}
