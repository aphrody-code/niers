//! `cond` — décodeur du **cadrage (framing)** des blobs de condition d'IEVR.
//!
//! Le même blob base64 « condition » apparaît **partout** dans les données du jeu : `openCond`
//! (gallery), `cond` (happen_event_npc, triggers `DATA_ITEM`), `condition` (trial_take_over),
//! `runCond` (soccer_drop), `aocCondition` (add_content_equip)… C'est le **système d'expressions
//! de condition** générique (déblocages, déclencheurs).
//!
//! ## Couche **cadrage** (ce module) — VALIDÉ contre le corpus réel entier
//!
//! Le **cadrage externe** du blob, prouvé sur **17 788 blobs réels** extraits de
//! `data/common/gamedata/**` (la donnée du jeu = vérité terrain du format,
//! `tests/cond_corpus_golden.rs`) :
//!
//! - `b[0..4]` = **version** (u32 big-endian), toujours `0` ou `1` ;
//! - **version 0** (17 754/17 754, 99,8 %) : `b[4]` = **longueur** du reste (= `len − 5`),
//!   `b[5]` = **opcode** de l'expression, `b[6..]` = charge utile ;
//! - **version 1** (34 blobs, 0,2 %) : **forme liste** (concaténation de sous-expressions) — le
//!   cadrage interne n'est pas encore reversé (`framing_valid_v0` renvoie `false`).
//!
//! ## Couche **sémantique** — voir [`crate::unlock_condition`] (clauses DÉCODÉES)
//!
//! La sémantique des clauses **est décodée** par [`crate::unlock_condition::decode_unlock_condition`]
//! (port 1:1 d'inagle `unlock-condition.ts`) : tokens `0x35`/`0x34`/`0x32`, namespaces story
//! (`0xB91936DA`, seuil = épisode) vs event-flag (CRC32), opcodes single/AND/trivial. **Validé**
//! contre les fixtures inagle **ET** corpus-wide (`cond_corpus_golden` décode les 17 788 blobs sans
//! erreur : 3158 story, 14 191 event-flag, 430 composite, 30 000 feuilles event). Ce module-ci sert
//! d'**inspecteur de cadrage léger** (version/opcode/bornes) ; pour la sémantique, utiliser
//! `unlock_condition`. **RESTE INCOMPLET** : le **groupement** de la forme liste v1 (31 blobs
//! uniques) + l'ancrage ultime contre l'évaluateur binaire `game::ValidConditionManager` (inagle
//! est la référence acceptée).
//!
//! ### Investigation v1 (2026-06-23, sans tricher)
//!
//! Les **valeurs** des feuilles v1 décodent correctement (`unlock_condition` scanne les tokens
//! `0x35/0x34/0x32` à plat depuis l'octet 6, comme v0 — vérifié : les v1 commencent aussi leurs
//! clauses à l'offset 6). Seul le **groupement** (liste de sous-conditions) est aplati. L'hypothèse
//! « v1 = `[version][suite de [len:1][sous-expr]]` » a été **réfutée contre les 31 blobs v1 réels**
//! (0/31 pavent exactement). Le découpage exact exige l'évaluateur binaire → **non deviné**.

use alloc::vec::Vec;

/// Cadrage décodé d'un blob de condition (en-tête + charge utile brute).
///
/// Seul le **cadrage** est interprété ; la charge utile (`payload`) reste **brute** (sa
/// structure de clauses n'est pas reversée — anti-hallucination).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CondBlob {
    /// `b[0..4]` — version du format (u32 **big-endian**) : `0` (expression simple) ou `1` (forme liste).
    pub version: u32,
    /// `b[4]` — longueur déclarée du reste (octets depuis `b[5]`). Pour version 0,
    /// vaut `payload.len() + 1` (validé sur tout le corpus, cf. [`CondBlob::framing_valid_v0`]).
    pub declared_len: u8,
    /// `b[5]` — opcode de l'expression (type d'opération ; sémantique non reversée).
    pub opcode: u8,
    /// `b[6..]` — charge utile **brute** (clauses non décodées).
    pub payload: Vec<u8>,
}

impl CondBlob {
    /// Décode le cadrage d'un blob de condition (octets bruts, déjà base64-décodés).
    ///
    /// `None` si trop court (`< 6` octets). N'interprète **que** l'en-tête ; ne valide pas la
    /// cohérence (utiliser [`CondBlob::framing_valid_v0`] pour ça).
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 6 {
            return None;
        }
        // En-tête **big-endian** : `00 00 00 00` → 0, `00 00 00 01` → 1 (validé sur le corpus réel).
        let version = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Some(Self {
            version,
            declared_len: bytes[4],
            opcode: bytes[5],
            payload: bytes[6..].to_vec(),
        })
    }

    /// Décode le cadrage d'un blob **encore encodé en base64**, tel qu'il apparaît dans les
    /// données du jeu (`openCond`, `cond`, `condition`, `runCond`, `aocCondition`).
    ///
    /// `None` si le base64 est invalide ou si le blob décodé fait moins de 6 octets.
    ///
    /// Le décodeur base64 est celui de [`crate::unlock_condition`], et c'est délibéré : les
    /// deux couches — cadrage ici, sémantique là-bas — lisent le **même** blob, et deux
    /// implémentations de base64 dans la même crate divergeraient au premier caractère de
    /// remplissage mal traité.
    #[must_use]
    pub fn parse_base64(encoded: &str) -> Option<Self> {
        Self::parse(&crate::unlock_condition::decode_base64(encoded)?)
    }

    /// `true` si le cadrage **version 0** est cohérent : `version == 0` et `declared_len` == longueur
    /// réelle du reste (`payload.len() + 1`). Prouvé sur 17 772/17 772 blobs version-0 réels.
    #[must_use]
    pub fn framing_valid_v0(&self) -> bool {
        self.version == 0 && self.declared_len as usize == self.payload.len() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blobs réels (base64-décodés) extraits des données du jeu — vérité terrain du cadrage.
    /// gallery `openCond`, happen_event `cond[0]`, happen_event `cond[1]` (2 clauses).
    const GALLERY: [u8; 20] = [
        0x00, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x35, 0xb9, 0x19, 0x36, 0xda, 0x00, 0x01, 0x00, 0x32,
        0x00, 0x00, 0x4e, 0x2a, 0x71,
    ];
    const HAPPEN1: [u8; 35] = [
        0x00, 0x00, 0x00, 0x00, 0x1e, 0x0b, 0x35, 0xb9, 0x19, 0x36, 0xda, 0x00, 0x01, 0x00, 0x32,
        0x00, 0x00, 0x27, 0x38, 0x71, 0x35, 0xb9, 0x19, 0x36, 0xda, 0x00, 0x01, 0x00, 0x32, 0x00,
        0x00, 0x75, 0x62, 0x79, 0x8f,
    ];

    #[test]
    fn parse_version0_framing_valide() {
        let b = CondBlob::parse(&GALLERY).expect("≥6 octets");
        assert_eq!(b.version, 0);
        assert_eq!(b.declared_len, 0x0f); // 15 = len(20) − 5
        assert_eq!(b.opcode, 0x05);
        assert_eq!(b.payload.len(), 14);
        assert!(b.framing_valid_v0(), "0x0f == payload.len()+1 (15)");
    }

    #[test]
    fn parse_multi_clauses_framing_valide() {
        let b = CondBlob::parse(&HAPPEN1).expect("≥6 octets");
        assert_eq!(b.version, 0);
        assert_eq!(b.declared_len, 0x1e); // 30 = len(35) − 5
        assert_eq!(b.opcode, 0x0b); // 2 clauses
        assert!(b.framing_valid_v0());
    }

    #[test]
    fn trop_court_rejete() {
        assert!(CondBlob::parse(&[0, 0, 0, 0, 5]).is_none());
        assert!(CondBlob::parse(&[]).is_none());
    }

    #[test]
    fn version1_non_v0() {
        // Préfixe version 1 (forme liste) : le cadrage v0 ne s'applique pas.
        let v1 = [0x00, 0x00, 0x00, 0x01, 0x2f, 0x4d, 0xaa, 0xbb];
        let b = CondBlob::parse(&v1).unwrap();
        assert_eq!(b.version, 1);
        assert!(
            !b.framing_valid_v0(),
            "version 1 ⇒ cadrage v0 invalide (forme liste, INCOMPLET)"
        );
    }
}
