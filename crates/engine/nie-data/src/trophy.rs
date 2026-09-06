//! `TrophyConfig` — port Rust de `trophy_config_*.cfg.bin.json` (Level-5 IEVR).
//!
//! Famille des « succès / trophées » (appelés *activity* dans les chaînes internes :
//! les textures portent le préfixe `activity_photo` / `activity_note`).
//!
//! ## Vérité terrain
//!
//! - Dump réel : `data/common/gamedata/trophy/trophy_config_7.00.12.00.cfg.bin.json`
//! - Format `entries` (nœuds positionnels), 5 nœuds racine :
//!   - `TROPHY_TARGET_MAP_INFO_LIST_BEG_0` — liste vide dans le dump réel (`var[0] = 0`,
//!     aucun enfant). Structure des entrées inconnue (jamais peuplée) : non modélisée.
//!   - `TEXTURE_BASE_PATH_0` — `var[0]` = chemin de base des textures
//!     (`"#/menu/220_img/activity_photo/<LG>/"`).
//!   - `TROPHY_REWARD_LIST_BEG_0` — 384 enfants `TROPHY_REWARD_N` (récompenses).
//!   - `TROPHY_TIER_LIST_BEG_0` — 436 paires `TROPHY_TIER_N` + `TROPHY_TIER_REF_REWARD_N`
//!     (paliers ; chaque palier référence une plage de récompenses).
//!   - `TROPHY_INFO_LIST_BEG_0` — 230 paires `TROPHY_INFO_N` + `TROPHY_INFO_REF_TIER_N`
//!     (trophées ; chaque trophée référence une plage de paliers).
//!
//! ## Modèle de jointure (offset/count)
//!
//! Comme `info_bookmark_config`, les nœuds `_REF_` portent `[offset, count]` :
//! - un `TROPHY_INFO_N` couvre `m_TierList[offset .. offset+count]` ;
//! - un `TROPHY_TIER_N` couvre `m_RewardList[offset .. offset+count]`.
//!
//! Les plages des paliers tuilent exactement la liste des récompenses (somme des `count` =
//! 384) ; celles des trophées tuilent exactement la liste des paliers (somme des `count` =
//! 436). Utiliser [`TrophyConfig::tiers_of`] et [`TrophyConfig::rewards_of`] pour résoudre.
//!
//! ## Champs positionnels (zone d'ombre honnête)
//!
//! Le parser TS de référence (`packages/inagle/src/parsers/trophy-config.ts`) n'extrait que
//! quelques champs de `TROPHY_INFO` (id, code, deux hash de texte) par heuristique de type ;
//! il ne nomme PAS les 18 variables. Ce port capture **toutes** les variables positionnelles
//! et documente chaque champ d'après l'observation du dump réel. Certaines variables restent
//! de sémantique incertaine (marquées « sémantique incertaine » ci-dessous) : leurs valeurs
//! sont néanmoins portées byte-exact.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{Node, owned};
use crate::hash::HashId;

// ─── TrophyReward ───────────────────────────────────────────────────────────────

/// Récompense — nœud `TROPHY_REWARD_N` (2 variables).
///
/// Vérité terrain : `TROPHY_REWARD_0` = var\[0\]=310910628 → `0x12881EA4`, var\[1\]=10.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrophyReward {
    /// var\[0\] — identifiant hash de la récompense (item octroyé).
    pub reward_id: HashId,
    /// var\[1\] — quantité octroyée (1, 2, 3, 5, 10, 15, 20 observés ; 10 pour `TROPHY_REWARD_0`).
    pub count: i64,
}

impl TrophyReward {
    /// Parse un nœud `TROPHY_REWARD_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            reward_id: node.hash(0),
            count: node.int(1),
        }
    }
}

// ─── TrophyTier ─────────────────────────────────────────────────────────────────

/// Palier — nœud `TROPHY_TIER_N` (8 variables) + son `TROPHY_TIER_REF_REWARD_N`.
///
/// Vérité terrain : `TROPHY_TIER_0` = `[1, 1, -412693377, 0, -1, -1, -1, -1]`,
/// ref reward = `[0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrophyTier {
    /// var\[0\] — catégorie du palier (0..3 observés).
    ///
    /// Les paliers de catégorie 0 (52 entrées) portent un `target` de type hash et des
    /// `aux` séquentiels ; les catégories 1..3 portent un `target` numérique (seuil) et
    /// des `aux` à -1.
    pub category: i64,
    /// var\[1\] — numéro de palier (séquentiel 1-basé sur toute la liste).
    pub tier_no: i64,
    /// var\[2\] — cible du palier : **polymorphe**. Seuil numérique (ex. 10, 50) pour les
    /// trophées de comptage ; identifiant hash cible (ex. `0xE766CC7F`) pour la catégorie 0.
    /// Voir [`TrophyTier::target_hash`].
    pub target: i64,
    /// var\[3\] — réservé (= 0 sur toutes les entrées du dump réel).
    pub reserved: i64,
    /// var\[4..8\] — paramètres auxiliaires, sémantique incertaine. -1 = non défini
    /// (cas des catégories 1..3) ; index séquentiels pour la catégorie 0.
    pub aux: Vec<i64>,
    /// `TROPHY_TIER_REF_REWARD_N` var\[0\] — index de début dans la liste des récompenses.
    pub reward_offset: i64,
    /// `TROPHY_TIER_REF_REWARD_N` var\[1\] — nombre de récompenses de ce palier.
    pub reward_count: i64,
}

impl TrophyTier {
    /// Parse le nœud principal `TROPHY_TIER_N` (le ref reward est attaché ensuite).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            category: node.int(0),
            tier_no: node.int(1),
            target: node.int(2),
            reserved: node.int(3),
            aux: (4..8).map(|i| node.int(i)).collect(),
            reward_offset: 0,
            reward_count: 0,
        }
    }

    /// Réinterprète `target` comme un `HashId` (utile pour la catégorie 0).
    #[must_use]
    pub fn target_hash(&self) -> HashId {
        HashId::from_i64(self.target)
    }
}

// ─── TrophyInfo ─────────────────────────────────────────────────────────────────

/// Trophée — nœud `TROPHY_INFO_N` (18 variables) + son `TROPHY_INFO_REF_TIER_N`.
///
/// Vérité terrain : `TROPHY_INFO_0` : `category=1`, `sort_no=1`,
/// `trophy_id=0x7D48804A`, `code="activity_story_miniquest_001"`,
/// `name_text_id=0x8840F978`, `debug_name="鍵をなくしたおじいさん"`,
/// `desc_text_id=0xC275B972`, ref tier = `[0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrophyInfo {
    /// var\[0\] — catégorie de trophée (1, 3, 4 observés).
    pub category: i64,
    /// var\[1\] — numéro d'ordre (séquentiel 1-basé sur toute la liste).
    pub sort_no: i64,
    /// var\[2\] — identifiant hash du trophée (`trophyId`).
    pub trophy_id: HashId,
    /// var\[3\] — code interne (`"activity_story_miniquest_001"`).
    pub code: String,
    /// var\[4\] — hash du nom affiché (jointure `trophy_text`).
    pub name_text_id: HashId,
    /// var\[5\] — nom de développement (japonais, parfois partagé entre trophées).
    pub debug_name: String,
    /// var\[6\] — hash de la description (jointure `trophy_text` ; 0 si absent).
    pub desc_text_id: HashId,
    /// var\[7\] — réservé (= 0 sur toutes les entrées du dump réel).
    pub reserved: i64,
    /// var\[8\] — condition d'ouverture : base64 opaque, ou `"0"` (sentinelle Int) si absent.
    pub open_cond: String,
    /// var\[9\] — hash auxiliaire, sémantique incertaine (0 si absent).
    pub cond_hash_a: HashId,
    /// var\[10\] — hash auxiliaire, sémantique incertaine (0 si absent).
    pub cond_hash_b: HashId,
    /// var\[11\] — hash auxiliaire, sémantique incertaine (0 si absent).
    pub cond_hash_c: HashId,
    /// var\[12\] — paramètre numérique, sémantique incertaine (1, 2, 318…).
    pub param_a: i64,
    /// var\[13\] — paramètre numérique, sémantique incertaine (2, 3, 4 observés).
    pub param_b: i64,
    /// var\[14\] — fichier texture « photo » (`activity_photo_*.g4tx`), `"0"` si absent.
    pub photo_tex_file_name: String,
    /// var\[15\] — fichier texture « note » (`activity_note_*.g4tx`), `"0"` si absent.
    pub note_tex_file_name: String,
    /// var\[16\] — drapeau (0/1), sémantique incertaine.
    pub flag_a: i64,
    /// var\[17\] — drapeau (0/1), sémantique incertaine.
    pub flag_b: i64,
    /// `TROPHY_INFO_REF_TIER_N` var\[0\] — index de début dans la liste des paliers.
    pub tier_offset: i64,
    /// `TROPHY_INFO_REF_TIER_N` var\[1\] — nombre de paliers de ce trophée.
    pub tier_count: i64,
}

impl TrophyInfo {
    /// Parse le nœud principal `TROPHY_INFO_N` (le ref tier est attaché ensuite).
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            category: node.int(0),
            sort_no: node.int(1),
            trophy_id: node.hash(2),
            code: owned(node.string(3)),
            name_text_id: node.hash(4),
            debug_name: owned(node.string(5)),
            desc_text_id: node.hash(6),
            reserved: node.int(7),
            open_cond: owned(node.string(8)),
            cond_hash_a: node.hash(9),
            cond_hash_b: node.hash(10),
            cond_hash_c: node.hash(11),
            param_a: node.int(12),
            param_b: node.int(13),
            photo_tex_file_name: owned(node.string(14)),
            note_tex_file_name: owned(node.string(15)),
            flag_a: node.int(16),
            flag_b: node.int(17),
            tier_offset: 0,
            tier_count: 0,
        }
    }
}

// ─── TrophyConfig ───────────────────────────────────────────────────────────────

/// Contenu complet d'un `trophy_config_*.cfg.bin.json`.
///
/// Utiliser [`parse_trophy_config`] pour construire depuis un `serde_json::Value`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrophyConfig {
    /// `TEXTURE_BASE_PATH_0` — chemin de base des textures (avec marqueur `<LG>`).
    pub texture_base_path: String,
    /// `TROPHY_REWARD_LIST_BEG_0` — 384 récompenses.
    pub rewards: Vec<TrophyReward>,
    /// `TROPHY_TIER_LIST_BEG_0` — 436 paliers.
    pub tiers: Vec<TrophyTier>,
    /// `TROPHY_INFO_LIST_BEG_0` — 230 trophées.
    pub infos: Vec<TrophyInfo>,
}

impl TrophyConfig {
    /// Renvoie la tranche de [`TrophyTier`] appartenant à un trophée.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn tiers_of(&self, info: &TrophyInfo) -> &[TrophyTier] {
        let start = info.tier_offset as usize;
        let end = start.saturating_add(info.tier_count as usize);
        self.tiers.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de [`TrophyReward`] appartenant à un palier.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn rewards_of(&self, tier: &TrophyTier) -> &[TrophyReward] {
        let start = tier.reward_offset as usize;
        let end = start.saturating_add(tier.reward_count as usize);
        self.rewards.get(start..end).unwrap_or(&[])
    }

    /// Recherche un trophée par son `code` interne. `None` si absent.
    #[must_use]
    pub fn find_info_by_code(&self, code: &str) -> Option<&TrophyInfo> {
        self.infos.iter().find(|i| i.code == code)
    }

    /// Recherche un trophée par son `trophy_id`. `None` si absent.
    #[must_use]
    pub fn find_info_by_trophy_id(&self, id: HashId) -> Option<&TrophyInfo> {
        self.infos.iter().find(|i| i.trophy_id == id)
    }
}

// ─── Parseur principal ──────────────────────────────────────────────────────────

/// Trouve le premier nœud racine `entries[]` dont le nom commence par `prefix`.
fn find_entry<'a>(root: &'a Value, prefix: &str) -> Option<Node<'a>> {
    let entries = root.get("entries")?.as_array()?;
    entries
        .iter()
        .map(Node::new)
        .find(|n| n.name().starts_with(prefix))
}

/// Parse un `trophy_config_*.cfg.bin.json` complet.
///
/// Renvoie un [`TrophyConfig`] avec les 3 listes peuplées et le chemin de base texture.
/// Les nœuds `_REF_` sont fusionnés dans le nœud principal qui les précède (offset/count).
///
/// Vérité terrain : `trophy_config_7.00.12.00.cfg.bin.json` →
/// 384 récompenses, 436 paliers, 230 trophées.
#[must_use]
pub fn parse_trophy_config(root: &Value) -> TrophyConfig {
    let texture_base_path = find_entry(root, "TEXTURE_BASE_PATH")
        .map(|n| owned(n.string(0)))
        .unwrap_or_default();

    let rewards = find_entry(root, "TROPHY_REWARD_LIST_BEG")
        .map(|list| {
            list.children()
                .into_iter()
                .filter(|c| c.name().starts_with("TROPHY_REWARD_"))
                .map(TrophyReward::from_node)
                .collect()
        })
        .unwrap_or_default();

    let tiers = parse_tiers(root);
    let infos = parse_infos(root);

    TrophyConfig {
        texture_base_path,
        rewards,
        tiers,
        infos,
    }
}

/// Résout le **nom localisé** d'un trophée via une liste `trophy_text` (issue de
/// [`crate::text::parse_text_file`]).
///
/// Port de la jointure de `trophy-config.ts` (`loadTextFile("trophy").get(nameHash)`) : clé
/// `name_text_id` (var4). `None` si absent.
#[must_use]
pub fn resolve_name<'a>(info: &TrophyInfo, trophy_text: &'a [(HashId, String)]) -> Option<&'a str> {
    crate::text::find_text(trophy_text, info.name_text_id)
}

/// Résout la **description localisée** d'un trophée via la **même** table `trophy_text`
/// (clé `desc_text_id` = var6 ; port de `loadTextFile("trophy").get(descHash)`). `None` si absent.
#[must_use]
pub fn resolve_description<'a>(
    info: &TrophyInfo,
    trophy_text: &'a [(HashId, String)],
) -> Option<&'a str> {
    crate::text::find_text(trophy_text, info.desc_text_id)
}

/// Décode la **condition de déblocage** d'un trophée (`open_cond`, var8, base64 opaque) via
/// [`crate::unlock_condition::decode_unlock_condition`].
///
/// `open_cond` vaut `"0"` (sentinelle Int) quand absent → condition triviale `Always`.
#[must_use]
pub fn decode_unlock(info: &TrophyInfo) -> crate::unlock_condition::UnlockCondition {
    crate::unlock_condition::decode_unlock_condition(&info.open_cond)
}
fn parse_tiers(root: &Value) -> Vec<TrophyTier> {
    let mut tiers = Vec::new();
    let Some(list) = find_entry(root, "TROPHY_TIER_LIST_BEG") else {
        return tiers;
    };
    for child in list.children() {
        let name = child.name();
        if name.contains("_REF_") {
            if let Some(last) = tiers.last_mut() {
                last.reward_offset = child.int(0);
                last.reward_count = child.int(1);
            }
        } else if name.starts_with("TROPHY_TIER_") {
            tiers.push(TrophyTier::from_node(child));
        }
    }
    tiers
}

/// Parse la liste des trophées en fusionnant chaque `TROPHY_INFO_REF_TIER_N` dans le
/// `TROPHY_INFO_N` qui le précède.
fn parse_infos(root: &Value) -> Vec<TrophyInfo> {
    let mut infos = Vec::new();
    let Some(list) = find_entry(root, "TROPHY_INFO_LIST_BEG") else {
        return infos;
    };
    for child in list.children() {
        let name = child.name();
        if name.contains("_REF_") {
            if let Some(last) = infos.last_mut() {
                last.tier_offset = child.int(0);
                last.tier_count = child.int(1);
            }
        } else if name.starts_with("TROPHY_INFO_") {
            infos.push(TrophyInfo::from_node(child));
        }
    }
    infos
}
