//! `nie-data` — modèles de données de jeu IEVR (Inazuma Eleven: Victory Road) portés
//! en Rust pur, `no_std + alloc` (donc `wasm32`-compatible sans std).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! Chaque structure, offset et valeur golden de ce crate est ancré sur le pipeline TS
//! de production `@rose-griffon/inagle` (`/home/ubuntu/rg/packages/inagle/src`) et sur
//! les vrais dumps `*.cfg.bin.json` d'IEVR (`/home/ubuntu/niers/data/common/gamedata`). Aucune
//! valeur n'est inventée : les modules citent leur fichier-source TS et le dump réel.
//!
//! ## Modules
//!
//! - [`hash`] — identifiants hash 32 bits Level-5 (parse/format hex `0xXXXXXXXX`, signé→non-signé).
//! - [`cfgbin`] — vues minimales sur le JSON `cfg.bin.json` (noeuds/variables) pour le parse.
//! - [`skill`] — `SkillInfo` + enums, port de `m_skillInfoList` (skill_config) + jointure skill_text.
//! - [`chara_param`] — `CharaParam`, port du noeud `CHARA_PARAM_INFO_*` (chara_param.cfg.bin).
//! - [`aura`] — `AuraCmd`, port du noeud `AURA_CMD_INFO_*` (aura_skill_config) + résolution hissatsu.
//! - [`item`] — `ItemInfo` + `ItemCategory`, port des noeuds `ITEM_*_INFO_*` (item_config).
//! - [`growth`] — tables de croissance + `calculate_stats` (interpolation Lv1→99).
//! - [`exp`] — `chara_exp_table_config` (XP/niveau + multiplicateurs de rareté).
//! - [`passive`] — `PassiveSkillInfo` + classification scope/boost (passive_skill_config).
//!
//! ## `no_std`
//!
//! Tout le crate est `#![no_std]` + `extern crate alloc`. Aucune dépendance `std` :
//! `serde_json` est compilé en `default-features = false, features = ["alloc"]`. Compatible
//! `wasm32-unknown-unknown` sans shim.
#![no_std]
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

extern crate alloc;

pub mod ability_learning;
pub mod academic_year;
pub mod activity;
pub mod add_content_equip;
pub mod add_model;
pub mod ai;
pub mod ai_type;
pub mod aura;
pub mod banner;
pub mod basara;
pub mod belong_team;
pub mod boost_grp;
pub mod capsule;
pub mod cfgbin;
pub mod change_aura_skill_config;
pub mod chara_bank;
pub mod chara_base;
pub mod chara_costume;
pub mod chara_description;
pub mod chara_details;
pub mod chara_edit;
pub mod chara_menu_resource;
pub mod chara_param;
pub mod chara_series;
pub mod chara_text;
pub mod chat_emote;
pub mod chronicle_top;
pub mod command;
pub mod cond;
pub mod craft;
pub mod ctrl_chara;
pub mod dictionary;
pub mod dungeon;
pub mod emblems;
pub mod enjoy_mode_team;
pub mod event_bustup;
pub mod event_map_tag;
pub mod event_subtitle;
pub mod exp;
pub mod extend_story;
pub mod fast_travel;
pub mod flag_config;
pub mod font_color;
pub mod formation;
pub mod friendmap;
pub mod gallery;
pub mod game_quest;
pub mod growth;
pub mod happen_event_npc;
pub mod hash;
pub mod help;
pub mod inacode;
pub mod input;
pub mod item;
pub mod item_emission;
pub mod light;
pub mod menu_setting;
pub mod mission;
pub mod movie;
pub mod music_app;
pub mod nfc;
pub mod opponent_team;
pub mod override_skill;
pub mod party;
pub mod passive;
pub mod passives;
pub mod phase;
pub mod phase_set;
pub mod photo_mode;
pub mod players_universe;
pub mod playstyle;
pub mod post;
pub mod quest;
pub mod real_skill_config;
pub mod record;
pub mod rpg_battle;
pub mod scene_archive;
pub mod search_word;
pub mod setting_menu;
pub mod shop;
pub mod skill;
pub mod skill_technic;
pub mod skill_view;
pub mod soccer;
pub mod soccer_chara_unique_rarity;
pub mod soccer_drop;
pub mod soccer_fixed_reward;
pub mod soccer_map_env;
pub mod soccer_opponent;
pub mod soccer_performance;
pub mod soccer_placement;
pub mod soccer_player_record;
pub mod soccer_rank;
pub mod soccer_suggest;
pub mod special_tactics;
pub mod stadium;
pub mod super_tactics;
pub mod system_unlock;
pub mod talk_select;
pub mod team;
pub mod team_build_config;
pub mod telop_waza;
pub mod text;
pub mod trial_take_over;
pub mod trick;
pub mod trigger;
pub mod trophy;
pub mod typed;
pub mod uniform;
pub mod unlock_condition;
pub mod update_notice;
pub mod user_name_plate;
pub mod video_waza;
pub mod vsroute;
pub mod weather;
pub mod win_treasure;

pub use hash::HashId;
