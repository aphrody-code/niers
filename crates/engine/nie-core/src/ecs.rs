//! Modèle de l'entity-component-system `lives::` de nie.exe — **faits STRUCTURELS reversés**
//! (offsets/algorithme d'identité), pas le comportement runtime des composants.
//!
//! Source : désassemblage du binaire courant `nie_eacpatched.exe` (2026-06-19, iced-x86/pefile),
//! validé byte-à-byte contre `zlib.crc32` pour l'identité de type. Anchors (build-spécifiques) :
//! - `0x140453910` = hasher CRC32 zlib standard (table `0x14178FEF0`) → **type-ID = CRC32(nom de classe)**.
//! - `0x140469080` = primitive de lookup composant-par-type-ID (46 appelants) ; registre = callback
//!   `0x14206EEC0` + liste chaînée `0x14206EEA8` (nœud `{type-ID @+0x70, next @+0x78}`).
//! - `0x1404691B0` = get-or-create du composant (453 o) → layout de stockage ci-dessous.
//!
//! Ce module donne deux choses **vérifiables hors-jeu** : (1) le calcul du type-ID d'un composant
//! (réutilisable pour chercher un composant dans le binaire / le dispatch) ; (2) les offsets du nœud
//! de composant et du descripteur (fondation d'un port futur). Le **comportement** (update, détection
//! de but…) reste gated par l'oracle differential-testing — non porté ici (anti-faux-FAIT).

/// Calcule l'identifiant de type d'un composant ECS = **CRC32 (zlib standard) du nom de classe**.
///
/// Reversé de `0x140453910` (CRC32 table-driven, init `0xFFFFFFFF`, poly réfléchi `0xEDB88320`,
/// finalisation `not`). Le registrar `0x140336550` calcule ce CRC32 sur le nom de classe et le cache
/// dans le slot type-ID `0x14210E3E4` ; le dispatch ECS keye les composants par cette valeur.
///
/// Implémentation bit-à-bit (équivalente à la table du binaire ; validée identique en test).
#[must_use]
pub fn component_type_id(name: &str) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in name.as_bytes() {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Layout d'un **nœud de composant** dans une entité (liste chaînée intrusive). Offsets reversés de
/// `0x1404691B0`. L'entité stocke la tête de sa liste de composants à [`ENTITY_COMPONENT_LIST_HEAD`].
pub mod component {
    /// `+0x08` — pointeur retour vers l'entité propriétaire.
    pub const OWNER_ENTITY: usize = 0x08;
    /// `+0x10` — composant suivant dans la liste chaînée intrusive de l'entité.
    pub const NEXT: usize = 0x10;
    /// `+0x18` — descripteur de type (voir [`super::descriptor`]).
    pub const DESCRIPTOR: usize = 0x18;
    /// `+0x20` — identifiant de type = `CRC32(nom)` (voir [`super::component_type_id`]).
    pub const TYPE_ID: usize = 0x20;
    /// `+0x24` — octet de flag (mis à 0 à la création).
    pub const FLAG: usize = 0x24;
    /// `+0x30` — pointeur vers la sous-table du descripteur.
    pub const SUBTABLE: usize = 0x30;
}

/// Layout d'un **descripteur de type** de composant. Offsets reversés de `0x1404691B0`/`0x140469160`.
pub mod descriptor {
    /// `+0x08` — pointeur de fonction **factory** (`call` produit une instance de composant).
    pub const FACTORY_FN: usize = 0x08;
    /// `+0x10` — sous-table (contient `+0x24` = count u16, `+0x40` = diviseur, `+0x1C` = champ).
    pub const SUBTABLE: usize = 0x10;
}

/// `entity + 0xE8` — tête de la **liste chaînée intrusive** des composants de l'entité. La création
/// d'un composant l'insère en tête (`comp.NEXT = [entity+0xE8] ; [entity+0xE8] = comp`).
pub const ENTITY_COMPONENT_LIST_HEAD: usize = 0xE8;

/// `entity + 0x79` — index de type d'entité (masqué `& 0x7F`), clé dans l'array de vtables global
/// `0x14206EE00` (dispatch par type d'entité).
pub const ENTITY_TYPE_INDEX: usize = 0x79;

#[cfg(test)]
mod tests {
    use super::*;

    /// **`CSoccerEventCheckComponent` = 0x271C0D99 est CONFIRMÉ dans le binaire** : son CRC32
    /// apparaît comme immédiat `.text` à 4 sites (les points d'accès de la détection de but) — c'est
    /// l'ancre qui valide le mécanisme « type-ID = CRC32(nom de classe) ». Les autres IDs ci-dessous
    /// sont **dérivés du mécanisme validé** (CRC32 = le hasher exact du binaire, cf.
    /// [`type_id_is_standard_zlib_crc32`]) : ils sont les type-IDs corrects, mais ne sont **pas**
    /// référencés par immédiat littéral (les composants non-event-check sont cherchés par ID caché
    /// au runtime — le registrar calcule+cache le CRC32). Cf. mémoire `c3-modele-but-anchors`.
    #[test]
    fn type_id_crc32_matches_binary() {
        // Confirmé byte dans le binaire (immédiat .text ×4) :
        assert_eq!(component_type_id("CSoccerEventCheckComponent"), 0x271C_0D99);
        // Dérivés du mécanisme validé (type-IDs corrects, non vus comme immédiats) :
        assert_eq!(
            component_type_id("SoccerCalcKeeperSaveComponent"),
            0x0C40_DCAC
        );
        assert_eq!(component_type_id("CSceneSoccer"), 0xB944_8ABA);
        assert_eq!(component_type_id("BallMoveGoalnet"), 0x9BE8_D63E);
    }

    /// Vecteur de référence CRC32 zlib canonique : `CRC32("123456789") == 0xCBF43926`.
    #[test]
    fn type_id_is_standard_zlib_crc32() {
        assert_eq!(component_type_id("123456789"), 0xCBF4_3926);
        assert_eq!(component_type_id(""), 0x0000_0000);
    }

    /// Les offsets de layout sont distincts et cohérents (nœud de composant).
    #[test]
    fn component_offsets_coherents() {
        assert_eq!(component::OWNER_ENTITY, 0x08);
        assert_eq!(component::NEXT, 0x10);
        assert_eq!(component::DESCRIPTOR, 0x18);
        assert_eq!(component::TYPE_ID, 0x20);
        assert_eq!(descriptor::FACTORY_FN, 0x08);
        assert_eq!(ENTITY_COMPONENT_LIST_HEAD, 0xE8);
    }
}
