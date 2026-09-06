//! Analyse mesurée des **planches de personnage** — les textures de `chr/_face/20_EDIT`.
//!
//! Une planche est une texture de part d'avatar : peau, œil, pupille, reflet, sourcil, bouche,
//! coiffure. Elle ne se lit pas comme une image ordinaire, et c'est tout le problème :
//!
//! * son canal alpha **n'est pas** son opacité — [`crate::image_out::teinter_par_canaux`] montre
//!   que chaque canal désigne une zone à teindre ;
//! * son dessin vit tantôt dans la planche de couleur (`05_mouth` peint quatre bouches au trait),
//!   tantôt dans son masque compagnon `<nom>msk` (`01_eye` est muette, le masque porte tout) ;
//! * la convention du masque **change d'une famille à l'autre** : sur la bouche le noir est
//!   l'encre et le rouge le fond, sur l'œil le noir est l'ouverture et le vert le tracé.
//!
//! Ces trois faits ont été établis planche par planche, à la main, sur une ou deux textures
//! chacun. Ce module en fait une **mesure**, applicable au corpus entier : [`mesurer`] chiffre une
//! planche décodée, [`analyser`] apparie couleur et masque dans un conteneur G4TX, et
//! [`Convention::deriver`] dit, à partir des seuls chiffres, comment la composer.
//!
//! Rien ici ne décide d'après un nom de famille. Une règle qui vaut pour `01_eye` doit se lire
//! dans les octets de `01_eye`, sinon elle n'est pas une règle mais une exception recopiée.
//!
//! # Ce que le module ne fait pas
//!
//! Il ne compose rien et ne réécrit aucun pixel : c'est le rôle de [`crate::image_out`], qui
//! s'appuie sur les prédicats définis ici pour ne pas les dupliquer.

extern crate alloc;

// `format!` ne sert qu'à `analyser`, derrière la feature `textures` : importer sans le garde
// laissait un `unused_imports` sur tout build par défaut.
#[cfg(feature = "textures")]
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

// ── Seuils ───────────────────────────────────────────────────────────────────
//
// Tous les seuils du module vivent ici, et nulle part ailleurs. Ils sont ceux qui étaient
// dispersés dans `image_out` ; les y avoir laissés en trois exemplaires faisait diverger les
// prédicats de mesure de ceux de composition.

/// Canal minimal pour qu'un pixel soit tenu pour le **fond rouge** d'un masque de zones.
pub const FOND_ROUGE_MIN: u8 = 160;

/// Canaux vert et bleu maximaux d'un fond rouge : au-delà, la teinte n'est plus franche.
pub const FOND_ROUGE_AUTRES_MAX: u8 = 96;

/// Canal minimal pour qu'une zone verte ou bleue soit tenue pour désignée.
pub const ZONE_VIVE_MIN: u8 = 128;

/// Plafond des trois canaux d'un pixel **noir**.
pub const NOIR_MAX: u8 = 32;

/// Plancher des trois canaux d'un pixel **blanc**.
pub const BLANC_MIN: u8 = 224;

/// Somme maximale des trois canaux d'un pixel d'**encre** — un trait dessiné, pas une teinte pâle.
pub const ENCRE_SOMME_MAX: u16 = 288;

/// Alpha minimal pour qu'un pixel d'encre compte : un trait transparent ne se voit pas.
pub const ENCRE_ALPHA_MIN: u8 = 128;

/// Part de pixels d'encre au-delà de laquelle une planche est tenue pour **dessinée**.
///
/// Un demi pour cent : `mouth_01` peint quatre bouches et dépasse largement, `eye_L_01` et ses
/// pâles ovales gris n'en produisent aucun.
pub const PART_ENCRE_TRACE: f32 = 0.005;

/// Part de fond rouge au-delà de laquelle un masque est tenu pour un **masque de zones**.
///
/// Un masque de découpe est gris — son canal rouge est l'opacité — et n'atteint jamais ce seuil.
pub const PART_FOND_ZONES: f32 = 0.10;

/// Part minimale d'une zone pour qu'elle compte comme désignée et non comme du bruit de bord.
pub const PART_ZONE_UTILE: f32 = 0.005;

/// Nombre maximal de couleurs distinctes recensées avant abandon du comptage.
///
/// Le compte exact n'apporte rien au-delà : ce qui se décide, c'est « une seule couleur »,
/// « une poignée », ou « un dégradé ». Le plafond garde la mesure bornée sur une 2048 × 1024.
pub const PLAFOND_COULEURS: usize = 4096;

// ── Zones ────────────────────────────────────────────────────────────────────

/// Nombre de zones distinguées par [`Zone`].
pub const NB_ZONES: usize = 6;

/// Classe de couleur d'un pixel, telle qu'un masque de zones les emploie.
///
/// La classification est exclusive et ordonnée : un pixel tombe dans la première classe qui
/// l'accepte. Elle vaut aussi bien pour une planche de couleur — où elle mesure ce que la planche
/// contient — que pour un masque, où elle nomme ses régions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zone {
    /// Les trois canaux sous [`NOIR_MAX`].
    Noir,
    /// Les trois canaux au-dessus de [`BLANC_MIN`].
    Blanc,
    /// Rouge franc : le fond d'un masque de zones.
    Rouge,
    /// Vert dominant : le tracé, sur les masques d'œil.
    Vert,
    /// Bleu dominant : l'ovale de pupille.
    Bleu,
    /// Tout le reste — une teinte intermédiaire ne désigne rien.
    Autre,
}

impl Zone {
    /// Indice de la zone dans les tableaux de [`Mesures`].
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Noir => 0,
            Self::Blanc => 1,
            Self::Rouge => 2,
            Self::Vert => 3,
            Self::Bleu => 4,
            Self::Autre => 5,
        }
    }

    /// Les six zones, dans l'ordre de leur indice.
    #[must_use]
    pub fn toutes() -> [Self; NB_ZONES] {
        [
            Self::Noir,
            Self::Blanc,
            Self::Rouge,
            Self::Vert,
            Self::Bleu,
            Self::Autre,
        ]
    }

    /// Nom court, pour l'affichage.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::Noir => "noir",
            Self::Blanc => "blanc",
            Self::Rouge => "rouge",
            Self::Vert => "vert",
            Self::Bleu => "bleu",
            Self::Autre => "autre",
        }
    }

    /// Classe un pixel RGB.
    #[must_use]
    pub fn du_pixel(r: u8, v: u8, b: u8) -> Self {
        if r < NOIR_MAX && v < NOIR_MAX && b < NOIR_MAX {
            Self::Noir
        } else if r > BLANC_MIN && v > BLANC_MIN && b > BLANC_MIN {
            Self::Blanc
        } else if r > FOND_ROUGE_MIN && v < FOND_ROUGE_AUTRES_MAX && b < FOND_ROUGE_AUTRES_MAX {
            Self::Rouge
        } else if v > ZONE_VIVE_MIN && v > r && v > b {
            Self::Vert
        } else if b > ZONE_VIVE_MIN && b > r && b > v {
            Self::Bleu
        } else {
            Self::Autre
        }
    }
}

// ── Mesures ──────────────────────────────────────────────────────────────────

/// Emprise normalisée d'un ensemble de pixels : `[u_min, v_min, u_max, v_max]` dans `[0, 1]`.
pub type Emprise = [f32; 4];

/// Ce qu'une planche décodée contient, en chiffres.
///
/// Toutes les parts sont dans `[0, 1]` et rapportées au nombre total de pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesures {
    /// Largeur en pixels.
    pub largeur: u32,
    /// Hauteur en pixels.
    pub hauteur: u32,
    /// Nombre de pixels mesurés.
    pub pixels: usize,
    /// Part de chaque [`Zone`], indexée par [`Zone::index`].
    pub parts: [f32; NB_ZONES],
    /// Emprise de chaque [`Zone`], indexée de même ; `None` si la zone est vide.
    pub emprises: [Option<Emprise>; NB_ZONES],
    /// Part de pixels d'**encre** — sombres ET opaques, donc un trait effectivement visible.
    pub part_encre: f32,
    /// Emprise de l'encre.
    pub emprise_encre: Option<Emprise>,
    /// Alpha moyen, dans `[0, 255]`.
    pub alpha_moyen: f32,
    /// Alpha minimal rencontré.
    pub alpha_min: u8,
    /// Alpha maximal rencontré.
    pub alpha_max: u8,
    /// Pour chaque canal RGBA, vrai si toutes ses valeurs sont égales.
    pub canaux_constants: [bool; 4],
    /// Nombre de couleurs RGBA distinctes, plafonné à [`PLAFOND_COULEURS`].
    pub couleurs: usize,
    /// Vrai si le plafond de comptage a été atteint — le compte est alors une borne inférieure.
    pub couleurs_plafonnees: bool,
    /// Couleur moyenne, canal par canal.
    pub couleur_moyenne: [u8; 3],
}

impl Mesures {
    /// Part d'une zone.
    #[must_use]
    pub fn part(&self, zone: Zone) -> f32 {
        self.parts[zone.index()]
    }

    /// Emprise d'une zone.
    #[must_use]
    pub fn emprise(&self, zone: Zone) -> Option<Emprise> {
        self.emprises[zone.index()]
    }

    /// La planche est-elle un **aplat** — une seule couleur sur toute sa surface ?
    ///
    /// C'est le cas de `04_eyebrow/eyebrow_00`, rouge pur sur 100 % des pixels : une planche qui
    /// ne porte aucun tracé et ne peut donc rien dessiner.
    #[must_use]
    pub fn est_aplat(&self) -> bool {
        self.pixels > 0 && self.couleurs == 1
    }

    /// La planche porte-t-elle un **trait dessiné** ?
    #[must_use]
    pub fn porte_un_trait(&self) -> bool {
        self.part_encre > PART_ENCRE_TRACE
    }

    /// Est-ce un **masque de zones** — un fond rouge franc et des régions d'une autre couleur ?
    ///
    /// Plus exigeant que [`crate::image_out::masque_de_zones`], qui ne regarde que le fond : ici
    /// une seconde zone est requise. La différence n'est pas cosmétique — `eyebrow_00msk` est un
    /// rouge pur, que le prédicat brut accepte et qui rend alors la planche entièrement
    /// transparente. Pour une mesure, « fond seul » et « fond plus régions » sont deux constats
    /// distincts, et le second seul décrit un masque exploitable.
    #[must_use]
    pub fn est_masque_de_zones(&self) -> bool {
        self.part(Zone::Rouge) > PART_FOND_ZONES
            && Zone::toutes()
                .iter()
                .any(|z| !matches!(z, Zone::Rouge) && self.part(*z) > PART_ZONE_UTILE)
    }

    /// La planche est-elle muette — sans information spatiale dans son canal rouge ?
    ///
    /// C'est le critère qu'emploie la composition pour décider si un masque mérite d'être posé en
    /// alpha : uniforme, il n'apporte rien et l'appliquer efface les variations de la planche.
    #[must_use]
    pub fn canal_uniforme(&self) -> bool {
        self.canaux_constants[0]
    }

    /// Les zones effectivement présentes, de la plus étendue à la plus rare, seuil
    /// [`PART_ZONE_UTILE`] appliqué.
    #[must_use]
    pub fn zones_presentes(&self) -> Vec<(Zone, f32)> {
        let mut v: Vec<(Zone, f32)> = Zone::toutes()
            .into_iter()
            .map(|z| (z, self.part(z)))
            .filter(|(_, p)| *p > PART_ZONE_UTILE)
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }
}

/// Mesure une planche RGBA8 décodée.
///
/// `rgba` doit contenir au moins `largeur × hauteur × 4` octets ; le surplus est ignoré. Rend
/// `None` si la taille annoncée est nulle ou si le tampon est trop court — ne jamais mesurer une
/// image partielle : les parts seraient calculées sur un dénominateur faux.
#[must_use]
pub fn mesurer(largeur: u32, hauteur: u32, rgba: &[u8]) -> Option<Mesures> {
    let pixels = largeur as usize * hauteur as usize;
    if pixels == 0 || rgba.len() < pixels * 4 {
        return None;
    }
    let utile = &rgba[..pixels * 4];

    let mut comptes = [0usize; NB_ZONES];
    // Boîtes en pixels : (x_min, y_min, x_max, y_max), initialisées à l'envers.
    let mut boites = [None::<(u32, u32, u32, u32)>; NB_ZONES];
    let mut boite_encre = None::<(u32, u32, u32, u32)>;
    let mut encre = 0usize;
    let mut somme_alpha = 0u64;
    let mut somme_rvb = [0u64; 3];
    let (mut alpha_min, mut alpha_max) = (255u8, 0u8);
    let premier: [u8; 4] = [utile[0], utile[1], utile[2], utile[3]];
    let mut constants = [true; 4];
    let mut palette = alloc::collections::BTreeSet::new();
    let mut plafonnee = false;

    for (i, p) in utile.chunks_exact(4).enumerate() {
        let (r, v, b, a) = (p[0], p[1], p[2], p[3]);
        let (x, y) = ((i % largeur as usize) as u32, (i / largeur as usize) as u32);

        let zone = Zone::du_pixel(r, v, b);
        comptes[zone.index()] += 1;
        etendre(&mut boites[zone.index()], x, y);

        if a > ENCRE_ALPHA_MIN && u16::from(r) + u16::from(v) + u16::from(b) < ENCRE_SOMME_MAX {
            encre += 1;
            etendre(&mut boite_encre, x, y);
        }

        somme_alpha += u64::from(a);
        somme_rvb[0] += u64::from(r);
        somme_rvb[1] += u64::from(v);
        somme_rvb[2] += u64::from(b);
        alpha_min = alpha_min.min(a);
        alpha_max = alpha_max.max(a);
        for c in 0..4 {
            constants[c] &= p[c] == premier[c];
        }
        if !plafonnee {
            palette.insert([r, v, b, a]);
            if palette.len() >= PLAFOND_COULEURS {
                plafonnee = true;
            }
        }
    }

    let n = pixels as f32;
    let mut parts = [0f32; NB_ZONES];
    let mut emprises = [None; NB_ZONES];
    for z in 0..NB_ZONES {
        parts[z] = comptes[z] as f32 / n;
        emprises[z] = boites[z].map(|b| normaliser(b, largeur, hauteur));
    }

    Some(Mesures {
        largeur,
        hauteur,
        pixels,
        parts,
        emprises,
        part_encre: encre as f32 / n,
        emprise_encre: boite_encre.map(|b| normaliser(b, largeur, hauteur)),
        alpha_moyen: somme_alpha as f32 / n,
        alpha_min,
        alpha_max,
        canaux_constants: constants,
        couleurs: palette.len(),
        couleurs_plafonnees: plafonnee,
        couleur_moyenne: [
            (somme_rvb[0] / pixels as u64) as u8,
            (somme_rvb[1] / pixels as u64) as u8,
            (somme_rvb[2] / pixels as u64) as u8,
        ],
    })
}

// ── Prédicats sans dimensions ────────────────────────────────────────────────
//
// La composition ([`crate::image_out`]) travaille sur des tampons dont elle connaît déjà la
// taille et n'a pas besoin d'une mesure complète. Ces trois fonctions sont la source unique de
// ses prédicats : les seuils ne vivent qu'ici, et une planche mesurée par [`mesurer`] répond donc
// exactement comme un tampon passé à la composition.

/// Part de pixels d'**encre** d'un tampon RGBA — sombres et opaques.
///
/// Rend `0.0` sur un tampon vide.
#[must_use]
pub fn part_encre_brute(rgba: &[u8]) -> f32 {
    let total = rgba.len() / 4;
    if total == 0 {
        return 0.0;
    }
    let encre = rgba
        .chunks_exact(4)
        .filter(|p| {
            p[3] > ENCRE_ALPHA_MIN
                && u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]) < ENCRE_SOMME_MAX
        })
        .count();
    encre as f32 / total as f32
}

/// Part de pixels d'une [`Zone`] donnée dans un tampon RGBA.
///
/// Rend `0.0` sur un tampon vide.
#[must_use]
pub fn part_zone_brute(rgba: &[u8], zone: Zone) -> f32 {
    let total = rgba.len() / 4;
    if total == 0 {
        return 0.0;
    }
    let n = rgba
        .chunks_exact(4)
        .filter(|p| Zone::du_pixel(p[0], p[1], p[2]) == zone)
        .count();
    n as f32 / total as f32
}

/// Vrai si le canal rouge d'un tampon RGBA est constant — donc sans information spatiale.
///
/// Un masque uniforme n'apporte rien : le poser en alpha efface les variations de la planche.
#[must_use]
pub fn canal_uniforme(rgba: &[u8]) -> bool {
    let Some(premier) = rgba.first().copied() else {
        return true;
    };
    rgba.iter().step_by(4).all(|&v| v == premier)
}

/// Étend une boîte englobante en pixels pour inclure `(x, y)`.
fn etendre(boite: &mut Option<(u32, u32, u32, u32)>, x: u32, y: u32) {
    *boite = Some(match *boite {
        None => (x, y, x, y),
        Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
    });
}

/// Convertit une boîte en pixels vers une emprise normalisée.
///
/// Le coin bas-droit est pris **inclusif** : un unique pixel occupe `1 / largeur`, pas zéro.
fn normaliser(boite: (u32, u32, u32, u32), largeur: u32, hauteur: u32) -> Emprise {
    let (x0, y0, x1, y1) = boite;
    [
        x0 as f32 / largeur as f32,
        y0 as f32 / hauteur as f32,
        (x1 + 1) as f32 / largeur as f32,
        (y1 + 1) as f32 / hauteur as f32,
    ]
}

// ── Rôle et convention ───────────────────────────────────────────────────────

/// Ce qu'une planche est, d'après ses seules mesures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Une seule couleur sur toute la surface — la planche ne dessine rien.
    Aplat,
    /// Un fond rouge et des régions désignées : c'est un masque, pas une image.
    Zones,
    /// Un trait dessiné, encre comprise : la planche porte le dessin.
    Trace,
    /// Ni aplat, ni zones, ni trait : une teinte qui varie sans rien désigner.
    Nuance,
}

impl Role {
    /// Nom court, pour l'affichage.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::Aplat => "aplat",
            Self::Zones => "zones",
            Self::Trace => "trace",
            Self::Nuance => "nuance",
        }
    }

    /// Déduit le rôle des mesures.
    ///
    /// L'ordre compte : un aplat rouge pur satisferait aussi le test du fond de zones, sans en
    /// être un — il n'a aucune région à désigner.
    #[must_use]
    pub fn deriver(m: &Mesures) -> Self {
        if m.est_aplat() {
            Self::Aplat
        } else if m.est_masque_de_zones() {
            Self::Zones
        } else if m.porte_un_trait() {
            Self::Trace
        } else {
            Self::Nuance
        }
    }
}

/// Comment composer une planche, déduit des mesures de la planche ET de son masque.
///
/// C'est la conclusion utile du module : elle remplace les tests par nom de famille qui
/// parsemaient la composition (`if rel.starts_with("01_eye")`) par une règle lisible dans les
/// octets. Une famille dont toutes les planches donnent la même convention justifie la règle ;
/// une famille qui en donne deux dit que la règle par famille était fausse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    /// Pas de masque exploitable : la planche se compose telle quelle.
    SansMasque,
    /// Masque gris dont le canal rouge EST l'opacité — à poser en alpha.
    Decoupe,
    /// Masque de zones, et la **couleur** porte le tracé : ne retirer que le fond rouge.
    ///
    /// Convention de `05_mouth`. Cf. [`crate::image_out::decouper_par_zones`].
    FondRouge,
    /// Masque de zones, et le **masque** porte le tracé en vert : l'alpha vient de cette zone.
    ///
    /// Convention de `01_eye`. Cf. [`crate::image_out::decouper_oeil`].
    TraceVert,
    /// Masque de zones dont la région désignée est **bleue**, et sans vert : les pupilles.
    ///
    /// Mesurée sur les 16 planches que la règle du vert laissait sans réponse — 14 pupilles, dont
    /// `pupil_L_01msk` : 64,06 % de rouge, 35,90 % de bleu, aucun vert. Le bleu n'y dessine pas un
    /// trait mais un **ovale plein qui occupe tout le carré**, ce qui est déjà une information :
    /// une planche de cette forme ne peut pas être destinée au dépliage du visage, où elle poserait
    /// un ovale au milieu de la figure — c'est exactement ce que le rendu actuel produit.
    ///
    /// La composition ne la découpe donc **pas** : le matériau d'accueil de `02_pupil` n'est pas
    /// établi, et découper une zone au bon endroit d'un dépliage douteux ne prouverait rien.
    /// Cf. [`crate::assemble::face_layer_slot`].
    ZoneBleue,
    /// La planche est un aplat : rien à composer, quoi que dise le masque.
    Aplat,
    /// Un masque existe mais ne désigne rien d'exploitable.
    Indeterminee,
}

impl Convention {
    /// Nom court, pour l'affichage.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Self::SansMasque => "sans-masque",
            Self::Decoupe => "decoupe",
            Self::FondRouge => "fond-rouge",
            Self::TraceVert => "trace-vert",
            Self::ZoneBleue => "zone-bleue",
            Self::Aplat => "aplat",
            Self::Indeterminee => "indeterminee",
        }
    }

    /// Déduit la convention de composition.
    ///
    /// La règle, dans l'ordre :
    ///
    /// 1. sans masque variant : une planche d'un seul coloris ne dessine rien
    ///    ([`Convention::Aplat`]), toute autre se compose seule ([`Convention::SansMasque`]) ;
    /// 2. masque non-zones (gris) → son rouge est l'opacité — [`Convention::Decoupe`] ;
    /// 3. masque de zones **et** couleur dessinée → [`Convention::FondRouge`] : le dessin est déjà
    ///    là, le masque ne fait que retirer le fond ;
    /// 4. masque de zones, couleur muette, **zone verte présente** → [`Convention::TraceVert`] :
    ///    le tracé n'existe que dans le masque, l'alpha se pose depuis le vert ;
    /// 5. à défaut de vert, **zone bleue présente** → [`Convention::ZoneBleue`] ;
    /// 6. sinon [`Convention::Indeterminee`] — mieux vaut le dire que composer au hasard.
    ///
    /// Le vert prime sur le bleu parce qu'un masque qui porte les deux est un masque d'œil, où le
    /// vert est le tracé de paupière ; le bleu seul est la marque des pupilles.
    ///
    /// **L'aplat ne prime pas sur le masque**, et c'est la mesure qui l'impose : les 32 planches
    /// de couleur de `03_highlight` sont blanches et identiques d'une variante à l'autre, tout
    /// leur dessin vivant dans le masque. Les écarter parce que leur couleur est unie effacerait
    /// les reflets. Un aplat n'est stérile que si son masque l'est aussi — c'est exactement le cas
    /// d'`eyebrow_00`, planche blanche sous un masque rouge à 100 % : la variante « sans sourcil ».
    #[must_use]
    pub fn deriver(couleur: &Mesures, masque: Option<&Mesures>) -> Self {
        let Some(m) = masque.filter(|m| !m.canal_uniforme()) else {
            return if couleur.est_aplat() {
                Self::Aplat
            } else {
                Self::SansMasque
            };
        };
        if !m.est_masque_de_zones() {
            return Self::Decoupe;
        }
        if couleur.porte_un_trait() {
            return Self::FondRouge;
        }
        if m.part(Zone::Vert) > PART_ZONE_UTILE {
            return Self::TraceVert;
        }
        if m.part(Zone::Bleu) > PART_ZONE_UTILE {
            return Self::ZoneBleue;
        }
        Self::Indeterminee
    }
}

// ── Fiche d'une planche dans son conteneur ───────────────────────────────────

/// Suffixe du masque compagnon d'une planche de couleur.
pub const SUFFIXE_MASQUE: &str = "msk";

/// Une planche de couleur, son masque s'il existe, et ce qu'on en déduit.
#[derive(Debug, Clone)]
pub struct Fiche {
    /// Nom de la planche de couleur, tel que le conteneur la nomme.
    pub nom: String,
    /// Mesures de la planche de couleur.
    pub couleur: Mesures,
    /// Rôle de la planche de couleur.
    pub role: Role,
    /// Nom du masque compagnon, s'il a été décodé aux mêmes dimensions.
    pub nom_masque: Option<String>,
    /// Mesures du masque.
    pub masque: Option<Mesures>,
    /// Rôle du masque.
    pub role_masque: Option<Role>,
    /// Convention de composition déduite.
    pub convention: Convention,
}

impl Fiche {
    /// Vrai si la planche ne peut rien dessiner : ni elle ni son masque ne portent de forme.
    ///
    /// Le cas se rencontre, et il est **intentionnel** : `04_eyebrow/eyebrow_00` est une planche
    /// unie sous un masque rouge à 100 %, soit la variante « sans sourcil » de la famille. La
    /// composer donnerait une surface de carnation opaque qui masquerait les couches déjà posées.
    /// Une pièce absente du modèle s'explique donc parfois par la donnée seule, sans qu'il faille
    /// soupçonner le compositeur — mais l'inverse vaut aussi : les 39 autres conteneurs de la
    /// même famille, eux, portent bien un tracé.
    #[must_use]
    pub fn est_muette(&self) -> bool {
        matches!(self.convention, Convention::Aplat)
    }
}

/// Analyse toutes les planches de couleur d'un conteneur G4TX, masques appariés.
///
/// L'appariement suit la convention du jeu : la planche `eye_L_01` a pour masque `eye_L_01msk`,
/// de mêmes dimensions. Un masque de dimensions différentes est ignoré plutôt que redimensionné —
/// il ne décrirait pas les mêmes pixels.
///
/// Rend une liste vide si le conteneur ne se parse pas ou ne porte aucune couleur de base.
#[cfg(feature = "textures")]
#[must_use]
pub fn analyser(g4tx: &[u8]) -> Vec<Fiche> {
    crate::g4tx::base_color_texture_names(g4tx)
        .into_iter()
        .filter_map(|nom| {
            let (w, h, rgba) = crate::g4tx_decode::decode_named_to_rgba(g4tx, &nom)?;
            let couleur = mesurer(w, h, &rgba)?;
            let nom_masque = format!("{nom}{SUFFIXE_MASQUE}");
            let masque = crate::g4tx_decode::decode_named_to_rgba(g4tx, &nom_masque)
                .filter(|(mw, mh, _)| (*mw, *mh) == (w, h))
                .and_then(|(mw, mh, m)| mesurer(mw, mh, &m));
            let convention = Convention::deriver(&couleur, masque.as_ref());
            Some(Fiche {
                role: Role::deriver(&couleur),
                role_masque: masque.as_ref().map(Role::deriver),
                nom,
                nom_masque: masque.as_ref().map(|_| nom_masque.clone()),
                couleur,
                masque,
                convention,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique une image RGBA unie.
    fn unie(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        c.iter()
            .copied()
            .cycle()
            .take((w * h * 4) as usize)
            .collect()
    }

    /// Peint un rectangle dans une image RGBA.
    fn peindre(img: &mut [u8], w: u32, rect: (u32, u32, u32, u32), c: [u8; 4]) {
        let (x0, y0, x1, y1) = rect;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * w + x) * 4) as usize;
                img[i..i + 4].copy_from_slice(&c);
            }
        }
    }

    #[test]
    fn un_aplat_est_reconnu_et_ne_dessine_rien() {
        let img = unie(8, 8, [255, 0, 0, 255]);
        let m = mesurer(8, 8, &img).expect("mesure");
        assert!(m.est_aplat());
        assert_eq!(m.couleurs, 1);
        assert_eq!(m.part(Zone::Rouge), 1.0);
        assert_eq!(Role::deriver(&m), Role::Aplat);
        // Sous un masque uniforme — le cas d'`eyebrow_00`, rouge à 100 % — rien à composer.
        let masque = mesurer(8, 8, &unie(8, 8, [200, 0, 0, 255])).expect("mesure");
        assert_eq!(Convention::deriver(&m, Some(&masque)), Convention::Aplat);
    }

    #[test]
    fn un_aplat_dont_le_masque_varie_se_decoupe_quand_meme() {
        // Cas des reflets : les 32 planches de couleur de `03_highlight` sont blanches et
        // identiques, tout leur dessin vit dans le masque. Les écarter les effacerait.
        let couleur = mesurer(16, 16, &unie(16, 16, [255, 255, 255, 255])).expect("mesure");
        assert!(couleur.est_aplat());
        let mut msk = unie(16, 16, [0, 0, 0, 255]);
        for y in 0..16u32 {
            peindre(&mut msk, 16, (0, y, 16, y + 1), [(y * 16) as u8; 4]);
        }
        let masque = mesurer(16, 16, &msk).expect("mesure");
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::Decoupe
        );
    }

    #[test]
    fn le_fond_rouge_seul_ne_fait_pas_un_masque_de_zones() {
        // 100 % de rouge : aucune région désignée, donc rien à découper.
        let m = mesurer(8, 8, &unie(8, 8, [255, 0, 0, 255])).expect("mesure");
        assert!(!m.est_masque_de_zones());
    }

    #[test]
    fn masque_de_zones_a_la_maniere_de_la_bouche() {
        // Fond rouge, encre noire sur 1/4 de la surface.
        let mut img = unie(16, 16, [255, 0, 0, 255]);
        peindre(&mut img, 16, (0, 0, 8, 8), [0, 0, 0, 255]);
        let m = mesurer(16, 16, &img).expect("mesure");
        assert!(m.est_masque_de_zones());
        assert!((m.part(Zone::Rouge) - 0.75).abs() < 1e-6);
        assert!((m.part(Zone::Noir) - 0.25).abs() < 1e-6);
        assert_eq!(Role::deriver(&m), Role::Zones);
        // L'emprise du noir est le quart haut-gauche, bornes incluses.
        assert_eq!(m.emprise(Zone::Noir), Some([0.0, 0.0, 0.5, 0.5]));
    }

    #[test]
    fn couleur_dessinee_plus_masque_de_zones_donne_fond_rouge() {
        // La planche porte le trait (noir opaque), le masque cerne sur fond rouge.
        let mut planche = unie(16, 16, [255, 255, 255, 255]);
        peindre(&mut planche, 16, (0, 0, 8, 8), [10, 10, 10, 255]);
        let couleur = mesurer(16, 16, &planche).expect("mesure");
        assert!(couleur.porte_un_trait());

        let mut msk = unie(16, 16, [255, 0, 0, 255]);
        peindre(&mut msk, 16, (0, 0, 8, 8), [0, 0, 0, 255]);
        let masque = mesurer(16, 16, &msk).expect("mesure");

        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::FondRouge
        );
    }

    #[test]
    fn couleur_muette_plus_zone_verte_donne_trace_vert() {
        // La planche ne porte rien : grise et transparente, comme `eye_L_01`.
        let couleur = mesurer(16, 16, &unie(16, 16, [211, 211, 211, 1])).expect("mesure");
        assert!(!couleur.porte_un_trait());

        let mut msk = unie(16, 16, [255, 0, 0, 255]);
        peindre(&mut msk, 16, (0, 0, 16, 4), [0, 200, 0, 255]);
        peindre(&mut msk, 16, (0, 8, 16, 12), [0, 0, 0, 255]);
        let masque = mesurer(16, 16, &msk).expect("mesure");
        assert!(masque.part(Zone::Vert) > PART_ZONE_UTILE);

        // Le cas réel d'`eye_L_01` : une planche grise qui varie à peine, dont le masque décide.
        let mut plancher = unie(16, 16, [211, 211, 211, 1]);
        peindre(&mut plancher, 16, (0, 0, 2, 2), [210, 211, 211, 1]);
        let couleur = mesurer(16, 16, &plancher).expect("mesure");
        assert!(!couleur.est_aplat());
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::TraceVert
        );
    }

    #[test]
    fn un_masque_bleu_sans_vert_est_celui_d_une_pupille() {
        // `pupil_L_01msk` : 64 % de rouge, 36 % de bleu, aucun vert.
        let mut plancher = unie(16, 16, [255, 255, 255, 255]);
        peindre(&mut plancher, 16, (0, 0, 2, 2), [254, 255, 255, 255]);
        let couleur = mesurer(16, 16, &plancher).expect("mesure");

        let mut msk = unie(16, 16, [255, 0, 0, 255]);
        peindre(&mut msk, 16, (4, 4, 12, 12), [0, 0, 255, 255]);
        let masque = mesurer(16, 16, &msk).expect("mesure");
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::ZoneBleue
        );

        // Le vert prime : un masque qui porte les deux est un masque d'œil.
        peindre(&mut msk, 16, (0, 0, 16, 2), [0, 200, 0, 255]);
        let masque = mesurer(16, 16, &msk).expect("mesure");
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::TraceVert
        );
    }

    #[test]
    fn masque_gris_est_une_decoupe() {
        let mut plancher = unie(16, 16, [211, 211, 211, 255]);
        peindre(&mut plancher, 16, (0, 0, 2, 2), [210, 211, 211, 255]);
        let couleur = mesurer(16, 16, &plancher).expect("mesure");

        // Un dégradé de gris : le canal rouge varie, aucun fond rouge franc.
        let mut msk = unie(16, 16, [0, 0, 0, 255]);
        for y in 0..16u32 {
            peindre(&mut msk, 16, (0, y, 16, y + 1), [(y * 16) as u8; 4]);
        }
        let masque = mesurer(16, 16, &msk).expect("mesure");
        assert!(!masque.est_masque_de_zones());
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::Decoupe
        );
    }

    #[test]
    fn masque_uniforme_equivaut_a_pas_de_masque() {
        let mut plancher = unie(16, 16, [211, 211, 211, 255]);
        peindre(&mut plancher, 16, (0, 0, 2, 2), [210, 211, 211, 255]);
        let couleur = mesurer(16, 16, &plancher).expect("mesure");
        let masque = mesurer(16, 16, &unie(16, 16, [128, 64, 64, 255])).expect("mesure");
        assert!(masque.canal_uniforme());
        assert_eq!(
            Convention::deriver(&couleur, Some(&masque)),
            Convention::SansMasque
        );
    }

    #[test]
    fn une_planche_tronquee_ne_se_mesure_pas() {
        // Un dénominateur faux fausserait toutes les parts : mieux vaut rien rendre.
        assert!(mesurer(16, 16, &[0u8; 64]).is_none());
        assert!(mesurer(0, 16, &[0u8; 64]).is_none());
    }

    #[test]
    fn les_zones_presentes_sont_triees_par_etendue() {
        let mut img = unie(16, 16, [255, 0, 0, 255]);
        peindre(&mut img, 16, (0, 0, 16, 4), [0, 0, 0, 255]);
        peindre(&mut img, 16, (0, 4, 16, 6), [0, 200, 0, 255]);
        let m = mesurer(16, 16, &img).expect("mesure");
        let zones: Vec<Zone> = m.zones_presentes().into_iter().map(|(z, _)| z).collect();
        assert_eq!(zones, alloc::vec![Zone::Rouge, Zone::Noir, Zone::Vert]);
    }
}
