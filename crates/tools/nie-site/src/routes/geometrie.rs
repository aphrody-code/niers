//! Les neuf familles **géométriques** du jeu, décodées en process — lot 9.1 du plan.
//!
//! ## Pourquoi elles étaient `manquant` ou `partiel`, et ce qui change
//!
//! `docs/VFS.md` en comptait **83 753, soit 32,8 % du VFS** : 67 878 `manquant` (« le décodeur
//! existe déjà ici, aucune route ne l'appelle ») et 15 875 `.g4mg` `partiel` (servis seulement
//! pour les codes que l'amont sait assembler). La mesure du 2026-09-06 (`awk` sur
//! `var/vfs/inventaire.txt`, 255 308 lignes) les compte fichier par fichier :
//!
//! | Suffixe | Fichiers | Plus gros | Total |
//! |---|---:|---:|---:|
//! | `.g4pk` | 45 591 | 12 316 032 o | 2 549 547 328 o |
//! | `.g4mg` | 15 875 | 12 296 576 o | — |
//! | `.objbin` | 12 190 | 15 024 o | 20 942 080 o |
//! | `.g4pkm` | 6 992 | 707 232 o | 169 979 872 o |
//! | `.g4cm` | 1 217 | 129 664 o | 8 910 272 o |
//! | `.col` | 1 150 | 1 043 532 o | 15 924 452 o |
//! | `.g4sk` | 339 | 158 708 o | 1 661 568 o |
//! | `.mevbin` | 328 | 22 448 o | 1 044 032 o |
//! | `.g4mt` | 71 | 1 765 056 o | 8 206 080 o |
//!
//! **Aucune dépendance nouvelle n'a été nécessaire.** Les neuf parseurs de `nie-formats` sont
//! derrière `#[cfg(feature = "std")]`, et `std` est une feature **par défaut** — le site les
//! liait déjà sans les appeler. C'est la définition même du câblage : le décodeur était là, la
//! route manquait. Seule la feature `serde` a été ajoutée, pour que `?forme=complet` rende la
//! structure décodée au lieu d'un `Debug` — un JSON public ne se sérialise pas par `Debug`.
//!
//! ## Décoder un fichier n'est pas assembler un modèle
//!
//! `.g4mg` est servi ici **et** par `/api/v1/3d`, et ce n'est pas un doublon : la 3D catalogue
//! des **entités assemblables** (`<code>/<code>.g4mg` plus la recette de l'amont, 7 466 codes
//! sur 7 679), quand cette route décode un **fichier**, quel qu'il soit — y compris les
//! maillages de décor, d'effet et de menu que l'amont ne sait pas assembler en GLB. C'est ce
//! qui fait tomber `partiel` à zéro sans rien promettre de faux : un décor décodé n'est pas un
//! personnage jouable, et les deux routes ne disent pas la même chose.
//!
//! ## Ce que chaque famille rend, et ce qu'elle ne rend pas
//!
//! Un décodeur qui « réussit » sans rien produire est le pire des résultats : il rassure. Deux
//! familles ne livrent qu'un en-tête, et le résumé le **dit** au lieu de le laisser croire :
//!
//! - `.col` — le conteneur `PXCL` est lu, son intérieur est du PhysX *cooked* que ce dépôt
//!   n'interprète pas. Le résumé porte `interieur_interprete: false`.
//! - `.g4mt` — [`nie_formats::g4mt::parse`] ne lit que l'en-tête ; l'animation vient de
//!   `Motion::parse`, qui rend `None` sur les conteneurs qu'il ne sait pas suivre. Le résumé
//!   porte alors `animation_decodee: false` plutôt qu'un compte de clips à zéro.
//!
//! Et une famille ne se lit **pas seule** : `.g4mg` a besoin de sa description, cf.
//! [`Compagnon`]. Elle est résolue par l'appelant (`super::formats`), là où l'index et le VFS
//! sont disponibles ; le décodeur reste une fonction pure, testable sans HTTP.
//!
//! ## Deux formes, et pourquoi la borne n'est pas la même
//!
//! `?forme=resume` (défaut) rend des **comptes** ; `?forme=complet` rend la structure entière.
//! La seconde est bornée par [`super::formats::TAILLE_MAX`] (4 Mio) parce qu'un JSON dépasse
//! plusieurs fois la taille de sa source et que l'ETag cesse de condenser au-delà de 8 Mio ; la
//! première est bornée par [`TAILLE_MAX_RESUME`] (16 Mio), qui couvre le plus gros `.g4pk` du
//! jeu (12 316 032 o) — un résumé ne grossit pas avec sa source.

use serde::Serialize;

use nie_formats::{col, g4cm, g4md, g4mg, g4mt, g4pk, g4pkm, g4sk, mevbin, objbin};

use crate::error::ErreurSite;

/// Taille au-delà de laquelle un fichier n'est plus lu pour un **résumé**.
///
/// 16 Mio : le plus gros `.g4pk` indexé pèse 12 316 032 o. La borne existe pour empêcher qu'un
/// fichier aberrant n'occupe la mémoire du service, pas pour écarter un cas réel.
pub const TAILLE_MAX_RESUME: usize = 16 * 1024 * 1024;

/// Une famille géométrique reconnue par son suffixe.
///
/// L'enum est la **source unique** : le suffixe, le jeton public, la ligne du tableau des
/// capacités et l'aiguillage du décodage en descendent tous. Ajouter une famille sans la
/// déclarer ici ne compile pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    /// Archive Level-5 (`.g4pk`) : une table de sous-fichiers nommés.
    G4pk,
    /// Paquet de menu (`.g4pkm`) : un squelette 2D et ses poses de liaison.
    G4pkm,
    /// Objet de menu (`.objbin`) : un objet racine et ses composants.
    Objbin,
    /// Animation de caméra (`.g4cm`) : clips, objets animés, canaux.
    G4cm,
    /// Collision (`.col`) : conteneur `PXCL`, intérieur PhysX non interprété.
    Col,
    /// Squelette (`.g4sk`) : hiérarchie d'os.
    G4sk,
    /// Événements d'animation (`.mevbin`) : motions et leurs événements datés.
    Mevbin,
    /// Animation squelettique (`.g4mt`) : clips, cibles, canaux.
    G4mt,
    /// Géométrie (`.g4mg`) : sous-mailles, sommets, triangles. **Illisible seule** — cf.
    /// [`Compagnon`].
    G4mg,
}

/// Les neuf familles, dans l'ordre décroissant de leur compte sur le VFS.
///
/// `(suffixe, famille, ce que le décodage produit)`.
pub const FAMILLES: [(&str, Famille, &str); 9] = [
    (".g4pk", Famille::G4pk, "table des sous-fichiers"),
    (".g4mg", Famille::G4mg, "sous-mailles, sommets et triangles"),
    (".objbin", Famille::Objbin, "objet de menu et ses composants"),
    (".g4pkm", Famille::G4pkm, "squelette 2D et poses de liaison"),
    (".g4cm", Famille::G4cm, "clips, objets et canaux de camera"),
    (".col", Famille::Col, "en-tete du conteneur PXCL"),
    (".g4sk", Famille::G4sk, "hierarchie d'os"),
    (".mevbin", Famille::Mevbin, "motions et evenements dates"),
    (".g4mt", Famille::G4mt, "clips et cibles d'animation"),
];

/// La description dont un `.g4mg` a besoin pour être lu — et qui n'est pas dans le `.g4mg`.
///
/// **Un `.g4mg` seul ne dit rien.** Les sommets y sont un tampon d'octets sans forme : ce sont
/// le stride, la disposition d'attributs et la table de sous-mailles du **G4MD** qui les
/// découpent. Décoder l'un sans l'autre ne rend pas un résultat pauvre, il ne rend rien.
///
/// Le G4MD vit à deux endroits, et la mesure du 2026-09-06 sur les 255 308 entrées de
/// `var/vfs/inventaire.txt` dit exactement lesquels — **aucun `.g4mg` n'est orphelin** :
///
/// | Où est la description | `.g4mg` concernés |
/// |---|---:|
/// | `.g4md` frère (`<base>.g4md`) | 8 955 |
/// | empaquetée dans le `.g4pkm` frère (`<base>.g4pkm`) | 6 920 |
/// | nulle part | **0** |
///
/// C'est le même mécanisme que l'amont applique aux cut-in `_waza`, et c'est pour cela que la
/// couverture est totale : le second cas n'est pas une exception, c'est la moitié du corpus.
#[derive(Debug, Clone)]
pub struct Compagnon {
    /// D'où vient la description : jeton `g4md` ou `g4pkm`.
    pub source: &'static str,
    /// Les octets du G4MD, déjà extraits de leur conteneur le cas échéant.
    pub octets: Vec<u8>,
}

impl Compagnon {
    /// Les chemins où chercher la description d'un `.g4mg`, dans l'ordre de préférence.
    ///
    /// Le `.g4md` d'abord : quand il existe en fichier libre, ouvrir le `.g4pkm` pour en
    /// extraire le même contenu serait du travail en pure perte.
    #[must_use]
    pub fn candidats(chemin_g4mg: &str) -> [String; 2] {
        let base = chemin_g4mg.trim_end_matches(".g4mg");
        [format!("{base}.g4md"), format!("{base}.g4pkm")]
    }

    /// Construit la description depuis les octets d'un candidat.
    ///
    /// Rend `None` quand le `.g4pkm` ne porte aucun G4MD — un conteneur peut n'avoir qu'un
    /// squelette.
    #[must_use]
    pub fn depuis(chemin: &str, octets: Vec<u8>) -> Option<Self> {
        if chemin.ends_with(".g4md") {
            return Some(Self {
                source: "g4md",
                octets,
            });
        }
        g4pkm::extract_g4md(&octets).map(|md| Self {
            source: "g4pkm",
            octets: md.to_vec(),
        })
    }
}

impl Famille {
    /// La famille d'un chemin, d'après son suffixe. `None` quand aucune ne correspond.
    ///
    /// Le suffixe est comparé **sans casse** : le VFS porte quelques chemins en majuscules, et
    /// une extension écartée pour sa casse rendrait un 400 qu'on attribuerait au format.
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

    /// Le jeton public de la famille — un contrat, jamais le nom d'une variante Rust.
    #[must_use]
    pub fn jeton(self) -> &'static str {
        self.suffixe().trim_start_matches('.')
    }

    /// Ce que le décodage de cette famille produit, en une phrase.
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

/// La forme demandée d'un décodage géométrique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forme {
    /// Des comptes, mesurés sur la structure décodée (défaut).
    Resume,
    /// La structure entière, telle que `nie-formats` la rend.
    Complet,
}

impl Forme {
    /// Reconnaît une forme, ou dit lesquelles existent.
    ///
    /// # Errors
    ///
    /// `Demande` sur une forme inconnue. Le message nomme les formes valides : un paramètre
    /// refusé sans dire ce qu'il attend envoie chercher au mauvais endroit.
    pub fn depuis(s: Option<&str>) -> Result<Self, ErreurSite> {
        match s.map(str::trim).filter(|f| !f.is_empty()) {
            None | Some("resume") => Ok(Self::Resume),
            Some("complet") => Ok(Self::Complet),
            Some(autre) => Err(ErreurSite::Demande(format!(
                "forme inconnue: {autre} (connues pour cette famille: resume, complet)"
            ))),
        }
    }
}

/// Le résumé d'un fichier géométrique : des comptes, une variante par famille.
///
/// `#[serde(tag = "famille")]` met le jeton dans le corps plutôt que dans un niveau
/// d'imbrication : un client lit `famille` sans savoir d'avance laquelle il a reçue.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "famille", rename_all = "snake_case")]
pub enum Resume {
    /// Archive `.g4pk`.
    G4pk {
        /// `archive` ou `menu` — le 5ᵉ octet du magic tranche, jamais le chemin.
        variante: &'static str,
        /// Version déclarée par l'en-tête.
        version: i32,
        /// Nombre de sous-fichiers de la table.
        sous_fichiers: usize,
        /// Somme de leurs tailles, en octets.
        sous_fichiers_octets: usize,
        /// Nombre de sous-fichiers dont le nom est résolu par la table de chaînes.
        noms_resolus: usize,
    },
    /// Paquet de menu `.g4pkm`.
    G4pkm {
        /// Nombre d'os du squelette 2D.
        os: usize,
        /// Nombre de noms distincts (un doublon garde la première pose).
        noms_distincts: usize,
        /// Nombre d'os racines (sans parent).
        racines: usize,
        /// Vrai quand le conteneur porte aussi un `.g4md` (le modèle).
        porte_un_g4md: bool,
    },
    /// Objet de menu `.objbin`.
    Objbin {
        /// Nom de l'objet racine.
        nom: String,
        /// Type moteur déclaré par `SETUP_BGN`.
        type_moteur: String,
        /// Nombre de composants attachés.
        composants: usize,
        /// Les types de composants présents, comptés — jetons choisis, pas des `Debug`.
        par_type: Vec<CompteType>,
        /// Chemins logiques référencés par l'objet, quand ils existent.
        references: Vec<Reference>,
    },
    /// Animation de caméra `.g4cm`.
    G4cm {
        /// Nombre de clips.
        clips: usize,
        /// Nombre de noms de la table de noms — il peut dépasser le nombre d'objets.
        noms: usize,
        /// Nombre d'objets animés.
        objets: usize,
        /// Nombre de canaux.
        canaux: usize,
        /// Nombre d'instants de la table de temps partagée.
        instants: usize,
    },
    /// Collision `.col`.
    Col {
        /// Taille de l'en-tête conteneur, en octets.
        entete_octets: usize,
        /// Décalage où commencent les données PhysX.
        donnees_decalage: usize,
        /// Vrai quand `header_size + data_size == file_size`.
        taille_coherente: bool,
        /// Toujours `false` : l'intérieur PhysX *cooked* n'est pas interprété par ce dépôt.
        interieur_interprete: bool,
    },
    /// Squelette `.g4sk`.
    G4sk {
        /// Nombre d'os déclaré par l'en-tête.
        os_declares: usize,
        /// Nombre d'os effectivement résolus.
        os_resolus: usize,
        /// Nombre d'os racines.
        racines: usize,
        /// Vrai quand la hiérarchie vient du scan heuristique et non de la table d'offsets —
        /// dans ce cas parents et noms sont **indicatifs**.
        heuristique: bool,
    },
    /// Événements d'animation `.mevbin`.
    Mevbin {
        /// Nombre de motions extraites.
        motions: usize,
        /// Nombre d'événements extraits.
        evenements: usize,
        /// Nombre de motions annoncé par l'en-tête.
        motions_declarees: i32,
        /// Nombre d'événements annoncé par l'en-tête.
        evenements_declares: i32,
        /// Vrai quand l'extrait correspond exactement à ce que l'en-tête annonce.
        conforme_a_l_entete: bool,
    },
    /// Géométrie `.g4mg`.
    G4mg {
        /// D'où venait la description : `g4md` (fichier frère) ou `g4pkm` (empaquetée).
        description: &'static str,
        /// Nombre de sous-mailles décodées.
        sous_mailles: usize,
        /// Nombre total de sommets.
        sommets: usize,
        /// Nombre total de triangles.
        triangles: usize,
        /// Nombre de matériaux déclarés par la description.
        materiaux: usize,
        /// Nombre d'os déclarés par la description (0 = maillage non skinné).
        os: usize,
        /// Nombre de sous-mailles portant des UV — sans elles, aucune texture ne s'applique.
        sous_mailles_texturees: usize,
    },
    /// Animation squelettique `.g4mt`.
    G4mt {
        /// Faux quand seul l'en-tête a pu être lu : les comptes sont alors nuls **parce que
        /// rien n'a été décodé**, ce qui n'est pas la même chose qu'une animation vide.
        animation_decodee: bool,
        /// Nombre de clips.
        clips: usize,
        /// Nombre de cibles (os visés par les canaux).
        cibles: usize,
        /// Somme des frames de tous les clips.
        frames: u32,
        /// Nombre de clips additifs (superposés à une pose de base).
        clips_additifs: usize,
    },
}

/// Un type de composant et son compte.
#[derive(Debug, Clone, Serialize)]
pub struct CompteType {
    /// Jeton du type (`render`, `animation`, `text`…).
    pub type_composant: &'static str,
    /// Nombre d'occurrences dans l'objet.
    pub nombre: usize,
}

/// Un chemin logique référencé par un objet de menu.
#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    /// Rôle de la référence (`g4pkm`, `g4tx`, `squelette`, `anime`).
    pub role: &'static str,
    /// Le chemin logique, tel que le fichier le porte — jamais réécrit.
    pub chemin: String,
}

/// Un fichier géométrique décodé.
#[derive(Debug, Clone, Serialize)]
pub struct Decodage {
    /// Chemin VFS décodé.
    pub chemin: String,
    /// Taille du fichier source, en octets.
    pub octets: usize,
    /// Jeton de la famille reconnue.
    pub format: &'static str,
    /// Ce que le décodage de cette famille produit, en une phrase.
    pub produit: &'static str,
    /// Les comptes.
    pub resume: Resume,
    /// La structure entière, seulement en `?forme=complet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub donnees: Option<serde_json::Value>,
}

/// Le jeton public d'un composant de menu.
fn jeton_composant(c: &objbin::MenuComponent) -> &'static str {
    use objbin::MenuComponent as C;
    match c {
        C::Render(_) => "render",
        C::Animation(_) => "animation",
        C::Text(_) => "text",
        C::Primitive(_) => "primitive",
        C::AttachLocator(_) => "attach_locator",
        C::Collision(_) => "collision",
        C::SoundCmd(_) => "sound_cmd",
        C::MeshVisible(_) => "mesh_visible",
        C::Unknown(_) => "inconnu",
    }
}

/// Traduit une erreur de parseur en `400`, en nommant la famille visée.
fn illisible(famille: Famille, e: &impl std::fmt::Display) -> ErreurSite {
    ErreurSite::Demande(format!("{} illisible: {e}", famille.jeton().to_uppercase()))
}

/// Décode des octets d'une famille géométrique. Séparée du handler pour être testable sans
/// HTTP ni VFS — c'est cette fonction que les tests falsifient.
///
/// # Errors
///
/// `Demande` quand les octets ne sont pas lisibles par le parseur de la famille.
pub fn decoder(
    chemin: &str,
    octets: &[u8],
    famille: Famille,
    forme: Forme,
    compagnon: Option<&Compagnon>,
) -> Result<Decodage, ErreurSite> {
    let complet = forme == Forme::Complet;
    let mut donnees = None;
    let resume = match famille {
        Famille::G4mg => {
            let c = compagnon.ok_or_else(|| {
                ErreurSite::Demande(
                    "un .g4mg ne se lit pas seul : sa description vit dans le .g4md frere ou \
                     dans le .g4pkm voisin, et aucun des deux n'a ete trouve"
                        .to_owned(),
                )
            })?;
            let md = g4md::parse(&c.octets)
                .map_err(|e| ErreurSite::Demande(format!("G4MD illisible: {e}")))?;
            let sous_mailles = g4mg::extract_geometry(octets, &md);
            let resume = Resume::G4mg {
                description: c.source,
                sous_mailles: sous_mailles.len(),
                sommets: sous_mailles.iter().map(|s| s.vertex_count).sum(),
                triangles: sous_mailles.iter().map(|s| s.indices.len() / 3).sum(),
                materiaux: md.material_base_names.len(),
                os: md.header.bone_count as usize,
                sous_mailles_texturees: sous_mailles.iter().filter(|s| !s.uv0.is_empty()).count(),
            };
            if complet {
                // La description ENTIÈRE et les métadonnées par sous-maille — mais **pas** les
                // tableaux de sommets. Un `.g4mg` de 12 Mio rendrait des centaines de mégaoctets
                // de JSON, que ni l'ETag (borne 8 Mio) ni un client ne sauraient traiter. Le
                // maillage complet se demande en GLB, sur `/model/{famille}/{code}.glb`.
                donnees = Some(serde_json::json!({
                    "description": en_valeur(&md)?,
                    "sous_mailles": sous_mailles
                        .iter()
                        .map(|s| serde_json::json!({
                            "index": s.index,
                            "sommets": s.vertex_count,
                            "triangles": s.indices.len() / 3,
                            "stride": s.stride,
                            "materiau": s.material_index,
                            "index32": s.index32,
                            "uv": !s.uv0.is_empty(),
                            "normales": !s.normals.is_empty(),
                            "couleurs": !s.colors.is_empty(),
                        }))
                        .collect::<Vec<_>>(),
                }));
            }
            resume
        }
        Famille::G4pk => {
            let a = g4pk::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&a)?);
            }
            Resume::G4pk {
                variante: match a.header.kind {
                    g4pk::G4pkKind::Archive => "archive",
                    g4pk::G4pkKind::Menu => "menu",
                },
                version: a.header.version,
                sous_fichiers: a.files.len(),
                sous_fichiers_octets: a.files.iter().map(|f| f.size).sum(),
                noms_resolus: a.files.iter().filter(|f| !f.name.is_empty()).count(),
            }
        }
        Famille::G4pkm => {
            let l = g4pkm::parse(octets).map_err(|e| illisible(famille, &e))?;
            if complet {
                donnees = Some(en_valeur(&l)?);
            }
            Resume::G4pkm {
                os: l.bones.len(),
                noms_distincts: l.world_pose_by_name.len(),
                racines: l.bones.iter().filter(|b| b.parent_index < 0).count(),
                porte_un_g4md: g4pkm::extract_g4md(octets).is_some(),
            }
        }
        Famille::Objbin => {
            let o = objbin::parse(octets).map_err(|e| illisible(famille, &e))?;
            let mut par_type: Vec<CompteType> = Vec::new();
            for c in &o.components {
                let t = jeton_composant(c);
                if let Some(e) = par_type.iter_mut().find(|e| e.type_composant == t) {
                    e.nombre += 1;
                } else {
                    par_type.push(CompteType {
                        type_composant: t,
                        nombre: 1,
                    });
                }
            }
            let references = [
                ("g4pkm", o.g4pkm_path.as_ref()),
                ("g4tx", o.g4tx_path.as_ref()),
                ("squelette", o.skeleton_path.as_ref()),
                ("anime", o.anime_path.as_ref()),
            ]
            .into_iter()
            .filter_map(|(role, c)| {
                c.map(|chemin| Reference {
                    role,
                    chemin: chemin.clone(),
                })
            })
            .collect();
            let resume = Resume::Objbin {
                nom: o.name.clone(),
                type_moteur: o.engine_type.clone(),
                composants: o.components.len(),
                par_type,
                references,
            };
            if complet {
                donnees = Some(en_valeur(&o)?);
            }
            resume
        }
        Famille::G4cm => {
            let a = g4cm::decode(octets).map_err(|e| illisible(famille, &e))?;
            let resume = Resume::G4cm {
                clips: a.clips.len(),
                noms: a.names.len(),
                objets: a.objects.len(),
                canaux: a.channels.len(),
                instants: a.times.len(),
            };
            if complet {
                donnees = Some(en_valeur(&a)?);
            }
            resume
        }
        Famille::Col => {
            let c = col::parse(octets).map_err(|e| illisible(famille, &e))?;
            let resume = Resume::Col {
                entete_octets: c.data_offset(),
                donnees_decalage: c.data_offset(),
                taille_coherente: c.is_size_consistent(),
                interieur_interprete: false,
            };
            if complet {
                donnees = Some(en_valeur(&c)?);
            }
            resume
        }
        Famille::G4sk => {
            let entete = g4sk::parse_header(octets).map_err(|e| illisible(famille, &e))?;
            let h = g4sk::parse_hierarchy(octets, &entete);
            let resume = Resume::G4sk {
                os_declares: entete.bone_count as usize,
                os_resolus: h.bones.len(),
                racines: h.bones.iter().filter(|b| b.parent_index < 0).count(),
                heuristique: h.heuristic,
            };
            if complet {
                donnees = Some(en_valeur(&h)?);
            }
            resume
        }
        Famille::Mevbin => {
            let d = mevbin::parse(octets).map_err(|e| illisible(famille, &e))?;
            let motions = d.motion_count();
            let evenements = d.parsed_event_count();
            let resume = Resume::Mevbin {
                motions,
                evenements,
                motions_declarees: d.header_motion_count,
                evenements_declares: d.header_event_count,
                conforme_a_l_entete: i32::try_from(motions) == Ok(d.header_motion_count)
                    && i32::try_from(evenements) == Ok(d.header_event_count),
            };
            if complet {
                donnees = Some(en_valeur(&d)?);
            }
            resume
        }
        Famille::G4mt => {
            // L'en-tête d'abord : il tranche entre « ce n'est pas un G4MT » (400) et « c'est
            // un G4MT dont l'animation ne se décode pas » (200, dit tel quel).
            let entete = g4mt::parse(octets).map_err(|e| illisible(famille, &e))?;
            match g4mt::Motion::parse(octets) {
                Some(m) => {
                    let resume = Resume::G4mt {
                        animation_decodee: true,
                        clips: m.clips.len(),
                        cibles: m.target_hashes.len(),
                        frames: m.clips.iter().map(g4mt::Clip::frame_count).sum(),
                        clips_additifs: m.clips.iter().filter(|c| c.is_additive()).count(),
                    };
                    if complet {
                        donnees = Some(en_valeur(&m)?);
                    }
                    resume
                }
                None => {
                    if complet {
                        donnees = Some(en_valeur(&entete)?);
                    }
                    Resume::G4mt {
                        animation_decodee: false,
                        clips: 0,
                        cibles: 0,
                        frames: 0,
                        clips_additifs: 0,
                    }
                }
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

/// Sérialise une structure de `nie-formats` en valeur JSON.
fn en_valeur<T: Serialize>(v: &T) -> Result<serde_json::Value, ErreurSite> {
    serde_json::to_value(v)
        .map_err(|e| ErreurSite::Interne(format!("structure non serialisable: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_neuf_familles_sont_distinctes_et_completes() {
        let suffixes: Vec<&str> = FAMILLES.into_iter().map(|(s, ..)| s).collect();
        assert_eq!(suffixes.len(), 9);
        let mut tries = suffixes.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), 9, "deux familles partagent un suffixe");
        for (s, f, produit) in FAMILLES {
            assert!(s.starts_with('.'), "{s}");
            assert_eq!(f.suffixe(), s);
            assert_eq!(f.jeton(), s.trim_start_matches('.'));
            assert!(!produit.is_empty());
        }
    }

    #[test]
    fn la_famille_se_reconnait_au_suffixe_sans_casse() {
        assert_eq!(
            Famille::depuis_chemin("data/common/chr/a.g4pk"),
            Some(Famille::G4pk)
        );
        assert_eq!(
            Famille::depuis_chemin("data/common/chr/A.OBJBIN"),
            Some(Famille::Objbin)
        );
        // `.g4pkm` ne doit pas être happé par `.g4pk` : les deux sont dans la table, et
        // l'ordre de déclaration met `.g4pk` en premier. Le suffixe complet tranche.
        assert_eq!(
            Famille::depuis_chemin("data/common/menu/a.g4pkm"),
            Some(Famille::G4pkm)
        );
        assert_eq!(Famille::depuis_chemin("data/common/a.cfg.bin"), None);
        assert_eq!(Famille::depuis_chemin("data/common/a"), None);
    }

    /// Le test qui prouve que le décodage **peut** échouer : sans lui, une famille dont le
    /// parseur accepterait n'importe quoi passerait pour verte.
    #[test]
    fn chaque_famille_refuse_des_octets_qui_ne_sont_pas_les_siens() {
        for (suffixe, famille, _) in FAMILLES {
            let e = decoder(
                &format!("data/x{suffixe}"),
                b"ceci n'est certainement pas un fichier du jeu",
                famille,
                Forme::Resume,
                None,
            )
            .unwrap_err();
            assert_eq!(e.statut().as_u16(), 400, "{suffixe}");
        }
    }

    #[test]
    fn les_deux_formes_sont_reconnues_et_les_autres_refusees() {
        assert_eq!(Forme::depuis(None).unwrap(), Forme::Resume);
        assert_eq!(Forme::depuis(Some("complet")).unwrap(), Forme::Complet);
        assert_eq!(Forme::depuis(Some(" ")).unwrap(), Forme::Resume);
        let e = Forme::depuis(Some("structure")).unwrap_err();
        assert_eq!(e.statut().as_u16(), 400);
    }

    /// Un `.col` minimal, construit ici : en-tête `PXCL` de 0x10 octets, `data_size` cohérent.
    /// Il prouve que le chemin nominal produit un résumé, et que `interieur_interprete` reste
    /// `false` — la seule affirmation honnête sur du PhysX *cooked*.
    #[test]
    fn un_conteneur_pxcl_minimal_se_resume() {
        let mut o = Vec::new();
        o.extend_from_slice(b"PXCL");
        o.extend_from_slice(&16u16.to_le_bytes()); // header_size
        o.extend_from_slice(&0u16.to_le_bytes()); // type_id
        o.extend_from_slice(&0u32.to_le_bytes()); // decompressed / inutilisé ici
        o.extend_from_slice(&16u32.to_le_bytes()); // data_size
        o.extend_from_slice(&[0u8; 16]); // le corps PhysX, non interprété
        let d = decoder("data/x.col", &o, Famille::Col, Forme::Resume, None).unwrap();
        assert_eq!(d.format, "col");
        assert_eq!(d.octets, 32);
        assert!(d.donnees.is_none(), "le resume ne porte pas les donnees");
        match d.resume {
            Resume::Col {
                interieur_interprete,
                entete_octets,
                ..
            } => {
                assert!(!interieur_interprete);
                assert_eq!(entete_octets, 16);
            }
            autre => panic!("mauvaise variante: {autre:?}"),
        }
    }

    /// Le JSON porte la famille **dans** le corps, à plat : un client lit `famille` sans
    /// connaître d'avance la variante reçue.
    #[test]
    fn le_json_porte_le_jeton_de_famille() {
        let r = Resume::G4mt {
            animation_decodee: false,
            clips: 0,
            cibles: 0,
            frames: 0,
            clips_additifs: 0,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["famille"], "g4mt");
        assert_eq!(v["animation_decodee"], false);
        // Le nom de la variante Rust ne doit apparaître nulle part.
        assert!(!serde_json::to_string(&r).unwrap().contains("G4mt"));
    }
}
