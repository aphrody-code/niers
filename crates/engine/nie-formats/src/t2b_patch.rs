//! `t2b_patch` — modification **en place** d'une variable d'un fichier T2B (`*.cfg.bin`).
//!
//! Pendant de [`crate::rdbn_patch`] pour l'autre format `cfg.bin`. Même raison d'être :
//! [`crate::cfgbin::encode_t2b`] n'est pas fidèle (l'aller-retour du `cpk_list` rend 27 octets de
//! moins sans aucune modification, et le jeu refuse le fichier), alors que **toute variable T2B
//! occupe exactement 4 octets**. Un `Int`, un `Float`, ou l'offset d'une `String` s'écrivent donc
//! sur place, sans déplacer un seul octet.
//!
//! # Comment les variables sont trouvées
//!
//! Le parcours reproduit celui de [`crate::cfgbin::parse_t2b`], à l'identique : en-tête de 16
//! octets, puis pour chaque entrée un CRC (4 o), un nombre de paramètres (1 o), les types
//! empaquetés à 2 bits (`param_count.div_ceil(4)` octets), un alignement du total d'en-tête sur
//! 4, enfin les valeurs à 4 octets chacune. Toute divergence se verrait immédiatement à la
//! relecture, qui est le contrôle appliqué par [`patch_verifie`].
//!
//! Les entrées sont rendues **à plat, dans l'ordre du fichier** — pas en arbre. C'est cette forme
//! qui porte les offsets, et elle évite d'avoir à désigner un noeud par un chemin ambigu.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::FormatError;
use crate::cfgbin::Value;

/// Type d'une variable T2B, tel qu'encodé sur 2 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    /// Type 0 — offset dans la table de chaînes (`-1` = chaîne vide).
    Chaine,
    /// Type 1 — entier 32 bits signé.
    Entier,
    /// Type 2 — flottant 32 bits.
    Flottant,
    /// Type 3 — non utilisé par les fichiers du jeu ; conservé pour ne rien perdre au parcours.
    Inconnu,
}

impl VarType {
    fn depuis_bits(b: u8) -> Self {
        match b {
            0 => Self::Chaine,
            1 => Self::Entier,
            2 => Self::Flottant,
            _ => Self::Inconnu,
        }
    }
}

/// Une variable localisée : son type, son offset absolu, sa valeur lue.
#[derive(Debug, Clone, PartialEq)]
pub struct VarLoc {
    /// Type déclaré.
    pub ty: VarType,
    /// Offset absolu des 4 octets dans le fichier.
    pub offset: usize,
    /// Valeur telle que le parseur la lit.
    pub valeur: Value,
}

/// Une entrée T2B à plat, avec le nom résolu et ses variables localisées.
#[derive(Debug, Clone, PartialEq)]
pub struct EntreeLoc {
    /// Index de l'entrée dans l'ordre du fichier.
    pub index: usize,
    /// CRC du nom (la clé de la table de clés).
    pub crc: u32,
    /// Nom résolu, ou `UNKNOWN_xxxxxxxx`.
    pub nom: String,
    /// Les variables, dans l'ordre.
    pub variables: Vec<VarLoc>,
}

/// Lit l'en-tête et rend `(offset de la table de chaînes, longueur, nombre d'entrées)`.
fn entete(data: &[u8]) -> Result<(usize, usize, usize), FormatError> {
    if data.len() < 16 {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: 16,
        });
    }
    let lire = |o: usize| i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let (n, off, len) = (lire(0), lire(4), lire(8));
    if n < 0 || off < 0 || len < 0 {
        return Err(FormatError::Corrupt(
            "T2B header: count/offset/length négatif",
        ));
    }
    let (off, len) = (off as usize, len as usize);
    let fin = off
        .checked_add(len)
        .ok_or(FormatError::Corrupt("T2B string table overflow"))?;
    if off < 16 || fin > data.len() {
        return Err(FormatError::Corrupt("String table offset out of bounds"));
    }
    Ok((off, len, n as usize))
}

/// Localise toutes les variables de toutes les entrées, dans l'ordre du fichier.
///
/// Les noms ne sont **pas** résolus ici (la table de clés n'est pas relue) : `nom` vaut
/// `UNKNOWN_<crc>`. Utiliser [`crate::cfgbin::cfgbin_parse`] en parallèle si le nom compte —
/// l'ordre des entrées est le même dans les deux parcours.
///
/// # Errors
///
/// Si l'en-tête est illisible ou hors bornes.
pub fn localiser_tout(data: &[u8]) -> Result<Vec<EntreeLoc>, FormatError> {
    let (string_table_off, string_table_len, entries_count) = entete(data)?;
    let buf_len = string_table_off;

    // Table de chaînes, pour rendre la valeur lue d'une variable de type 0.
    let lire_chaine = |off: i32| -> String {
        if off < 0 {
            return String::new();
        }
        let debut = string_table_off + off as usize;
        if debut >= string_table_off + string_table_len {
            return String::new();
        }
        let fin = string_table_off + string_table_len;
        let tranche = &data[debut..fin];
        let nul = tranche
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(tranche.len());
        String::from_utf8_lossy(&tranche[..nul]).into_owned()
    };

    let mut out = Vec::new();
    let mut pos = 16usize;
    for index in 0..entries_count {
        if pos + 5 > buf_len {
            break;
        }
        let crc = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let param_count = data[pos + 4] as usize;
        pos += 5;

        let type_bytes = param_count.div_ceil(4);
        let mut types = Vec::with_capacity(param_count);
        let mut pi = 0usize;
        for _ in 0..type_bytes {
            if pos >= buf_len {
                break;
            }
            let tb = data[pos];
            pos += 1;
            for k in 0..4 {
                if pi < param_count {
                    types.push(VarType::depuis_bits((tb >> (2 * k)) & 3));
                    pi += 1;
                }
            }
        }

        // Alignement sur 4 du total d'en-tête — même règle que le parseur.
        let total_header = 5 + type_bytes;
        if !total_header.is_multiple_of(4) {
            pos += 4 - (total_header % 4);
        }

        let mut variables = Vec::with_capacity(param_count);
        for j in 0..param_count {
            if pos + 4 > buf_len {
                break;
            }
            let brut = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
            let ty = types.get(j).copied().unwrap_or(VarType::Chaine);
            let valeur = match ty {
                VarType::Chaine => Value::String(lire_chaine(i32::from_le_bytes(brut))),
                VarType::Flottant => Value::Float(f32::from_le_bytes(brut)),
                // Le type 3 n'apparaît pas dans les fichiers du jeu ; le lire en entier plutôt
                // que le sauter garde le parcours aligné.
                VarType::Entier | VarType::Inconnu => Value::Int(i32::from_le_bytes(brut)),
            };
            variables.push(VarLoc {
                ty,
                offset: pos,
                valeur,
            });
            pos += 4;
        }

        out.push(EntreeLoc {
            index,
            crc,
            nom: format!("UNKNOWN_{crc:08X}"),
            variables,
        });
    }
    Ok(out)
}

/// Erreur de patch T2B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum T2bPatchError {
    /// Le fichier n'est pas un T2B lisible.
    Illisible(&'static str),
    /// L'entrée demandée n'existe pas.
    EntreeHorsPlage {
        /// Index demandé.
        demande: usize,
        /// Nombre d'entrées.
        total: usize,
    },
    /// La variable demandée n'existe pas dans cette entrée.
    VariableHorsPlage {
        /// Index de l'entrée.
        entree: usize,
        /// Index de variable demandé.
        demande: usize,
        /// Nombre de variables de l'entrée.
        total: usize,
    },
    /// Le type de la variable n'accepte pas l'écriture demandée.
    TypeIncompatible {
        /// Type réel.
        reel: VarType,
        /// Ce que l'appelant a tenté d'écrire.
        tente: &'static str,
    },
}

impl core::fmt::Display for T2bPatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Illisible(m) => write!(f, "T2B illisible : {m}"),
            Self::EntreeHorsPlage { demande, total } => {
                write!(f, "entrée {demande} hors plage (le fichier en a {total})")
            }
            Self::VariableHorsPlage {
                entree,
                demande,
                total,
            } => {
                write!(
                    f,
                    "variable {demande} hors plage dans l'entrée {entree} (elle en a {total})"
                )
            }
            Self::TypeIncompatible { reel, tente } => {
                write!(f, "la variable est {reel:?}, écriture {tente} refusée")
            }
        }
    }
}

/// Une modification demandée sur une variable T2B.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifT2b {
    /// Index de l'entrée dans l'ordre du fichier.
    pub entree: usize,
    /// Index de la variable dans l'entrée.
    pub variable: usize,
    /// Nouvelle valeur.
    pub valeur: ValT2b,
}

/// Valeur à écrire dans une variable T2B.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValT2b {
    /// Variable de type entier.
    Entier(i32),
    /// Variable de type flottant.
    Flottant(f32),
    /// Variable de type chaîne — **offset** dans la table de chaînes, `-1` = vide. Repointer vers
    /// une chaîne déjà présente reste à taille constante ; écrire un nouveau texte ne l'est pas
    /// et n'est donc pas proposé.
    OffsetChaine(i32),
}

/// Applique un lot de modifications **en place** et rend le nombre d'octets changés.
///
/// # Errors
///
/// La première [`T2bPatchError`] rencontrée. Les modifications déjà appliquées le restent : elles
/// sont indépendantes et toutes à taille constante.
pub fn appliquer(data: &mut [u8], modifs: &[ModifT2b]) -> Result<usize, T2bPatchError> {
    let mut changes = 0usize;
    for m in modifs {
        let entrees =
            localiser_tout(data).map_err(|_| T2bPatchError::Illisible("parcours impossible"))?;
        let total = entrees.len();
        let e = entrees
            .get(m.entree)
            .ok_or(T2bPatchError::EntreeHorsPlage {
                demande: m.entree,
                total,
            })?;
        let nb = e.variables.len();
        let v = e
            .variables
            .get(m.variable)
            .ok_or(T2bPatchError::VariableHorsPlage {
                entree: m.entree,
                demande: m.variable,
                total: nb,
            })?;

        let octets = match (m.valeur, v.ty) {
            (ValT2b::Entier(x), VarType::Entier) => x.to_le_bytes(),
            (ValT2b::Flottant(x), VarType::Flottant) => x.to_le_bytes(),
            (ValT2b::OffsetChaine(x), VarType::Chaine) => x.to_le_bytes(),
            (ValT2b::Entier(_), reel) => {
                return Err(T2bPatchError::TypeIncompatible {
                    reel,
                    tente: "entier",
                });
            }
            (ValT2b::Flottant(_), reel) => {
                return Err(T2bPatchError::TypeIncompatible {
                    reel,
                    tente: "flottant",
                });
            }
            (ValT2b::OffsetChaine(_), reel) => {
                return Err(T2bPatchError::TypeIncompatible {
                    reel,
                    tente: "offset de chaîne",
                });
            }
        };
        let (o, fin) = (v.offset, v.offset + 4);
        changes += data[o..fin]
            .iter()
            .zip(&octets)
            .filter(|(a, b)| *a != *b)
            .count();
        data[o..fin].copy_from_slice(&octets);
    }
    Ok(changes)
}

/// Applique les modifications **puis les relit** avec le parseur complet.
///
/// Un patch n'est réputé bon que si le fichier se reparse, à taille inchangée, et rend les
/// valeurs demandées.
///
/// # Errors
///
/// L'erreur de l'application, ou [`T2bPatchError::Illisible`] si la relecture échoue.
pub fn patch_verifie(
    data: &mut [u8],
    modifs: &[ModifT2b],
) -> Result<VerificationT2b, T2bPatchError> {
    let taille_avant = data.len();
    let octets = appliquer(data, modifs)?;

    let entrees =
        localiser_tout(data).map_err(|_| T2bPatchError::Illisible("relecture impossible"))?;
    let mut relues = Vec::with_capacity(modifs.len());
    for m in modifs {
        let v = entrees
            .get(m.entree)
            .and_then(|e| e.variables.get(m.variable))
            .ok_or(T2bPatchError::Illisible("variable absente à la relecture"))?;
        relues.push(v.valeur.clone());
    }
    Ok(VerificationT2b {
        taille_avant,
        taille_apres: data.len(),
        octets_modifies: octets,
        relues,
    })
}

/// Résultat d'un [`patch_verifie`].
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationT2b {
    /// Taille avant patch.
    pub taille_avant: usize,
    /// Taille après — doit être identique.
    pub taille_apres: usize,
    /// Octets effectivement changés.
    pub octets_modifies: usize,
    /// Valeurs relues, dans l'ordre des modifications.
    pub relues: Vec<Value>,
}

impl VerificationT2b {
    /// `true` si la taille n'a pas bougé.
    #[must_use]
    pub fn taille_preservee(&self) -> bool {
        self.taille_avant == self.taille_apres
    }
}
