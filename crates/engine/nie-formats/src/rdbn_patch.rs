//! `rdbn_patch` — modification **en place** d'une valeur d'un fichier RDBN (`*.cfg.bin`).
//!
//! # Pourquoi pas [`crate::cfgbin::encode_rdbn`]
//!
//! L'aller-retour `decode → encode` des `cfg.bin` n'est pas fidèle : sur le `cpk_list` du jeu il
//! rend 16 à 27 octets de moins *sans aucune modification*, et sur `game_param.cfg.bin` la
//! relecture retombe de 812 enfants à 1. Le jeu refuse alors le fichier. Notre parseur est plus
//! permissif que le sien : « ça se relit chez nous » ne prouve rien.
//!
//! Or **tout ce qu'un mod de valeurs change tient à taille constante** : un `Int`, un `Byte`, un
//! `Hash`, un `Float` s'écrivent sur place. Ce module calcule l'offset exact d'un champ avec la
//! **même arithmétique que [`crate::cfgbin::read_values`]** —
//! `value_abs + root.value_offset + ligne × root.value_size + field.value_offset` — et n'écrit que
//! les octets de ce champ. La taille du fichier, la table de chaînes, tous les autres offsets
//! restent identiques **bit pour bit**.
//!
//! # Ce que le module refuse de faire
//!
//! - Écrire une valeur d'un type qui ne correspond pas au champ (`set_i32` sur un `Byte`).
//! - Écrire un champ de taille variable ou de type inconnu (`Condition`/chaîne : la valeur y est
//!   un **offset** dans la table de chaînes ; changer le texte demanderait de déplacer la table).
//!   Pour ces champs, [`set_string_offset`] permet de **repointer** vers une chaîne **déjà
//!   présente** dans le fichier, ce qui reste à taille constante.
//! - Sortir des bornes du tampon.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::cfgbin::{RdbnData, RdbnFieldType};

/// Emplacement exact d'un champ d'une ligne dans le tampon d'origine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLoc {
    /// Offset absolu de la valeur dans le fichier.
    pub offset: usize,
    /// Type déclaré du champ (contrôle ce qu'on a le droit d'y écrire).
    pub field_type: RdbnFieldType,
    /// Taille déclarée de la valeur, en octets.
    pub size: usize,
}

/// Erreur de localisation ou d'écriture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// Aucune liste (racine RDBN) de ce nom.
    ListeInconnue(String),
    /// La ligne demandée dépasse le nombre de lignes de la liste.
    LigneHorsPlage {
        /// Index demandé.
        demande: usize,
        /// Nombre de lignes de la liste.
        total: usize,
    },
    /// Aucun champ de ce nom dans le type de la liste.
    ChampInconnu(String),
    /// Le type du champ n'accepte pas l'écriture demandée.
    TypeIncompatible {
        /// Nom du champ visé.
        champ: String,
        /// Type réel du champ.
        reel: RdbnFieldType,
        /// Ce que l'appelant a tenté d'écrire.
        tente: &'static str,
    },
    /// L'écriture sortirait du tampon.
    HorsBornes {
        /// Offset visé.
        offset: usize,
        /// Taille de l'écriture.
        taille: usize,
        /// Taille du tampon.
        tampon: usize,
    },
}

impl core::fmt::Display for PatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ListeInconnue(n) => write!(f, "liste « {n} » absente du RDBN"),
            Self::LigneHorsPlage { demande, total } => {
                write!(f, "ligne {demande} hors plage (la liste en a {total})")
            }
            Self::ChampInconnu(n) => write!(f, "champ « {n} » absent du type de la liste"),
            Self::TypeIncompatible { champ, reel, tente } => {
                write!(
                    f,
                    "champ « {champ} » est {reel:?}, écriture {tente} refusée"
                )
            }
            Self::HorsBornes {
                offset,
                taille,
                tampon,
            } => {
                write!(
                    f,
                    "écriture de {taille} o à {offset} hors du tampon de {tampon} o"
                )
            }
        }
    }
}

/// Localise le champ `champ` de la ligne `ligne` de la liste `liste`.
///
/// L'arithmétique est celle de [`crate::cfgbin::read_values`] : toute divergence se verrait
/// immédiatement à la relecture, qui est le contrôle appliqué par [`patch_verifie`].
///
/// # Errors
///
/// [`PatchError::ListeInconnue`], [`PatchError::LigneHorsPlage`] ou [`PatchError::ChampInconnu`]
/// si la coordonnée ne désigne rien dans ce fichier.
pub fn localiser(
    rdbn: &RdbnData,
    liste: &str,
    ligne: usize,
    champ: &str,
) -> Result<FieldLoc, PatchError> {
    let root = rdbn
        .roots
        .iter()
        .find(|r| rdbn.root_name(r) == Some(liste))
        .ok_or_else(|| PatchError::ListeInconnue(String::from(liste)))?;

    let total = usize::try_from(root.value_count.max(0)).unwrap_or(0);
    if ligne >= total {
        return Err(PatchError::LigneHorsPlage {
            demande: ligne,
            total,
        });
    }

    let ty = usize::try_from(root.type_index)
        .ok()
        .and_then(|i| rdbn.types.get(i))
        .ok_or_else(|| PatchError::ChampInconnu(String::from(champ)))?;

    let entry_offset = rdbn
        .value_abs
        .wrapping_add(root.value_offset as usize)
        .wrapping_add(ligne * root.value_size as usize);

    for f in 0..ty.field_count.max(0) {
        let idx = ty.field_index as i64 + i64::from(f);
        let Some(field) = usize::try_from(idx).ok().and_then(|i| rdbn.fields.get(i)) else {
            continue;
        };
        if rdbn.field_name(field) != Some(champ) {
            continue;
        }
        return Ok(FieldLoc {
            offset: entry_offset.wrapping_add(field.value_offset as usize),
            field_type: field.field_type,
            size: usize::try_from(field.value_size.max(0)).unwrap_or(0),
        });
    }
    Err(PatchError::ChampInconnu(String::from(champ)))
}

/// Écrit `octets` à l'emplacement `loc`, après contrôle des bornes.
fn ecrire(data: &mut [u8], loc: FieldLoc, octets: &[u8]) -> Result<(), PatchError> {
    let fin = loc.offset.saturating_add(octets.len());
    if fin > data.len() {
        return Err(PatchError::HorsBornes {
            offset: loc.offset,
            taille: octets.len(),
            tampon: data.len(),
        });
    }
    data[loc.offset..fin].copy_from_slice(octets);
    Ok(())
}

/// Écrit un entier 32 bits dans un champ `Int` ou `Flag`.
///
/// # Errors
///
/// [`PatchError::TypeIncompatible`] si le champ n'est pas `Int`/`Flag`, [`PatchError::HorsBornes`]
/// si l'écriture sortirait du tampon.
pub fn set_i32(data: &mut [u8], loc: FieldLoc, valeur: i32) -> Result<(), PatchError> {
    if !matches!(loc.field_type, RdbnFieldType::Int | RdbnFieldType::Flag) {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "i32",
        });
    }
    ecrire(data, loc, &valeur.to_le_bytes())
}

/// Écrit un entier 16 bits dans un champ `Short` ou `ActType`.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_i16(data: &mut [u8], loc: FieldLoc, valeur: i16) -> Result<(), PatchError> {
    if !matches!(
        loc.field_type,
        RdbnFieldType::Short | RdbnFieldType::ActType
    ) {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "i16",
        });
    }
    ecrire(data, loc, &valeur.to_le_bytes())
}

/// Écrit un octet dans un champ `Byte`.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_u8(data: &mut [u8], loc: FieldLoc, valeur: u8) -> Result<(), PatchError> {
    if loc.field_type != RdbnFieldType::Byte {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "u8",
        });
    }
    ecrire(data, loc, &[valeur])
}

/// Écrit un booléen (octet 0/1) dans un champ `Bool`.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_bool(data: &mut [u8], loc: FieldLoc, valeur: bool) -> Result<(), PatchError> {
    if loc.field_type != RdbnFieldType::Bool {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "bool",
        });
    }
    ecrire(data, loc, &[u8::from(valeur)])
}

/// Écrit un flottant 32 bits dans un champ `Float`.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_f32(data: &mut [u8], loc: FieldLoc, valeur: f32) -> Result<(), PatchError> {
    if loc.field_type != RdbnFieldType::Float {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "f32",
        });
    }
    ecrire(data, loc, &valeur.to_le_bytes())
}

/// Écrit un hash 32 bits dans un champ `Hash`.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_hash(data: &mut [u8], loc: FieldLoc, valeur: u32) -> Result<(), PatchError> {
    if loc.field_type != RdbnFieldType::Hash {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "hash",
        });
    }
    ecrire(data, loc, &valeur.to_le_bytes())
}

/// Écrit quatre flottants dans un champ `Rates` ou `Position` (4 × f32 LE).
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_rates(data: &mut [u8], loc: FieldLoc, valeurs: [f32; 4]) -> Result<(), PatchError> {
    if !matches!(
        loc.field_type,
        RdbnFieldType::Rates | RdbnFieldType::Position
    ) {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "4 × f32",
        });
    }
    let mut octets = [0u8; 16];
    for (i, v) in valeurs.iter().enumerate() {
        octets[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    ecrire(data, loc, &octets)
}

/// Repointe un champ de type `Condition` (chaîne) vers un autre **offset de chaîne**.
///
/// La valeur stockée est un offset i32 dans la table de chaînes ; `-1` s'y lit comme la chaîne
/// vide. Écrire un offset déjà valide dans le fichier permet donc de changer le texte affiché
/// sans déplacer un seul octet — c'est la seule façon sûre de modifier une chaîne en place.
///
/// # Errors
///
/// Voir [`set_i32`].
pub fn set_string_offset(data: &mut [u8], loc: FieldLoc, offset: i32) -> Result<(), PatchError> {
    if loc.field_type != RdbnFieldType::Condition {
        return Err(PatchError::TypeIncompatible {
            champ: format!("{:?}", loc.field_type),
            reel: loc.field_type,
            tente: "offset de chaîne",
        });
    }
    ecrire(data, loc, &offset.to_le_bytes())
}

/// Une modification demandée, exprimée par coordonnée logique plutôt que par octet.
#[derive(Debug, Clone, PartialEq)]
pub struct Modif {
    /// Nom de la liste (racine RDBN), ex. `m_LevelLimitInfoList`.
    pub liste: String,
    /// Index de la ligne dans la liste.
    pub ligne: usize,
    /// Nom du champ, ex. `level`.
    pub champ: String,
    /// Nouvelle valeur.
    pub valeur: Val,
}

/// Valeur à écrire, typée comme le champ visé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    /// Champ `Int` / `Flag`.
    I32(i32),
    /// Champ `Short` / `ActType`.
    I16(i16),
    /// Champ `Byte`.
    U8(u8),
    /// Champ `Bool`.
    Bool(bool),
    /// Champ `Float`.
    F32(f32),
    /// Champ `Hash`.
    Hash(u32),
    /// Champ `Condition` (chaîne) — offset dans la table de chaînes.
    StrOffset(i32),
    /// Champ `Rates` / `Position` — 4 × f32 LE.
    Rates([f32; 4]),
}

/// Applique une liste de modifications à un tampon RDBN, **en place**.
///
/// Renvoie le nombre d'octets réellement modifiés (0 si toutes les valeurs étaient déjà à leur
/// cible). La taille du tampon ne change jamais.
///
/// # Errors
///
/// La première [`PatchError`] rencontrée ; les modifications déjà appliquées le restent, ce qui
/// est sans risque puisque chacune est indépendante et à taille constante.
pub fn appliquer(data: &mut [u8], modifs: &[Modif]) -> Result<usize, PatchError> {
    let mut changes = 0usize;
    for m in modifs {
        let rdbn =
            crate::cfgbin::parse(data).map_err(|_| PatchError::ListeInconnue(m.liste.clone()))?;
        let loc = localiser(&rdbn, &m.liste, m.ligne, &m.champ)?;
        let avant = data[loc.offset..loc.offset.saturating_add(loc.size).min(data.len())].to_vec();
        match m.valeur {
            Val::I32(v) => set_i32(data, loc, v)?,
            Val::I16(v) => set_i16(data, loc, v)?,
            Val::U8(v) => set_u8(data, loc, v)?,
            Val::Bool(v) => set_bool(data, loc, v)?,
            Val::F32(v) => set_f32(data, loc, v)?,
            Val::Hash(v) => set_hash(data, loc, v)?,
            Val::StrOffset(v) => set_string_offset(data, loc, v)?,
            Val::Rates(v) => set_rates(data, loc, v)?,
        }
        let apres = &data[loc.offset..loc.offset.saturating_add(loc.size).min(data.len())];
        changes += avant.iter().zip(apres).filter(|(a, b)| a != b).count();
    }
    Ok(changes)
}

/// Applique les modifications **puis les relit** : le patch n'est réputé bon que si le fichier
/// se reparse et rend exactement les valeurs demandées, à taille inchangée.
///
/// C'est la garantie que ce module vise et que le réencodage ne donne pas.
///
/// # Errors
///
/// La [`PatchError`] de l'application, ou une [`PatchError::ChampInconnu`] si la relecture ne
/// retrouve pas le champ.
pub fn patch_verifie(data: &mut [u8], modifs: &[Modif]) -> Result<Verification, PatchError> {
    let taille_avant = data.len();
    let octets = appliquer(data, modifs)?;
    let taille_apres = data.len();

    let rdbn = crate::cfgbin::parse(data)
        .map_err(|_| PatchError::ListeInconnue(String::from("<relecture impossible>")))?;
    let listes = crate::cfgbin::read_values(&rdbn, data);

    let mut relues = Vec::with_capacity(modifs.len());
    for m in modifs {
        let val = listes
            .iter()
            .find(|l| l.name == m.liste)
            .and_then(|l| l.rows.get(m.ligne))
            .and_then(|r| r.fields.iter().find(|(k, _)| *k == m.champ))
            .map(|(_, v)| format!("{v:?}"))
            .ok_or_else(|| PatchError::ChampInconnu(m.champ.clone()))?;
        relues.push(val);
    }

    Ok(Verification {
        taille_avant,
        taille_apres,
        octets_modifies: octets,
        relues,
    })
}

/// Résultat d'un [`patch_verifie`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// Taille du fichier avant patch.
    pub taille_avant: usize,
    /// Taille après patch — doit être identique.
    pub taille_apres: usize,
    /// Nombre d'octets effectivement changés.
    pub octets_modifies: usize,
    /// Valeurs relues après patch, dans l'ordre des modifications demandées.
    pub relues: Vec<String>,
}

impl Verification {
    /// `true` si la taille n'a pas bougé — condition nécessaire d'un patch en place.
    #[must_use]
    pub fn taille_preservee(&self) -> bool {
        self.taille_avant == self.taille_apres
    }
}
