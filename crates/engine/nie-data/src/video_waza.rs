//! `video_waza` — vidéos d'événement / cinématiques (`event/event_movie_config_*.cfg.bin`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `packages/inagle/src/parsers/video-waza.ts`
//!   (`parseVideoWazaContent`) — **port 1:1**.
//! - Données : `data/common/gamedata/event/event_movie_config_0.00.00.cfg.bin` (VFS IEVR),
//!   format **RDBN** (`lists`), **3 listes** (noms de champs confirmés sur le vrai dump) :
//!   - `m_EventMovieInfoList` (`EVENT_MOVIE_INFO`) : `eventId, menuId, captionId, moviePath,
//!     bgmName, fedeInTime, fedeOutTime, staffrollDataName` ;
//!   - `m_EventSubtitleMenuDataList` (`EVENT_SUBTITLE_MENU_DATA`) : `subtitleId, menuCrc,
//!     layerCrc, textLocateCrc, usedGroupCrc` ;
//!   - `m_EventSongCaptionDataList` (`EVENT_SONG_CAPTION_DATA`) : `captionId, startFrame,
//!     endFrame, captionName`.
//!
//! La ligne pivot est l'entrée movie (`eventId` stable). La légende liée par `captionId`
//! (sauf `0x00000000`) est attachée à chaque movie ; les listes brutes restent disponibles.
//!
//! Ligne 0 réelle : `eventId=0xDB464E8F, moviePath="common/movie/yw-y1_evop_0010.usm",
//! bgmName=0xC1EEF2A7, fedeInTime=1, captionId=0x00000000 → caption=None`.
//!
//! `moviePath`/`captionName` sont des chaînes ; `staffrollDataName` est **mixte** (un hash
//! sentinelle `0xFFFFFFFF` = aucun, ou un nom `"op_01"`) donc conservé en chaîne brute, comme
//! inagle. Les autres identifiants sont des hash.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{field_hash, field_str, list_values};
use crate::hash::HashId;

/// Entrée de `m_EventMovieInfoList` (`EVENT_MOVIE_INFO`).
///
/// `fede_in_time`/`fede_out_time` sont des **flottants** dans le RDBN (champ `number` côté
/// inagle) → la structure ne dérive pas `Eq`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMovieInfo {
    /// `eventId` — clé primaire stable de la cinématique.
    pub event_id: HashId,
    /// `menuId` — menu associé (`0x00000000` si aucun).
    pub menu_id: HashId,
    /// `captionId` — légende liée (`0x00000000` si aucune).
    pub caption_id: HashId,
    /// `moviePath` — chemin du `.usm`.
    pub movie_path: String,
    /// `bgmName` — hash du cue BGM.
    pub bgm_name: HashId,
    /// `fedeInTime` — durée de fondu d'entrée (flottant, unités du jeu).
    pub fede_in_time: f64,
    /// `fedeOutTime` — durée de fondu de sortie (flottant).
    pub fede_out_time: f64,
    /// `staffrollDataName` — nom des données de staffroll, ou hash sentinelle `0xFFFFFFFF`
    /// (« aucun »). Conservé en chaîne brute (champ mixte hash/chaîne).
    pub staffroll_data_name: String,
}

/// Entrée de `m_EventSubtitleMenuDataList` (`EVENT_SUBTITLE_MENU_DATA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventSubtitleMenuData {
    /// `subtitleId`.
    pub subtitle_id: HashId,
    /// `menuCrc`.
    pub menu_crc: HashId,
    /// `layerCrc`.
    pub layer_crc: HashId,
    /// `textLocateCrc`.
    pub text_locate_crc: HashId,
    /// `usedGroupCrc`.
    pub used_group_crc: HashId,
}

/// Entrée de `m_EventSongCaptionDataList` (`EVENT_SONG_CAPTION_DATA`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventSongCaptionData {
    /// `captionId`.
    pub caption_id: HashId,
    /// `startFrame`.
    pub start_frame: i64,
    /// `endFrame`.
    pub end_frame: i64,
    /// `captionName`.
    pub caption_name: String,
}

/// Vidéo d'événement résolue : l'entrée movie + sa légende liée éventuelle (port de
/// `ParsedVideoWaza`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedVideoWaza {
    /// L'entrée movie.
    pub movie: EventMovieInfo,
    /// La légende liée par `captionId` (`None` si `captionId == 0x00000000` ou absente).
    pub caption: Option<EventSongCaptionData>,
}

/// Résultat complet du parse (port de `VideoWazaDatabase`).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VideoWazaDatabase {
    /// Movies résolus (avec légende liée).
    pub videos: Vec<ParsedVideoWaza>,
    /// Liste brute des sous-titres.
    pub subtitles: Vec<EventSubtitleMenuData>,
    /// Liste brute des légendes de chanson.
    pub captions: Vec<EventSongCaptionData>,
}

/// Lit un champ flottant (les `fede*Time` sont des `Float` RDBN).
fn field_f64(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

/// Lit un champ entier tolérant aux nombres JSON flottants (`220.0` → `220`).
fn field_num_i64(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

fn parse_movie(v: &Value) -> EventMovieInfo {
    EventMovieInfo {
        event_id: field_hash(v, "eventId"),
        menu_id: field_hash(v, "menuId"),
        caption_id: field_hash(v, "captionId"),
        movie_path: field_str(v, "moviePath").unwrap_or("").into(),
        bgm_name: field_hash(v, "bgmName"),
        fede_in_time: field_f64(v, "fedeInTime"),
        fede_out_time: field_f64(v, "fedeOutTime"),
        staffroll_data_name: field_str(v, "staffrollDataName").unwrap_or("").into(),
    }
}

fn parse_caption(v: &Value) -> EventSongCaptionData {
    EventSongCaptionData {
        caption_id: field_hash(v, "captionId"),
        start_frame: field_num_i64(v, "startFrame"),
        end_frame: field_num_i64(v, "endFrame"),
        caption_name: field_str(v, "captionName").unwrap_or("").into(),
    }
}

fn parse_subtitle(v: &Value) -> EventSubtitleMenuData {
    EventSubtitleMenuData {
        subtitle_id: field_hash(v, "subtitleId"),
        menu_crc: field_hash(v, "menuCrc"),
        layer_crc: field_hash(v, "layerCrc"),
        text_locate_crc: field_hash(v, "textLocateCrc"),
        used_group_crc: field_hash(v, "usedGroupCrc"),
    }
}

/// Parse les 3 listes d'un `event_movie_config_*.cfg.bin.json` désérialisé et joint chaque movie
/// à sa légende (`captionId`, sauf `0x00000000`). Port de `parseVideoWazaContent`.
#[must_use]
pub fn parse_video_waza(root: &Value) -> VideoWazaDatabase {
    let movies: Vec<EventMovieInfo> = list_values(root, "m_EventMovieInfoList")
        .map(|vs| vs.iter().map(parse_movie).collect())
        .unwrap_or_default();
    let subtitles: Vec<EventSubtitleMenuData> = list_values(root, "m_EventSubtitleMenuDataList")
        .map(|vs| vs.iter().map(parse_subtitle).collect())
        .unwrap_or_default();
    let captions: Vec<EventSongCaptionData> = list_values(root, "m_EventSongCaptionDataList")
        .map(|vs| vs.iter().map(parse_caption).collect())
        .unwrap_or_default();

    let videos: Vec<ParsedVideoWaza> = movies
        .into_iter()
        .map(|m| {
            let caption = if m.caption_id == HashId::ZERO {
                None
            } else {
                captions
                    .iter()
                    .find(|c| c.caption_id == m.caption_id)
                    .cloned()
            };
            ParsedVideoWaza { movie: m, caption }
        })
        .collect();

    VideoWazaDatabase {
        videos,
        subtitles,
        captions,
    }
}

impl VideoWazaDatabase {
    /// Recherche une vidéo par `eventId` (port de l'accès `byId.get`).
    #[must_use]
    pub fn find_by_event_id(&self, event_id: HashId) -> Option<&ParsedVideoWaza> {
        self.videos.iter().find(|v| v.movie.event_id == event_id)
    }
}
