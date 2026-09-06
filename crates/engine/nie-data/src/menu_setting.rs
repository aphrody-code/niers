//! Famille `menu_setting` — port Rust de `*_menu_setting.cfg.bin` (Level-5 IEVR), la
//! **définition d'un écran de menu** : la liste des *layers* (couches d'objbin) qui le
//! composent, les ressources g4tx partagées, les commandes et les groupes de focus.
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/menu/cfg/main_menu_setting.cfg.bin` (VFS IEVR),
//!   format **T2B** (`entries`). 485 fichiers `*_menu_setting.cfg.bin` (un par écran).
//! - **PAS de parseur inagle** : RE originale niers. Validée **end-to-end** par auto-cohérence
//!   forte — pour CHAQUE layer, `var[0] == CRC32(var[1])` (l'identifiant EST le CRC32 du nom ;
//!   vérifié sur les 13 layers de `main_menu`, cf. `tests/menu_setting_golden.rs`).
//!
//! ## Rôle (sous-système « driver de menu », D1.c-driver)
//!
//! C'est la **SCÈNE** que le moteur énumère pour construire un écran : le manager
//! `0x14109D190` (RE `nie.exe`) lit cette liste de layers (`{layerId, name, valid}` en
//! mémoire), charge l'objbin de chacun, puis exécute le script Lua associé. **Découverte
//! structurante** : un écran compose des layers de **plusieurs préfixes** d'objbin — p.ex.
//! `main_menu` mêle `mainmenu90_*` (fond/en-tête/onglets), `cmn01_*` (icônes communes),
//! `mainmenu01_*` (button-guides) et `rpg00_*`. Le filtre par préfixe de nom de
//! `build_sprite_list` (`basename.starts_with("mainmenu01")`) ne voit donc qu'une PARTIE
//! de l'écran ; la composition correcte se lit ICI.
//!
//! ## Structure (T2B `entries`)
//!
//! Le fichier porte 7 listes ; ce module modélise les deux exploitables aujourd'hui :
//! - `MENU_LAYER_INFO_LIST_BEG > MENU_LAYER_INFO*` — un layer par enfant.
//! - `MENU_RES_LIST_BEG > MENU_RES*` — une ressource g4tx partagée par enfant.
//!
//! ### Variables positionnelles d'un `MENU_LAYER_INFO`
//!
//! | idx | type   | sémantique                                                      |
//! |-----|--------|-----------------------------------------------------------------|
//! | 0   | Int    | `layer_id` = **CRC32 du nom** (réinterprété u32) — VÉRIFIÉ      |
//! | 1   | String | `name` (nom du layer, ex. `mainmenu01_06_base_button_guide`)    |
//! | 2   | String | `objbin_path` (chemin logique de l'objbin du layer)            |
//! | 3.. | Int    | `params` — drapeaux/groupes (draw/focus/visibilité) ; valeurs   |
//! |     |        | observées mais sémantique non confirmée → préservées telles     |
//!
//! Les deux derniers `params` valent `1` (probable `visible`/`valid`) sur tout `main_menu`,
//! mais on ne les NOMME pas faute de contre-exemple (discipline anti-faux-FAIT).

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, walk_named};
use crate::hash::HashId;

/// Un *layer* d'écran de menu (`MENU_LAYER_INFO`) : une couche d'objbin à composer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuLayerInfo {
    /// var\[0\] — identifiant du layer = **CRC32 du nom** (réinterprété u32). VÉRIFIÉ.
    pub layer_id: HashId,
    /// var\[1\] — nom du layer (= base du préfixe d'objbin, ex. `mainmenu90_01_header`).
    pub name: String,
    /// var\[2\] — chemin logique de l'objbin de ce layer.
    pub objbin_path: String,
    /// var\[3..\] — drapeaux/groupes (draw/focus/visibilité) préservés bruts (sémantique TBD).
    pub params: Vec<i64>,
}

/// Une ressource g4tx partagée par l'écran (`MENU_RES`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuResource {
    /// var\[0\] — chemin logique g4tx (`#/menu/...` ou avec `<LG>` pour la locale).
    pub logical_path: String,
    /// var\[1\] — drapeau (observé `0`).
    pub kind: i64,
}

/// Une commande de l'écran (`MENU_CMD_INFO`) : action liée à un layer (le driver dispatche
/// `CMD_FCS_BACK`/`CMD_FCS_NEXT`/`CMD_FUNCTION`… via les natives funcLua, D1.c-driver brique b/c).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuCommand {
    /// var\[0\] — `layer_id` (CRC) du layer porteur de la commande (réf. `MenuLayerInfo`).
    pub layer_id: HashId,
    /// var\[1\] — hash d'identité de la commande.
    pub command_hash: HashId,
    /// var\[2\] — nom symbolique (`"CMD_FCS_BACK"`, `"CMD_FCS_NEXT"`, `"CMD_FUNCTION"`…).
    pub name: String,
    /// var\[3..\] — hashes d'arguments (cibles de focus / handlers) + drapeau ; bruts (sémantique TBD).
    pub args: Vec<HashId>,
}

/// État de groupe d'un layer (`MENU_LAYER_GROUP_BASE`) : drapeaux par layer (joints par `layer_id`).
/// Le 1ᵉʳ drapeau (`flag0`) vaut `1` pour le layer INTERACTIF (celui qui porte les `MenuCommand`),
/// `0` pour les layers passifs (fond, en-tête) — apparie avec `MENU_CMD_INFO` pour la brique (b).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuLayerGroupBase {
    /// var\[0\] — `layer_id` (CRC) du layer concerné (réf. `MenuLayerInfo`).
    pub layer_id: HashId,
    /// var\[1..\] — drapeaux de groupe/état (sémantique partielle : `flag0`=interactif). Bruts.
    pub flags: Vec<i64>,
}

/// Un groupe de layers nommé (`MENU_LAYER_GROUP`) : l'écran lui-même, désigné par son nom
/// logique (`"main_menu"`), avec la plage de `MENU_LAYER_GROUP_BASE` qu'il couvre.
///
/// `group_id == CRC32(name)` comme pour les layers — vérifié sur **449 des 451** groupes du
/// corpus. Les 2 exceptions vivent dans le seul `organization_member_menu_setting` : un nom
/// vide, et un nom non-ASCII que le décodeur T2B rend en `U+FFFD` (cf. `from_utf8_lossy` de
/// `nie_formats::cfgbin`) — les octets d'origine étant perdus, le CRC n'y est plus
/// recalculable. Ce n'est donc pas un contre-exemple à l'invariant, mais une perte amont.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuLayerGroup {
    /// var\[0\] — identifiant du groupe = CRC32 du nom (réinterprété u32).
    pub group_id: HashId,
    /// var\[1\] — nom logique du groupe (= nom de l'écran, ex. `main_menu`).
    pub name: String,
    /// var\[2..\] — drapeaux (arité 3 ou 4 selon l'écran) préservés bruts.
    pub flags: Vec<i64>,
}

/// Une plage `{start, count}` désignant une tranche contiguë d'une autre liste du fichier
/// (nœuds `*_REF_*`).
///
/// **Invariant vérifié** : en ignorant les plages de `count == 0` (des trous, fréquents), les
/// plages restantes forment une **partition contiguë et exhaustive** de la liste cible —
/// 304/304 pour `LAYER_GROUP → LAYER_GROUP_BASE`, 207/207 pour `FOCUS_GROUP → FOCUS_BASE_INFO`,
/// 31/31 pour `FOCUS_SHIFT → FOCUS_SHIFT_BASE_INFO`. C'est ce qui rattache chaque groupe à ses
/// éléments : le i-ème groupe possède la i-ème plage non vide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuRefRange {
    /// var\[0\] — index du premier élément dans la liste cible.
    pub start: usize,
    /// var\[1\] — nombre d'éléments ; `0` = plage vide (trou).
    pub count: usize,
}

/// Un élément focusable (`MENU_FOCUS_BASE_INFO`) : l'unité de navigation d'un écran.
///
/// C'est la brique que le driver parcourt sur un appui directionnel. Sur `main_menu` il y en a
/// **9**, tous portant le même `role`.
///
/// `role` (var\[0\]) ne prend que **13 valeurs distinctes sur les 748 éléments** du corpus : ce
/// n'est donc pas une identité par bouton mais un **rôle/type** de focus, partagé entre écrans.
/// Aucun nom-source connu ne le résout à ce jour → conservé brut, non baptisé.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuFocusBaseInfo {
    /// var\[0\] — hash de rôle du focus (13 valeurs distinctes dans tout le corpus).
    pub role: HashId,
    /// var\[1\] — paramètre ; `0` sur 741 des 748 éléments (7 valeurs uniques ailleurs).
    pub param: i64,
    /// var\[2\] — second paramètre ; `0` partout dans le corpus observé.
    pub param2: i64,
}

/// Un groupe de focus (`MENU_FOCUS_GROUP`) : rattache une tranche d'éléments focusables au
/// layer qui les porte.
///
/// `layer_id` désigne le layer interactif — sur `main_menu`, `0x0BF14058`
/// (`mainmenu90_02_2_header_tab_icon`), qui est aussi le seul layer marqué interactif par
/// [`MenuLayerGroupBase`] **et** le porteur des 4 [`MenuCommand`] : triple concordance.
///
/// La référence est **globale, pas locale au fichier** : 665 des 669 groupes du corpus pointent
/// un layer déclaré dans le même écran, les 4 restants pointent un layer déclaré dans un
/// *autre* écran (p.ex. `soccer_team_dock` → `mainmenu90_31_doc_item`). Ne pas traiter une
/// résolution locale infructueuse comme une erreur de parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuFocusGroup {
    /// var\[0\] — `layer_id` (CRC32) du layer porteur du focus.
    pub layer_id: HashId,
    /// var\[1..\] — drapeaux (arité 2 ou 4 selon l'écran) préservés bruts.
    pub flags: Vec<i64>,
}

/// Une règle de déplacement de focus (`MENU_FOCUS_SHIFT_BASE_INFO`), et le groupe
/// (`MENU_FOCUS_SHIFT`) qui en agrège une tranche.
///
/// Listes **absentes de `main_menu`** : présentes sur 31 des 304 écrans seulement. Les
/// variables sont préservées brutes — leur sémantique (direction, cible) n'est pas établie et
/// n'est donc pas nommée.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuFocusShiftBaseInfo {
    /// var\[0..\] — champs bruts (arité 4 ou 6 selon l'écran), sémantique TBD.
    pub values: Vec<i64>,
}

/// Définition complète d'un écran de menu (`*_menu_setting.cfg.bin`).
///
/// Les **9 listes** du format sont modélisées. Fréquence dans le corpus des 304 écrans :
/// `layers`, `resources`, `commands`, `layer_groups`, `groups` → 304 ; `focus_groups` → 207 ;
/// `focus_base_infos` → 203 ; `focus_shifts` / `focus_shift_base_infos` → 31.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MenuSetting {
    /// Layers composant l'écran, dans l'ordre du fichier (= ordre de composition).
    pub layers: Vec<MenuLayerInfo>,
    /// Ressources g4tx partagées référencées par l'écran.
    pub resources: Vec<MenuResource>,
    /// Commandes liées aux layers (back/next/function), dans l'ordre du fichier.
    pub commands: Vec<MenuCommand>,
    /// États de groupe par layer (drapeaux d'interactivité/visibilité), dans l'ordre du fichier.
    pub layer_groups: Vec<MenuLayerGroupBase>,
    /// Groupes de layers nommés (`MENU_LAYER_GROUP`) — en pratique l'écran lui-même.
    pub groups: Vec<MenuLayerGroup>,
    /// Plages `MENU_LAYER_GROUP → MENU_LAYER_GROUP_BASE`, dans l'ordre du fichier.
    pub group_refs: Vec<MenuRefRange>,
    /// Éléments focusables de l'écran, dans l'ordre du fichier.
    pub focus_base_infos: Vec<MenuFocusBaseInfo>,
    /// Groupes de focus (layer porteur + drapeaux).
    pub focus_groups: Vec<MenuFocusGroup>,
    /// Plages `MENU_FOCUS_GROUP → MENU_FOCUS_BASE_INFO`, dans l'ordre du fichier.
    pub focus_group_refs: Vec<MenuRefRange>,
    /// Règles de déplacement de focus (31 écrans sur 304).
    pub focus_shift_base_infos: Vec<MenuFocusShiftBaseInfo>,
    /// Groupes de déplacement de focus (`MENU_FOCUS_SHIFT`) : une variable unique chacun,
    /// préservée brute (99 occurrences dans le corpus, toutes d'arité 1).
    pub focus_shifts: Vec<i64>,
    /// Plages `MENU_FOCUS_SHIFT → MENU_FOCUS_SHIFT_BASE_INFO`, dans l'ordre du fichier.
    pub focus_shift_refs: Vec<MenuRefRange>,
}

/// Vrai si `name` est un marqueur de conteneur de liste (`*_LIST_BEG`/`*_LIST_END`), à
/// ignorer lors de la collecte des enfants (le `walk_named` par préfixe les voit aussi).
fn is_list_marker(name: &str) -> bool {
    name.contains("LIST_BEG") || name.contains("LIST_END")
}

/// Parse un `*_menu_setting.cfg.bin.json` (forme iecode T2B `entries`) en [`MenuSetting`].
///
/// Règles :
/// - un `MENU_LAYER_INFO` n'est retenu que s'il porte au moins `var[0..2]` (id, nom, objbin) ;
/// - les conteneurs `*_LIST_BEG`/`*_LIST_END` (vus par le parcours préfixe) sont ignorés ;
/// - l'ordre du fichier est conservé (= ordre de composition de l'écran).
#[must_use]
pub fn parse(root: &Value) -> MenuSetting {
    let mut layers = Vec::new();
    walk_named(root, "MENU_LAYER_INFO", |node: Node| {
        if is_list_marker(node.name()) || node.var_count() < 3 {
            return;
        }
        layers.push(MenuLayerInfo {
            layer_id: node.hash(0),
            name: String::from(node.string(1)),
            objbin_path: String::from(node.string(2)),
            params: (3..node.var_count()).map(|i| node.int(i)).collect(),
        });
    });

    let mut resources = Vec::new();
    walk_named(root, "MENU_RES", |node: Node| {
        if is_list_marker(node.name()) || node.var_count() == 0 {
            return;
        }
        resources.push(MenuResource {
            logical_path: String::from(node.string(0)),
            kind: node.int(1),
        });
    });

    let mut commands = Vec::new();
    walk_named(root, "MENU_CMD_INFO", |node: Node| {
        if is_list_marker(node.name()) || node.var_count() < 3 {
            return;
        }
        commands.push(MenuCommand {
            layer_id: node.hash(0),
            command_hash: node.hash(1),
            name: String::from(node.string(2)),
            args: (3..node.var_count()).map(|i| node.hash(i)).collect(),
        });
    });

    // `MENU_LAYER_GROUP_BASE` (préfixe distinct de `MENU_LAYER_GROUP`/`MENU_LAYER_INFO`) — drapeaux
    // par layer. On exclut aussi le noeud `_REF_LAYER_GROUP_BASE` (référence start/count, pas un état).
    let mut layer_groups = Vec::new();
    walk_named(root, "MENU_LAYER_GROUP_BASE", |node: Node| {
        if is_list_marker(node.name()) || node.name().contains("_REF_") || node.var_count() < 2 {
            return;
        }
        layer_groups.push(MenuLayerGroupBase {
            layer_id: node.hash(0),
            flags: (1..node.var_count()).map(|i| node.int(i)).collect(),
        });
    });

    // `MENU_LAYER_GROUP` — le groupe nommé (l'écran lui-même). Le parcours par préfixe voit
    // aussi `MENU_LAYER_GROUP_BASE*` (les états par layer) et `MENU_LAYER_GROUP_REF_*` (la
    // plage) : tous deux commencent par le même préfixe, on les écarte explicitement.
    let mut groups = Vec::new();
    walk_named(root, "MENU_LAYER_GROUP", |node: Node| {
        let n = node.name();
        if is_list_marker(n)
            || n.starts_with("MENU_LAYER_GROUP_BASE")
            || n.contains("_REF_")
            || node.var_count() < 2
        {
            return;
        }
        groups.push(MenuLayerGroup {
            group_id: node.hash(0),
            name: String::from(node.string(1)),
            flags: (2..node.var_count()).map(|i| node.int(i)).collect(),
        });
    });
    let group_refs = parse_refs(root, "MENU_LAYER_GROUP_REF");

    let mut focus_base_infos = Vec::new();
    walk_named(root, "MENU_FOCUS_BASE_INFO", |node: Node| {
        if is_list_marker(node.name()) || node.var_count() < 3 {
            return;
        }
        focus_base_infos.push(MenuFocusBaseInfo {
            role: node.hash(0),
            param: node.int(1),
            param2: node.int(2),
        });
    });

    let mut focus_groups = Vec::new();
    walk_named(root, "MENU_FOCUS_GROUP", |node: Node| {
        let n = node.name();
        if is_list_marker(n) || n.contains("_REF_") || node.var_count() == 0 {
            return;
        }
        focus_groups.push(MenuFocusGroup {
            layer_id: node.hash(0),
            flags: (1..node.var_count()).map(|i| node.int(i)).collect(),
        });
    });
    let focus_group_refs = parse_refs(root, "MENU_FOCUS_GROUP_REF");

    let mut focus_shift_base_infos = Vec::new();
    walk_named(root, "MENU_FOCUS_SHIFT_BASE_INFO", |node: Node| {
        if is_list_marker(node.name()) || node.var_count() == 0 {
            return;
        }
        focus_shift_base_infos.push(MenuFocusShiftBaseInfo {
            values: (0..node.var_count()).map(|i| node.int(i)).collect(),
        });
    });

    // `MENU_FOCUS_SHIFT` seul : écarter `..._BASE_INFO` et `..._REF_...`, qui partagent le préfixe.
    let mut focus_shifts = Vec::new();
    walk_named(root, "MENU_FOCUS_SHIFT", |node: Node| {
        let n = node.name();
        if is_list_marker(n)
            || n.starts_with("MENU_FOCUS_SHIFT_BASE_INFO")
            || n.contains("_REF_")
            || node.var_count() == 0
        {
            return;
        }
        focus_shifts.push(node.int(0));
    });
    let focus_shift_refs = parse_refs(root, "MENU_FOCUS_SHIFT_REF");

    MenuSetting {
        layers,
        resources,
        commands,
        layer_groups,
        groups,
        group_refs,
        focus_base_infos,
        focus_groups,
        focus_group_refs,
        focus_shift_base_infos,
        focus_shifts,
        focus_shift_refs,
    }
}

/// Collecte les plages `{start, count}` des nœuds `*_REF_*` portant `prefix`.
///
/// Les valeurs négatives (jamais observées dans le corpus) retombent sur `0` plutôt que de
/// paniquer : un fichier hostile ne doit pas faire tomber un parseur de données.
fn parse_refs(root: &Value, prefix: &str) -> Vec<MenuRefRange> {
    let mut out = Vec::new();
    walk_named(root, prefix, |node: Node| {
        if is_list_marker(node.name()) || node.var_count() < 2 {
            return;
        }
        out.push(MenuRefRange {
            start: usize::try_from(node.int(0)).unwrap_or(0),
            count: usize::try_from(node.int(1)).unwrap_or(0),
        });
    });
    out
}

impl MenuSetting {
    /// Cherche un layer par nom exact.
    #[must_use]
    pub fn layer_by_name(&self, name: &str) -> Option<&MenuLayerInfo> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Cherche un layer par identifiant (CRC32).
    #[must_use]
    pub fn layer_by_id(&self, id: HashId) -> Option<&MenuLayerInfo> {
        self.layers.iter().find(|l| l.layer_id == id)
    }

    /// `layer_id` du layer INTERACTIF (drapeau de groupe `flag0 == 1` ; en pratique unique sur
    /// `main_menu`, c'est celui qui porte les `MenuCommand`). `None` si aucun.
    #[must_use]
    pub fn interactive_layer_id(&self) -> Option<HashId> {
        self.layer_groups
            .iter()
            .find(|g| g.flags.first() == Some(&1))
            .map(|g| g.layer_id)
    }

    /// Vrai si **chaque** layer satisfait l'invariant `layer_id == CRC32(name)` — la propriété
    /// byte-exact qui prouve l'interprétation positionnelle des champs (`var[0]` = CRC32 de
    /// `var[1]`). Vérifié à 100 % sur les 304 écrans réels (cf. `tests/menu_setting_golden.rs`).
    /// Vide → `true` (vacuité).
    #[must_use]
    pub fn layer_hashes_consistent(&self) -> bool {
        self.layers
            .iter()
            .all(|l| l.layer_id == HashId(crate::unlock_condition::crc32_str(&l.name)))
    }

    /// Vrai si chaque groupe nommé satisfait `group_id == CRC32(name)`.
    ///
    /// Les noms vides ou non-ASCII sont **exclus du contrôle** : le décodeur T2B amont rend les
    /// octets non-UTF-8 en `U+FFFD` (perte irréversible), le CRC n'y est donc plus recalculable.
    /// Sans cette exclusion, `organization_member_menu_setting` — le seul cas du corpus —
    /// échouerait pour une raison étrangère au parseur.
    #[must_use]
    pub fn group_hashes_consistent(&self) -> bool {
        self.groups
            .iter()
            .filter(|g| !g.name.is_empty() && g.name.is_ascii())
            .all(|g| g.group_id == HashId(crate::unlock_condition::crc32_str(&g.name)))
    }

    /// Vrai si les plages `refs`, une fois les vides écartées, forment une partition contiguë
    /// et exhaustive de `target_len` éléments.
    #[must_use]
    fn refs_partition(refs: &[MenuRefRange], target_len: usize) -> bool {
        let mut cursor = 0usize;
        for r in refs.iter().filter(|r| r.count > 0) {
            if r.start != cursor {
                return false;
            }
            cursor += r.count;
        }
        cursor == target_len
    }

    /// Vrai si les **trois** familles de plages partitionnent correctement leur liste cible.
    ///
    /// Invariant vérifié sur tout le corpus : 304/304 (`layer_group`), 207/207 (`focus_group`),
    /// 31/31 (`focus_shift`). Une famille absente est vacuement vraie.
    #[must_use]
    pub fn refs_consistent(&self) -> bool {
        Self::refs_partition(&self.group_refs, self.layer_groups.len())
            && Self::refs_partition(&self.focus_group_refs, self.focus_base_infos.len())
            && Self::refs_partition(&self.focus_shift_refs, self.focus_shift_base_infos.len())
    }

    /// Vrai si chaque famille compte **autant de plages que de groupes** (appariement positionnel
    /// 1:1, plages vides comprises). Vérifié 304/304, 207/207, 31/31 sur le corpus — c'est ce qui
    /// autorise [`Self::focus_elements`] et [`Self::group_layer_states`] à indexer par rang.
    #[must_use]
    pub fn refs_pair_groups(&self) -> bool {
        self.group_refs.len() == self.groups.len()
            && self.focus_group_refs.len() == self.focus_groups.len()
            && self.focus_shift_refs.len() == self.focus_shifts.len()
    }

    /// Éléments focusables du `index`-ième groupe de focus (tranche désignée par sa plage).
    ///
    /// `None` si l'index n'existe pas ou si la plage sort de la liste ; une plage vide donne une
    /// tranche vide (cas normal, pas une erreur).
    #[must_use]
    pub fn focus_elements(&self, index: usize) -> Option<&[MenuFocusBaseInfo]> {
        let r = self.focus_group_refs.get(index)?;
        self.focus_base_infos
            .get(r.start..r.start.checked_add(r.count)?)
    }

    /// États de layer du `index`-ième groupe nommé (tranche de `layer_groups`).
    #[must_use]
    pub fn group_layer_states(&self, index: usize) -> Option<&[MenuLayerGroupBase]> {
        let r = self.group_refs.get(index)?;
        self.layer_groups
            .get(r.start..r.start.checked_add(r.count)?)
    }

    /// Nombre total d'éléments focusables de l'écran (= unités de navigation).
    #[must_use]
    pub fn focus_count(&self) -> usize {
        self.focus_base_infos.len()
    }
}
