//! Cinq familles de plus, décodées **en process** — le lot que la matrice de couverture a fait
//! apparaître.
//!
//! Elles ne sont pas géométriques (c'est pourquoi elles ne sont pas dans [`super::geometrie`]),
//! mais elles partagent tout le reste : un parseur déjà écrit dans `nie-formats`, aucune route
//! qui l'appelle, et un corpus mesuré. Ce que la matrice du § 4 a chiffré le 2026-09-06 :
//!
//! | Suffixe | Fichiers | Parseur | Ce qu'il était |
//! |---|---:|---|---|
//! | `.p3lip` | **21 047** | `nie_formats::lip` | invisible : `/f` en rendait les octets |
//! | `.g4nv` | 160 | `nie_formats::navm` | classé `bloqué` à tort |
//! | `.g4ma` | 35 | `nie_formats::g4ma` | classé `bloqué` à tort |
//! | `.g4vs` | 4 | `nie_formats::g4vs` | classé `bloqué` à tort |
//! | `.g4la` | 4 | `nie_formats::g4la` | classé `bloqué` à tort |
//!
//! **Les quatre dernières n'ont jamais été bloquées.** `docs/VFS.md` les disait « aucun
//! parseur » alors que les quatre modules sont écrits, documentés et validés byte sur les
//! fichiers réels du VFS. La distinction `manquant`/`bloqué` est celle entre écrire une route
//! et faire du reverse : les confondre promet des semaines là où il y a des heures.
//!
//! **Et `.p3lip` — le plus gros corpus non servi du jeu — ne se voyait pas du tout**, parce que
//! `/f/{*chemin}` en rend les octets et qu'une carte comptant cela pour `servi` ne pouvait pas
//! le signaler.
//!
//! Ce que ce module NE prétend pas : `.g4ma`, `.g4vs` et `.g4la` n'ont que leur **en-tête**
//! interprété — leur corps n'est pas reversé. La ligne « produit » de chaque famille le dit,
//! parce qu'un client qui reçoit un JSON sans savoir jusqu'où il va lire croit tout avoir.

use serde::Serialize;

use nie_formats::{g4la, g4ma, g4vs, lip, navm};

use super::geometrie::Forme;
use crate::error::ErreurSite;

/// Une famille reconnue par ce module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    /// Piste de lip-sync (`.p3lip`) : visèmes datés d'une réplique.
    Lip,
    /// Maillage de navigation (`.g4nv`, magic **NAVM**) : sommets, polygones, arêtes.
    Navm,
    /// Animation de matériau (`.g4ma`) : en-tête Level-5, corps non interprété.
    G4ma,
    /// Effet d'événement (`.g4vs`) : en-tête Level-5, corps non interprété.
    G4vs,
    /// Animation de lumière d'événement (`.g4la`) : en-tête Level-5, corps non interprété.
    G4la,
}

/// Les cinq familles, dans l'ordre décroissant de leur compte sur le VFS.
///
/// `(suffixe, famille, ce que le décodage produit)`.
pub const FAMILLES: [(&str, Famille, &str); 5] = [
    (".p3lip", Famille::Lip, "visemes dates de la replique"),
    (
        ".g4nv",
        Famille::Navm,
        "sommets, polygones et aretes du maillage de navigation",
    ),
    (
        ".g4ma",
        Famille::G4ma,
        "en-tete Level-5 d'animation de materiau, corps non interprete",
    ),
    (
        ".g4vs",
        Famille::G4vs,
        "en-tete Level-5 d'effet d'evenement, corps non interprete",
    ),
    (
        ".g4la",
        Famille::G4la,
        "en-tete Level-5 d'animation de lumiere, corps non interprete",
    ),
];

impl Famille {
    /// La famille d'un chemin, d'après son suffixe, sans casse.
    #[must_use]
    pub fn depuis_chemin(chemin: &str) -> Option<Self> {
        let bas = chemin.to_ascii_lowercase();
        FAMILLES
            .into_iter()
            .find_map(|(s, f, _)| bas.ends_with(s).then_some(f))
    }

    /// Le suffixe de la famille, point compris.
    #[must_use]
    pub fn suffixe(self) -> &'static str {
        Self::ligne(self).0
    }

    /// Le jeton public de la famille.
    #[must_use]
    pub fn jeton(self) -> &'static str {
        self.suffixe().trim_start_matches('.')
    }

    /// Ce que le décodage produit, en une phrase.
    #[must_use]
    pub fn produit(self) -> &'static str {
        Self::ligne(self).2
    }

    fn ligne(self) -> (&'static str, Self, &'static str) {
        FAMILLES
            .into_iter()
            .find(|(_, f, _)| *f == self)
            .unwrap_or((".inconnu", self, "inconnu"))
    }
}

/// Reconnaît une famille de ce module au **magic**, quand le suffixe n'a rien dit.
///
/// `lip` n'y est pas : son magic est `lip\0`, quatre octets dont un nul, et le tester ici ferait
/// doublon avec `lip::parse` qui le vérifie déjà. Les trois conteneurs Level-5 y sont, eux,
/// parce qu'un fichier du VFS peut porter leur magic sous un suffixe de révision.
#[must_use]
pub fn famille_au_magic(octets: &[u8]) -> Option<Famille> {
    if navm::is_navm(octets) {
        return Some(Famille::Navm);
    }
    if g4ma::is_g4ma(octets) {
        return Some(Famille::G4ma);
    }
    if g4vs::is_g4vs(octets) {
        return Some(Famille::G4vs);
    }
    if g4la::is_g4la(octets) {
        return Some(Famille::G4la);
    }
    None
}

/// L'en-tête commun Level-5, republié en clair.
#[derive(Debug, Clone, Serialize)]
pub struct Entete {
    /// Le magic, en ASCII lisible (`G4MA`, `G4VS`, `G4LA`, `NAVM`).
    pub magic: String,
    /// Taille de l'en-tête déclarée.
    pub taille_entete: u16,
    /// Identifiant de type déclaré.
    pub type_id: u16,
    /// Alignement déclaré.
    pub alignement: u16,
    /// Taille des données déclarée.
    pub taille_donnees: u32,
    /// `taille_entete + taille_donnees == taille du fichier` — l'invariant structurel.
    ///
    /// Publié plutôt que vérifié en silence : un fichier incohérent n'est pas une erreur du
    /// service, c'est un fait sur le fichier, et le client doit pouvoir le voir.
    pub taille_coherente: bool,
}

impl Entete {
    fn depuis(h: &nie_formats::level5::Level5Header, taille: usize) -> Self {
        Self {
            magic: String::from_utf8_lossy(&h.magic.to_le_bytes()).into_owned(),
            taille_entete: h.header_size,
            type_id: h.type_id,
            alignement: h.align,
            taille_donnees: h.data_size,
            taille_coherente: h.is_size_consistent(taille),
        }
    }
}

/// Les comptes, une variante par famille.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "famille", rename_all = "snake_case")]
pub enum Resume {
    /// Piste de lip-sync.
    P3lip {
        /// Durée de la réplique, en secondes.
        duree_s: f32,
        /// Images-clés, sentinelles comprises.
        images_cles: usize,
        /// Visèmes réellement jouables (hors sentinelles de début et de fin).
        visemes_jouables: usize,
        /// Les indices de visème distincts rencontrés, triés.
        visemes_distincts: Vec<u8>,
        /// Les canaux distincts rencontrés, triés.
        canaux: Vec<u8>,
    },
    /// Maillage de navigation.
    G4nv {
        /// L'en-tête Level-5.
        entete: Entete,
        /// Nombre de sommets.
        sommets: usize,
        /// Nombre de polygones.
        polygones: usize,
        /// Nombre d'arêtes.
        aretes: usize,
        /// Index de sommet (trois par polygone).
        coins: usize,
        /// Index d'arête, groupés par polygone.
        references_aretes: usize,
        /// Octets de remplissage après la dernière arête.
        remplissage: usize,
    },
    /// Conteneur Level-5 dont seul l'en-tête est interprété.
    ConteneurLevel5 {
        /// Le jeton de la famille (`g4ma`, `g4vs`, `g4la`).
        conteneur: &'static str,
        /// L'en-tête Level-5.
        entete: Entete,
    },
}

/// Un fichier décodé.
#[derive(Debug, Clone, Serialize)]
pub struct Decodage {
    /// Chemin VFS décodé.
    pub chemin: String,
    /// Taille du fichier source, en octets.
    pub octets: usize,
    /// Jeton de la famille reconnue.
    pub format: &'static str,
    /// Ce que le décodage produit, en une phrase.
    pub produit: &'static str,
    /// Les comptes.
    pub resume: Resume,
    /// La structure entière, seulement en `?forme=complet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub donnees: Option<serde_json::Value>,
}

/// Traduit une erreur de parseur en `400`, en nommant la famille visée.
fn illisible(famille: Famille, e: &impl std::fmt::Display) -> ErreurSite {
    ErreurSite::Demande(format!("{} illisible: {e}", famille.jeton().to_uppercase()))
}

fn en_valeur<T: Serialize>(v: &T) -> Result<serde_json::Value, ErreurSite> {
    serde_json::to_value(v).map_err(|e| ErreurSite::Interne(format!("non serialisable: {e}")))
}

/// Décode des octets d'une de ces cinq familles.
///
/// Fonction **pure** : ni HTTP ni VFS, comme celle de [`super::geometrie`]. C'est ce qui permet
/// aux tests de la falsifier sur des octets fabriqués.
///
/// # Errors
///
/// `Demande` quand les octets ne sont pas lisibles par le parseur de la famille.
pub fn decoder(
    chemin: &str,
    octets: &[u8],
    famille: Famille,
    forme: Forme,
) -> Result<Decodage, ErreurSite> {
    let complet = forme == Forme::Complet;
    let mut donnees = None;
    let resume = match famille {
        Famille::Lip => {
            let l = lip::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&l)?);
            }
            let mut visemes: Vec<u8> = l
                .frames
                .iter()
                .filter(|f| !f.is_sentinel())
                .map(|f| f.viseme)
                .collect();
            visemes.sort_unstable();
            visemes.dedup();
            let mut canaux: Vec<u8> = l.frames.iter().map(|f| f.channel).collect();
            canaux.sort_unstable();
            canaux.dedup();
            Resume::P3lip {
                duree_s: l.duration_s,
                images_cles: l.frames.len(),
                visemes_jouables: l.playable_count(),
                visemes_distincts: visemes,
                canaux,
            }
        }
        Famille::Navm => {
            let n = navm::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&n)?);
            }
            Resume::G4nv {
                entete: Entete::depuis(&n.header, n.file_size),
                sommets: n.vertices.len(),
                polygones: n.polygons.len(),
                aretes: n.edges.len(),
                coins: n.corners.len(),
                references_aretes: n.edge_refs.len(),
                remplissage: n.padding_len,
            }
        }
        Famille::G4ma => {
            let f = g4ma::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&f)?);
            }
            Resume::ConteneurLevel5 {
                conteneur: "g4ma",
                entete: Entete::depuis(&f.header, f.file_size),
            }
        }
        Famille::G4vs => {
            let f = g4vs::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&f)?);
            }
            Resume::ConteneurLevel5 {
                conteneur: "g4vs",
                entete: Entete::depuis(&f.header, f.file_size),
            }
        }
        Famille::G4la => {
            let f = g4la::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&f)?);
            }
            Resume::ConteneurLevel5 {
                conteneur: "g4la",
                entete: Entete::depuis(&f.header, f.file_size),
            }
        }
    };
    Ok(Decodage {
        chemin: chemin.to_owned(),
        octets: octets.len(),
        format: famille.jeton(),
        produit: famille.produit(),
        resume,
        donnees,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un `.p3lip` minimal : magic `lip\0`, taille déclarée = taille réelle, deux sentinelles.
    fn p3lip_temoin() -> Vec<u8> {
        // L'en-tête fait 0xB0 octets et chaque image-clé 8 : 0xB0 + 2 × 8 = 0xC0.
        let mut o = vec![0u8; 0xC0];
        o[0..4].copy_from_slice(b"lip\0");
        o[0x08..0x0c].copy_from_slice(&0xC0u32.to_le_bytes());
        o
    }

    #[test]
    fn les_suffixes_sont_uniques_et_reconnus() {
        let mut suffixes: Vec<&str> = FAMILLES.iter().map(|(s, ..)| *s).collect();
        let avant = suffixes.len();
        suffixes.sort_unstable();
        suffixes.dedup();
        assert_eq!(suffixes.len(), avant, "suffixe declare deux fois");
        for (s, f, _) in FAMILLES {
            assert_eq!(Famille::depuis_chemin(&format!("data/x{s}")), Some(f));
            // Le VFS porte des chemins en majuscules : une extension écartée pour sa casse
            // rendrait un 400 qu'on attribuerait au format.
            assert_eq!(
                Famille::depuis_chemin(&format!("data/X{}", s.to_uppercase())),
                Some(f)
            );
        }
        assert_eq!(Famille::depuis_chemin("data/x.g4pk"), None);
    }

    #[test]
    fn le_magic_ne_reconnait_pas_n_importe_quoi() {
        assert_eq!(famille_au_magic(b"BLOCK_LIST_BEGIN...."), None);
        assert_eq!(famille_au_magic(&[]), None);
        // `lip\0` n'est PAS dans la reconnaissance au magic : c'est délibéré, `lip::parse` le
        // vérifie lui-même. Un test le fige, faute de quoi on le « corrigerait » un jour.
        assert_eq!(famille_au_magic(&p3lip_temoin()), None);
    }

    #[test]
    fn un_p3lip_se_decode_et_ses_comptes_tiennent() {
        let o = p3lip_temoin();
        let d = decoder("data/x.p3lip", &o, Famille::Lip, Forme::Resume).expect("p3lip lisible");
        assert_eq!(d.format, "p3lip");
        assert_eq!(d.octets, o.len());
        assert!(d.donnees.is_none(), "le resume ne porte pas la structure");
        let Resume::P3lip {
            images_cles,
            visemes_jouables,
            ..
        } = d.resume
        else {
            panic!("mauvaise variante de resume");
        };
        assert_eq!(images_cles, 2, "0xC0 - 0xB0 = 2 images de 8 octets");
        assert_eq!(
            visemes_jouables, 2,
            "le viseme 0 n.est PAS une sentinelle : seuls 254 et 255 le sont"
        );

        // `?forme=complet` rend la structure ; sans elle, rien. La différence se teste, sinon
        // un paramètre accepté et ignoré passe inaperçu.
        let d = decoder("data/x.p3lip", &o, Famille::Lip, Forme::Complet).expect("p3lip lisible");
        assert!(d.donnees.is_some());
    }

    #[test]
    fn un_fichier_illisible_dit_quelle_famille_a_echoue() {
        let e = decoder("data/x.p3lip", b"pas du lip", Famille::Lip, Forme::Resume)
            .expect_err("doit refuser");
        assert!(format!("{e}").contains("P3LIP"), "{e}");
    }
}
