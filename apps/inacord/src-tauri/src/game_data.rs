//! Données de jeu STATIQUES (techniques pour l'instant) — indépendant du VFS "brut"
//! (`vfs_ls`/`vfs_describe`) et du miroir wiki azalee (`wikiDb`/`nameResolve`) : lecture directe
//! des `.cfg.bin` du jeu, décodés par les VRAIS parseurs typés de `nie-data` (déjà une
//! dépendance déclarée mais jamais câblée dans une commande jusqu'ici — cf. demande
//! utilisatrice « toutes les features, api et code de niers et des crates doivent être
//! utilisable et utilisé dans l'app »).
//!
//! Le pont bytes→JSON forme "inagle" qu'attendent les parseurs `nie-data` (`parse_skill_config`,
//! et ~115 autres modules du même crate) existe déjà et est TESTÉ : [`nie_explore::bridge`]
//! (`t2b_to_json`/`rdbn_to_json`), utilisé par `niers vfs cat`. Ce module ne fait que l'appeler
//! avec les bons chemins VFS — pas de nouvelle logique de décodage, zéro doublon.
//!
//! Un seul module câblé pour l'instant (techniques) : les ~115 autres (personnages, objets,
//! auras, boutiques, quêtes…) suivent EXACTEMENT le même patron (`list_values`/`walk_named` +
//! bridge), à étendre au besoin.

use nie_data::skill::{SkillInfo, SkillTextMaps};
use nie_formats::cfgbin::{CfgEntry, Value as CfgValue};
use nie_formats::vfs::Vfs;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Technique (hissatsu) — port applati de `nie_data::skill::SkillInfo` + son texte joint
/// (`skillNameId`/`skillDescId` résolus via `skill_text.cfg.bin`), pour l'IPC/l'export TS.
#[derive(Serialize, specta::Type)]
pub struct SkillDto {
    pub skill_id: String,
    pub skill_id_str: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub element: String,
    pub category: String,
    pub power_min: i32,
    pub power_max: i32,
    pub consume_tp: i32,
    pub recast_time: i32,
    pub eldorado: bool,
}

/// Premier chemin VFS (ordre alphabétique) dont le nom de fichier satisfait `pred` — même
/// convention de résolution dynamique que `nie-game/examples/export_*.rs` (les `.cfg.bin` sont
/// suffixés par version, ex. `skill_config_4.00.17.00.cfg.bin` : pas de nom fixe possible).
fn find_path(vfs: &Vfs, pred: impl Fn(&str) -> bool) -> Option<String> {
    vfs.iter().map(|(p, _)| p.to_string()).filter(|p| pred(p)).min()
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Charge un `.cfg.bin` **RDBN** du VFS (chemin résolu dynamiquement, cf. [`find_path`]) et le
/// convertit en JSON forme "inagle" `{"lists":[…]}` via le pont déjà vérifié
/// [`nie_explore::bridge::rdbn_to_json`] — symétrique de [`load_t2b`].
///
/// 67 des 110 modules `nie-data` lisent du RDBN : ce chemin était jusqu'ici écrit à la main dans
/// `parse_skills` uniquement, ce qui obligeait chaque nouvelle famille à le redupliquer.
/// Contrairement au T2B, aucune désambiguïsation de noms n'est nécessaire — le RDBN nomme ses
/// listes et ses champs, `read_values` en sort des lignes déjà clés/valeurs.
fn load_rdbn(vfs: &Vfs, pred: impl Fn(&str) -> bool, what: &str) -> Result<Value, String> {
    let path = find_path(vfs, pred).ok_or_else(|| format!("{what} introuvable dans le VFS monté"))?;
    let bytes = vfs.read(&path).map_err(|e| e.to_string())?;
    let rdbn = nie_formats::cfgbin::parse(&bytes).map_err(|e| format!("parse RDBN {path} : {e}"))?;
    Ok(nie_explore::bridge::rdbn_to_json(&nie_formats::cfgbin::read_values(&rdbn, &bytes)))
}

/// Parse `skill_config` (+ `skill_text` FR si présent) → `SkillInfo` bruts + textes joints.
/// Factorisé depuis [`list_skills`] pour être réutilisé par [`find_skill`] (résolution par nom/ID
/// pour le pont Blender, cf. `blender_build_skill_scene` dans `lib.rs`) sans reparser deux fois.
fn parse_skills(vfs: &Vfs) -> Result<(Vec<SkillInfo>, SkillTextMaps), String> {
    let config_json = load_rdbn(
        vfs,
        |p| p.contains("/skill/") && base_name(p).starts_with("skill_config") && base_name(p).ends_with(".cfg.bin"),
        "skill_config",
    )?;
    let skills = nie_data::skill::parse_skill_config(&config_json);

    // Absence de la table FR → noms/descriptions `None`, jamais un échec de la liste entière
    // (`parse_skill_text` accepte les deux formes de nommage, indexée ou brute).
    let maps = load_text_json(vfs, "skill")
        .map(|j| nie_data::skill::parse_skill_text(&j))
        .unwrap_or_default();
    Ok((skills, maps))
}

/// Trouve UNE technique par requête libre : `skill_id_str` exact (ex. `whs00340`) en priorité,
/// sinon sous-chaîne insensible à la casse dans `skill_id_str` OU le nom FR résolu (ex. « Savoir
/// suprême »). `None` si aucune ne correspond — jamais un premier résultat approximatif imposé en
/// silence. Utilisé par `blender_build_skill_scene` (`lib.rs`) pour résoudre une requête utilisateur
/// libre (« Savoir Suprême ») en `SkillInfo` (→ `cutin_assets()` pour les chemins VFS du cut-in).
pub fn find_skill(vfs: &Vfs, query: &str) -> Result<Option<SkillInfo>, String> {
    let (skills, maps) = parse_skills(vfs)?;
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(None);
    }
    if let Some(exact) = skills.iter().find(|s| s.skill_id_str.eq_ignore_ascii_case(&q)) {
        return Ok(Some(exact.clone()));
    }
    Ok(skills
        .into_iter()
        .find(|s| {
            s.skill_id_str.to_lowercase().contains(&q)
                || nie_data::skill::join_skill_text(s, &maps).name.is_some_and(|n| n.to_lowercase().contains(&q))
        }))
}

/// Liste toutes les techniques du jeu (`m_skillInfoList` de `skill_config`), noms/descriptions
/// FR joints depuis `skill_text.cfg.bin` si trouvé (sinon `name`/`description` restent `None` —
/// jamais une chaîne devinée).
pub fn list_skills(vfs: &Vfs) -> Result<Vec<SkillDto>, String> {
    let (skills, maps) = parse_skills(vfs)?;

    Ok(skills
        .iter()
        .map(|s| {
            let text = nie_data::skill::join_skill_text(s, &maps);
            let element = s.element();
            let category = s.category();
            SkillDto {
                skill_id: s.skill_id.to_hex(),
                skill_id_str: s.skill_id_str.clone(),
                name: text.name,
                description: text.description,
                element: element.names().map(|(fr, _, _)| fr.to_string()).unwrap_or_else(|| format!("? ({})", s.element)),
                category: category.names().map(|(fr, _, _)| fr.to_string()).unwrap_or_else(|| format!("? ({})", s.category)),
                power_min: s.power_min as i32,
                power_max: s.power_max as i32,
                consume_tp: s.consume_tp as i32,
                recast_time: s.recast_time as i32,
                eldorado: s.eldorado,
            }
        })
        .collect())
}

/// Convertit une valeur T2B (`nie_formats::cfgbin::Value`) en JSON forme "inagle" — même mapping
/// que [`nie_explore::bridge::t2b_value_to_json`] (privé, donc reproduit ici plutôt qu'importé).
fn t2b_value_to_json(v: &CfgValue) -> Value {
    match v {
        CfgValue::String(s) => json!({ "type": "String", "value": s }),
        CfgValue::Int(i) => json!({ "type": "Int", "value": i.to_string() }),
        CfgValue::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
    }
}

/// Convertit UNE liste de `CfgEntry` frères en JSON **avec suffixe d'index par nom dupliqué**
/// (`TEXT_INFO` → `TEXT_INFO_0`, `TEXT_INFO_1`, …) — c'est la forme "inagle"/iecode RÉELLE
/// qu'attendent `walk_named`/[`nie_data::text::parse_text_file`] (prefix-match `"TEXT_INFO_"` +
/// exclusion du noeud `"…_BEGIN"`), PAS la forme brute de [`nie_explore::bridge::t2b_to_json`]
/// (noms non désambiguïsés, ex. plusieurs frères tous nommés `"TEXT_INFO"`) : `walk_named`
/// matcherait alors 0 noeud (bug constaté : `list_items`/`list_auras`/`list_trophies`/
/// `list_quests` résolvaient 0 texte avant ce correctif, malgré des fichiers texte trouvés et
/// parsés sans erreur). Port fidèle du `to_iecode` local dupliqué dans CHAQUE
/// `nie-game/examples/export_{items,auras,trophies,quests}.rs` (déjà validé end-to-end sur le
/// vrai jeu) — factorisé ici une fois pour toutes plutôt que redupliqué une 5ᵉ fois.
fn to_indexed_json(siblings: &[CfgEntry]) -> Vec<Value> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            json!({
                "name": name,
                "variables": e.variables.iter().map(t2b_value_to_json).collect::<Vec<_>>(),
                "children": to_indexed_json(&e.children),
            })
        })
        .collect()
}

/// Charge un `.cfg.bin` T2B du VFS (chemin résolu dynamiquement, cf. [`find_path`]) et le
/// convertit en JSON forme "inagle" **indexée** (cf. [`to_indexed_json`]) — factorisé pour les
/// modules `nie-data` ci-dessous (item/aura/trophy/quest, config ET texte).
fn load_t2b(vfs: &Vfs, pred: impl Fn(&str) -> bool, what: &str) -> Result<Value, String> {
    let path = find_path(vfs, pred).ok_or_else(|| format!("{what} introuvable dans le VFS monté"))?;
    let bytes = vfs.read(&path).map_err(|e| e.to_string())?;
    let cfg = nie_formats::cfgbin::parse_t2b(&bytes).map_err(|e| format!("parse T2B {path} : {e}"))?;
    Ok(json!({ "entries": to_indexed_json(&cfg.entries) }))
}

/// Charge la table de texte FR d'un `text_type` convivial (`"skill"`, `"item"`, `"team"`, …) sous
/// sa forme JSON indexée.
///
/// Le nom de fichier vient de [`nie_data::text::text_file_name`] — la table `TEXT_FILES` (43
/// entrées, port 1:1 d'inagle) qui EST le catalogue des familles de texte du jeu. Chaque `list_*`
/// recodait auparavant son propre prédicat en dur (`p.contains("/fr/") &&
/// base_name(p).starts_with("skill_text")`, déjà dupliqué 5 fois), au risque de coller à un nom
/// approximatif ; ici un type inconnu échoue franchement au lieu de chercher un fichier inexistant.
fn load_text_json(vfs: &Vfs, text_type: &str) -> Result<Value, String> {
    load_text_json_lang(vfs, text_type, "fr")
}

/// Table de texte d'un `text_type` dans une LANGUE donnée (`fr`, `en`, `ja`, `de`, `es`, `it`,
/// `pt`, `zh_hans`, `zh_hant` — les neuf dossiers de `data/common/text/`, relevés sur
/// l'installation, cf. [`LANGUES`]). Généralise [`load_text_json`], qui forçait `fr`.
fn load_text_json_lang(vfs: &Vfs, text_type: &str, langue: &str) -> Result<Value, String> {
    let stem = nie_data::text::text_file_name(text_type)
        .ok_or_else(|| format!("type de texte inconnu : {text_type} (cf. nie_data::text::TEXT_FILES)"))?;
    let file = format!("{stem}.cfg.bin");
    let dossier = format!("/text/{langue}/");
    load_t2b(vfs, |p| p.contains(&dossier) && base_name(p) == file, &format!("{file} {langue}"))
}

/// Table de texte FR d'un `text_type` convivial, déjà parsée en `(hashId, texte)` — la forme
/// qu'attendent tous les `resolve_*`/`find_text` de `nie-data`. Cf. [`load_text_json`].
fn load_text(vfs: &Vfs, text_type: &str) -> Result<Vec<(nie_data::HashId, String)>, String> {
    Ok(nie_data::text::parse_text_file(&load_text_json(vfs, text_type)?))
}

/// Objet (arme/consommable/costume/…) — port applati de `nie_data::item::ItemInfo` + son texte
/// joint (`item_text.cfg.bin`, mêmes noms ET descriptions), pour l'IPC/l'export TS. N'inclut que
/// les objets à nom résolu (comme `nie-game/examples/export_items.rs`, roster réel).
#[derive(Serialize, specta::Type)]
pub struct ItemDto {
    pub item_id: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    /// `f64` et pas `i64` : `specta` refuse d’exporter les types « BigInt » vers TypeScript
    /// (perte de précision silencieuse) — refus FATAL qui faisait paniquer l’export au démarrage.
    /// Un prix du jeu tient très en dessous des 2⁵³ entiers exacts d’un `f64`.
    pub price: Option<f64>,
    pub internal_code: Option<String>,
}

/// Liste tous les objets du jeu (`item_config`), noms/descriptions FR joints depuis `item_text`.
pub fn list_items(vfs: &Vfs) -> Result<Vec<ItemDto>, String> {
    let config = load_t2b(vfs, |p| p.contains("/gamedata/item/") && base_name(p).starts_with("item_config") && base_name(p).ends_with(".cfg.bin"), "item_config")?;
    let items = nie_data::item::parse_all_items(&config);
    let text = load_text(vfs, "item")?;

    Ok(items
        .iter()
        .filter_map(|it| {
            let name = nie_data::item::resolve_name(it, &text)?;
            Some(ItemDto {
                item_id: it.item_id.to_hex(),
                category: format!("{:?}", it.category),
                name: name.to_string(),
                description: nie_data::item::resolve_description(it, &text).map(str::to_string),
                price: it.price.map(|p| p as f64),
                internal_code: it.internal_code.clone(),
            })
        })
        .collect())
}

/// Avatar/Keshin (aura) — port applati de `nie_data::aura::AuraCmd` + son texte joint
/// (`skill_text.cfg.bin`, même table que les techniques). Le contenu signature d'IEVR.
#[derive(Serialize, specta::Type)]
pub struct AuraDto {
    pub aura_id: String,
    pub asset_code: String,
    pub name: String,
    pub description: Option<String>,
    pub element: String,
    pub sub_type: String,
}

/// Liste tous les Avatar/Keshin (`aura_skill_config`), noms/descriptions FR joints depuis `skill_text`.
pub fn list_auras(vfs: &Vfs) -> Result<Vec<AuraDto>, String> {
    let config = load_t2b(vfs, |p| p.contains("/gamedata/") && base_name(p).starts_with("aura_skill_config") && base_name(p).ends_with(".cfg.bin"), "aura_skill_config")?;
    let auras = nie_data::aura::parse_all_aura_cmds(&config);
    let text = load_text(vfs, "skill")?;

    Ok(auras
        .iter()
        .filter_map(|a| {
            let name = nie_data::aura::resolve_name(a, &text)?;
            Some(AuraDto {
                aura_id: a.aura_id.to_hex(),
                asset_code: a.asset_code.clone(),
                name: name.to_string(),
                description: nie_data::aura::resolve_description(a, &text).map(str::to_string),
                element: format!("{:?}", a.element()),
                sub_type: format!("{:?}", a.sub_type),
            })
        })
        .collect())
}

/// Succès (trophy) — port applati de `nie_data::trophy::TrophyInfo` + son texte joint
/// (`trophy_text.cfg.bin`) + condition de déblocage décodée (`decode_unlock`).
#[derive(Serialize, specta::Type)]
pub struct TrophyDto {
    pub trophy_id: String,
    pub code: String,
    /// `f64` et pas `i64` : `specta` refuse d’exporter les types « BigInt » vers TypeScript
    /// (perte de précision silencieuse) et le refus fait paniquer l’export au démarrage. Les
    /// catégories du jeu sont de petits entiers, exactement représentables en `f64`.
    pub category: f64,
    pub name: String,
    pub description: Option<String>,
    pub unlock_kind: String,
    pub story_episode: Option<u32>,
}

/// Liste tous les succès (`trophy_config`), noms/descriptions FR joints depuis `trophy_text`,
/// condition de déblocage décodée (story/event-flag/composite/always).
pub fn list_trophies(vfs: &Vfs) -> Result<Vec<TrophyDto>, String> {
    let config_json = load_t2b(vfs, |p| p.contains("/gamedata/") && base_name(p).starts_with("trophy_config") && base_name(p).ends_with(".cfg.bin"), "trophy_config")?;
    let config = nie_data::trophy::parse_trophy_config(&config_json);
    let text = load_text(vfs, "trophy")?;

    Ok(config
        .infos
        .iter()
        .filter_map(|t| {
            let name = nie_data::trophy::resolve_name(t, &text)?;
            let cond = nie_data::trophy::decode_unlock(t);
            use nie_data::unlock_condition::UnlockType as U;
            Some(TrophyDto {
                trophy_id: t.trophy_id.to_hex(),
                code: t.code.clone(),
                category: t.category as f64,
                name: name.to_string(),
                description: nie_data::trophy::resolve_description(t, &text).map(str::to_string),
                unlock_kind: match cond.kind {
                    U::Always => "always",
                    U::Story => "story",
                    U::EventFlag => "eventFlag",
                    U::Composite => "composite",
                }
                .to_string(),
                story_episode: cond.story_episode,
            })
        })
        .collect())
}

/// Quête — port applati de `nie_data::quest::ParsedQuest` + son titre joint
/// (`quest_title_text.cfg.bin`). N'inclut que les quêtes à titre résolu (roster réel).
#[derive(Serialize, specta::Type)]
pub struct QuestDto {
    pub quest_id: String,
    /// `f64`, cf. [`TrophyDto::category`] — même contrainte `specta`.
    pub phase: f64,
    /// `f64`, cf. [`TrophyDto::category`] — même contrainte `specta`.
    pub quest_type: f64,
    pub title: String,
    pub image: Option<String>,
}

/// Liste toutes les quêtes (`quest_config`), titres FR joints depuis `quest_title_text`.
pub fn list_quests(vfs: &Vfs) -> Result<Vec<QuestDto>, String> {
    let config = load_t2b(vfs, |p| p.contains("/gamedata/quest/") && base_name(p).starts_with("quest_config") && base_name(p).ends_with(".cfg.bin"), "quest_config")?;
    let quests = nie_data::quest::parse_quest_config(&config);
    let titles = load_text(vfs, "quest_title")?;

    Ok(quests
        .iter()
        .filter_map(|q| {
            let title = nie_data::quest::resolve_title(q, &titles)?;
            Some(QuestDto {
                quest_id: q.quest_id.to_hex(),
                phase: q.phase as f64,
                quest_type: q.quest_type as f64,
                title: title.to_string(),
                image: q.image.clone(),
            })
        })
        .collect())
}

/// Personnage sélectionnable pour le calculateur de stats (§4.2 roadmap) — `nie_data::chara_param
/// ::CharaParam` joint à `chara_base`/`chara_text` pour un nom affichable. N'inclut que les
/// entrées à nom résolu (roster réel, même convention que les autres `list_*`).
#[derive(Serialize, specta::Type)]
pub struct CharaPickerDto {
    pub chara_param_id: String,
    pub name: String,
    pub main_position: String,
    pub sub_position: String,
}

/// Liste les personnages sélectionnables (`chara_param` joint à `chara_base`+`chara_text` pour
/// le nom). Un `chara_base_id` peut avoir plusieurs `CharaParam` (variantes de tenue/costume) —
/// toutes sont listées, différenciées par leur `chara_param_id`.
pub fn list_chara_picker(vfs: &Vfs) -> Result<Vec<CharaPickerDto>, String> {
    let param_json = load_t2b(vfs, |p| base_name(p).starts_with("chara_param_1") && base_name(p).ends_with(".cfg.bin"), "chara_param")?;
    let params = nie_data::chara_param::parse_all_chara_params(&param_json);

    let base_json = load_t2b(vfs, |p| p.contains("/character/") && base_name(p).starts_with("chara_base_1") && base_name(p).ends_with(".cfg.bin"), "chara_base")?;
    let bases = nie_data::chara_base::parse_all_chara_base(&base_json);

    let nouns = nie_data::chara_text::parse_all_nouns(&load_text_json(vfs, "chara")?);

    Ok(params
        .iter()
        .filter_map(|cp| {
            let base = nie_data::chara_base::find_by_chara_id(&bases, cp.chara_base_id)?;
            let first = nie_data::chara_base::resolve_first_name(base, &nouns)?;
            let last = nie_data::chara_base::resolve_last_name(base, &nouns);
            let name = match last {
                Some(l) => format!("{first} {l}"),
                None => first.to_string(),
            };
            Some(CharaPickerDto {
                chara_param_id: cp.chara_param_id.to_hex(),
                name,
                main_position: nie_data::chara_param::position_code_owned(cp.main_position).unwrap_or_else(|| "?".to_string()),
                sub_position: nie_data::chara_param::position_code_owned(cp.sub_position).unwrap_or_else(|| "—".to_string()),
            })
        })
        .collect())
}

/// Bloc de 7 stats calculées (`nie_core::stats::StatBlock`), pour l'IPC/l'export TS.
#[derive(Serialize, specta::Type)]
pub struct StatBlockDto {
    pub kc: u16,
    pub cr: u16,
    pub tc: u16,
    pub pr: u16,
    pub ps: u16,
    pub ag: u16,
    pub it: u16,
    pub total: u32,
}

impl From<nie_core::stats::StatBlock> for StatBlockDto {
    fn from(s: nie_core::stats::StatBlock) -> Self {
        StatBlockDto { kc: s.kc, cr: s.cr, tc: s.tc, pr: s.pr, ps: s.ps, ag: s.ag, it: s.it, total: s.total() }
    }
}

/// Calcule les stats d'un personnage (`chara_param_id`, cf. [`list_chara_picker`]) à un niveau et
/// une rareté donnés — `nie_core::growth::calculate_stats` sur les tables de croissance IEVR
/// EMBARQUÉES (`GrowthTables::load_embedded`, byte-exactes, pas besoin de reparser
/// `growth_table_config` du VFS). `rarity_code` : 0=N, 2=R, 3=SR, 4=SSR, 5=UR, 6=LR, 7=Legend,
/// 20=BASARA (converti en rang de croissance en interne par `calculate_stats`, cf. doc
/// `GrowthParams::chara_rank`). `play_style` = var[5] du noeud `CHARA_PARAM_INFO` (cf.
/// `nie_data::playstyle`), lu directement depuis `raw_variables` (absent du struct `CharaParam`
/// typé — même limitation documentée que `TeamSetup::from_chara_params_and_levels`).
pub fn calculate_character_stats(vfs: &Vfs, chara_param_id: &str, level: u8, rarity_code: u8) -> Result<StatBlockDto, String> {
    let param_json = load_t2b(vfs, |p| base_name(p).starts_with("chara_param_1") && base_name(p).ends_with(".cfg.bin"), "chara_param")?;
    let params = nie_data::chara_param::parse_all_chara_params(&param_json);
    let cp = params
        .iter()
        .find(|cp| cp.chara_param_id.to_hex() == chara_param_id)
        .ok_or_else(|| format!("chara_param_id {chara_param_id} introuvable"))?;

    let play_style = cp.raw_variables.get(5).copied().unwrap_or(0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let growth_params = nie_core::growth::GrowthParams {
        main_position: cp.main_position as u8,
        sub_position: cp.sub_position as u8,
        growth_pattern: cp.growth_pattern as u8,
        chara_rank: rarity_code,
        play_style: play_style as u8,
    };
    let tables = nie_core::growth::GrowthTables::load_embedded();
    Ok(nie_core::growth::calculate_stats(&tables, &growth_params, level).into())
}

/// Décode N'IMPORTE QUEL `.cfg.bin` du VFS (RDBN *ou* T2B, détecté via `nie_formats::cfgbin::
/// is_rdbn`) vers la forme JSON "inagle" (`{"lists":[...]}` ou `{"entries":[...]}`) — couvre
/// TOUS les fichiers de configuration du jeu (plusieurs centaines sous `data/common/gamedata/`
/// et `data/common/text/`), pas seulement les modules `nie-data` câblés individuellement avec un
/// DTO typé (`list_skills` ci-dessus) — cf. demande utilisatrice « niers doit couvrir tout
/// nie.exe ». Générique : réutilise le pont déjà vérifié `nie_explore::bridge`, aucun nouveau
/// parseur par format.
pub fn decode_cfgbin(vfs: &Vfs, path: &str) -> Result<Value, String> {
    let bytes = vfs.read(path).map_err(|e| e.to_string())?;
    if nie_formats::cfgbin::is_rdbn(&bytes) {
        let rdbn = nie_formats::cfgbin::parse(&bytes).map_err(|e| format!("parse RDBN {path} : {e}"))?;
        let lists = nie_formats::cfgbin::read_values(&rdbn, &bytes);
        Ok(nie_explore::bridge::rdbn_to_json(&lists))
    } else {
        let cfg = nie_formats::cfgbin::parse_t2b(&bytes).map_err(|e| format!("parse T2B {path} : {e}"))?;
        Ok(nie_explore::bridge::t2b_to_json(&cfg))
    }
}

// ─── Modules nie-data supplémentaires (§4.1 ROADMAP) ─────────────────────────────────────────
//
// Même patron que `list_items`/`list_auras` : `load_t2b` (bridge déjà testé) → parseur typé de
// `nie-data` → DTO applati. Aucun décodage nouveau, aucune logique dupliquée. Les entiers passent
// en `f64` (`specta` refuse les BigInt vers TypeScript, cf. [`ItemDto::price`]).

/// Boutique du jeu (`shop_config`) — nom localisé joint depuis `shop_text`, plus l'inventaire
/// (identifiants d'objets, résolus en noms quand `item_text` les connaît).
#[derive(Serialize, specta::Type)]
pub struct ShopDto {
    pub shop_id: String,
    pub name: Option<String>,
    pub item_count: u32,
    /// Noms des objets en vente, quand ils sont résolus (sinon leur hash hexadécimal).
    pub items: Vec<String>,
}

/// Liste les boutiques (`shop_config`), noms FR joints depuis `shop_text`, inventaire résolu
/// contre `item_config`+`item_text` — sans quoi la vue n'afficherait que des hachages.
pub fn list_shops(vfs: &Vfs) -> Result<Vec<ShopDto>, String> {
    let config = load_t2b(
        vfs,
        |p| p.contains("/gamedata/") && base_name(p).starts_with("shop_config") && base_name(p).ends_with(".cfg.bin"),
        "shop_config",
    )?;
    let shops = nie_data::shop::parse_shop_config(&config);

    // Textes de boutique : absents de certaines versions — l'absence ne doit pas faire échouer la
    // liste entière (le nom devient simplement `None`).
    let shop_text = load_text(vfs, "shop").unwrap_or_default();

    // Index nom d'objet par identifiant, pour rendre l'inventaire lisible.
    let item_names: HashMap<String, String> = list_items(vfs)
        .unwrap_or_default()
        .into_iter()
        .map(|i| (i.item_id, i.name))
        .collect();

    Ok(shops
        .iter()
        .map(|s| ShopDto {
            shop_id: s.shop_id.to_hex(),
            name: nie_data::shop::resolve_name(s, &shop_text).map(str::to_string),
            item_count: s.item_count() as u32,
            items: s
                .items
                .iter()
                .map(|id| {
                    let hex = id.to_hex();
                    item_names.get(&hex).cloned().unwrap_or(hex)
                })
                .collect(),
        })
        .collect())
}

/// Stade/terrain (`soccer_option_field_info` du `stadium_config`) — chemin d'image et condition
/// de déblocage tels que parsés par `nie_data::stadium`.
#[derive(Serialize, specta::Type)]
pub struct StadiumDto {
    pub field_id: String,
    pub name: String,
    pub image_path: String,
    pub index: f64,
    pub locked: bool,
}

/// Liste les stades (`stadium_config`).
pub fn list_stadiums(vfs: &Vfs) -> Result<Vec<StadiumDto>, String> {
    let config = load_t2b(
        vfs,
        // Le fichier ne s'appelle PAS `stadium_config` : les stades vivent dans la liste
        // `SOCCER_OPTION_FIELD_INFO_*` de `soccer/soccer_game_option.cfg.bin` (cf. l'en-tête de
        // `nie_data::stadium`). L'ancien prédicat ne trouvait rien et l'onglet « Stades » de
        // l'encyclopédie affichait « stadium_config introuvable dans le VFS monté ».
        |p| p.contains("/gamedata/soccer/") && base_name(p).starts_with("soccer_game_option") && base_name(p).ends_with(".cfg.bin"),
        "soccer_game_option (stades)",
    )?;
    Ok(nie_data::stadium::parse_stadium_config(&config)
        .iter()
        .map(|s| StadiumDto {
            field_id: s.field_id_hex(),
            name: s.name.clone(),
            image_path: s.image_path.clone(),
            index: s.index as f64,
            // Une condition non vide = déblocable, donc verrouillé au départ (l'entrée 0 est la
            // seule sans condition dans le dump réel).
            locked: !s.condition.is_empty(),
        })
        .collect())
}

/// Capacité passive (`passive_skill_config`) — nom/description joints depuis `skill_text`
/// (même table que les techniques), portée et type de boost classifiés par `nie_data::passive`.
#[derive(Serialize, specta::Type)]
pub struct PassiveDto {
    pub passive_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub rarity: f64,
    pub scope: String,
    pub boost_type: String,
    pub effect_params: Vec<f64>,
}

/// Liste les capacités passives (`passive_skill_config`).
pub fn list_passives(vfs: &Vfs) -> Result<Vec<PassiveDto>, String> {
    let config = load_t2b(
        vfs,
        |p| {
            p.contains("/gamedata/")
                && base_name(p).starts_with("passive_skill_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "passive_skill_config",
    )?;
    let passives = nie_data::passive::parse_passives(&config);
    let text = load_text(vfs, "skill").unwrap_or_default();

    Ok(passives
        .iter()
        .map(|p| PassiveDto {
            passive_id: p.passive_id.to_hex(),
            name: nie_data::text::find_text(&text, p.name_id).map(str::to_string),
            description: nie_data::text::find_text(&text, p.desc_id).map(str::to_string),
            rarity: p.rarity as f64,
            scope: format!("{:?}", p.scope),
            boost_type: format!("{:?}", p.boost_type),
            effect_params: p.effect_params.clone().unwrap_or_default(),
        })
        .collect())
}

/// Tactique spéciale (`special_tactics_config`) — nom/description localisés, élément, puissance,
/// et nombre d'effets rattachés (résolus par les tranches `REF_EFFECT` de `nie_data`).
#[derive(Serialize, specta::Type)]
pub struct SpecialTacticsDto {
    pub tactics_id: String,
    pub internal_code: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub element: String,
    pub power: f64,
    pub recast_time: f64,
    pub effect_count: u32,
    pub partner_count: u32,
}

/// Liste les tactiques spéciales (`special_tactics_config`).
pub fn list_special_tactics(vfs: &Vfs) -> Result<Vec<SpecialTacticsDto>, String> {
    let config = load_t2b(
        vfs,
        |p| {
            p.contains("/gamedata/")
                && base_name(p).starts_with("special_tactics_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "special_tactics_config",
    )?;
    let cfg = nie_data::special_tactics::parse_special_tactics(&config);
    // Le libellé d'une tactique vit dans `tactics_text` quand il existe, sinon dans `skill_text`
    // (les deux tables partagent le même schéma `TEXT_INFO`). `tactics_text` est ABSENT de
    // `nie_data::text::TEXT_FILES` — il n'a donc pas de type convivial et garde son prédicat en
    // dur, contrairement au repli `skill` qui passe par [`load_text`].
    let mut text = load_t2b(vfs, |p| p.contains("/text/fr/") && base_name(p).starts_with("tactics_text"), "tactics_text fr")
        .map(|j| nie_data::text::parse_text_file(&j))
        .unwrap_or_default();
    if text.is_empty() {
        text = load_text(vfs, "skill").unwrap_or_default();
    }

    Ok(cfg
        .infos
        .iter()
        .enumerate()
        .map(|(i, t)| SpecialTacticsDto {
            tactics_id: t.tactics_id.to_hex(),
            internal_code: t.internal_code.clone(),
            name: nie_data::text::find_text(&text, t.name_text_id).map(str::to_string),
            description: nie_data::text::find_text(&text, t.desc_text_id).map(str::to_string),
            element: t.element_name().to_string(),
            power: t.power as f64,
            recast_time: t.recast_time as f64,
            effect_count: cfg.effects_of(i).len() as u32,
            partner_count: t.partner_ids.len() as u32,
        })
        .collect())
}

// ─── Familles RDBN à noms autoportés (§4.1 ROADMAP, second lot) ──────────────────────────────
//
// Même patron que ci-dessus mais côté RDBN : [`load_rdbn`] → parseur typé `nie-data` → DTO
// applati. Aucune de ces familles ne demande de jointure texte devinée : leurs libellés sont soit
// portés par la donnée elle-même (emblèmes, tricks, activités, chemins de galerie), soit résolus
// par une jointure DÉJÀ validée end-to-end (équipes ↔ `team_text`), soit inexistants dans cette
// version du jeu (formations, uniformes — identifiants bruts affichés tels quels).

/// Écusson d'équipe (`emblem_resource_*`) — une entrée `EMBLEM_RESOURCE_INFO`.
#[derive(Serialize, specta::Type)]
pub struct EmblemDto {
    pub emblem_id: String,
    pub emblem_name: String,
    pub small_file_path: String,
    pub small_tex_name: String,
    pub large_file_path: String,
    pub large_tex_name: String,
    pub base_path: String,
    /// Entrée gabarit : ses chemins portent le jeton `<resourceID>` à substituer par un
    /// `emblem_name` concret (cf. `nie_data::emblems::resolve_resource_id`).
    pub is_template: bool,
}

/// Liste les écussons (`emblem_resource_*`). Le fichier live n'en contient que 2 (le gabarit
/// `default` + `em010001`) : les écussons d'équipe réels sont matérialisés depuis le gabarit,
/// c'est une propriété du jeu, pas un décodage partiel.
pub fn list_emblems(vfs: &Vfs) -> Result<Vec<EmblemDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| p.contains("/gamedata/menu/") && base_name(p).starts_with("emblem_resource") && base_name(p).ends_with(".cfg.bin"),
        "emblem_resource",
    )?;
    Ok(nie_data::emblems::parse_emblem_resources(&config)
        .iter()
        .map(|e| EmblemDto {
            emblem_id: e.emblem_id.clone(),
            emblem_name: e.emblem_name.clone(),
            small_file_path: e.small_file_path.clone(),
            small_tex_name: e.small_tex_name.clone(),
            large_file_path: e.large_file_path.clone(),
            large_tex_name: e.large_tex_name.clone(),
            base_path: e.base_path.clone(),
            is_template: e.is_template,
        })
        .collect())
}

/// Illustration de la galerie (`gallery_config`) — chemins d'image et condition d'ouverture
/// décodée (`open_cond`, blob base64, via `nie_data::unlock_condition`).
#[derive(Serialize, specta::Type)]
pub struct GalleryDto {
    pub gallery_id: String,
    pub img_path: String,
    pub thumb_path: String,
    /// `f64`, cf. [`TrophyDto::category`] — même contrainte `specta`.
    pub need_token_num: f64,
    /// `f64`, cf. [`TrophyDto::category`] — même contrainte `specta`.
    pub flg_no: f64,
    pub unlock_kind: String,
    pub story_episode: Option<u32>,
}

/// Liste les illustrations de la galerie (`gallery_config`, 360 entrées dans le dump réel).
pub fn list_gallery(vfs: &Vfs) -> Result<Vec<GalleryDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| p.contains("/gamedata/gallery/") && base_name(p).starts_with("gallery_config") && base_name(p).ends_with(".cfg.bin"),
        "gallery_config",
    )?;
    use nie_data::unlock_condition::UnlockType as U;
    Ok(nie_data::gallery::parse_gallery_config(&config)
        .entries
        .iter()
        .map(|g| {
            let cond = g.decode_open_cond();
            GalleryDto {
                gallery_id: g.gallery_id.to_hex(),
                img_path: g.img_path.clone(),
                thumb_path: g.thumb_path.clone(),
                need_token_num: g.need_token_num as f64,
                flg_no: g.flg_no as f64,
                unlock_kind: match cond.kind {
                    U::Always => "always",
                    U::Story => "story",
                    U::EventFlag => "eventFlag",
                    U::Composite => "composite",
                }
                .to_string(),
                story_episode: cond.story_episode,
            }
        })
        .collect())
}

/// Feinte/dribble (`trick_config`) — nom interne, catégorie classifiée, événements déclenchés.
#[derive(Serialize, specta::Type)]
pub struct TrickDto {
    pub trick_id: String,
    pub trick_id_name: String,
    pub trick_name: String,
    pub category: String,
    pub event_id_name: String,
    pub fail_event_id_name: String,
    pub has_fail_event: bool,
}

/// Liste les feintes (`skill/trick_config.cfg.bin` — `soccer/trick_config.cfg.bin` est un
/// doublon de même taille, un seul est lu).
pub fn list_tricks(vfs: &Vfs) -> Result<Vec<TrickDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| p.contains("/gamedata/skill/") && base_name(p) == "trick_config.cfg.bin",
        "trick_config",
    )?;
    Ok(nie_data::trick::parse_trick_config(&config)
        .iter()
        .map(|t| TrickDto {
            trick_id: t.trick_id.to_hex(),
            trick_id_name: t.trick_id_name.clone(),
            trick_name: t.trick_name.clone(),
            category: t.category_name(),
            event_id_name: t.event_id_name.clone(),
            fail_event_id_name: t.fail_event_id_name.clone(),
            has_fail_event: t.has_fail_event(),
        })
        .collect())
}

/// Activité/sous-tâche de l'arbre de progression (`activity_config`, format T2B).
#[derive(Serialize, specta::Type)]
pub struct ActivityDto {
    pub id: String,
    pub name: String,
    /// `f64`, cf. [`TrophyDto::category`] — `1` = racine, `5` = sous-tâche (observé).
    pub kind: f64,
    pub parent_id: String,
    pub is_root: bool,
    /// Taille du blob `data` (base64) en caractères. Le blob lui-même n'est PAS décodé (aucune
    /// source de référence sur sa sémantique) : l'exposer brut donnerait une colonne illisible.
    pub data_len: f64,
}

/// Liste les activités (`system/activity_config.cfg.bin`, 13 entrées dans le dump réel). Seule
/// famille **T2B** du lot : passe donc par [`load_t2b`] (JSON indexé), sans quoi `walk_named`
/// matcherait 0 noeud.
pub fn list_activities(vfs: &Vfs) -> Result<Vec<ActivityDto>, String> {
    let config = load_t2b(
        vfs,
        |p| p.contains("/gamedata/system/") && base_name(p) == "activity_config.cfg.bin",
        "activity_config",
    )?;
    Ok(nie_data::activity::parse_activity_config(&config)
        .iter()
        .map(|a| ActivityDto {
            id: a.id.to_hex(),
            name: a.name.clone(),
            kind: a.kind as f64,
            parent_id: a.parent_id.to_hex(),
            is_root: a.is_root(),
            data_len: a.data.len() as f64,
        })
        .collect())
}

/// Saisons de la franchise, dans l'ordre des numéros d'apparition — `nie_data::belong_team::
/// Season` n'expose pas d'itérateur, la liste est donc explicitée ici (libellé affiché ↔ variante).
const SEASONS: [(nie_data::belong_team::Season, &str); 9] = {
    use nie_data::belong_team::Season as S;
    [
        (S::Ie1, "IE1"),
        (S::Ie2, "IE2"),
        (S::Ie3, "IE3"),
        (S::Go1, "GO1"),
        (S::Go2, "GO2"),
        (S::Go3, "GO3"),
        (S::Ares, "Ares"),
        (S::Orion, "Orion"),
        (S::V, "Victory Road"),
    ]
};

/// Équipe d'appartenance (`belong_team_config`) — nom FR joint depuis `team_text`, saisons
/// d'apparition, emblème/maillot Victory Road.
#[derive(Serialize, specta::Type)]
pub struct BelongTeamDto {
    pub team_id: String,
    pub name: Option<String>,
    /// `f64`, cf. [`TrophyDto::category`] — ordre de tri dans le classeur.
    pub binder_order: f64,
    /// Saisons où l'équipe apparaît (`teamNumber_* > 0`), libellés de [`SEASONS`].
    pub seasons: Vec<String>,
    pub emblem_id_v: String,
    pub kit_id_v: String,
}

/// Liste les équipes d'appartenance (`belong_team_config`, 208 lignes), noms FR joints depuis
/// `team_text` — jointure transposée de `nie-game/examples/export_teams.rs` (déjà validée
/// end-to-end sur le vrai jeu), pas réinventée ici.
pub fn list_belong_teams(vfs: &Vfs) -> Result<Vec<BelongTeamDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/character/")
                && base_name(p).starts_with("belong_team_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "belong_team_config",
    )?;
    let team_text = load_text(vfs, "team")?;

    Ok(nie_data::belong_team::parse_belong_team_config(&config)
        .iter()
        .map(|t| BelongTeamDto {
            team_id: t.belong_team_id.to_hex(),
            name: nie_data::belong_team::resolve_team_name(t, &team_text).map(str::to_string),
            binder_order: t.binder_team_order_type as f64,
            seasons: SEASONS.iter().filter(|(s, _)| t.appears_in(*s)).map(|(_, l)| (*l).to_string()).collect(),
            emblem_id_v: t.team_emblem_id_v.to_hex(),
            kit_id_v: t.team_kit_v.to_hex(),
        })
        .collect())
}

/// Formation de terrain (`formation_config`) — puissances offensive/défensive et tranche de
/// placements. Les libellés RESTENT des identifiants bruts : `formation_text.cfg.bin` n'existe
/// pas dans cette version du jeu (vérifié, cf. la note en fin de `nie_data::formation`), donc
/// `noun_id`/`desc_id` ne se résolvent nulle part — afficher un nom ici serait une invention.
#[derive(Serialize, specta::Type)]
pub struct FormationDto {
    pub form_id: String,
    pub noun_id: String,
    pub desc_id: String,
    /// `f64`, cf. [`TrophyDto::category`] — index de départ dans la liste des placements.
    pub placement_offset: f64,
    /// `f64`, cf. [`TrophyDto::category`].
    pub placement_count: f64,
    /// `f64`, cf. [`TrophyDto::category`].
    pub power_offense: f64,
    /// `f64`, cf. [`TrophyDto::category`].
    pub power_defense: f64,
    /// Codes de position (`position_id`) des placements réellement rattachés, dans l'ordre du
    /// terrain — la seule lecture humaine possible d'une formation sans table de texte.
    pub positions: Vec<f64>,
}

/// Liste les formations (`formation_config`, 115 formations / 1073 placements dans le dump réel).
pub fn list_formations(vfs: &Vfs) -> Result<Vec<FormationDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/formation/")
                && base_name(p).starts_with("formation_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "formation_config",
    )?;
    let cfg = nie_data::formation::parse_formation_config(&config);
    Ok(cfg
        .formations
        .iter()
        .map(|f| FormationDto {
            form_id: f.form_id.to_hex(),
            noun_id: f.noun_id.to_hex(),
            desc_id: f.desc_id.to_hex(),
            placement_offset: f.placement_offset as f64,
            placement_count: f.placement_count as f64,
            power_offense: f.power_offense as f64,
            power_defense: f.power_defense as f64,
            positions: cfg.placements_of(f).iter().map(|p| p.position_id as f64).collect(),
        })
        .collect())
}

/// Uniforme (`uniform_config`) — une ligne `UNIFORM_INFO` jointe à sa tranche de modèles
/// (`UniformConfig::resolve_rows`). Comme les formations, l'entrée n'a pas de nom résoluble :
/// `name_id` est un CRC sans table de texte associée dans cette version du jeu.
#[derive(Serialize, specta::Type)]
pub struct UniformDto {
    pub name_id: String,
    /// `f64`, cf. [`TrophyDto::category`] — index de départ dans `m_UniformModelInfoList`.
    pub model_start: f64,
    /// `f64`, cf. [`TrophyDto::category`] — nombre de modèles annoncé par la donnée.
    pub model_count: f64,
    /// Nombre de modèles RÉELLEMENT résolus (la tranche est bornée à la taille de la liste :
    /// un écart avec `model_count` signale une tranche débordante dans la donnée du jeu).
    pub resolved_count: f64,
    /// `typeId` du 1er modèle de la tranche, `None` si la tranche est vide.
    pub type_id: Option<f64>,
    /// CRC du modèle de maillot joueur de champ du 1er modèle de la tranche.
    pub fielder_model_id: Option<String>,
    /// CRC du modèle de maillot gardien du 1er modèle de la tranche.
    pub keeper_model_id: Option<String>,
}

/// Liste les uniformes (`character/uniform_config_*`, 627 uniformes / 1247 modèles dans le dump
/// réel) — `item/uniform_config_0.00.00.cfg.bin` (747 octets) est une autre table, exclue par le
/// filtre de dossier.
pub fn list_uniforms(vfs: &Vfs) -> Result<Vec<UniformDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/character/")
                && base_name(p).starts_with("uniform_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "uniform_config",
    )?;
    let cfg = nie_data::uniform::parse_uniform_config(&config);
    Ok(cfg
        .resolve_rows()
        .iter()
        .map(|r| UniformDto {
            name_id: r.name_id.to_hex(),
            model_start: r.model_start as f64,
            model_count: r.model_count as f64,
            resolved_count: r.models.len() as f64,
            type_id: r.type_id.map(|t| t as f64),
            fielder_model_id: r.models.first().map(|m| m.uniform_fielder_model_id_crc.to_hex()),
            keeper_model_id: r.models.first().map(|m| m.uniform_keeper_model_id_crc.to_hex()),
        })
        .collect())
}

// ─── Familles au-delà du wiki azalee (§4.3 ROADMAP) ──────────────────────────────────────────
//
// Même patron que `list_shops`/`list_uniforms` : `load_t2b`/`load_rdbn` (ponts déjà testés) →
// parseur typé de `nie-data` → DTO applati, entiers en `f64` (`specta` refuse les BigInt).
// Ces huit familles n'ont AUCUN équivalent dans l'encyclopédie du wiki : personnages complets
// (avec leurs techniques apprises), équipes adverses, vidéos, bande-son, dictionnaire, butin,
// capsules, courbe d'expérience.

/// Personnage jouable/PNJ — `chara_param` joint à `chara_base` (identité), `chara_text` (nom),
/// `chara_description_text` (description), `chara_series` (série d'origine), `belong_team_config`
/// (équipe) et `skill_config` (techniques apprises). C'est la fiche complète, pas le sélecteur
/// réduit de [`CharaPickerDto`] (qui reste séparé : le calculateur de stats n'a besoin que du nom
/// et des positions, et le recharger complet le ralentirait sans raison).
#[derive(Serialize, specta::Type)]
pub struct CharaDto {
    pub chara_param_id: String,
    pub chara_base_id: String,
    /// Code interne (`c01000010`) — ouvre l'éditeur de propriétés (modèle, textures, sons).
    pub internal_code: String,
    pub name: String,
    pub description: Option<String>,
    /// `1` = masculin, `2` = féminin dans la donnée ; rendu tel quel, non interprété.
    pub gender: f64,
    pub element: String,
    pub main_position: String,
    pub sub_position: String,
    /// `f64`, cf. [`ItemDto::price`] — pattern de croissance (entrée des tables `growth`).
    pub growth_pattern: f64,
    pub series: Option<String>,
    pub team: Option<String>,
    /// Techniques apprises, `« niveau — nom »` quand le nom se résout (sinon le hash).
    pub skills: Vec<String>,
    /// `f64`, cf. [`ItemDto::price`].
    pub skill_count: f64,
    /// Stats au **niveau 99, rang de rareté UR** (code 5), calculées par
    /// `nie_core::growth::calculate_stats` sur les tables embarquées. Le rang N'EST PAS dans
    /// `chara_param` (il dépend de l'exemplaire possédé) : c'est une base de COMPARAISON commune,
    /// affichée comme telle, pas la fiche d'un personnage précis — pour un couple (niveau,
    /// rareté) choisi, c'est `game_data_calculate_stats` qui répond.
    pub stats: StatBlockDto,
}

/// Liste TOUS les personnages du jeu avec leur fiche complète. Un `chara_base_id` peut porter
/// plusieurs `chara_param` (variantes de tenue) : chacune est une ligne, différenciée par son
/// `chara_param_id`. Les entrées sans nom résolu sont écartées (même convention que les autres
/// `list_*` : pas de ligne fantôme sans identité lisible).
pub fn list_charas(vfs: &Vfs) -> Result<Vec<CharaDto>, String> {
    let param_json = load_t2b(
        vfs,
        |p| base_name(p).starts_with("chara_param_1") && base_name(p).ends_with(".cfg.bin"),
        "chara_param",
    )?;
    let params = nie_data::chara_param::parse_all_chara_params(&param_json);

    let base_json = load_t2b(
        vfs,
        |p| p.contains("/character/") && base_name(p).starts_with("chara_base_1") && base_name(p).ends_with(".cfg.bin"),
        "chara_base",
    )?;
    let bases = nie_data::chara_base::parse_all_chara_base(&base_json);

    let nouns = nie_data::chara_text::parse_all_nouns(&load_text_json(vfs, "chara")?);
    // Sources d'enrichissement TOUTES facultatives : une table absente retire une colonne, elle
    // ne fait jamais échouer la liste entière.
    let descriptions = load_text_json(vfs, "chara_description")
        .map(|j| nie_data::chara_description::parse_chara_descriptions(&j))
        .unwrap_or_default();
    let series = load_rdbn(
        vfs,
        |p| p.contains("/gamedata/character/") && base_name(p).starts_with("chara_series_config") && base_name(p).ends_with(".cfg.bin"),
        "chara_series_config",
    )
    .map(|j| nie_data::chara_series::parse_chara_series_config(&j))
    .unwrap_or_default();

    // Équipes : hash → nom FR, via la même jointure que `list_belong_teams`.
    let teams: HashMap<String, String> = match (
        load_rdbn(
            vfs,
            |p| p.contains("/gamedata/character/") && base_name(p).starts_with("belong_team_config") && base_name(p).ends_with(".cfg.bin"),
            "belong_team_config",
        ),
        load_text(vfs, "team"),
    ) {
        (Ok(cfg), Ok(txt)) => nie_data::belong_team::parse_belong_team_config(&cfg)
            .iter()
            .filter_map(|t| {
                nie_data::belong_team::resolve_team_name(t, &txt).map(|n| (t.belong_team_id.to_hex(), n.to_string()))
            })
            .collect(),
        _ => HashMap::new(),
    };

    // Techniques : hash → nom FR (ou code interne à défaut de table de texte).
    // Tables de croissance EMBARQUÉES (byte-exactes, aucun fichier du VFS à reparser) : chargées
    // une fois pour les ~6 000 personnages, le calcul par ligne n'est qu'une recherche de table.
    let tables = nie_core::growth::GrowthTables::load_embedded();

    let skill_names: HashMap<String, String> = list_skills(vfs)
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.skill_id, s.name.unwrap_or(s.skill_id_str)))
        .collect();

    Ok(params
        .iter()
        .filter_map(|cp| {
            let base = nie_data::chara_base::find_by_chara_id(&bases, cp.chara_base_id)?;
            let first = nie_data::chara_base::resolve_first_name(base, &nouns)?;
            let name = match nie_data::chara_base::resolve_last_name(base, &nouns) {
                Some(l) => format!("{first} {l}"),
                None => first.to_string(),
            };
            Some(CharaDto {
                chara_param_id: cp.chara_param_id.to_hex(),
                chara_base_id: cp.chara_base_id.to_hex(),
                internal_code: base.internal_code.clone(),
                name,
                description: nie_data::chara_base::resolve_description(base, &descriptions).map(str::to_string),
                gender: base.gender as f64,
                element: nie_data::chara_param::element_id_to_names(cp.element)
                    .map_or_else(|| format!("? ({})", cp.element), |(fr, _, _)| fr.to_string()),
                main_position: nie_data::chara_param::position_code_owned(cp.main_position).unwrap_or_else(|| "?".to_string()),
                sub_position: nie_data::chara_param::position_code_owned(cp.sub_position).unwrap_or_else(|| "—".to_string()),
                growth_pattern: cp.growth_pattern as f64,
                series: nie_data::chara_base::resolve_series_name_fr(base, &series).map(str::to_string),
                team: base.belong_team_id.and_then(|t| teams.get(&t.to_hex()).cloned()),
                skills: cp
                    .skills
                    .iter()
                    .map(|s| {
                        let hex = s.skill_id.to_hex();
                        let nom = skill_names.get(&hex).cloned().unwrap_or(hex);
                        format!("Nv {} — {nom}", s.learn_level)
                    })
                    .collect(),
                skill_count: cp.skills.len() as f64,
                stats: {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let p = nie_core::growth::GrowthParams {
                        main_position: cp.main_position as u8,
                        sub_position: cp.sub_position as u8,
                        growth_pattern: cp.growth_pattern as u8,
                        chara_rank: 5,
                        play_style: cp.raw_variables.get(5).copied().unwrap_or(0) as u8,
                    };
                    nie_core::growth::calculate_stats(&tables, &p, 99).into()
                },
            })
        })
        .collect())
}

/// Équipe adverse rencontrable (`opponent_team_config`) — nom d'équipe résolu via
/// `belong_team_config` + `team_text`, difficulté, condition d'ouverture.
#[derive(Serialize, specta::Type)]
pub struct OpponentTeamDto {
    pub opponent_id: String,
    pub team_id: String,
    pub team_name: Option<String>,
    /// `f64`, cf. [`ItemDto::price`].
    pub team_type: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub difficulty_type: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub flag_no: f64,
    /// Condition d'ouverture, telle quelle (mini-langage du jeu, non interprété ici).
    pub open_cond: String,
    pub formation_cond: String,
    pub bg_texture_name: String,
    pub game_id: String,
}

/// Liste les équipes adverses (`team/opponent_team_config_*`).
pub fn list_opponent_teams(vfs: &Vfs) -> Result<Vec<OpponentTeamDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/team/")
                && base_name(p).starts_with("opponent_team_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "opponent_team_config",
    )?;
    let teams: HashMap<String, String> = match (
        load_rdbn(
            vfs,
            |p| p.contains("/gamedata/character/") && base_name(p).starts_with("belong_team_config") && base_name(p).ends_with(".cfg.bin"),
            "belong_team_config",
        ),
        load_text(vfs, "team"),
    ) {
        (Ok(cfg), Ok(txt)) => nie_data::belong_team::parse_belong_team_config(&cfg)
            .iter()
            .filter_map(|t| {
                nie_data::belong_team::resolve_team_name(t, &txt).map(|n| (t.belong_team_id.to_hex(), n.to_string()))
            })
            .collect(),
        _ => HashMap::new(),
    };

    Ok(nie_data::opponent_team::parse_opponent_team_config(&config)
        .opponents
        .iter()
        .map(|o| OpponentTeamDto {
            opponent_id: o.opponent_id.to_hex(),
            team_id: o.team_id.to_hex(),
            team_name: teams.get(&o.team_id.to_hex()).cloned(),
            team_type: o.team_type as f64,
            difficulty_type: o.difficulty_type as f64,
            flag_no: o.flag_no as f64,
            open_cond: o.open_cond.clone(),
            formation_cond: o.formation_cond.clone(),
            bg_texture_name: o.bg_texture_name.clone(),
            game_id: o.game_id.to_hex(),
        })
        .collect())
}

/// Vidéo du jeu (`movie_playing_config`) — chemin USM, BGM, sous-titres.
#[derive(Serialize, specta::Type)]
pub struct MovieDto {
    pub movie_id: String,
    pub movie_path: String,
    pub bgm_name: String,
    /// `true` si la vidéo porte une table de sous-titres (≠ [`nie_data::movie::NONE_SENTINEL`]).
    pub has_subtitles: bool,
    pub subtitle_text_path: String,
    pub staffroll_data_name: String,
    pub fade_in: f64,
    pub fade_out: f64,
}

/// Liste les vidéos (`movie/movie_playing_config_*`).
pub fn list_movies(vfs: &Vfs) -> Result<Vec<MovieDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/movie/")
                && base_name(p).starts_with("movie_playing_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "movie_playing_config",
    )?;
    Ok(nie_data::movie::parse_movie_playing_config(&config)
        .playing_infos
        .iter()
        .map(|m| MovieDto {
            movie_id: m.movie_id.to_hex(),
            movie_path: m.movie_path.clone(),
            bgm_name: m.bgm_name.to_hex(),
            has_subtitles: m.subtitle_text_path != nie_data::movie::NONE_SENTINEL && !m.subtitle_text_path.is_empty(),
            subtitle_text_path: m.subtitle_text_path.clone(),
            staffroll_data_name: m.staffroll_data_name.clone(),
            fade_in: f64::from(m.fede_in_time),
            fade_out: f64::from(m.fede_out_time),
        })
        .collect())
}

/// Piste de la bande-son (`music_app_config`) — nom FR joint depuis `music_name_text`.
#[derive(Serialize, specta::Type)]
pub struct MusicDto {
    pub entry_id: String,
    pub music_id: String,
    pub name: Option<String>,
    /// `f64`, cf. [`ItemDto::price`].
    pub app_category: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub track_no: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub variant: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub volume: f64,
    /// `f64`, cf. [`ItemDto::price`] — index de tri 1-basé du lecteur du jeu.
    pub sort_index: f64,
    /// `true` si la piste porte un chemin audio (105/108 dans le dump de référence).
    pub has_path: bool,
}

/// Liste les pistes du lecteur de musique (`music_app/music_app_config.cfg.bin`).
pub fn list_musics(vfs: &Vfs) -> Result<Vec<MusicDto>, String> {
    let config = load_t2b(
        vfs,
        |p| p.contains("/gamedata/music_app/") && base_name(p).starts_with("music_app_config") && base_name(p).ends_with(".cfg.bin"),
        "music_app_config",
    )?;
    // Table facultative : sans elle, `name` reste `None` — jamais un nom inventé.
    let noms = load_text(vfs, "music").unwrap_or_default();
    Ok(nie_data::music_app::parse_music_app_config(&config)
        .items
        .iter()
        .map(|m| MusicDto {
            entry_id: m.entry_id.to_hex(),
            music_id: m.music_id.to_hex(),
            name: nie_data::text::find_text(&noms, m.music_id)
                .or_else(|| nie_data::text::find_text(&noms, m.entry_id))
                .map(str::to_string),
            app_category: m.app_category as f64,
            track_no: m.track_no as f64,
            variant: m.variant as f64,
            volume: m.volume as f64,
            sort_index: m.sort_index as f64,
            has_path: m.has_path(),
        })
        .collect())
}

/// Entrée du dictionnaire in-game (`dictionary_config`) — le « bestiaire » des personnages
/// observables, avec leur habitat résolu (`map_text`).
#[derive(Serialize, specta::Type)]
pub struct DictionaryDto {
    pub chara_id: String,
    /// Nom du personnage quand `chara_base`+`chara_text` le résolvent.
    pub name: Option<String>,
    pub habitat: Option<String>,
    pub habitat_file: Option<String>,
    /// `f64`, cf. [`ItemDto::price`] — numéro d'affichage dans le dictionnaire.
    pub view_dict_no: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub category: f64,
    /// `f64`, cf. [`ItemDto::price`].
    pub sub_category: f64,
    /// `true` si l'entrée est affrontable.
    pub is_battle: bool,
    /// Nombre d'actions d'observation rattachées.
    pub observation_count: f64,
    pub medal_id: String,
    pub weapon_item_id: String,
}

/// Liste les entrées du dictionnaire (`dictionary/dictionary_config_*`).
pub fn list_dictionary(vfs: &Vfs) -> Result<Vec<DictionaryDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/dictionary/")
                && base_name(p).starts_with("dictionary_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "dictionary_config",
    )?;
    let cfg = nie_data::dictionary::parse_dictionary_config(&config);

    // Noms des personnages : mêmes jointures que `list_charas`, facultatives.
    let noms: HashMap<String, String> = match (
        load_t2b(
            vfs,
            |p| p.contains("/character/") && base_name(p).starts_with("chara_base_1") && base_name(p).ends_with(".cfg.bin"),
            "chara_base",
        ),
        load_text_json(vfs, "chara"),
    ) {
        (Ok(bj), Ok(tj)) => {
            let nouns = nie_data::chara_text::parse_all_nouns(&tj);
            nie_data::chara_base::parse_all_chara_base(&bj)
                .iter()
                .filter_map(|b| {
                    let first = nie_data::chara_base::resolve_first_name(b, &nouns)?;
                    let nom = match nie_data::chara_base::resolve_last_name(b, &nouns) {
                        Some(l) => format!("{first} {l}"),
                        None => first.to_string(),
                    };
                    Some((b.chara_id.to_hex(), nom))
                })
                .collect()
        }
        _ => HashMap::new(),
    };
    let cartes = load_text(vfs, "map").unwrap_or_default();

    Ok(cfg
        .params
        .iter()
        .map(|p| {
            let habitat = cfg.habitats.iter().find(|h| h.habitat_id == p.habitat_id);
            DictionaryDto {
                chara_id: p.chara_id.to_hex(),
                name: noms.get(&p.chara_id.to_hex()).cloned(),
                habitat: habitat.and_then(|h| nie_data::text::find_text(&cartes, h.map_name_id)).map(str::to_string),
                habitat_file: habitat.map(|h| h.file_name.clone()),
                view_dict_no: p.view_dict_no as f64,
                category: p.category as f64,
                sub_category: p.sub_category as f64,
                is_battle: p.is_buttle,
                observation_count: p.observation_count as f64,
                medal_id: p.medal_id.to_hex(),
                weapon_item_id: p.weapon_item_id.to_hex(),
            }
        })
        .collect())
}

/// Palier de la courbe d'expérience (`chara_exp_table_config`).
#[derive(Serialize, specta::Type)]
pub struct ExpLevelDto {
    /// `f64`, cf. [`ItemDto::price`].
    pub level: f64,
    /// EXP nécessaire pour ce niveau depuis le précédent.
    pub need_exp: f64,
    /// EXP cumulée depuis le niveau 1 — la valeur que le jeu affiche, calculée ici.
    pub cumulative: f64,
}

/// Courbe d'expérience des personnages (`character/chara_exp_table_config_*`).
pub fn list_exp_table(vfs: &Vfs) -> Result<Vec<ExpLevelDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/character/")
                && base_name(p).starts_with("chara_exp_table_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "chara_exp_table_config",
    )?;
    let table = nie_data::exp::parse_exp_table(&config);
    let mut cumul = 0.0;
    Ok(table
        .exp_table
        .iter()
        .map(|e| {
            cumul += e.need_exp as f64;
            ExpLevelDto { level: e.level as f64, need_exp: e.need_exp as f64, cumulative: cumul }
        })
        .collect())
}

/// Ligne de butin (`soccer_drop_config`) — un personnage-esprit tirable, avec son poids et sa
/// condition d'apparition.
#[derive(Serialize, specta::Type)]
pub struct DropDto {
    pub chara_id: String,
    pub name: Option<String>,
    /// `f64`, cf. [`ItemDto::price`] — poids de tirage (probabilité relative dans sa table).
    pub weight: f64,
    /// Part du poids dans le total de la table, en pourcentage — la seule lecture humaine d'un
    /// poids brut.
    pub share_pct: f64,
    /// Condition d'exécution telle quelle (mini-langage du jeu, non interprété).
    pub run_cond: String,
}

/// Liste les personnages tirables au butin (`soccer/soccer_drop_config_*`, liste
/// `spirit_table_data`), noms résolus quand `chara_base`+`chara_text` les connaissent.
pub fn list_drops(vfs: &Vfs) -> Result<Vec<DropDto>, String> {
    let config = load_rdbn(
        vfs,
        |p| {
            p.contains("/gamedata/soccer/")
                && base_name(p).starts_with("soccer_drop_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "soccer_drop_config",
    )?;
    let cfg = nie_data::soccer_drop::parse_soccer_drop_config(&config);

    let noms: HashMap<String, String> = match (
        load_t2b(
            vfs,
            |p| p.contains("/character/") && base_name(p).starts_with("chara_base_1") && base_name(p).ends_with(".cfg.bin"),
            "chara_base",
        ),
        load_text_json(vfs, "chara"),
    ) {
        (Ok(bj), Ok(tj)) => {
            let nouns = nie_data::chara_text::parse_all_nouns(&tj);
            nie_data::chara_base::parse_all_chara_base(&bj)
                .iter()
                .filter_map(|b| {
                    let first = nie_data::chara_base::resolve_first_name(b, &nouns)?;
                    let nom = match nie_data::chara_base::resolve_last_name(b, &nouns) {
                        Some(l) => format!("{first} {l}"),
                        None => first.to_string(),
                    };
                    Some((b.chara_id.to_hex(), nom))
                })
                .collect()
        }
        _ => HashMap::new(),
    };

    let total: f64 = cfg.spirit_table_data.iter().map(|s| s.weight as f64).sum();
    Ok(cfg
        .spirit_table_data
        .iter()
        .map(|s| DropDto {
            chara_id: s.chara_id.to_hex(),
            name: noms.get(&s.chara_id.to_hex()).cloned(),
            weight: s.weight as f64,
            share_pct: if total > 0.0 { s.weight as f64 * 100.0 / total } else { 0.0 },
            run_cond: s.run_cond.clone(),
        })
        .collect())
}

/// Table de gain de capsules (`capsule_config`) — une ligne par rang, avec son taux.
#[derive(Serialize, specta::Type)]
pub struct CapsuleRateDto {
    pub table_id: String,
    /// `f64`, cf. [`ItemDto::price`] — rang de rareté du lot.
    pub rank: f64,
    /// Taux brut de la donnée (base 10 000 dans le dump de référence).
    pub rate: f64,
    /// Part du taux dans le total de sa table, en pourcentage.
    pub share_pct: f64,
}

/// Liste les taux de tirage des capsules (`capsule/capsule_config_*`, liste `lot_rank_rates`).
pub fn list_capsule_rates(vfs: &Vfs) -> Result<Vec<CapsuleRateDto>, String> {
    let config = load_t2b(
        vfs,
        |p| {
            p.contains("/gamedata/capsule/")
                && base_name(p).starts_with("capsule_config")
                && base_name(p).ends_with(".cfg.bin")
        },
        "capsule_config",
    )?;
    let db = nie_data::capsule::parse_capsule_database(&config);
    let mut out = Vec::new();
    for table in &db.lot_rank_rates {
        let total: f64 = table.rates.iter().map(|r| r.rate as f64).sum();
        for r in &table.rates {
            out.push(CapsuleRateDto {
                table_id: table.id.to_hex(),
                rank: r.rank as f64,
                rate: r.rate as f64,
                share_pct: if total > 0.0 { r.rate as f64 * 100.0 / total } else { 0.0 },
            });
        }
    }
    Ok(out)
}


// ─── Index multilingue des noms (traducteur) ─────────────────────────────────────────────────

/// Les langues du jeu, dans l'ordre d'affichage. Relevé sur l'installation Steam :
/// `data/common/text/` porte `ja`, `fr`, `en`, `de`, `es`, `it`, `pt`, `zh_hans`, `zh_hant`
/// (plus `common`, `event`, `map`, qui ne sont pas des langues).
pub const LANGUES: [&str; 9] = ["fr", "en", "ja", "de", "es", "it", "pt", "zh_hans", "zh_hant"];

/// Un nom dans une langue.
#[derive(Serialize, specta::Type)]
pub struct NomLangueDto {
    /// Code de langue (`fr`, `ja`, `zh_hans`…).
    pub langue: String,
    pub nom: String,
}

/// Une entité du jeu et ses noms dans toutes les langues où elle en a un.
#[derive(Serialize, specta::Type)]
pub struct NomsDto {
    /// Famille : `chara`, `skill` ou `item`.
    pub kind: String,
    /// Code interne (`c01000010`, `whs00340`) ou hash à défaut — la clé qui ouvre l'éditeur.
    pub code: String,
    pub noms: Vec<NomLangueDto>,
}

/// Index multilingue des noms, construit DIRECTEMENT depuis le jeu monté.
///
/// C'est ce qui rend le traducteur utilisable sans le miroir du wiki : celui-ci n'est ni requis
/// ni toujours présent (vérifié à l'écran — « Index indisponible : unable to open database
/// file », 0 nom indexé, sur une machine où le jeu était pourtant monté). Et il va plus loin que
/// le site, qui n'expose que FR/EN/JA/romaji : les **neuf** langues de `data/common/text/` sont
/// lues, y compris l'allemand, l'espagnol, l'italien, le portugais et les deux chinois.
///
/// Trois familles couvertes — personnages, techniques, objets — celles dont le nom est la clé
/// d'entrée d'une recherche. Une langue absente ou une table illisible est simplement sautée :
/// l'index rend ce qu'il a, jamais une erreur pour toute la commande.
pub fn list_noms(vfs: &Vfs) -> Result<Vec<NomsDto>, String> {
    use std::collections::BTreeMap;

    // code interne → (kind, noms par langue). `BTreeMap` : sortie stable d'un appel à l'autre.
    let mut index: BTreeMap<String, (String, Vec<NomLangueDto>)> = BTreeMap::new();

    // — Personnages : `chara_base` donne le code interne, `chara_text` le nom par langue.
    let bases = load_t2b(
        vfs,
        |p| p.contains("/character/") && base_name(p).starts_with("chara_base_1") && base_name(p).ends_with(".cfg.bin"),
        "chara_base",
    )
    .map(|j| nie_data::chara_base::parse_all_chara_base(&j))
    .unwrap_or_default();

    // — Techniques : `skill_config` donne le code interne (`whs00340`), `skill_text` le nom.
    let skills = load_rdbn(
        vfs,
        |p| p.contains("/skill/") && base_name(p).starts_with("skill_config") && base_name(p).ends_with(".cfg.bin"),
        "skill_config",
    )
    .map(|j| nie_data::skill::parse_skill_config(&j))
    .unwrap_or_default();

    // — Objets : `item_config` donne le code interne, `item_text` le nom.
    let items = load_t2b(
        vfs,
        |p| p.contains("/gamedata/item/") && base_name(p).starts_with("item_config") && base_name(p).ends_with(".cfg.bin"),
        "item_config",
    )
    .map(|j| nie_data::item::parse_all_items(&j))
    .unwrap_or_default();

    for langue in LANGUES {
        if let Ok(j) = load_text_json_lang(vfs, "chara", langue) {
            let nouns = nie_data::chara_text::parse_all_nouns(&j);
            for b in &bases {
                let Some(first) = nie_data::chara_base::resolve_first_name(b, &nouns) else { continue };
                let nom = match nie_data::chara_base::resolve_last_name(b, &nouns) {
                    Some(l) => format!("{first} {l}"),
                    None => first.to_string(),
                };
                let cle = if b.internal_code.is_empty() { b.chara_id.to_hex() } else { b.internal_code.clone() };
                index
                    .entry(cle)
                    .or_insert_with(|| ("chara".to_string(), Vec::new()))
                    .1
                    .push(NomLangueDto { langue: langue.to_string(), nom });
            }
        }

        if let Ok(j) = load_text_json_lang(vfs, "skill", langue) {
            let maps = nie_data::skill::parse_skill_text(&j);
            for s in &skills {
                let Some(nom) = nie_data::skill::join_skill_text(s, &maps).name else { continue };
                index
                    .entry(s.skill_id_str.clone())
                    .or_insert_with(|| ("skill".to_string(), Vec::new()))
                    .1
                    .push(NomLangueDto { langue: langue.to_string(), nom });
            }
        }

        if let Ok(j) = load_text_json_lang(vfs, "item", langue) {
            let textes = nie_data::text::parse_text_file(&j);
            for it in &items {
                let Some(nom) = nie_data::item::resolve_name(it, &textes) else { continue };
                let cle = it.internal_code.clone().unwrap_or_else(|| it.item_id.to_hex());
                index
                    .entry(cle)
                    .or_insert_with(|| ("item".to_string(), Vec::new()))
                    .1
                    .push(NomLangueDto { langue: langue.to_string(), nom: nom.to_string() });
            }
        }
    }

    Ok(index
        .into_iter()
        .map(|(code, (kind, noms))| NomsDto { kind, code, noms })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie le pont bytes→JSON→`nie-data` de bout en bout sur le VRAI jeu, pas juste la
    /// compilation. Valeur de référence issue du doc-comment de `nie_data::skill`
    /// (« Première valeur vérifiée (whs00010, « Trampoline du tonnerre »)… power 70→440,
    /// element=1 (Vent), category=1 (Tir) »).
    #[test]
    fn list_skills_sur_le_vrai_jeu() {
        let dir = nie_formats::vfs::resolve_game_dir().to_string_lossy().into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip list_skills_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let skills = list_skills(&vfs).expect("list_skills");
        assert!(skills.len() > 1000, "attendu > 1000 techniques (2627 dans le dump v4 de référence), obtenu {}", skills.len());

        let whs00010 = skills.iter().find(|s| s.skill_id_str == "whs00010").expect("whs00010 introuvable");
        assert_eq!(whs00010.power_min, 70);
        assert_eq!(whs00010.power_max, 440);
        assert_eq!(whs00010.element, "Vent");
        assert_eq!(whs00010.category, "Tir");
        eprintln!(
            "whs00010 : {} — {:?}",
            whs00010.skill_id_str,
            whs00010.name
        );
    }

    /// Charge le VFS réel, ou `None` (skip, pas d'échec) si le jeu est absent de ce poste —
    /// factorisé pour les 4 tests `list_*_sur_le_vrai_jeu` ci-dessous, même convention que
    /// `list_skills_sur_le_vrai_jeu`.
    fn real_vfs_or_skip(test_name: &str) -> Option<Vfs> {
        let dir = nie_formats::vfs::resolve_game_dir().to_string_lossy().into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip {test_name} : jeu absent");
            return None;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");
        Some(vfs)
    }

    /// Cf. `nie-game/examples/export_items.rs` (référence déjà validée end-to-end,
    /// `docs/PLAN.md` B′3 : « 1767 noms / 324 descriptions fr »).
    #[test]
    fn list_items_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_items_sur_le_vrai_jeu") else { return };
        let items = list_items(&vfs).expect("list_items");
        assert!(items.len() > 1000, "attendu > 1000 objets résolus (1767 de référence), obtenu {}", items.len());
        eprintln!("{} objets résolus, ex. : {}", items.len(), items[0].name);
    }

    /// Cf. `nie-game/examples/export_auras.rs` (référence déjà validée end-to-end,
    /// `docs/PLAN.md` C2 : « 443/443 auras résolues »).
    #[test]
    fn list_auras_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_auras_sur_le_vrai_jeu") else { return };
        let auras = list_auras(&vfs).expect("list_auras");
        assert!(auras.len() > 400, "attendu > 400 Avatar/Keshin résolus (443 de référence), obtenu {}", auras.len());
        eprintln!("{} Avatar/Keshin résolus, ex. : {}", auras.len(), auras[0].name);
    }

    /// Cf. `nie-game/examples/export_trophies.rs` (référence déjà validée end-to-end,
    /// `docs/PLAN.md` C2 : « 231/231 noms résolus »).
    #[test]
    fn list_trophies_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_trophies_sur_le_vrai_jeu") else { return };
        let trophies = list_trophies(&vfs).expect("list_trophies");
        assert!(trophies.len() > 200, "attendu > 200 succès résolus (231 de référence), obtenu {}", trophies.len());
        eprintln!("{} succès résolus, ex. : {}", trophies.len(), trophies[0].name);
    }

    /// Cf. `nie-game/examples/export_quests.rs` (référence déjà validée end-to-end,
    /// `docs/PLAN.md` C2 : « 182/182 titres fr »).
    #[test]
    fn list_quests_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_quests_sur_le_vrai_jeu") else { return };
        let quests = list_quests(&vfs).expect("list_quests");
        assert!(quests.len() > 150, "attendu > 150 quêtes résolues (182 de référence), obtenu {}", quests.len());
        eprintln!("{} quêtes résolues, ex. : {}", quests.len(), quests[0].title);
    }

    /// Cf. `nie-game/examples/export_characters.rs` (référence déjà validée end-to-end,
    /// `docs/PLAN.md` C2 : « 6470/7223 prénoms résolus »). Puis calcule les stats du
    /// premier personnage de la liste à Lv50/rang N — vérifie juste que le calcul ABOUTIT à des
    /// valeurs plausibles (pas un golden byte-exact : ça dépend du perso pris en tête de liste,
    /// non déterministe entre versions du jeu — `calculate_stats` lui-même EST déjà golden-testé
    /// dans `nie-core`).
    #[test]
    fn chara_picker_et_calcul_stats_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("chara_picker_et_calcul_stats_sur_le_vrai_jeu") else { return };
        let roster = list_chara_picker(&vfs).expect("list_chara_picker");
        assert!(roster.len() > 5000, "attendu > 5000 personnages résolus (6470 de référence), obtenu {}", roster.len());
        eprintln!("{} personnages résolus, ex. : {} ({})", roster.len(), roster[0].name, roster[0].main_position);

        let stats = calculate_character_stats(&vfs, &roster[0].chara_param_id, 50, 0).expect("calculate_character_stats");
        assert!(stats.total > 0, "stats nulles pour {} — calcul cassé", roster[0].name);
        eprintln!("{} Lv50 rang N : total={} (Kc{} Cr{} Tc{} Pr{} Ps{} Ag{} It{})", roster[0].name, stats.total, stats.kc, stats.cr, stats.tc, stats.pr, stats.ps, stats.ag, stats.it);
    }

    /// Écussons : le fichier live (`emblem_resource_0.04.18`, 1336 octets) n'a que **2** lignes
    /// — le gabarit `default` (chemins à jeton `<resourceID>`) et `em010001`. C'est la vérité
    /// terrain relevée dans `nie_data::emblems`, pas un décodage incomplet.
    #[test]
    fn list_emblems_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_emblems_sur_le_vrai_jeu") else { return };
        let emblems = list_emblems(&vfs).expect("list_emblems");
        assert_eq!(emblems.len(), 2, "attendu 2 écussons (gabarit + em010001), obtenu {}", emblems.len());
        assert_eq!(emblems.iter().filter(|e| e.is_template).count(), 1, "attendu 1 gabarit — sans lui la substitution <resourceID> est impossible");
        assert_eq!(emblems[0].base_path, "#/menu/");
        assert!(emblems.iter().any(|e| e.emblem_name == "em010001"));
    }

    /// Galerie : 360 entrées dans `gallery_config_1.03.71.00` (compte du doc-comment de
    /// `nie_data::gallery`), la 1re débloquée par la progression de l'histoire (épisode 1).
    #[test]
    fn list_gallery_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_gallery_sur_le_vrai_jeu") else { return };
        let gallery = list_gallery(&vfs).expect("list_gallery");
        assert_eq!(gallery.len(), 360, "attendu 360 illustrations, obtenu {}", gallery.len());
        assert_eq!(gallery[0].img_path, "img_story_ev01_main_0010");
        // La condition d'ouverture est un blob base64 : la décoder est le seul moyen de vérifier
        // que `unlock_condition` est branché et pas court-circuité.
        assert_eq!(gallery[0].unlock_kind, "story");
        assert_eq!(gallery[0].story_episode, Some(1));
    }

    /// Feintes : 9 lignes `m_trickInfoList` dans `skill/trick_config.cfg.bin` (vérifié par
    /// `niers vfs cat` sur le jeu monté).
    #[test]
    fn list_tricks_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_tricks_sur_le_vrai_jeu") else { return };
        let tricks = list_tricks(&vfs).expect("list_tricks");
        assert_eq!(tricks.len(), 9, "attendu 9 feintes, obtenu {}", tricks.len());
        assert!(tricks.iter().all(|t| !t.trick_id_name.is_empty()), "une feinte sans nom interne — parsing RDBN décalé");
        assert_eq!(tricks[0].trick_id_name, "whs0010");
        assert_eq!(tricks[0].category, "Tir");
    }

    /// Activités : 13 entrées `ACTIVITY_CONFIG_*` (compte du doc-comment de `nie_data::activity`),
    /// dont la racine `StoryMode`. Seule famille T2B du lot — vérifie donc AUSSI que le JSON
    /// indexé de `load_t2b` fait bien matcher `walk_named` (0 entrée = le bug des frères homonymes).
    #[test]
    fn list_activities_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_activities_sur_le_vrai_jeu") else { return };
        let activities = list_activities(&vfs).expect("list_activities");
        assert_eq!(activities.len(), 13, "attendu 13 activités, obtenu {}", activities.len());
        let roots: Vec<&str> = activities.iter().filter(|a| a.is_root).map(|a| a.name.as_str()).collect();
        assert_eq!(roots, ["StoryMode"], "attendu une seule racine `StoryMode`, obtenu {roots:?}");
        assert!(activities.iter().any(|a| a.name == "StoryMode_SubTask_09"));
    }

    /// Équipes : 208 lignes `m_belongTeamInfoList`, noms joints via `team_text` fr — même
    /// jointure que `nie-game/examples/export_teams.rs`. Le seuil porte sur les noms RÉSOLUS :
    /// une jointure cassée donnerait 208 lignes et 0 nom, ce que le compte brut ne verrait pas.
    #[test]
    fn list_belong_teams_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_belong_teams_sur_le_vrai_jeu") else { return };
        let teams = list_belong_teams(&vfs).expect("list_belong_teams");
        assert_eq!(teams.len(), 208, "attendu 208 équipes, obtenu {}", teams.len());
        let named = teams.iter().filter(|t| t.name.is_some()).count();
        assert_eq!(named, 208, "attendu 208 noms d'équipe résolus, obtenu {named} — jointure team_text cassée");
        assert_eq!(teams[0].name.as_deref(), Some("Raimon"));
    }

    /// Formations : 115 formations / 1073 placements (comptes relevés par `niers vfs cat` sur
    /// `formation_config_0.02.16.cfg.bin`). Aucun nom attendu — `formation_text.cfg.bin` n'existe
    /// pas dans cette version du jeu.
    #[test]
    fn list_formations_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_formations_sur_le_vrai_jeu") else { return };
        let formations = list_formations(&vfs).expect("list_formations");
        assert_eq!(formations.len(), 115, "attendu 115 formations, obtenu {}", formations.len());
        // Les 1073 lignes de `m_SoccerFormPlacementInfoList` sont TOUTES couvertes par les
        // tranches `placementInfo` : une somme inférieure signalerait une tranche débordante
        // (`placements_of` renvoie alors `&[]` en silence).
        let placements: usize = formations.iter().map(|f| f.positions.len()).sum();
        assert_eq!(placements, 1073, "attendu 1073 placements rattachés au total, obtenu {placements}");
    }

    /// Uniformes : 627 lignes `m_UniformInfoList` sur 1247 modèles (comptes relevés par
    /// `niers vfs cat` sur `character/uniform_config_1.03.52.00.cfg.bin`).
    #[test]
    fn list_uniforms_sur_le_vrai_jeu() {
        let Some(vfs) = real_vfs_or_skip("list_uniforms_sur_le_vrai_jeu") else { return };
        let uniforms = list_uniforms(&vfs).expect("list_uniforms");
        assert_eq!(uniforms.len(), 627, "attendu 627 uniformes, obtenu {}", uniforms.len());
        let resolus = uniforms.iter().filter(|u| u.resolved_count > 0.0).count();
        assert_eq!(resolus, 627, "attendu 627 tranches de modèles résolues (aucune vide), obtenu {resolus}");
    }

    /// Vérifie que le décodeur GÉNÉRIQUE (`decode_cfgbin`) marche sur un large échantillon
    /// de VRAIS `.cfg.bin` du jeu, pas seulement `skill_config` — preuve de couverture large
    /// (« niers doit couvrir tout nie.exe »), RDBN et T2B mélangés, sans aucun crash/erreur.
    #[test]
    fn decode_cfgbin_sur_un_echantillon_large() {
        let dir = nie_formats::vfs::resolve_game_dir().to_string_lossy().into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip decode_cfgbin_sur_un_echantillon_large : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let candidates: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.ends_with(".cfg.bin") && (p.contains("/gamedata/") || p.contains("/text/")))
            .collect();
        assert!(candidates.len() > 100, "attendu > 100 .cfg.bin dans gamedata/text, obtenu {}", candidates.len());

        // Échantillon déterministe (pas aléatoire) : un fichier sur N, réparti sur tout
        // l'éventail alphabétique plutôt que les N premiers d'un même sous-dossier.
        let step = (candidates.len() / 60).max(1);
        let mut ok = 0usize;
        let mut failed: Vec<(String, String)> = Vec::new();
        for path in candidates.iter().step_by(step) {
            match decode_cfgbin(&vfs, path) {
                Ok(json) => {
                    assert!(json.get("lists").is_some() || json.get("entries").is_some(), "{path} : forme JSON inattendue");
                    ok += 1;
                }
                Err(e) => failed.push((path.clone(), e)),
            }
        }
        eprintln!("decode_cfgbin : {ok} décodés sans erreur, {} échecs sur {} testés (sur {} candidats)", failed.len(), ok + failed.len(), candidates.len());
        for (p, e) in &failed {
            eprintln!("  échec {p} : {e}");
        }
        // Tolérance : quelques formats exotiques (tables sans lists/entries, ex. fichiers texte
        // non-config) peuvent échouer sans invalider la couverture générale.
        assert!(
            failed.len() * 5 < ok + failed.len(),
            "trop d'échecs de décodage ({}/{}) pour un décodeur censé couvrir tout nie.exe",
            failed.len(),
            ok + failed.len()
        );
    }
    /// Exerce les HUIT familles ajoutées (§4.3) sur le VRAI jeu : chacune doit décoder son
    /// `.cfg.bin` et rendre des lignes. Un `list_*` qui rendrait `Ok(vec![])` serait un onglet
    /// vide dans l'encyclopédie — un faux vert exactement comme une suite à « 0 passed ».
    #[test]
    fn nouvelles_familles_sur_le_vrai_jeu() {
        let dir = nie_formats::vfs::resolve_game_dir().to_string_lossy().into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !nie_formats::vfs::donnees_disponibles(&data_dir) {
            eprintln!("skip nouvelles_familles_sur_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("vfs init");

        let charas = list_charas(&vfs).expect("list_charas");
        assert!(charas.len() > 1000, "attendu > 1000 personnages, obtenu {}", charas.len());
        assert!(
            charas.iter().any(|c| c.stats.total > 0 && !c.internal_code.is_empty()),
            "aucun personnage avec code interne ET stats calculées"
        );

        macro_rules! non_vide {
            ($f:ident) => {{
                let v = $f(&vfs).unwrap_or_else(|e| panic!("{} : {e}", stringify!($f)));
                assert!(!v.is_empty(), "{} rend une liste vide", stringify!($f));
                v.len()
            }};
        }
        let n_opp = non_vide!(list_opponent_teams);
        let n_mov = non_vide!(list_movies);
        let n_mus = non_vide!(list_musics);
        let n_dic = non_vide!(list_dictionary);
        let n_exp = non_vide!(list_exp_table);
        let n_drp = non_vide!(list_drops);
        let n_cap = non_vide!(list_capsule_rates);
        eprintln!(
            "familles : charas={} adversaires={n_opp} videos={n_mov} musiques={n_mus} \
             dictionnaire={n_dic} exp={n_exp} butin={n_drp} capsules={n_cap}",
            charas.len()
        );
    }
}
