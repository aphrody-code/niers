//! `event_subtitle` — sous-titres timecodés, dialogue brut et table *washa* des cutscenes
//! voicées (`gamedata/event/subtitle/<lang>/Subtitle_*`, `text/<lang>/event/*`,
//! `text/event/*_map`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! Port **1:1** des parseurs par-fichier d'inagle `packages/inagle/src/parsers/event-subtitles.ts`
//! (`parseSubtitleFile`, `loadTextMap`, `loadWashaMap`). L'agrégation multi-langue/multi-fichier
//! d'inagle (scan de répertoires, choix de langue canonique) reste **côté appelant** ; ce module
//! porte les **trois extractions par-fichier**, chacune ancrée sur le layout réel du dump :
//!
//! - **Subtitle** (`EV_SUBTITLE_DATA_LIST_BEG_0` → `EV_SUBTITLE_DATA_<n>`, ≥ 5 variables) :
//!   `var[0]` = hash texte (Int), `var[1..4]` = timings (Float, secondes). Pas de texte ici.
//! - **Dialogue** (`TEXT_INFO_BEGIN_0` → `TEXT_INFO_<n>`, ≥ 3 variables) : `var[0]` = hash,
//!   `var[2]` = texte localisé **BRUT** (tags `<MNT:…>` / furigana `[漢字/かな]` **conservés** —
//!   fidélité aux octets source, **pas** de `sanitize_text`).
//! - **Washa** (`TEXT_WASHA_MAP_BEGIN_0` → `TEXT_WASHA_MAP_<n>`, ≥ 16 variables) : `var[0]` =
//!   hash, `var[5]` = flag lip-sync (String, ex. `"no_lip"`), `var[15]` = label canonique
//!   `eventId_bloc_ligne`. Une chaîne vide donne `None`.
//!
//! Le hash (`var[0]`, Int signé invariant inter-langue) est la **clé de jointure** entre les
//! trois familles.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::walk_named;
use crate::hash::HashId;

/// Une ligne de sous-titre timecodée (port de `ParsedSubtitleRow`).
///
/// Contient des `f64` → pas de `Eq`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubtitleRow {
    /// `var[0]` — hash texte (clé de jointure).
    pub text_hash: HashId,
    /// `var[1]` — début d'affichage (s).
    pub show_start: f64,
    /// `var[2]` — fin d'affichage (s).
    pub show_end: f64,
    /// `var[3]` — timing 3 (s).
    pub t3: f64,
    /// `var[4]` — timing 4 (s).
    pub t4: f64,
}

/// Une ligne de dialogue localisé brut (port de l'entrée de `loadTextMap`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventTextLine {
    /// `var[0]` — hash texte (clé de jointure).
    pub text_hash: HashId,
    /// `var[2]` — texte localisé **brut** (tags + furigana conservés).
    pub raw_text: String,
}

/// Une entrée de la table washa (port de `WashaEntry`, + le hash).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WashaEntry {
    /// `var[0]` — hash texte (clé de jointure).
    pub text_hash: HashId,
    /// `var[5]` — flag lip-sync (`None` si vide).
    pub lip_sync: Option<String>,
    /// `var[15]` — label canonique `eventId_bloc_ligne` (`None` si vide).
    pub label: Option<String>,
}

/// Parse un fichier `Subtitle_<id>` désérialisé → lignes timecodées (port de `parseSubtitleFile`).
#[must_use]
pub fn parse_subtitle_file(root: &Value) -> Vec<SubtitleRow> {
    let mut out = Vec::new();
    walk_named(root, "EV_SUBTITLE_DATA_", |node| {
        let name = node.name();
        if name.contains("LIST") || name.contains("BEG") {
            return;
        }
        if node.var_count() >= 5 {
            out.push(SubtitleRow {
                text_hash: node.hash(0),
                show_start: node.var(1).map_or(0.0, |v| v.as_f64()),
                show_end: node.var(2).map_or(0.0, |v| v.as_f64()),
                t3: node.var(3).map_or(0.0, |v| v.as_f64()),
                t4: node.var(4).map_or(0.0, |v| v.as_f64()),
            });
        }
    });
    out
}

/// Parse un fichier de dialogue `text/<lang>/event/<id>` désérialisé → lignes brutes
/// (port de `loadTextMap`). Le texte de `var[2]` est conservé **tel quel** (pas de nettoyage).
#[must_use]
pub fn parse_event_text(root: &Value) -> Vec<EventTextLine> {
    let mut out = Vec::new();
    walk_named(root, "TEXT_INFO_", |node| {
        if node.name().contains("BEGIN") || node.var_count() < 3 {
            return;
        }
        out.push(EventTextLine {
            text_hash: node.hash(0),
            raw_text: node.string(2).into(),
        });
    });
    out
}

/// Parse une table washa `text/event/<id>_map` désérialisée → entrées (port de `loadWashaMap`).
#[must_use]
pub fn parse_washa_map(root: &Value) -> Vec<WashaEntry> {
    let mut out = Vec::new();
    walk_named(root, "TEXT_WASHA_MAP_", |node| {
        if node.name().contains("BEGIN") || node.var_count() < 16 {
            return;
        }
        let non_empty = |s: &str| (!s.is_empty()).then(|| String::from(s));
        out.push(WashaEntry {
            text_hash: node.hash(0),
            lip_sync: non_empty(node.string(5)),
            label: non_empty(node.string(15)),
        });
    });
    out
}

/// Résout le texte brut d'un hash dans une liste issue de [`parse_event_text`] (premier match).
#[must_use]
pub fn find_event_text(lines: &[EventTextLine], text_hash: HashId) -> Option<&str> {
    lines
        .iter()
        .find(|l| l.text_hash == text_hash)
        .map(|l| l.raw_text.as_str())
}
