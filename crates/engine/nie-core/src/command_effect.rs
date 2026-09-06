//! Tables de slots d'effets de commande (passive skills, tactiques, super-
//! tactiques, gimmicks de stade, team build).
//!
//! Squelette des conteneurs runtime qui reçoivent les effets pendant la sim.
//! Capacités / strides / sentinelles EXACTES du constructeur décompilé.
//!
//! # Sources RE
//!
//! - C décompilé : `refs/iecode-re/research/ghidra-export/decompiled/soccer_command_effect.c`
//!   (`FUN_1403f6d60`, counts/strides L24-30, corps L62-199).
//! - Données d'effet réelles : `common/gamedata/skill/passive_skill_effect_config.cfg.bin.json`
//!   — `m_soccerPassiveSkillEffectList`=8 (effectId + effectParam1..8),
//!   `m_soccerPassiveSkillEffectInfoList`=5, `m_soccerPassiveSkillEffectRangeList`=1.
//!   Schéma porté : `packages/inagle/src/parsers/passive-skill-effect-config.ts`.

// ============================================================================
// Constantes EXACTES du constructeur décompilé (soccer_command_effect.c)
// ============================================================================

/// Slots de tactiques spéciales : `0x20` = 32 (L64).
pub const SPECIAL_TACTICS_COUNT: usize = 0x20;
/// Stride d'un slot `SpecialTactics` : `0xC` bytes (L69).
pub const SPECIAL_TACTICS_STRIDE: usize = 0xC;
/// Flags par défaut `SpecialTactics` : `0x304` (L68).
pub const SPECIAL_TACTICS_FLAGS: u16 = 0x304;

/// Slots de super-tactiques : `8` (L91, boucle déroulée 8×).
pub const SUPER_TACTICS_COUNT: usize = 8;
/// Type d'effet par défaut `SuperTactics` : `3` (L93).
pub const SUPER_TACTICS_TYPE: u8 = 3;

/// Slots de passive skills : `0x4C2` = 1218 (L122).
pub const PASSIVE_SKILL_COUNT: usize = 0x4C2;
/// Stride d'un slot `PassiveSkill` : `0x14` bytes (L129).
pub const PASSIVE_SKILL_STRIDE: usize = 0x14;
/// Flags par défaut `PassiveSkill` : `0x307` (L125).
pub const PASSIVE_SKILL_FLAGS: u16 = 0x307;

/// Slots de gimmicks de stade : `0x1D0` = 464 (L154).
pub const STADIUM_GIMMICK_COUNT: usize = 0x1D0;
/// Stride d'un struct d'effet `StadiumGimmick` : `0xE8` bytes (L154).
pub const STADIUM_GIMMICK_STRIDE: usize = 0xE8;

/// Slots de l'effet de commande principal : `0x540` = 1344 (L167).
pub const COMMAND_EFFECT_COUNT: usize = 0x540;
/// Stride d'un struct d'effet principal : `0xE8` bytes (L167).
pub const COMMAND_EFFECT_STRIDE: usize = 0xE8;

/// Slots de team build : `8` (L196).
pub const TEAM_BUILD_COUNT: usize = 8;

/// Sentinelle d'ID invalide sur un octet (`SpecialTactics`/`SuperTactics`/Gimmick).
pub const ID_INVALID_U8: u8 = 0xFF;
/// Sentinelle d'ID invalide sur 32 bits (`PassiveSkill` / `TeamBuild`).
pub const ID_INVALID_U32: u32 = 0xFFFF_FFFF;

// ============================================================================
// Slots
// ============================================================================

/// Slot de tactique spéciale (stride 0xC, id 0xFF, flags 0x304).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecialTacticsSlot {
    /// ID (octet bas) — `0xFF` à l'init (L66).
    pub id: u8,
    /// Données (dword) — `0` à l'init (L67).
    pub data: u32,
    /// Flags (word) — `0x304` à l'init (L68).
    pub flags: u16,
}

impl Default for SpecialTacticsSlot {
    fn default() -> Self {
        Self {
            id: ID_INVALID_U8,
            data: 0,
            flags: SPECIAL_TACTICS_FLAGS,
        }
    }
}

/// Slot de super-tactique (id 0xFF, type 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperTacticsSlot {
    /// ID (octet bas) — `0xFF` à l'init (L92…).
    pub id: u8,
    /// Type d'effet — `3` à l'init (L93…).
    pub effect_type: u8,
}

impl Default for SuperTacticsSlot {
    fn default() -> Self {
        Self {
            id: ID_INVALID_U8,
            effect_type: SUPER_TACTICS_TYPE,
        }
    }
}

/// Slot de passive skill (stride 0x14, id 0xFFFFFFFF, flags 0x307).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassiveSkillSlot {
    /// ID (dword) — `0xFFFFFFFF` à l'init (L128).
    pub id: u32,
    /// Flags (word) — `0x307` à l'init (L125).
    pub flags: u16,
}

impl Default for PassiveSkillSlot {
    fn default() -> Self {
        Self {
            id: ID_INVALID_U32,
            flags: PASSIVE_SKILL_FLAGS,
        }
    }
}

/// Slot de gimmick de stade (id 0xFF, type 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StadiumGimmickSlot {
    /// ID (octet bas) — `0xFF` à l'init (L149).
    pub id: u8,
    /// Type d'effet — `3` à l'init (L150).
    pub effect_type: u8,
}

impl Default for StadiumGimmickSlot {
    fn default() -> Self {
        Self {
            id: ID_INVALID_U8,
            effect_type: 3,
        }
    }
}

/// Slot de team build (id 0xFFFFFFFF + octet bas 0xFF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamBuildSlot {
    /// ID (dword) — `0xFFFFFFFF` à l'init (L177…).
    pub id: u32,
    /// Octet de marquage — `0xFF` à l'init (L178…).
    pub marker: u8,
}

impl Default for TeamBuildSlot {
    fn default() -> Self {
        Self {
            id: ID_INVALID_U32,
            marker: ID_INVALID_U8,
        }
    }
}

/// Conteneur runtime des effets de commande de match.
///
/// Chaque catégorie est un `Vec` borné aux capacités exactes décompilées,
/// initialisé aux sentinelles `Default` de chaque slot.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoccerCommandEffect {
    /// 32 slots de tactiques spéciales.
    pub special_tactics: Vec<SpecialTacticsSlot>,
    /// 8 slots de super-tactiques.
    pub super_tactics: Vec<SuperTacticsSlot>,
    /// 1218 slots de passive skills.
    pub passive_skill: Vec<PassiveSkillSlot>,
    /// 464 slots de gimmicks de stade.
    pub stadium_gimmick: Vec<StadiumGimmickSlot>,
    /// 1344 slots d'effet de commande principal.
    pub command_effect: Vec<PassiveSkillSlot>,
    /// 8 slots de team build.
    pub team_build: Vec<TeamBuildSlot>,
}

impl Default for SoccerCommandEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl SoccerCommandEffect {
    /// Crée le conteneur avec toutes les catégories initialisées aux sentinelles.
    ///
    /// Reproduit l'effet du constructeur `FUN_1403f6d60` : chaque tableau est
    /// alloué à sa capacité exacte et chaque slot est posé à sa sentinelle.
    /// Le slot principal `command_effect` réutilise le layout passive-skill
    /// (id 0xFFFFFFFF) car le ctor le traite comme un `CommandEffectBase`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            special_tactics: vec![SpecialTacticsSlot::default(); SPECIAL_TACTICS_COUNT],
            super_tactics: vec![SuperTacticsSlot::default(); SUPER_TACTICS_COUNT],
            passive_skill: vec![PassiveSkillSlot::default(); PASSIVE_SKILL_COUNT],
            stadium_gimmick: vec![StadiumGimmickSlot::default(); STADIUM_GIMMICK_COUNT],
            command_effect: vec![PassiveSkillSlot::default(); COMMAND_EFFECT_COUNT],
            team_build: vec![TeamBuildSlot::default(); TEAM_BUILD_COUNT],
        }
    }
}

// ============================================================================
// Données d'effet passif réelles (passive_skill_effect_config)
// ============================================================================

/// Effet passif : ID + 8 paramètres flottants.
///
/// Source : `m_soccerPassiveSkillEffectList` (passive-skill-effect-config.ts).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassiveSkillEffect {
    /// Hash hex de l'effet (`effectId`).
    pub effect_id: String,
    /// 8 paramètres flottants (`effectParam1..8`).
    pub params: [f32; 8],
}

/// Base de données des effets passifs réels.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassiveSkillEffectDb {
    /// Types de portée (`m_soccerPassiveSkillEffectRangeList`).
    pub ranges: Vec<i32>,
    /// Effets (`m_soccerPassiveSkillEffectList`).
    pub effects: Vec<PassiveSkillEffect>,
}

impl PassiveSkillEffectDb {
    /// Retourne l'effet par `effect_id` (hex), ou `None`.
    #[must_use]
    pub fn get_effect(&self, effect_id: &str) -> Option<&PassiveSkillEffect> {
        self.effects.iter().find(|e| e.effect_id == effect_id)
    }
}

#[cfg(feature = "data")]
mod loader {
    //! Chargement des vrais effets passifs embarqués
    //! (`data/passive_skill_effect.json`).

    use super::{PassiveSkillEffect, PassiveSkillEffectDb};

    /// JSON projeté des effets passifs, embarqué à la compilation.
    const PASSIVE_JSON: &str = include_str!("../data/passive_skill_effect.json");

    #[derive(serde::Deserialize)]
    struct RawDb {
        ranges: Vec<i32>,
        effects: Vec<(String, [f32; 8])>,
    }

    impl PassiveSkillEffectDb {
        /// Charge les vrais effets passifs IEVR embarqués.
        ///
        /// # Panics
        ///
        /// Panique si le JSON embarqué est corrompu (figé à la compilation).
        #[must_use]
        pub fn load_embedded() -> Self {
            let raw: RawDb = serde_json::from_str(PASSIVE_JSON)
                .expect("passive_skill_effect.json embarqué valide");
            Self {
                ranges: raw.ranges,
                effects: raw
                    .effects
                    .into_iter()
                    .map(|(effect_id, params)| PassiveSkillEffect { effect_id, params })
                    .collect(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden : capacités == constantes hex décompilées.
    #[test]
    fn capacities_match_decompiled_constants() {
        let ce = SoccerCommandEffect::new();
        assert_eq!(ce.special_tactics.len(), 0x20); // 32
        assert_eq!(ce.super_tactics.len(), 8);
        assert_eq!(ce.passive_skill.len(), 0x4C2); // 1218
        assert_eq!(ce.stadium_gimmick.len(), 0x1D0); // 464
        assert_eq!(ce.command_effect.len(), 0x540); // 1344
        assert_eq!(ce.team_build.len(), 8);
    }

    /// Golden : un slot fraîchement init == sentinelle + flags décompilés.
    #[test]
    fn fresh_slots_have_sentinels_and_flags() {
        let ce = SoccerCommandEffect::new();

        // SpecialTactics : id 0xFF, flags 0x304.
        let st = ce.special_tactics[0];
        assert_eq!(st.id, 0xFF);
        assert_eq!(st.flags, 0x304);
        assert_eq!(st.data, 0);

        // SuperTactics : id 0xFF, type 3.
        let su = ce.super_tactics[0];
        assert_eq!(su.id, 0xFF);
        assert_eq!(su.effect_type, 3);

        // PassiveSkill : id 0xFFFFFFFF, flags 0x307.
        let ps = ce.passive_skill[0];
        assert_eq!(ps.id, 0xFFFF_FFFF);
        assert_eq!(ps.flags, 0x307);

        // StadiumGimmick : id 0xFF, type 3.
        let sg = ce.stadium_gimmick[0];
        assert_eq!(sg.id, 0xFF);
        assert_eq!(sg.effect_type, 3);

        // TeamBuild : id 0xFFFFFFFF, marker 0xFF.
        let tb = ce.team_build[0];
        assert_eq!(tb.id, 0xFFFF_FFFF);
        assert_eq!(tb.marker, 0xFF);
    }

    /// Vérifie que les strides exposés matchent le décompilé.
    #[test]
    fn strides_match_decompiled() {
        assert_eq!(SPECIAL_TACTICS_STRIDE, 0xC);
        assert_eq!(PASSIVE_SKILL_STRIDE, 0x14);
        assert_eq!(STADIUM_GIMMICK_STRIDE, 0xE8);
        assert_eq!(COMMAND_EFFECT_STRIDE, 0xE8);
    }
}

#[cfg(all(test, feature = "data"))]
mod golden_tests {
    use super::*;

    /// Golden : comptes des listes réelles d'effets passifs.
    #[test]
    fn embedded_passive_counts() {
        let db = PassiveSkillEffectDb::load_embedded();
        assert_eq!(db.effects.len(), 8, "m_soccerPassiveSkillEffectList = 8");
        assert_eq!(
            db.ranges.len(),
            1,
            "m_soccerPassiveSkillEffectRangeList = 1"
        );
    }

    /// Golden : premier effet réel (0x20DFBB4B, param1 = 1.5).
    #[test]
    fn embedded_first_effect() {
        let db = PassiveSkillEffectDb::load_embedded();
        let e = &db.effects[0];
        assert_eq!(e.effect_id, "0x20DFBB4B");
        assert_eq!(e.params[0], 1.5);
        assert_eq!(e.params[1], 0.0);
        // Lookup par id.
        assert!(db.get_effect("0x20DFBB4B").is_some());
        assert!(db.get_effect("0xDEADBEEF").is_none());
    }
}
