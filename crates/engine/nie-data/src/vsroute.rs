//! Port Rust de la famille `vsroute/*.cfg.bin.json` (Level-5 IEVR) — les
//! parcours de matchs « chronique » (Chronicle VS Route).
//!
//! ## Vérité terrain
//!
//! Trois fichiers dans `data/common/gamedata/vsroute/` :
//!
//! ### `chronicle_vs_route_config_*.cfg.bin.json` (format `lists`, version 100)
//!
//! Deux versions présentes (golden = `5.00.30`, la plus récente ;
//! `2.00.16` plus ancienne). Sept listes :
//!
//! | liste | typeName | rôle |
//! |-------|----------|------|
//! | `m_unlockPieceInfoList` | `UNLOCK_PIECE_INFO` | déblocages de cases |
//! | `m_pieceMoveRouteInfoList` | `PIECE_MOVE_ROUTE_INFO` | arêtes de déplacement (4 directions) |
//! | `m_pieceEventInfoList` | `PIECE_EVENT_INFO` | événements bustup avant/après match |
//! | `m_pieceInfoList` | `PIECE_INFO` | cases du plateau (nœud central) |
//! | `m_chronicleVsRouteInfoList` | `CHRONICLE_VS_ROUTE_INFO` | les routes (plage de cases) |
//! | `m_pieceIconInfoList` | `PIECE_ICON_INFO` | icônes de cases |
//! | `m_gateOpenInfoList` | `GATE_OPEN_INFO` | portes verrouillées |
//!
//! Les listes `PIECE_INFO` et `CHRONICLE_VS_ROUTE_INFO` référencent les autres
//! listes par couples `[offset, count]` (résolus via [`pieces_of`](ChronicleVsRouteConfig::pieces_of),
//! [`move_routes_of`](ChronicleVsRouteConfig::move_routes_of), etc.).
//!
//! ### `chronicle_vs_route_opponent_info_0.00.00.cfg.bin.json` (format `lists`, version 100)
//!
//! Une liste `m_vsRouteOpponentInfoList` (`VS_ROUTE_OPPONENT_INFO`) : les
//! adversaires associés à un `battleId`, avec 4 conditions opaques.
//!
//! ### `chronicle_vs_route_trigger_0.04.78.cfg.bin.json` (format `entries`)
//!
//! Un nœud `DATA_COUNT_0` (var\[0\] = nombre d'items) suivi de N nœuds
//! `DATA_ITEM_N` à 7 variables positionnelles. Déclencheurs de la route.
//!
//! ## Zones d'ombre honnêtes
//!
//! - Les champs `condition*` / `cond*` sont des blobs binaires encodés en base64
//!   (sémantique non décodée ; conservés tels quels comme [`String`]). Le décodeur
//!   inagle laisse parfois fuiter une sentinelle (caractère U+FFFD + nom de type)
//!   à la place d'une chaîne vide ; on la normalise en `""` (voir [`clean`]).
//! - `piecePos` / `piecePosOffset` sont deux `f32` little-endian empaquetés en hex
//!   (ex. `"000040C00000A0C0"`) ; conservés en hex brut (non décodés faute de source).
//! - La sémantique des 7 variables de `DATA_ITEM` n'est pas documentée par inagle
//!   (`buildVsRouteDatabase` ne lit que `m_chronicleVsRouteInfoList`) : nommage
//!   positionnel honnête.
//!
//! Source de référence inagle : `packages/inagle/src/parsers/gameplay-config.ts`
//! (`buildVsRouteDatabase`) — ne lit que `m_chronicleVsRouteInfoList` ; la structure
//! complète ci-dessous est reconstruite octet-pour-octet depuis les dumps réels.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::{
    Node, field_bool, field_hash, field_i64, field_str, list_values, owned, walk_named,
};
use crate::hash::HashId;

// ─── Helpers locaux ──────────────────────────────────────────────────────────

/// Normalise une chaîne du dump : la sentinelle inagle (chaîne vide encodée en
/// `U+FFFD` + nom de type qui fuit) devient `""`. Sinon, la chaîne est conservée.
#[must_use]
fn clean(s: &str) -> String {
    if s.starts_with('\u{FFFD}') {
        String::new()
    } else {
        owned(s)
    }
}

/// Lit un couple `[offset, count]` d'un champ tableau JSON (`(0, 0)` si absent).
#[must_use]
fn field_pair(v: &Value, key: &str) -> (i64, i64) {
    let arr = v.get(key).and_then(Value::as_array);
    let off = arr
        .and_then(|a| a.first())
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cnt = arr
        .and_then(|a| a.get(1))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (off, cnt)
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG — chronicle_vs_route_config
// ═══════════════════════════════════════════════════════════════════════════════

// ─── UnlockPieceInfo ───────────────────────────────────────────────────────────

/// Entrée `m_unlockPieceInfoList` (`UNLOCK_PIECE_INFO`) — déblocage d'une case.
///
/// Vérité terrain : `chronicle_vs_route_config_5.00.30` `m_unlockPieceInfoList[0]`
/// = `{unlockPieceId: 0x009FC304, unlockEvent: true, dlcNo: 0}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnlockPieceInfo {
    /// `unlockPieceId` — case débloquée.
    pub unlock_piece_id: HashId,
    /// `unlockEvent` — `true` si le déblocage déclenche un événement.
    pub unlock_event: bool,
    /// `dlcNo` — numéro de DLC requis (0 = jeu de base).
    pub dlc_no: i64,
}

impl UnlockPieceInfo {
    /// Parse une entrée. `None` si `unlockPieceId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let unlock_piece_id = field_hash(v, "unlockPieceId");
        if unlock_piece_id.is_zero() {
            return None;
        }
        Some(Self {
            unlock_piece_id,
            unlock_event: field_bool(v, "unlockEvent").unwrap_or(false),
            dlc_no: field_i64(v, "dlcNo").unwrap_or(0),
        })
    }
}

// ─── PieceMoveRouteInfo ──────────────────────────────────────────────────────

/// Entrée `m_pieceMoveRouteInfoList` (`PIECE_MOVE_ROUTE_INFO`) — arêtes de
/// déplacement d'une case vers les 4 diagonales (HautGauche/BasGauche/HautDroite/BasDroite).
///
/// Vérité terrain : `m_pieceMoveRouteInfoList[1]`
/// = `{moveRouteLU: 0x009FC304, moveRouteLD: 0, moveRouteRU: 0x32A9A186, moveRouteRD: 0}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PieceMoveRouteInfo {
    /// `moveRouteLU` — case en haut à gauche (`HashId::ZERO` si aucune).
    pub move_route_lu: HashId,
    /// `moveRouteLD` — case en bas à gauche.
    pub move_route_ld: HashId,
    /// `moveRouteRU` — case en haut à droite.
    pub move_route_ru: HashId,
    /// `moveRouteRD` — case en bas à droite.
    pub move_route_rd: HashId,
}

impl PieceMoveRouteInfo {
    /// Parse une entrée (toujours `Some` : une arête entièrement nulle est licite).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            move_route_lu: field_hash(v, "moveRouteLU"),
            move_route_ld: field_hash(v, "moveRouteLD"),
            move_route_ru: field_hash(v, "moveRouteRU"),
            move_route_rd: field_hash(v, "moveRouteRD"),
        })
    }
}

// ─── PieceEventInfo ──────────────────────────────────────────────────────────

/// Entrée `m_pieceEventInfoList` (`PIECE_EVENT_INFO`) — événements bustup
/// avant/après le match d'une case.
///
/// Vérité terrain : `m_pieceEventInfoList[0]`
/// = `{paramCrc1: 0xA04868C3, paramCrc2: 0, paramNum1: 0, paramNum2: 0,
/// bustupEventPreSoccer: "ev21_01000", bustupEventPostWinSoccer: "ev21_01100"}`.
/// Certaines entrées ont des `bustupEvent*` vides (sentinelle normalisée en `""`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PieceEventInfo {
    /// `paramCrc1` — premier paramètre CRC.
    pub param_crc1: HashId,
    /// `paramCrc2` — second paramètre CRC.
    pub param_crc2: HashId,
    /// `paramNum1` — premier paramètre numérique.
    pub param_num1: i64,
    /// `paramNum2` — second paramètre numérique.
    pub param_num2: i64,
    /// `bustupEventPreSoccer` — événement bustup avant le match (`""` si absent).
    pub bustup_event_pre_soccer: String,
    /// `bustupEventPostWinSoccer` — événement bustup après victoire (`""` si absent).
    pub bustup_event_post_win_soccer: String,
}

impl PieceEventInfo {
    /// Parse une entrée (toujours `Some`).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            param_crc1: field_hash(v, "paramCrc1"),
            param_crc2: field_hash(v, "paramCrc2"),
            param_num1: field_i64(v, "paramNum1").unwrap_or(0),
            param_num2: field_i64(v, "paramNum2").unwrap_or(0),
            bustup_event_pre_soccer: clean(field_str(v, "bustupEventPreSoccer").unwrap_or("")),
            bustup_event_post_win_soccer: clean(
                field_str(v, "bustupEventPostWinSoccer").unwrap_or(""),
            ),
        })
    }
}

// ─── PieceInfo ───────────────────────────────────────────────────────────────

/// Entrée `m_pieceInfoList` (`PIECE_INFO`) — une case du plateau de route.
///
/// Nœud central : référence les autres listes via les couples `[offset, count]`
/// `moveRouteInfo`, `unlockPieceInfo`, `eventInfoRef`.
///
/// Vérité terrain : `m_pieceInfoList[0]` (5.00.30)
/// = `{routeType: 0, pieceId: 0x32A9A186, eventType: 6, statusFlagIndex: 1,
/// routeProgressIndex: 1, …, piecePos: "000040C00000A0C0",
/// IconInfoId: 0xDD5767B2, bgmId: 0x3E772F0A, moveRouteInfo: [0, 1], …}`.
///
/// Note version : `bgmId_futureMapCleared` est absent en `2.00.16`
/// (→ `HashId::ZERO`), présent en `5.00.30`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PieceInfo {
    /// `routeType` — type de route (0 = normal, 255 = case spéciale/terminale).
    pub route_type: i64,
    /// `pieceId` — identifiant de la case.
    pub piece_id: HashId,
    /// `eventType` — type d'événement de la case.
    pub event_type: i64,
    /// `statusFlagIndex` — index du flag d'état.
    pub status_flag_index: i64,
    /// `routeProgressIndex` — index de progression sur la route.
    pub route_progress_index: i64,
    /// `addDlcNo` — numéro de DLC ajoutant cette case (0 = jeu de base).
    pub add_dlc_no: i64,
    /// `piecePos` — position empaquetée (deux `f32` LE en hex, non décodés).
    pub piece_pos: String,
    /// `piecePosOffset` — décalage de position empaqueté (idem, hex brut).
    pub piece_pos_offset: String,
    /// `IconInfoId` — référence vers `m_pieceIconInfoList`.
    pub icon_info_id: HashId,
    /// `IconInfoIndex` — index résolu de l'icône.
    pub icon_info_index: i64,
    /// `bgmId` — musique de fond de la case.
    pub bgm_id: HashId,
    /// `bgmId_futureMapCleared` — BGM alternatif (carte du futur nettoyée ; absent en 2.00.16).
    pub bgm_id_future_map_cleared: HashId,
    /// `canObtainGateKey` — `true` si la case donne une clé de porte.
    pub can_obtain_gate_key: bool,
    /// `gateKeyUsePieceId` — case consommant la clé de porte.
    pub gate_key_use_piece_id: HashId,
    /// `gateOpenInfoId` — référence vers `m_gateOpenInfoList`.
    pub gate_open_info_id: HashId,
    /// `gateOpenInfoIndex` — index résolu de la porte.
    pub gate_open_info_index: i64,
    /// `condition` — blob de condition base64 (sentinelle normalisée en `""`).
    pub condition: String,
    /// `moveRouteInfo[0]` — offset dans `m_pieceMoveRouteInfoList`.
    pub move_route_offset: i64,
    /// `moveRouteInfo[1]` — nombre d'arêtes.
    pub move_route_count: i64,
    /// `unlockPieceInfo[0]` — offset dans `m_unlockPieceInfoList`.
    pub unlock_piece_offset: i64,
    /// `unlockPieceInfo[1]` — nombre de déblocages.
    pub unlock_piece_count: i64,
    /// `eventInfoRef[0]` — offset dans `m_pieceEventInfoList`.
    pub event_info_offset: i64,
    /// `eventInfoRef[1]` — nombre d'événements.
    pub event_info_count: i64,
    /// `showNewRouteEffect` — `true` si un effet « nouvelle route » s'affiche.
    pub show_new_route_effect: bool,
    /// `needUpdate` — `true` si la case nécessite une mise à jour.
    pub need_update: bool,
    /// `updateLockPieceId` — case verrouillant la mise à jour.
    pub update_lock_piece_id: HashId,
    /// `lockedGlobalBitFlagId` — flag global de verrouillage.
    pub locked_global_bit_flag_id: HashId,
}

impl PieceInfo {
    /// Parse une entrée. `None` si `pieceId` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let piece_id = field_hash(v, "pieceId");
        if piece_id.is_zero() {
            return None;
        }
        let (move_route_offset, move_route_count) = field_pair(v, "moveRouteInfo");
        let (unlock_piece_offset, unlock_piece_count) = field_pair(v, "unlockPieceInfo");
        let (event_info_offset, event_info_count) = field_pair(v, "eventInfoRef");
        Some(Self {
            route_type: field_i64(v, "routeType").unwrap_or(0),
            piece_id,
            event_type: field_i64(v, "eventType").unwrap_or(0),
            status_flag_index: field_i64(v, "statusFlagIndex").unwrap_or(0),
            route_progress_index: field_i64(v, "routeProgressIndex").unwrap_or(0),
            add_dlc_no: field_i64(v, "addDlcNo").unwrap_or(0),
            piece_pos: owned(field_str(v, "piecePos").unwrap_or("")),
            piece_pos_offset: owned(field_str(v, "piecePosOffset").unwrap_or("")),
            icon_info_id: field_hash(v, "IconInfoId"),
            icon_info_index: field_i64(v, "IconInfoIndex").unwrap_or(0),
            bgm_id: field_hash(v, "bgmId"),
            bgm_id_future_map_cleared: field_hash(v, "bgmId_futureMapCleared"),
            can_obtain_gate_key: field_bool(v, "canObtainGateKey").unwrap_or(false),
            gate_key_use_piece_id: field_hash(v, "gateKeyUsePieceId"),
            gate_open_info_id: field_hash(v, "gateOpenInfoId"),
            gate_open_info_index: field_i64(v, "gateOpenInfoIndex").unwrap_or(0),
            condition: clean(field_str(v, "condition").unwrap_or("")),
            move_route_offset,
            move_route_count,
            unlock_piece_offset,
            unlock_piece_count,
            event_info_offset,
            event_info_count,
            show_new_route_effect: field_bool(v, "showNewRouteEffect").unwrap_or(false),
            need_update: field_bool(v, "needUpdate").unwrap_or(false),
            update_lock_piece_id: field_hash(v, "updateLockPieceId"),
            locked_global_bit_flag_id: field_hash(v, "lockedGlobalBitFlagId"),
        })
    }
}

// ─── ChronicleVsRouteInfo ────────────────────────────────────────────────────

/// Entrée `m_chronicleVsRouteInfoList` (`CHRONICLE_VS_ROUTE_INFO`) — une route.
///
/// `pieceInfo` = `[offset, count]` pointe une plage de [`PieceInfo`].
///
/// Vérité terrain : `m_chronicleVsRouteInfoList[0]`
/// = `{id: 0x72978C53, routeMapType: 0, pieceInfo: [0, 15]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChronicleVsRouteInfo {
    /// `id` — identifiant de la route (recoupe `quest_title` côté texte).
    pub id: HashId,
    /// `routeMapType` — type de carte (0 = standard, 1 = carte alternative).
    pub route_map_type: i64,
    /// `pieceInfo[0]` — offset dans `m_pieceInfoList`.
    pub piece_offset: i64,
    /// `pieceInfo[1]` — nombre de cases de la route.
    pub piece_count: i64,
}

impl ChronicleVsRouteInfo {
    /// Parse une entrée. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        let (piece_offset, piece_count) = field_pair(v, "pieceInfo");
        Some(Self {
            id,
            route_map_type: field_i64(v, "routeMapType").unwrap_or(0),
            piece_offset,
            piece_count,
        })
    }
}

// ─── PieceIconInfo ───────────────────────────────────────────────────────────

/// Entrée `m_pieceIconInfoList` (`PIECE_ICON_INFO`) — icône d'une case.
///
/// Vérité terrain : `m_pieceIconInfoList[2]`
/// = `{id: 0xB659ACF7, textureName: 0x24988232, texturePath: 0x3C637EBE}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PieceIconInfo {
    /// `id` — identifiant de l'icône (référencé par `PieceInfo.icon_info_id`).
    pub id: HashId,
    /// `textureName` — hash du nom de texture.
    pub texture_name: HashId,
    /// `texturePath` — hash du chemin de texture.
    pub texture_path: HashId,
}

impl PieceIconInfo {
    /// Parse une entrée. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            texture_name: field_hash(v, "textureName"),
            texture_path: field_hash(v, "texturePath"),
        })
    }
}

// ─── GateOpenInfo ────────────────────────────────────────────────────────────

/// Entrée `m_gateOpenInfoList` (`GATE_OPEN_INFO`) — une porte verrouillée.
///
/// Vérité terrain : `m_gateOpenInfoList[0]`
/// = `{id: 0xF9076F90, unlockFlag1: 0x0DFFC8B6, unlockFlag2: 0x94F6990C,
/// unlockFlag3: 0, condition1: "AAAAACEFNcl4...", condition2: "...", condition3: ""}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GateOpenInfo {
    /// `id` — identifiant de la porte (référencé par `PieceInfo.gate_open_info_id`).
    pub id: HashId,
    /// `unlockFlag1` — premier flag de déverrouillage.
    pub unlock_flag1: HashId,
    /// `unlockFlag2` — deuxième flag de déverrouillage.
    pub unlock_flag2: HashId,
    /// `unlockFlag3` — troisième flag de déverrouillage.
    pub unlock_flag3: HashId,
    /// `condition1` — blob de condition base64 (sentinelle normalisée en `""`).
    pub condition1: String,
    /// `condition2` — blob de condition base64.
    pub condition2: String,
    /// `condition3` — blob de condition base64.
    pub condition3: String,
}

impl GateOpenInfo {
    /// Parse une entrée. `None` si `id` est nul.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = field_hash(v, "id");
        if id.is_zero() {
            return None;
        }
        Some(Self {
            id,
            unlock_flag1: field_hash(v, "unlockFlag1"),
            unlock_flag2: field_hash(v, "unlockFlag2"),
            unlock_flag3: field_hash(v, "unlockFlag3"),
            condition1: clean(field_str(v, "condition1").unwrap_or("")),
            condition2: clean(field_str(v, "condition2").unwrap_or("")),
            condition3: clean(field_str(v, "condition3").unwrap_or("")),
        })
    }
}

// ─── ChronicleVsRouteConfig ──────────────────────────────────────────────────

/// Contenu complet d'un `chronicle_vs_route_config_*.cfg.bin.json` (7 listes).
///
/// Utiliser [`parse_chronicle_vs_route_config`] pour construire depuis un JSON.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChronicleVsRouteConfig {
    /// `m_unlockPieceInfoList`.
    pub unlock_pieces: Vec<UnlockPieceInfo>,
    /// `m_pieceMoveRouteInfoList`.
    pub move_routes: Vec<PieceMoveRouteInfo>,
    /// `m_pieceEventInfoList`.
    pub events: Vec<PieceEventInfo>,
    /// `m_pieceInfoList`.
    pub pieces: Vec<PieceInfo>,
    /// `m_chronicleVsRouteInfoList`.
    pub routes: Vec<ChronicleVsRouteInfo>,
    /// `m_pieceIconInfoList`.
    pub icons: Vec<PieceIconInfo>,
    /// `m_gateOpenInfoList`.
    pub gates: Vec<GateOpenInfo>,
}

impl ChronicleVsRouteConfig {
    /// Renvoie la tranche de [`PieceInfo`] appartenant à une route.
    ///
    /// Retourne `&[]` si la plage dépasse la taille de la liste.
    #[must_use]
    pub fn pieces_of(&self, route: &ChronicleVsRouteInfo) -> &[PieceInfo] {
        let start = route.piece_offset as usize;
        let end = start.saturating_add(route.piece_count as usize);
        self.pieces.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de [`PieceMoveRouteInfo`] d'une case.
    #[must_use]
    pub fn move_routes_of(&self, piece: &PieceInfo) -> &[PieceMoveRouteInfo] {
        let start = piece.move_route_offset as usize;
        let end = start.saturating_add(piece.move_route_count as usize);
        self.move_routes.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de [`UnlockPieceInfo`] d'une case.
    #[must_use]
    pub fn unlock_pieces_of(&self, piece: &PieceInfo) -> &[UnlockPieceInfo] {
        let start = piece.unlock_piece_offset as usize;
        let end = start.saturating_add(piece.unlock_piece_count as usize);
        self.unlock_pieces.get(start..end).unwrap_or(&[])
    }

    /// Renvoie la tranche de [`PieceEventInfo`] d'une case.
    #[must_use]
    pub fn events_of(&self, piece: &PieceInfo) -> &[PieceEventInfo] {
        let start = piece.event_info_offset as usize;
        let end = start.saturating_add(piece.event_info_count as usize);
        self.events.get(start..end).unwrap_or(&[])
    }

    /// Recherche une case par `pieceId`. `None` si absente.
    #[must_use]
    pub fn find_piece(&self, id: HashId) -> Option<&PieceInfo> {
        self.pieces.iter().find(|p| p.piece_id == id)
    }

    /// Recherche une icône par `id`. `None` si absente.
    #[must_use]
    pub fn find_icon(&self, id: HashId) -> Option<&PieceIconInfo> {
        self.icons.iter().find(|i| i.id == id)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OPPONENT — chronicle_vs_route_opponent_info
// ═══════════════════════════════════════════════════════════════════════════════

/// Entrée `m_vsRouteOpponentInfoList` (`VS_ROUTE_OPPONENT_INFO`) — un adversaire.
///
/// Vérité terrain : `chronicle_vs_route_opponent_info_0.00.00`
/// `m_vsRouteOpponentInfoList[0]`
/// = `{category: 0, battleId: 0xD750C819, sortOrder: 1,
/// cond1: "AAAAABgFNaNVuJ8...", cond2: "", cond3: "", cond4: ""}`.
///
/// Note : ce dump contient des entrées « vides » (`battleId = 0`) — celles-ci
/// sont conservées car `sortOrder` peut différer (pas de clé valide à filtrer) ;
/// voir [`parse_opponent_info`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VsRouteOpponentInfo {
    /// `category` — catégorie d'adversaire.
    pub category: i64,
    /// `battleId` — identifiant du combat (peut être `HashId::ZERO` pour une entrée vide).
    pub battle_id: HashId,
    /// `sortOrder` — ordre d'affichage.
    pub sort_order: i64,
    /// `cond1` — blob de condition base64 (sentinelle normalisée en `""`).
    pub cond1: String,
    /// `cond2` — blob de condition base64.
    pub cond2: String,
    /// `cond3` — blob de condition base64.
    pub cond3: String,
    /// `cond4` — blob de condition base64.
    pub cond4: String,
}

impl VsRouteOpponentInfo {
    /// Parse une entrée (toujours `Some` : les entrées à `battleId` nul sont licites).
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            category: field_i64(v, "category").unwrap_or(0),
            battle_id: field_hash(v, "battleId"),
            sort_order: field_i64(v, "sortOrder").unwrap_or(0),
            cond1: clean(field_str(v, "cond1").unwrap_or("")),
            cond2: clean(field_str(v, "cond2").unwrap_or("")),
            cond3: clean(field_str(v, "cond3").unwrap_or("")),
            cond4: clean(field_str(v, "cond4").unwrap_or("")),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIGGER — chronicle_vs_route_trigger (format `entries`)
// ═══════════════════════════════════════════════════════════════════════════════

/// Nœud `DATA_ITEM_N` du `chronicle_vs_route_trigger_*.cfg.bin.json` — un
/// déclencheur de route (7 variables positionnelles, sémantique non documentée).
///
/// Vérité terrain : `DATA_ITEM_0`
/// = `[505, 0, 0, "AAAAABgFNZ6ZhIw…", 0, 0, 1]` ;
/// `DATA_ITEM_1` = `[504, 428143173, -186917087, "AAAAABgF…", 2, 0, 2]`.
///
/// Le nœud `DATA_COUNT_0` (var\[0\] = 63) précède la liste et donne le compte.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VsRouteTrigger {
    /// var\[0\] — code de type de déclencheur (502..=506 dans le dump réel).
    pub kind: i64,
    /// var\[1\] — premier paramètre hash (`HashId::ZERO` si nul).
    pub param1: HashId,
    /// var\[2\] — second paramètre hash.
    pub param2: HashId,
    /// var\[3\] — blob de condition base64, ou `"0"` si absent (chaîne brute).
    pub condition: String,
    /// var\[4\] — premier argument numérique.
    pub arg1: i64,
    /// var\[5\] — second argument numérique.
    pub arg2: i64,
    /// var\[6\] — troisième argument numérique (souvent l'ordre séquentiel).
    pub arg3: i64,
}

impl VsRouteTrigger {
    /// Parse un nœud `DATA_ITEM_N`.
    #[must_use]
    pub fn from_node(node: Node<'_>) -> Self {
        Self {
            kind: node.int(0),
            param1: node.hash(1),
            param2: node.hash(2),
            condition: clean(node.string(3)),
            arg1: node.int(4),
            arg2: node.int(5),
            arg3: node.int(6),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Parseurs principaux
// ═══════════════════════════════════════════════════════════════════════════════

/// Parse un `chronicle_vs_route_config_*.cfg.bin.json` complet (7 listes).
///
/// Vérité terrain : `5.00.30` → 235 unlock, 238 move_routes, 159 events,
/// 238 pieces, 10 routes, 143 icons, 1 gate. `2.00.16` → 220/222/151/223/8/136/1.
#[must_use]
pub fn parse_chronicle_vs_route_config(root: &Value) -> ChronicleVsRouteConfig {
    ChronicleVsRouteConfig {
        unlock_pieces: extract_list(root, "m_unlockPieceInfoList", UnlockPieceInfo::from_value),
        move_routes: extract_list(
            root,
            "m_pieceMoveRouteInfoList",
            PieceMoveRouteInfo::from_value,
        ),
        events: extract_list(root, "m_pieceEventInfoList", PieceEventInfo::from_value),
        pieces: extract_list(root, "m_pieceInfoList", PieceInfo::from_value),
        routes: extract_list(
            root,
            "m_chronicleVsRouteInfoList",
            ChronicleVsRouteInfo::from_value,
        ),
        icons: extract_list(root, "m_pieceIconInfoList", PieceIconInfo::from_value),
        gates: extract_list(root, "m_gateOpenInfoList", GateOpenInfo::from_value),
    }
}

/// Parse un `chronicle_vs_route_opponent_info_*.cfg.bin.json`.
///
/// Renvoie la liste `m_vsRouteOpponentInfoList`. Vérité terrain : 4 entrées
/// (dont 3 à `battleId` nul, conservées).
#[must_use]
pub fn parse_opponent_info(root: &Value) -> Vec<VsRouteOpponentInfo> {
    extract_list(
        root,
        "m_vsRouteOpponentInfoList",
        VsRouteOpponentInfo::from_value,
    )
}

/// Parse un `chronicle_vs_route_trigger_*.cfg.bin.json` (format `entries`).
///
/// Renvoie tous les `DATA_ITEM_N`. Le nœud `DATA_COUNT_0` est ignoré.
/// Vérité terrain : 63 `DATA_ITEM` (`DATA_COUNT_0.var[0] = 63`).
#[must_use]
pub fn parse_trigger(root: &Value) -> Vec<VsRouteTrigger> {
    let mut out = Vec::new();
    walk_named(root, "DATA_ITEM", |node| {
        out.push(VsRouteTrigger::from_node(node));
    });
    out
}

/// Parcourt une liste nommée du JSON et applique `f` à chaque entrée.
fn extract_list<T, F>(root: &Value, name: &str, f: F) -> Vec<T>
where
    F: Fn(&Value) -> Option<T>,
{
    let mut out = Vec::new();
    if let Some(values) = list_values(root, name) {
        for v in values {
            if let Some(item) = f(v) {
                out.push(item);
            }
        }
    }
    out
}
