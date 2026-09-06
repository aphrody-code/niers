//! Pipeline unifiée passive skill IEVR : joueur, équipe et lots.
//!
//! ## Sources
//!
//! | Source | Chemin | Contenu |
//! |--------|--------|---------|
//! | `passive_skill_config_5.00.07.00` | `common/gamedata/skill/` | 1716 instances joueur (tous string_ids + raretés) |
//! | `passive_skill_effect_config` | `common/gamedata/skill/` | ScPassiveSkillEffectInfo (range/effect indices) |
//! | `soccer_team_passive_config_0.00.00` | `common/gamedata/soccer/` | 21 passives d'équipe |
//! | `team_passive_lot_table_config_0.00.00` | `common/gamedata/character/` | 653 lots avec poids et conditions |
//! | `skill_text.cfg.bin.json` | `common/text/{fr,en,ja}/` | Textes NOUN_INFO (effectId→texte FR/EN/JA) |
//! | `soccer_team_passive_text.cfg.bin.json` | `common/text/{fr,en,ja}/` | Textes team passives (JA uniquement dans toutes locales) |
//!
//! ## Structure cfg.bin vérifiée sur dumps réels
//!
//! `PASSIVE_SKILL_INFO_N` = `[passiveId, effectId, nameId, descId, rarity, element, stringId(String)]`
//! - `vars[0]` = `passiveId` (signé→u32, hash unique de l'instance)
//! - `vars[1]` = `effectId` (signé→u32 = hash NOUN_INFO dans skill_text)
//! - `vars[4]` = `rarity` (6 = Legendary, 0 = Common)
//! - `vars[5]` = `element` (0=neutre, 1=vent, 2=forêt, 3=feu, 4=montagne)
//! - `vars[6]` = `stringId` (String, ex: `"ps10001"`, `"mps20014"`)
//!
//! `PASSIVE_SKILL_EFFECT_N` = `[effectTypeId, param1(Float), INVALID×7]`
//! - `vars[0]` = `effectTypeId` (type d'effet moteur, ex `0x8A52A068` = ShotAt boost)
//! - `vars[1]` = param principal (Float, ex `0.5` = +0.5%)
//! - Sentinelle `0xC4DC849A` = `-992181094` = « INVALID » (paramètre non utilisé)
//!
//! `PASSIVE_SKILL_INFO_REF_EFFECT_N` = `[effectIndex, count]`
//! → lie l'INFO_N à EFFECT_effectIndex…effectIndex+count-1
//!
//! `PASSIVE_SKILL_INFO_REF_BUFF_ICON_N` = `[iconIndex, count]`
//! → lie l'INFO_N à BUFF_ICON_iconIndex
//!
//! `SOCCER_TEAM_PASSIVE_DATA` = `{ teamPassiveId, effectId, effectValueMax, effectValueMin, teamPassiveTextId }`
//!
//! `TEAM_PASSIVE_LOT_DATA` = `{ id, lotWeight, condition, rarityEnableFlag }`
//!
//! ## Lacunes inagle comblées
//!
//! - inagle_passives = 128 entrées (dédoublonnées par effectId) — ici : 1716 instances avec stringId + rareté
//! - `effect_value` et `stat_boost` vides dans inagle → ici : params réels par instance (0.5%, 0.6%…)
//! - `[CPASSIVE01]+<VALUE>` non résolu dans inagle → ici : texte avec valeur injectée
//! - `element` par passive (0-4) non exposé → ici : champ dédié
//! - `buff_icon_type` absent d'inagle → ici : parsé
//! - 21 team passives parsés avec poids de lot + conditions (absent d'inagle)
//! - `string_id` (clé cross-ref wiki/jeu) absent d'inagle → ici : champ dédié
//!
//! ## Opacités restantes
//!
//! - `stat_hash` dans soccer/passive_skill_effect_config GRAND_TOTAL_INFO n'est pas dans
//!   le dictionnaire CRC32 connu — le mapping stat_hash→nom de stat est probablement
//!   dans un enum C# IECODE.Core non encore RE.
//! - Les coordinateurs/managers n'ont PAS de cfg.bin dédié — leurs passives viennent
//!   du wiki externe (inagle_coordinators), non parsables depuis gamedata seul.
//! - `maxValue` dans GRAND_TOTAL_INFO = indice dans une table de stacking, pas un pourcentage.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, field_str, list_values};
use crate::hash::HashId;

// ============================================================================
// Constante
// ============================================================================

/// Sentinelle « paramètre non utilisé » dans les PASSIVE_SKILL_EFFECT (-992181094 signé = 0xC4DC849A).
/// Source : crc32-dictionary.json `"0xc4dc849a": "INVALID"`.
const INVALID_PARAM: i64 = -992181094_i64;

// ============================================================================
// Types publics
// ============================================================================

/// Texte localisé (fr/en/ja). Pour les passives joueur, les 3 locales sont disponibles.
/// Pour les team passives, seul `ja` est populé (données non localisées dans le jeu).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LocalizedText {
    /// Texte français (ou `None` si absent).
    pub fr: Option<String>,
    /// Texte anglais (ou `None` si absent).
    pub en: Option<String>,
    /// Texte japonais (ou `None` si absent).
    pub ja: Option<String>,
}

impl LocalizedText {
    /// Retourne le premier texte non vide (fr > en > ja).
    #[must_use]
    pub fn best(&self) -> Option<&str> {
        self.fr
            .as_deref()
            .or(self.en.as_deref())
            .or(self.ja.as_deref())
    }
}

/// Instance d'un passif joueur (une entrée `PASSIVE_SKILL_INFO_N`).
///
/// Une seule « famille » de passif (ex `ps10001`) génère plusieurs instances (ps10001_00..04)
/// correspondant aux différentes raretés. Chaque instance a des `effect_params` distincts.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerPassive {
    /// Hash unique de l'instance (vars[0] signé→u32).
    pub passive_id: HashId,
    /// Hash du texte NOUN_INFO (vars[1] = clé dans skill_text).
    pub effect_id: HashId,
    /// String ID lisible (ex `"ps10001"`, `"mps20014_04"`). Clé wiki cross-ref.
    pub string_id: String,
    /// Niveau de rareté (vars[4] : 0=Common, 6=Legendary en pratique).
    pub rarity: u8,
    /// Élément (vars[5] : 0=neutre, 1=vent/風, 2=forêt/林, 3=feu/火, 4=montagne/山).
    pub element: u8,
    /// Type d'icône de buff (PASSIVE_SKILL_BUFF_ICON index, `None` si non lié).
    pub buff_icon_type: Option<u8>,
    /// Paramètres d'effet de l'instance (après décodage sentinelles INVALID).
    /// Le param[0] est la valeur principale (ex `0.5` = +0.5%).
    pub effect_params: Vec<f64>,
    /// Texte localisé brut (avec placeholders `[CPASSIVE01]+<VALUE>%[C]` intacts).
    pub text_raw: LocalizedText,
    /// Texte localisé résolu (`[CPASSIVE01]` et `<VALUE>` remplacés par la valeur réelle).
    pub text_resolved: LocalizedText,
}

impl PlayerPassive {
    /// Valeur principale de l'effet (param[0]), en pourcentage lisible.
    /// Ex: `0.5` → `0.5` (interprétable comme +0.5% ou +50% selon le contexte).
    #[must_use]
    pub fn main_value(&self) -> Option<f64> {
        self.effect_params.first().copied()
    }
}

/// Passif d'équipe (`SOCCER_TEAM_PASSIVE_DATA`).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamPassive {
    /// Hash du passif d'équipe.
    pub team_passive_id: HashId,
    /// Hash de l'effet moteur (un seul effectId `0xBEBB2EE8` pour tous les team passives).
    pub effect_id: HashId,
    /// Valeur minimale de l'effet (en % ou points selon l'effet).
    pub effect_value_min: i64,
    /// Valeur maximale de l'effet.
    pub effect_value_max: i64,
    /// Hash de résolution du texte (soccer_team_passive_text).
    pub text_id: HashId,
    /// Texte localisé (JA uniquement dans toutes les locales du jeu).
    pub text: LocalizedText,
}

/// Un lot de team passive (`TEAM_PASSIVE_LOT_DATA`).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamPassiveLot {
    /// Hash de l'item de lot (correspond à un `ScPassiveSkillEffectInfo.id`).
    pub id: HashId,
    /// Poids dans le tirage au sort (plus élevé = plus fréquent).
    pub lot_weight: i64,
    /// Condition de déverrouillage (vide si aucune).
    pub condition: String,
    /// `true` si la rareté est prise en compte dans ce lot.
    pub rarity_enable_flag: bool,
}

/// Base de données unifiée de toutes les passives.
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnifiedPassiveDb {
    /// Toutes les instances de passives joueur (1716 dans la version 5.00.07).
    pub player: Vec<PlayerPassive>,
    /// Passives d'équipe (21 dans la version 0.00.00).
    pub team: Vec<TeamPassive>,
    /// Lots de team passive (653 dans la version 0.00.00).
    pub lots: Vec<TeamPassiveLot>,
    /// Nombre d'effectIds joueur uniques (= nombre de familles de passives).
    pub unique_effect_count: usize,
    /// Nombre de string_ids uniques.
    pub unique_string_id_count: usize,
}

// ============================================================================
// Résolution de texte
// ============================================================================

/// Remplace les placeholders `[CPASSIVE01]` et `<VALUE>` par la valeur numérique.
///
/// Le texte brut du jeu contient `[CPASSIVE01]+<VALUE>%[C]` où :
/// - `[CPASSIVE01]` = balise de couleur (supprimée)
/// - `<VALUE>` = valeur à injecter
/// - `[C]` = fin de balise couleur (supprimée)
///
/// # Exemple
/// ```
/// # use nie_data::passives::resolve_text_placeholder;
/// // Le template du jeu a un espace entre la valeur et le % : "+0.50 %"
/// let raw = "ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément";
/// let resolved = resolve_text_placeholder(raw, 0.5_f64);
/// assert!(resolved.contains("+0.50"), "valeur non injectée: {resolved:?}");
/// assert!(!resolved.contains("[CPASSIVE01]"), "balise couleur non supprimée");
/// ```
#[must_use]
pub fn resolve_text_placeholder(template: &str, value: f64) -> String {
    // Remplacer d'abord <VALUE> par la valeur formatée
    // En no_std, pas de `f64::trunc()` dans l'API core stable. On approxime :
    // si la valeur est un entier (frac == 0.0), on supprime les décimales.
    #[allow(clippy::cast_precision_loss)]
    let is_integer = (value - (value as i64) as f64).abs() < 1e-9 && value.abs() < 1_000_000.0;
    let value_str = if is_integer {
        alloc::format!("{:.0}", value)
    } else {
        alloc::format!("{value:.2}")
    };
    let s = template.replace("<VALUE>", &value_str);
    // Supprimer les balises couleur [CPASSIVE01], [CPASSIVE02], [C]
    let s = remove_color_tags(&s);
    // Normaliser les retours à la ligne échappés
    s.replace("\\n", "\n")
}

/// Supprime les balises couleur Level-5 : `[CPASSIVE01]`, `[CPASSIVE02]`, `[C]`.
fn remove_color_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            // Lire jusqu'à ']' ou fin
            let mut tag = String::new();
            let mut closed = false;
            for nc in chars.by_ref() {
                if nc == ']' {
                    closed = true;
                    break;
                }
                tag.push(nc);
            }
            // Garder le tag seulement si ce n'est pas une balise couleur connue
            if closed && (tag.starts_with('C') || tag == "C") {
                // Balise couleur → supprimée
            } else if closed {
                // Tag inconnu → conserver (ex: `[風属性/かぜぞくせい]`)
                out.push('[');
                out.push_str(&tag);
                out.push(']');
            } else {
                // Pas fermé → conserver le '[' et le contenu
                out.push('[');
                out.push_str(&tag);
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ============================================================================
// Chargement des textes skill_text (NOUN_INFO)
// ============================================================================

/// Charge les textes NOUN_INFO d'un fichier `skill_text.cfg.bin.json`.
/// Retourne `{hash_u32 → texte}` (vars[0] signé→u32, texte = vars[5]).
///
/// Structure vérifiée sur dump réel :
/// `NOUN_INFO_N` = `[hash(Int), 0(Int), ""(String)×3, texte(String), ""×4, 0×5]`
#[must_use]
pub fn load_noun_texts(root: &Value) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for entry in entries {
            let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
            if !name.starts_with("NOUN_INFO_BEGIN") {
                continue;
            }
            if let Some(children) = entry.get("children").and_then(Value::as_array) {
                for child in children {
                    if let Some(child_name) = child.get("name").and_then(Value::as_str)
                        && !child_name.starts_with("NOUN_INFO_")
                    {
                        continue;
                    }
                    if let Some(vars) = child.get("variables").and_then(Value::as_array) {
                        // vars[0] = hash (signé i64 → u32)
                        let hash = vars
                            .first()
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<i64>().ok())
                            .map(|i| i as u32)
                            .unwrap_or(0);
                        // vars[5] = texte
                        let text = vars
                            .get(5)
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if hash != 0 && !text.is_empty() {
                            map.insert(hash, text.to_string());
                        }
                    }
                }
            }
        }
    }
    map
}

// ============================================================================
// Chargement des textes soccer_team_passive_text (TEXT_INFO)
// ============================================================================

/// Charge les textes TEXT_INFO d'un fichier `soccer_team_passive_text.cfg.bin.json`.
/// Retourne `{hash_u32 → texte}` (vars[0] signé→u32, texte = vars[2]).
///
/// Note : dans toutes les locales (fr/en/ja), les textes sont en japonais.
/// Le jeu n'a pas localisé ces textes — c'est voulu, pas un bug du parseur.
#[must_use]
pub fn load_team_passive_texts(root: &Value) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    if let Some(entries) = root.get("entries").and_then(Value::as_array) {
        for entry in entries {
            let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
            if !name.starts_with("TEXT_INFO_BEGIN") {
                continue;
            }
            if let Some(children) = entry.get("children").and_then(Value::as_array) {
                for child in children {
                    if let Some(vars) = child.get("variables").and_then(Value::as_array) {
                        let hash = vars
                            .first()
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<i64>().ok())
                            .map(|i| i as u32)
                            .unwrap_or(0);
                        // vars[2] = texte dans soccer_team_passive_text
                        let text = vars
                            .get(2)
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if hash != 0 && !text.is_empty() {
                            map.insert(hash, text.to_string());
                        }
                    }
                }
            }
        }
    }
    map
}

// ============================================================================
// Parse des passives joueur (passive_skill_config)
// ============================================================================

/// Parse toutes les instances de passives joueur depuis un `passive_skill_config_*.cfg.bin.json`.
///
/// Construit une jointure complète :
/// - `PASSIVE_SKILL_INFO_N` → stringId, passiveId, effectId, rarity, element
/// - `PASSIVE_SKILL_INFO_REF_EFFECT_N` → index dans `PASSIVE_SKILL_EFFECT_*`
/// - `PASSIVE_SKILL_INFO_REF_BUFF_ICON_N` → index dans `PASSIVE_SKILL_BUFF_ICON_*`
/// - Textes NOUN_INFO par effectId (fr/en/ja)
/// - Résolution du placeholder `<VALUE>` avec le param réel
///
/// Retourne 1716 instances pour la version 5.00.07 (vs 128 uniques dans inagle).
#[must_use]
pub fn parse_player_passives(
    root: &Value,
    text_fr: &BTreeMap<u32, String>,
    text_en: &BTreeMap<u32, String>,
    text_ja: &BTreeMap<u32, String>,
) -> Vec<PlayerPassive> {
    let entries = match root.get("entries").and_then(Value::as_array) {
        Some(e) => e,
        None => return Vec::new(),
    };

    // Étape 1 : collecter les effets (PASSIVE_SKILL_EFFECT_*)
    // Map: index → (effectTypeId, params sans INVALID)
    let mut effect_params: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    let mut buff_icons: BTreeMap<usize, u8> = BTreeMap::new();

    for entry in entries {
        let entry_name = entry.get("name").and_then(Value::as_str).unwrap_or("");

        if entry_name.contains("PASSIVE_SKILL_EFFECT_LIST")
            && let Some(children) = entry.get("children").and_then(Value::as_array)
        {
            for (idx, child) in children.iter().enumerate() {
                let child_name = child.get("name").and_then(Value::as_str).unwrap_or("");
                if !child_name.starts_with("PASSIVE_SKILL_EFFECT_") || child_name.contains("_LIST_")
                {
                    continue;
                }
                let vars = match child.get("variables").and_then(Value::as_array) {
                    Some(v) => v,
                    None => continue,
                };
                // vars[1..] = params (Float), filtrer les INVALID
                let mut params: Vec<f64> = Vec::new();
                for v in vars.iter().skip(1) {
                    let raw = v.get("value").and_then(Value::as_str).unwrap_or("0");
                    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
                    let parsed: i64 = raw.parse().unwrap_or(0);
                    if parsed == INVALID_PARAM {
                        break; // Les sentinelles marquent la fin des params réels
                    }
                    if ty == "Float" {
                        let f: f64 = raw.replace(',', ".").parse().unwrap_or(0.0);
                        params.push(f);
                    } else {
                        params.push(parsed as f64);
                    }
                }
                effect_params.insert(idx, params);
            }
        }

        if entry_name.contains("PASSIVE_SKILL_BUFF_ICON_LIST")
            && let Some(children) = entry.get("children").and_then(Value::as_array)
        {
            for (idx, child) in children.iter().enumerate() {
                let child_name = child.get("name").and_then(Value::as_str).unwrap_or("");
                if !child_name.starts_with("PASSIVE_SKILL_BUFF_ICON_")
                    || child_name.contains("_LIST_")
                {
                    continue;
                }
                let icon_val = child
                    .get("variables")
                    .and_then(Value::as_array)
                    .and_then(|v| v.first())
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                buff_icons.insert(idx, icon_val);
            }
        }
    }

    // Étape 2 : collecter les REF_EFFECT et REF_BUFF_ICON par infoId
    // Map: infoId → (effectIndex, buffIconIndex)
    let mut ref_effects: BTreeMap<usize, (usize, usize)> = BTreeMap::new(); // infoId → (effectIdx, count)
    let mut ref_icons: BTreeMap<usize, usize> = BTreeMap::new(); // infoId → iconIdx

    for entry in entries {
        let entry_name = entry.get("name").and_then(Value::as_str).unwrap_or("");
        if !entry_name.contains("PASSIVE_SKILL_INFO_LIST") {
            continue;
        }
        if let Some(children) = entry.get("children").and_then(Value::as_array) {
            for child in children {
                let child_name = child.get("name").and_then(Value::as_str).unwrap_or("");
                if child_name.starts_with("PASSIVE_SKILL_INFO_REF_EFFECT_") {
                    let id_str = child_name.trim_start_matches("PASSIVE_SKILL_INFO_REF_EFFECT_");
                    if let Ok(info_id) = id_str.parse::<usize>() {
                        let vars = child.get("variables").and_then(Value::as_array);
                        let eff_idx = vars
                            .and_then(|v| v.first())
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let count = vars
                            .and_then(|v| v.get(1))
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(1);
                        ref_effects.insert(info_id, (eff_idx, count));
                    }
                } else if child_name.starts_with("PASSIVE_SKILL_INFO_REF_BUFF_ICON_") {
                    let id_str = child_name.trim_start_matches("PASSIVE_SKILL_INFO_REF_BUFF_ICON_");
                    if let Ok(info_id) = id_str.parse::<usize>() {
                        let icon_idx = child
                            .get("variables")
                            .and_then(Value::as_array)
                            .and_then(|v| v.first())
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        ref_icons.insert(info_id, icon_idx);
                    }
                }
            }
        }
    }

    // Étape 3 : construire les PlayerPassive
    let mut out: Vec<PlayerPassive> = Vec::new();

    for entry in entries {
        let entry_name = entry.get("name").and_then(Value::as_str).unwrap_or("");
        if !entry_name.contains("PASSIVE_SKILL_INFO_LIST") {
            continue;
        }
        if let Some(children) = entry.get("children").and_then(Value::as_array) {
            for child in children {
                let child_name = child.get("name").and_then(Value::as_str).unwrap_or("");
                if !child_name.starts_with("PASSIVE_SKILL_INFO_")
                    || child_name.contains("_REF_")
                    || child_name.contains("_LIST_")
                {
                    continue;
                }
                // Extraire l'index N de PASSIVE_SKILL_INFO_N
                let info_id_str = child_name.trim_start_matches("PASSIVE_SKILL_INFO_");
                let info_id = info_id_str.parse::<usize>().unwrap_or(usize::MAX);

                let vars = match child.get("variables").and_then(Value::as_array) {
                    Some(v) if v.len() >= 5 => v,
                    _ => continue,
                };

                // vars[0] = passiveId (signé→u32)
                let passive_id = vars
                    .first()
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(HashId::from_i64)
                    .unwrap_or(HashId::ZERO);

                // Filtrer les passiveId nuls ou sentinelles (0, 1, 2)
                if passive_id.get() <= 2 {
                    continue;
                }

                // vars[1] = effectId (hash NOUN_INFO = clé texte)
                let effect_id = vars
                    .get(1)
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(HashId::from_i64)
                    .unwrap_or(HashId::ZERO);

                // vars[4] = rarity
                let rarity = vars
                    .get(4)
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);

                // vars[5] = element
                let element = vars
                    .get(5)
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);

                // vars[6] = stringId (String)
                let string_id = vars
                    .get(6)
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                // Résoudre les params d'effet via REF_EFFECT
                let eff_params: Vec<f64> =
                    if let Some((eff_idx, _count)) = ref_effects.get(&info_id) {
                        effect_params.get(eff_idx).cloned().unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                // Résoudre le buff icon via REF_BUFF_ICON
                let buff_icon: Option<u8> = ref_icons
                    .get(&info_id)
                    .and_then(|icon_idx| buff_icons.get(icon_idx))
                    .copied();

                // Texte brut (NOUN_INFO par effectId)
                let eff_u32 = effect_id.get();
                let raw_fr = text_fr.get(&eff_u32).cloned();
                let raw_en = text_en.get(&eff_u32).cloned();
                let raw_ja = text_ja.get(&eff_u32).cloned();

                let text_raw = LocalizedText {
                    fr: raw_fr.clone(),
                    en: raw_en.clone(),
                    ja: raw_ja.clone(),
                };

                // Texte résolu (placeholder → valeur)
                let main_val = eff_params.first().copied().unwrap_or(0.0);
                let text_resolved = LocalizedText {
                    fr: raw_fr
                        .as_deref()
                        .map(|t| resolve_text_placeholder(t, main_val)),
                    en: raw_en
                        .as_deref()
                        .map(|t| resolve_text_placeholder(t, main_val)),
                    ja: raw_ja
                        .as_deref()
                        .map(|t| resolve_text_placeholder(t, main_val)),
                };

                out.push(PlayerPassive {
                    passive_id,
                    effect_id,
                    string_id,
                    rarity,
                    element,
                    buff_icon_type: buff_icon,
                    effect_params: eff_params,
                    text_raw,
                    text_resolved,
                });
            }
        }
    }

    out
}

// ============================================================================
// Parse des team passives (soccer_team_passive_config)
// ============================================================================

/// Parse les passives d'équipe depuis `soccer_team_passive_config_*.cfg.bin.json`.
///
/// Source : liste `m_soccerTeamPassiveDataList` (format `{ version, lists }`,
/// différent du format `entries` des passives joueur).
///
/// Retourne 21 entrées pour la version 0.00.00.
#[must_use]
pub fn parse_team_passives(root: &Value, text_ja: &BTreeMap<u32, String>) -> Vec<TeamPassive> {
    let values = match list_values(root, "m_soccerTeamPassiveDataList") {
        Some(v) => v,
        None => return Vec::new(),
    };

    values
        .iter()
        .map(|v| {
            let team_passive_id = field_hash(v, "teamPassiveId");
            let effect_id = field_hash(v, "effectId");
            let effect_value_min = field_i64(v, "effectValueMin").unwrap_or(0);
            let effect_value_max = field_i64(v, "effectValueMax").unwrap_or(0);
            let text_id = field_hash(v, "teamPassiveTextId");

            // Résolution texte (JA dans toutes les locales)
            let text_ja_str = text_ja.get(&text_id.get()).cloned();
            let text = LocalizedText {
                fr: None,
                en: None,
                ja: text_ja_str,
            };

            TeamPassive {
                team_passive_id,
                effect_id,
                effect_value_min,
                effect_value_max,
                text_id,
                text,
            }
        })
        .collect()
}

// ============================================================================
// Parse des lots (team_passive_lot_table_config)
// ============================================================================

/// Parse les lots de tirage depuis `team_passive_lot_table_config_*.cfg.bin.json`.
///
/// Source : liste `m_teamPassiveLotDataList` (format `{ version, lists }`).
/// Retourne 653 entrées pour la version 0.00.00.
#[must_use]
pub fn parse_lots(root: &Value) -> Vec<TeamPassiveLot> {
    let values = match list_values(root, "m_teamPassiveLotDataList") {
        Some(v) => v,
        None => return Vec::new(),
    };

    values
        .iter()
        .map(|v| {
            let id = field_hash(v, "id");
            let lot_weight = field_i64(v, "lotWeight").unwrap_or(0);
            let condition = field_str(v, "condition").unwrap_or("").to_string();
            let rarity_enable_flag = v
                .get("rarityEnableFlag")
                .and_then(Value::as_i64)
                .map(|n| n != 0)
                .unwrap_or(false);

            TeamPassiveLot {
                id,
                lot_weight,
                condition,
                rarity_enable_flag,
            }
        })
        .collect()
}

// ============================================================================
// Sortie JSON unifiée
// ============================================================================

/// Sérialise la base de données unifiée en JSON (schéma commun passive/team/lots).
///
/// Schéma de sortie :
/// ```json
/// {
///   "meta": { "player_count": 1716, "team_count": 21, "lot_count": 653,
///             "unique_effect_count": 128, "unique_string_id_count": 1716 },
///   "player": [ { "passive_id": "0x...", "string_id": "ps10001", "effect_id": "0x...",
///                 "rarity": 6, "element": 0, "buff_icon_type": 2,
///                 "effect_params": [0.5], "text_fr": "...", "text_en": "...", "text_ja": "...",
///                 "text_fr_raw": "...[CPASSIVE01]+<VALUE>%..." } ],
///   "team":   [ { "team_passive_id": "0x...", "effect_id": "0x...",
///                 "value_min": 0, "value_max": 8, "text_ja": "キャプテンのブロック" } ],
///   "lots":   [ { "id": "0x...", "lot_weight": 50, "condition": "", "rarity_enable": true } ]
/// }
/// ```
#[cfg(feature = "serde")]
#[must_use]
pub fn to_json(db: &UnifiedPassiveDb) -> serde_json::Value {
    use serde_json::json;

    let player_arr: Vec<serde_json::Value> = db
        .player
        .iter()
        .map(|p| {
            json!({
                "passive_id": p.passive_id.to_hex(),
                "string_id": p.string_id,
                "effect_id": p.effect_id.to_hex(),
                "rarity": p.rarity,
                "element": p.element,
                "element_name": element_name(p.element),
                "buff_icon_type": p.buff_icon_type,
                "effect_params": p.effect_params,
                "main_value": p.main_value(),
                "text_fr": p.text_resolved.fr,
                "text_en": p.text_resolved.en,
                "text_ja": p.text_resolved.ja,
                "text_fr_raw": p.text_raw.fr,
            })
        })
        .collect();

    let team_arr: Vec<serde_json::Value> = db
        .team
        .iter()
        .map(|t| {
            json!({
                "team_passive_id": t.team_passive_id.to_hex(),
                "effect_id": t.effect_id.to_hex(),
                "value_min": t.effect_value_min,
                "value_max": t.effect_value_max,
                "text_id": t.text_id.to_hex(),
                "text_ja": t.text.ja,
            })
        })
        .collect();

    let lots_arr: Vec<serde_json::Value> = db
        .lots
        .iter()
        .map(|l| {
            json!({
                "id": l.id.to_hex(),
                "lot_weight": l.lot_weight,
                "condition": l.condition,
                "rarity_enable": l.rarity_enable_flag,
            })
        })
        .collect();

    json!({
        "meta": {
            "player_count": db.player.len(),
            "team_count": db.team.len(),
            "lot_count": db.lots.len(),
            "unique_effect_count": db.unique_effect_count,
            "unique_string_id_count": db.unique_string_id_count,
        },
        "player": player_arr,
        "team": team_arr,
        "lots": lots_arr,
    })
}

/// Retourne le nom de l'élément depuis son id (0-4).
/// Source : `game-constants.json` `listItemType=4` = element.
#[must_use]
pub fn element_name(element: u8) -> &'static str {
    match element {
        0 => "neutral",
        1 => "wind",
        2 => "forest",
        3 => "fire",
        4 => "mountain",
        _ => "unknown",
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_placeholder_shot_at() {
        // Texte réel vérifié sur dump fr/skill_text.cfg.bin.json NOUN_INFO_926
        let raw = "ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément";
        let resolved = resolve_text_placeholder(raw, 0.5_f64);
        // Le template du jeu a un espace entre <VALUE> et % : "+0.50 %"
        assert!(
            resolved.contains("+0.50"),
            "valeur non injectée: resolved={resolved:?}"
        );
        assert!(
            !resolved.contains("[CPASSIVE01]"),
            "balise couleur non supprimée: {resolved:?}"
        );
        assert!(
            !resolved.contains("[C]"),
            "balise fin non supprimée: {resolved:?}"
        );
        assert!(
            !resolved.contains("<VALUE>"),
            "placeholder non remplacé: {resolved:?}"
        );
    }

    #[test]
    fn resolve_placeholder_entier() {
        let raw = "Équipe [CPASSIVE01]+<VALUE>%[C]";
        let resolved = resolve_text_placeholder(raw, 8.0_f64);
        assert!(resolved.contains("+8%"), "resolved={resolved:?}");
        assert!(!resolved.contains("[CPASSIVE01]"));
    }

    #[test]
    fn resolve_placeholder_preserve_japanese_tags() {
        // Les tags japonais [風属性/かぜぞくせい] ne doivent PAS être supprimés
        let raw = "[風属性/かぜぞくせい][選手/せんしゅ]のシュートＡＴ[CPASSIVE01]＋<VALUE>％[C]";
        let resolved = resolve_text_placeholder(raw, 1.0_f64);
        assert!(
            resolved.contains("[風属性/かぜぞくせい]"),
            "tag JA supprimé: {resolved:?}"
        );
        assert!(
            !resolved.contains("[CPASSIVE01]"),
            "couleur non supprimée: {resolved:?}"
        );
    }

    #[test]
    fn load_noun_texts_from_real_structure() {
        // Simule un extrait du fichier fr/skill_text.cfg.bin.json vérifié
        let json_str = r#"{"entries":[{
            "name": "NOUN_INFO_BEGIN_0",
            "variables": [{"type":"Int","value":"1658"}],
            "children": [
                {
                    "name": "NOUN_INFO_926",
                    "variables": [
                        {"type":"Int","value":"1105141741"},
                        {"type":"Int","value":"0"},
                        {"type":"String","value":""},
                        {"type":"String","value":""},
                        {"type":"String","value":""},
                        {"type":"String","value":"ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément"},
                        {"type":"String","value":""}
                    ],
                    "children": []
                }
            ]
        }]}"#;
        let root: Value = serde_json::from_str(json_str).unwrap();
        let texts = load_noun_texts(&root);
        // effectId 1105141741 (signé) = u32 1105141741 (< 2^31, positif)
        assert_eq!(
            texts.get(&1105141741_u32).map(|s| s.as_str()),
            Some("ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément")
        );
    }

    #[test]
    fn parse_player_passives_minimal() {
        // Structure minimale vérifiée sur passive_skill_config_5.00.07.00.cfg.bin.json
        let json_str = r#"{"entries":[
            {
                "name": "PASSIVE_SKILL_EFFECT_LIST_BEG_0",
                "variables": [{"type":"Int","value":"1700"}],
                "children": [
                    {
                        "name": "PASSIVE_SKILL_EFFECT_0",
                        "variables": [
                            {"type":"Int","value":"-1974296472"},
                            {"type":"Float","value":"0,5"},
                            {"type":"Int","value":"-992181094"}
                        ],
                        "children": []
                    }
                ]
            },
            {
                "name": "PASSIVE_SKILL_BUFF_ICON_LIST_BEG_0",
                "variables": [{"type":"Int","value":"108"}],
                "children": [
                    {
                        "name": "PASSIVE_SKILL_BUFF_ICON_0",
                        "variables": [{"type":"Int","value":"2"}],
                        "children": []
                    }
                ]
            },
            {
                "name": "PASSIVE_SKILL_INFO_LIST_BEG_0",
                "variables": [{"type":"Int","value":"1716"},{"type":"Int","value":"1"}],
                "children": [
                    {
                        "name": "PASSIVE_SKILL_INFO_0",
                        "variables": [
                            {"type":"Int","value":"975948532"},
                            {"type":"Int","value":"1105141741"},
                            {"type":"Int","value":"0"},
                            {"type":"Int","value":"0"},
                            {"type":"Int","value":"6"},
                            {"type":"Int","value":"0"},
                            {"type":"String","value":"ps10001"}
                        ],
                        "children": []
                    },
                    {
                        "name": "PASSIVE_SKILL_INFO_REF_BUFF_ICON_0",
                        "variables": [{"type":"Int","value":"0"},{"type":"Int","value":"1"}],
                        "children": []
                    },
                    {
                        "name": "PASSIVE_SKILL_INFO_REF_EFFECT_0",
                        "variables": [{"type":"Int","value":"0"},{"type":"Int","value":"1"}],
                        "children": []
                    }
                ]
            }
        ]}"#;
        let root: Value = serde_json::from_str(json_str).unwrap();

        // Textes : effectId 1105141741 → texte FR
        let mut text_fr = BTreeMap::new();
        text_fr.insert(
            1105141741_u32,
            "ATT des tirs [CPASSIVE01]+<VALUE> %[C] \\npour les joueurs du même élément"
                .to_string(),
        );
        let text_en = BTreeMap::new();
        let text_ja = BTreeMap::new();

        let passives = parse_player_passives(&root, &text_fr, &text_en, &text_ja);

        assert_eq!(passives.len(), 1, "doit parser 1 passive");
        let p = &passives[0];
        assert_eq!(p.passive_id.to_hex(), "0x3A2BCAF4"); // 975948532 → u32
        assert_eq!(p.effect_id.to_hex(), "0x41DF1FED"); // 1105141741 → u32
        assert_eq!(p.string_id, "ps10001");
        assert_eq!(p.rarity, 6);
        assert_eq!(p.element, 0);
        assert_eq!(p.buff_icon_type, Some(2));
        assert!(
            (p.effect_params[0] - 0.5).abs() < 1e-6,
            "param={}",
            p.effect_params[0]
        );
        // Texte résolu
        let resolved_fr = p.text_resolved.fr.as_deref().unwrap_or("");
        assert!(resolved_fr.contains("+0.50"), "resolved={resolved_fr:?}");
        assert!(!resolved_fr.contains("[CPASSIVE01]"));
    }

    #[test]
    fn parse_team_passives_minimal() {
        // Structure minimale vérifiée sur soccer_team_passive_config_0.00.00.cfg.bin.json
        let json_str = r#"{"version":100,"lists":[{
            "name": "m_soccerTeamPassiveDataList",
            "typeName": "SOCCER_TEAM_PASSIVE_DATA",
            "values": [
                {
                    "teamPassiveId": "0x2A7D4552",
                    "effectId": "0xBEBB2EE8",
                    "effectValueMax": 8,
                    "effectValueMin": 0,
                    "teamPassiveTextId": "0x44FECFAB"
                }
            ]
        }]}"#;
        let root: Value = serde_json::from_str(json_str).unwrap();

        // Texte JA vérifié sur soccer_team_passive_text.cfg.bin.json
        let mut text_ja = BTreeMap::new();
        text_ja.insert(0x44FECFABu32, "キャプテンのブロック".to_string());

        let team = parse_team_passives(&root, &text_ja);
        assert_eq!(team.len(), 1);
        let t = &team[0];
        assert_eq!(t.team_passive_id.to_hex(), "0x2A7D4552");
        assert_eq!(t.effect_id.to_hex(), "0xBEBB2EE8");
        assert_eq!(t.effect_value_min, 0);
        assert_eq!(t.effect_value_max, 8);
        assert_eq!(t.text.ja.as_deref(), Some("キャプテンのブロック"));
    }

    #[test]
    fn parse_lots_minimal() {
        let json_str = r#"{"version":100,"lists":[{
            "name": "m_teamPassiveLotDataList",
            "typeName": "TEAM_PASSIVE_LOT_DATA",
            "values": [
                {"id": "0x5E4A1D85", "lotWeight": 50, "condition": "", "rarityEnableFlag": 1},
                {"id": "0xC7434C3F", "lotWeight": 30, "condition": "", "rarityEnableFlag": 0}
            ]
        }]}"#;
        let root: Value = serde_json::from_str(json_str).unwrap();
        let lots = parse_lots(&root);
        assert_eq!(lots.len(), 2);
        assert_eq!(lots[0].id.to_hex(), "0x5E4A1D85");
        assert_eq!(lots[0].lot_weight, 50);
        assert!(lots[0].rarity_enable_flag);
        assert!(!lots[1].rarity_enable_flag);
    }

    #[test]
    fn element_names_correct() {
        assert_eq!(element_name(0), "neutral");
        assert_eq!(element_name(1), "wind");
        assert_eq!(element_name(2), "forest");
        assert_eq!(element_name(3), "fire");
        assert_eq!(element_name(4), "mountain");
    }
}
