//! Dispatch **typé** des `cfg.bin` : d'une `Value` au format iecode (`lists`/`entries`) +
//! d'une clé de famille (dérivée du nom de fichier), vers la structure de jeu nommée
//! correspondante sérialisée en JSON. Partagé entre `nie-model-serve` (route `/typed`) et
//! `nie-wasm` (décodage in-browser) — **source unique** des 93 familles couvertes.
//!
//! `no_std + alloc` : `serde_json::to_value` fonctionne en mode `alloc`. Gated `serde`
//! (les structures de famille ne dérivent `Serialize` que sous cette feature).
#![cfg(feature = "serde")]

use alloc::string::{String, ToString};
use serde_json::Value;

/// Dérive la **clé de famille** d'un `cfg.bin` depuis son chemin/nom : nom de base, suffixes
/// `.json`/`.cfg.bin` retirés, suffixe de version `_<chiffres.points>` final retiré.
/// Ex. `formation_config_0.02.16.cfg.bin` -> `formation_config`,
/// `phase_set_c21_0.00.00.cfg.bin` -> `phase_set_c21`, `record_config.cfg.bin` -> `record_config`.
#[must_use]
pub fn family_key(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    let base = base.strip_suffix(".json").unwrap_or(base);
    let base = base.strip_suffix(".cfg.bin").unwrap_or(base);
    if let Some(idx) = base.rfind('_') {
        let tail = &base[idx + 1..];
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return base[..idx].to_string();
        }
    }
    base.to_string()
}

/// Dispatche un `root` (forme iecode `lists`/`entries`) vers le parseur typé de la famille
/// `key`, et renvoie `(label, json)`. `None` si aucune famille typée ne correspond (le caller
/// renvoie alors le générique). Couvre 93 familles game-data (37 d'origine + 56 rapatriées
/// de nie-model-serve le 2026-06-21), parseurs validés golden byte-exact.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn decode_by_key(key: &str, root: &Value) -> Option<(&'static str, Value)> {
    use serde_json::to_value;
    macro_rules! t {
        ($label:literal, $call:expr) => {
            to_value($call).ok().map(|v| ($label, v))
        };
    }
    match key {
        "formation_config" => t!("formation", crate::formation::parse_formation_config(root)),
        "font_color" => t!("font_color", crate::font_color::parse_font_colors(root)),
        // Éditeur d'avatar : deux fichiers, deux familles distinctes (`family_key` les sépare
        // puisque seule la version en queue est retirée). Cf. `niers avatar`.
        "chara_edit" => t!("chara_edit", crate::chara_edit::parse_chara_edit(root)),
        "chara_edit_parts_type_config" => {
            t!(
                "chara_edit_parts_type",
                crate::chara_edit::parse_chara_edit_parts_type_config(root)
            )
        }
        "item_config" => t!("item", crate::item::parse_all_items(root)),
        "skill_config" => t!("skill", crate::skill::parse_skill_config(root)),
        "mission_config" => t!("mission", crate::mission::parse_mission_config(root)),
        "aura_skill_config" => t!("aura", crate::aura::parse_all_aura_cmds(root)),
        "trophy_config" => t!("trophy", crate::trophy::parse_trophy_config(root)),
        "gallery_config" => t!("gallery", crate::gallery::parse_gallery_config(root)),
        "record_config" => t!("record", crate::record::parse_record_config(root)),
        "dictionary_config" => t!(
            "dictionary",
            crate::dictionary::parse_dictionary_config(root)
        ),
        "help_list_config" => t!("help", crate::help::parse_help_list_config(root)),
        "setting_list_config" => {
            t!(
                "setting_menu",
                crate::setting_menu::parse_setting_list_config(root)
            )
        }
        "scene_archive_config" => {
            t!(
                "scene_archive",
                crate::scene_archive::parse_scene_archive_config(root)
            )
        }
        "music_app_config" => t!("music_app", crate::music_app::parse_music_app_config(root)),
        "chara_param" => t!(
            "chara_param",
            crate::chara_param::parse_all_chara_params(root)
        ),
        "chara_exp_table_config" => t!("exp", crate::exp::parse_exp_table(root)),
        "soccer_game_config" => t!("soccer_game", crate::soccer::parse_soccer_game_config(root)),
        "game_quest_config" => t!(
            "game_quest",
            crate::game_quest::parse_game_quest_config(root)
        ),
        "soccer_opponent_info" => {
            t!(
                "soccer_opponent",
                crate::soccer_opponent::parse_soccer_opponent_config(root)
            )
        }
        "soccer_fixed_reward_spirit_config" => {
            t!(
                "soccer_fixed_reward",
                crate::soccer_fixed_reward::parse_soccer_fixed_reward_config(root)
            )
        }
        "soccer_chara_placement" => {
            t!(
                "soccer_placement",
                crate::soccer_placement::parse_soccer_placement_config(root)
            )
        }
        "soccer_rank_config" => {
            t!(
                "soccer_rank",
                crate::soccer_rank::parse_soccer_rank_config(root)
            )
        }
        "soccer_player_record_config" => t!(
            "soccer_player_record",
            crate::soccer_player_record::parse_soccer_player_record_config(root)
        ),
        "soccer_game_map_enviroment_config" => {
            t!(
                "soccer_map_env",
                crate::soccer_map_env::parse_soccer_map_env_config(root)
            )
        }
        "soccer_drop_config" => {
            t!(
                "soccer_drop",
                crate::soccer_drop::parse_soccer_drop_config(root)
            )
        }
        "soccer_suggest_config" => {
            t!(
                "soccer_suggest",
                crate::soccer_suggest::parse_soccer_suggest_config(root)
            )
        }
        // Parseurs soccer existants + golden-testés mais qui n'étaient PAS routés → azalee.
        "soccer_focus_battle_effect_config" => {
            t!(
                "soccer_focus_battle",
                crate::soccer::parse_focus_battle_effect_config(root)
            )
        }
        "soccer_technic_config" => {
            t!("soccer_technic", crate::soccer::parse_technic_config(root))
        }
        "soccer_basic_effect_config" => {
            t!(
                "soccer_basic_effect",
                crate::soccer::parse_basic_effect_config(root)
            )
        }
        "soccer_game_additional_config" => {
            t!(
                "soccer_game_additional",
                crate::soccer::parse_soccer_game_additional_config(root)
            )
        }
        "enjoy_mode_team_config" => {
            t!(
                "enjoy_mode_team",
                crate::enjoy_mode_team::parse_enjoy_mode_team_config(root)
            )
        }
        "system_unlock_window_config" => t!(
            "system_unlock_window",
            crate::system_unlock::parse_system_unlock_window_config(root)
        ),
        "happen_event_npc_common" => t!(
            "happen_event_npc",
            crate::happen_event_npc::parse_happen_event_npc_common(root)
        ),
        "flag_config" => t!("flag_config", crate::flag_config::parse_flag_config(root)),
        // Nom de fichier réel = `talk_select_config.cfg_<ver>.cfg.bin` → family_key garde le `.cfg`.
        "talk_select_config.cfg" => {
            t!(
                "talk_select",
                crate::talk_select::parse_talk_select_config(root)
            )
        }
        "trial_take_over_config" => {
            t!(
                "trial_take_over",
                crate::trial_take_over::parse_trial_take_over_config(root)
            )
        }
        "ai_type_config" => t!("ai_type", crate::ai_type::parse_ai_type_config(root)),
        "tutorial_banner_config" => t!("banner", crate::banner::parse_banner_config(root)),
        "boost_player_group_config" => {
            t!("boost_grp", crate::boost_grp::parse_boost_grp_config(root))
        }
        "chronicle_top_caravan_config" => {
            t!(
                "chronicle_top",
                crate::chronicle_top::parse_chronicle_top_caravan_config(root)
            )
        }
        "craft_obj_config" => t!("craft", crate::craft::parse_craft_obj_config(root)),
        "fast_travel_config" => t!(
            "fast_travel",
            crate::fast_travel::parse_fast_travel_config(root)
        ),
        "friendmap_config" => t!("friendmap", crate::friendmap::parse_friendmap_config(root)),
        "light_overwrite_config" => t!("light", crate::light::parse_light_overwrite_config(root)),
        "photo_mode_random_pose_config" => {
            t!(
                "photo_mode",
                crate::photo_mode::parse_photo_mode_random_pose_config(root)
            )
        }
        "info_bookmark_config" => {
            t!(
                "search_word",
                crate::search_word::parse_info_bookmark_config(root)
            )
        }
        "update_notice_config" => {
            t!(
                "update_notice",
                crate::update_notice::parse_update_notice_config(root)
            )
        }
        "user_name_plate_config" => {
            t!(
                "user_name_plate",
                crate::user_name_plate::parse_user_name_plate_config(root)
            )
        }
        "chronicle_vs_route_config" => {
            t!(
                "vsroute",
                crate::vsroute::parse_chronicle_vs_route_config(root)
            )
        }
        "weather_convert" => t!("weather", crate::weather::parse_weather_convert(root)),
        "gimmick_system_num_config" => {
            t!(
                "dungeon",
                crate::dungeon::parse_gimmick_system_num_config(root)
            )
        }
        "soccer_club_room_config" => {
            t!(
                "chara_bank",
                crate::chara_bank::parse_soccer_club_room_config(root)
            )
        }
        "advent_calendar_config" => {
            t!(
                "advent_calendar",
                crate::post::parse_advent_calendar_config(root)
            )
        }
        "delivery_config" => t!("delivery", crate::post::parse_delivery_config(root)),
        "delivery_list_config" => t!(
            "delivery_list",
            crate::post::parse_delivery_list_config(root)
        ),
        "password_list_config" => t!(
            "password_list",
            crate::post::parse_password_list_config(root)
        ),
        "post_notice_config" => t!("post_notice", crate::post::parse_post_notice_config(root)),
        "skill_view_preset_config" => {
            t!(
                "skill_view",
                crate::skill_view::parse_skill_view_preset_config(root)
            )
        }
        // ── 56 familles rapatriées de nie-model-serve (2026-06-21, dédup Phase 1a) ──────
        // Étaient décodées en structuré côté serveur mais renvoyées en « generic » côté
        // navigateur (nie-wasm passait par cette table à 37 arms) → incohérence corrigée.
        "uniform_config" => t!("uniform", crate::uniform::parse_uniform_config(root)),
        "players_universe_config" => {
            t!(
                "players_universe",
                crate::players_universe::parse_players_universe_config(root)
            )
        }
        "players_universe_event_config" => t!(
            "players_universe_event",
            crate::players_universe::parse_players_universe_event_config(root)
        ),
        "nfc_lottery_config" => t!("nfc_lottery", crate::nfc::parse_nfc_lottery_config(root)),
        "search_word_config" => {
            t!(
                "search_word",
                crate::search_word::parse_search_word_config(root)
            )
        }
        "passive_skill_config" => t!("passive", crate::passive::parse_passives(root)),
        "soccer_ai_cmd_config" => t!("soccer_ai_cmd", crate::ai::parse_soccer_ai_cmd_config(root)),
        "soccer_user_ai_config" => {
            t!(
                "soccer_user_ai",
                crate::ai::parse_soccer_user_ai_config(root)
            )
        }
        "strategy_ai_config" => t!("strategy_ai", crate::ai::parse_strategy_ai_config(root)),
        "tactics_ai_config" => t!("tactics_ai", crate::ai::parse_tactics_ai_config(root)),
        "adaptive_trigger_def" => {
            t!(
                "adaptive_trigger",
                crate::input::parse_adaptive_trigger_def(root)
            )
        }
        "haptic_feedback_def" => {
            t!(
                "haptic_feedback",
                crate::input::parse_haptic_feedback_def(root)
            )
        }
        "vibration_def" => t!("vibration", crate::input::parse_vibration_def(root)),
        "basara_chara_config" => t!(
            "basara_chara",
            crate::basara::parse_basara_chara_config(root)
        ),
        "belong_team_config" => t!(
            "belong_team",
            crate::belong_team::parse_belong_team_config(root)
        ),
        "capsule_config" => t!("capsule", crate::capsule::parse_capsule_database(root)),
        "change_aura_skill_config" => t!(
            "change_aura_skill",
            crate::change_aura_skill_config::parse_change_aura_skill_config(root)
        ),
        "chara_base" => t!("chara_base", crate::chara_base::parse_all_chara_base(root)),
        "chara_costume" => t!(
            "chara_costume",
            crate::chara_costume::parse_all_chara_costumes(root)
        ),
        "chara_description_text" => t!(
            "chara_description",
            crate::chara_description::parse_chara_descriptions(root)
        ),
        "chara_details_config" => t!(
            "chara_details",
            crate::chara_details::parse_chara_details(root)
        ),
        "chara_menu_resource" => t!(
            "chara_menu_resource",
            crate::chara_menu_resource::parse_chara_menu_resource(root)
        ),
        "chara_series_config" => t!(
            "chara_series",
            crate::chara_series::parse_chara_series_config(root)
        ),
        "chat_emote_config" => t!(
            "chat_emote",
            crate::chat_emote::parse_chat_emote_config(root)
        ),
        "chat_emote_def_set_config" => t!(
            "chat_emote_def_set",
            crate::chat_emote::parse_chat_emote_def_set_config(root)
        ),
        "soccer_cmd_action" => t!("soccer_cmd_action", crate::command::parse_cmd_actions(root)),
        "rpg_cmd_action" => t!("rpg_cmd_action", crate::command::parse_cmd_actions(root)),
        "soccer_cmd_event" => t!("soccer_cmd_event", crate::command::parse_cmd_events(root)),
        "rpg_cmd_event" => t!("rpg_cmd_event", crate::command::parse_cmd_events(root)),
        "chara_cmd_event_common" => t!(
            "chara_cmd_common",
            crate::command::parse_chara_cmd_common(root)
        ),
        "craft_theme_config" => t!("craft_theme", crate::craft::parse_craft_theme_config(root)),
        "ctrl_chara_config" => t!("ctrl_chara", crate::ctrl_chara::parse_all_ctrl_chara(root)),
        "emblem_resource" => t!("emblem", crate::emblems::parse_emblem_resources(root)),
        "extend_story_data_config" => t!(
            "extend_story",
            crate::extend_story::parse_extend_story_config(root)
        ),
        "inacode_config" => t!("inacode", crate::inacode::parse_inacode_config(root)),
        "item_emission_rarity_table_config" => t!(
            "item_emission",
            crate::item_emission::parse_item_emission_rates(root)
        ),
        "msa999999_trigger" => t!(
            "mission_trigger",
            crate::mission::parse_mission_trigger(root)
        ),
        "movie_playing_config" => t!(
            "movie_playing",
            crate::movie::parse_movie_playing_config(root)
        ),
        "opponent_team_config" => t!(
            "opponent_team",
            crate::opponent_team::parse_opponent_team_config(root)
        ),
        "override_skill_config" => t!(
            "override_skill",
            crate::override_skill::parse_override_skill_config(root)
        ),
        "party_departure" => t!("party_departure", crate::party::parse_party_departure(root)),
        "supecify_party0.00.00" => t!("specify_party", crate::party::parse_specify_party(root)),
        "guest_limit_config" => t!("guest_limit", crate::party::parse_guest_limit_config(root)),
        "phase_title_config" => t!("phase_title", crate::phase::parse_phase_title_config(root)),
        "quest_config" => t!("quest", crate::quest::parse_quest_config(root)),
        "real_skill_config" => t!(
            "real_skill",
            crate::real_skill_config::parse_real_skill_config(root)
        ),
        "rpg_battle_rule_config" => t!(
            "rpg_battle_rule",
            crate::rpg_battle::parse_rule_config(root)
        ),
        "rpg_battle_status_pattern_config" => t!(
            "rpg_battle_status_pattern",
            crate::rpg_battle::parse_status_pattern_config(root)
        ),
        "rpg_battle_chara_swap_motion_config" => t!(
            "rpg_battle_chara_swap_motion",
            crate::rpg_battle::parse_chara_swap_motion_config(root)
        ),
        "rpg_battle_party_config" => t!(
            "rpg_battle_party",
            crate::rpg_battle::parse_party_config(root)
        ),
        "rpg_battle_cmd_event_config" => t!(
            "rpg_battle_cmd_event",
            crate::rpg_battle::parse_cmd_event_config(root)
        ),
        "rpg_battle_cmd_obj_config" => t!(
            "rpg_battle_cmd_obj",
            crate::rpg_battle::parse_cmd_obj_config(root)
        ),
        "rpg_battle_cmd_set_config" => t!(
            "rpg_battle_cmd_set",
            crate::rpg_battle::parse_cmd_set_config(root)
        ),
        "rpg_battle_add_status_config" => t!(
            "rpg_battle_add_status",
            crate::rpg_battle::parse_add_status_config(root)
        ),
        "shop_config" => t!("shop", crate::shop::parse_shop_config(root)),
        "skill_technic_config" => t!(
            "skill_technic",
            crate::skill_technic::parse_skill_technic_config(root)
        ),
        // Sous-titres du mode Histoire : ~1321 fichiers `Subtitle_ev<NN>_<bloc>` (clé par-événement,
        // un même format `EV_SUBTITLE_DATA`). Dispatch par PRÉFIXE → tout le dialogue d'histoire
        // (lignes timecodées) devient décodable typé (route /typed + nie-wasm).
        k if k.starts_with("Subtitle_ev") => {
            t!(
                "event_subtitle",
                crate::event_subtitle::parse_subtitle_file(root)
            )
        }
        // Définitions d'écrans de menu : ~304 fichiers `<écran>_menu_setting` (un par écran), même
        // format T2B (MENU_LAYER_INFO/CMD/RES…). Dispatch par SUFFIXE → toutes les structures d'écran
        // décodables typé (support du driver de menu + explorateur azalee).
        k if k.ends_with("_menu_setting") => t!("menu_setting", crate::menu_setting::parse(root)),
        // Déclencheurs de scripting (~287 fichiers `*_trigger` : qsb/qsa quêtes, fbtl_cro matchs, c21…)
        // — DATA_COUNT/DATA_ITEM avec condition décodée. Dispatch par SUFFIXE. (Les fichiers d'autre
        // forme renvoient un TriggerConfig vide, sans danger.)
        k if k.ends_with("_trigger") => t!("trigger", crate::trigger::parse_trigger(root)),
        // Setup de phases de match (~182 fichiers `*_phase_set` : fbtl_cro/fbtl_qs) — DATA_ITEM
        // (ints + conditions décodées). Dispatch par SUFFIXE.
        k if k.ends_with("_phase_set") => t!("phase_set", crate::phase_set::parse_phase_set(root)),
        // Portraits de dialogue par chapitre (~34 fichiers `event_bustup_talk_data_config_c<NN>`).
        k if k.starts_with("event_bustup_talk_data_config") => {
            t!(
                "event_bustup",
                crate::event_bustup::parse_event_bustup_talk(root)
            )
        }
        "event_map_tag_config" => {
            t!(
                "event_map_tag",
                crate::event_map_tag::parse_event_map_tag_config(root)
            )
        }
        _ => None,
    }
}
