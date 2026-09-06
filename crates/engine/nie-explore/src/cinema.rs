//! Catalogue des cinématiques — **une seule construction, trois façades**.
//!
//! Les 97 films d'*Inazuma Eleven: Victory Road* sont des conteneurs USM/Sofdec2. Trois surfaces
//! du dépôt en publiaient chacune sa propre fiche : `niers video` (CLI), `nie-model-serve`
//! (route `/video/catalog.json` que consomme la page `/videos` d'azalée) et l'explorateur Tauri.
//! Les trois lisaient les mêmes octets et n'en disaient pas la même chose : le serveur ignorait
//! la bande-son externe que la CLI joignait déjà, et rapportait `octets: 0` pour les pistes.
//! Ce module est la fiche, une fois pour toutes ; les façades ne font plus que la sérialiser.
//!
//! ## Deux profondeurs, et pourquoi le choix compte
//!
//! * [`apercu`] passe par [`usm::inspecter`] : les blocs sont parcourus, **aucune image n'est
//!   retenue**. C'est le seul profil tenable pour un catalogue de 97 films — un [`usm::Usm`]
//!   complet garde jusqu'à 312 Mo pour un seul d'entre eux.
//! * [`complet`] démultiplexe et **remuxe** pour mesurer ce que le conteneur web coûte
//!   (`conteneur_octets`, `cles`, `gain_remux`) et pour lire les dimensions dans le bitstream.
//!   Réservé à la fiche d'un film.
//!
//! La distinction n'est pas cosmétique : `nie-model-serve` vit sous un watchdog qui le redémarre
//! au-delà de 90 % de son `MemoryHigh`. Construire le catalogue par `demuxer_nomme` faisait
//! monter son RSS à 8,3 Gio et **le faisait tuer en pleine requête** — la page `/videos` recevait
//! une connexion fermée, jamais un catalogue.
//!
//! ## Ce que la fiche porte, et d'où ça vient
//!
//! | Champ | Source |
//! |---|---|
//! | `rubrique`, `langue`, `nom` | conventions de nommage du jeu ([`usm::rubrique_de`], [`usm::langue_de`]) |
//! | `codec`, `largeur`, `images`, `cadence` | en-tête `VIDEO_HDRINFO` du conteneur |
//! | `audio` | pistes portées par le conteneur — **2 films sur 97** |
//! | `bande_son` | `anime_stream.acb`, résolue par [`crate::bande_son`] — 30 films |
//! | `gamedata` | `movie_playing_config` / `event_movie_config` |

use std::collections::BTreeMap;
use std::path::Path;

use nie_formats::usm::{self, Apercu, CodecVideo, Usm};
use nie_formats::vfs::Vfs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bande_son;

/// Dossier VFS de référence des films.
///
/// `data/dx11/movie` porte les mêmes 97 noms dans une variante à plus haut débit ; c'est
/// `common` qui sert de catalogue parce que lui seul est complet, la variante étant résolue à la
/// lecture (cf. `variante_jumelle` côté serveur).
pub const DOSSIER_FILMS: &str = "data/common/movie";

/// Une piste sonore portée par le conteneur du film lui-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PisteInterne {
    /// Numéro de canal déclaré par le conteneur.
    pub canal: u8,
    /// Codec de la piste (`hca`, `adx`…).
    pub codec: String,
    /// Fréquence d'échantillonnage, en hertz.
    pub frequence: u32,
    /// Nombre de canaux.
    pub canaux: u32,
    /// Taille de la piste, en octets.
    ///
    /// Lue dans le conteneur, donc renseignée même en aperçu — contrairement aux octets
    /// eux-mêmes, que l'aperçu ne retient pas.
    pub octets: u64,
}

/// La bande-son d'un film qui n'en porte pas dans son conteneur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BandeSon {
    /// Nom de la cue dans `anime_stream`.
    pub cue: String,
    /// Identifiant AFS2 de la forme d'onde, dans `anime_stream.awb`.
    pub awb_id: u16,
    /// Codec déclaré par la banque.
    pub codec: String,
    /// Fréquence d'échantillonnage, en hertz.
    pub frequence: u32,
    /// Nombre de canaux.
    pub canaux: u32,
    /// Durée de la cue, en millisecondes — ce que le jeu joue.
    pub duree_ms: u32,
    /// Durée de la forme d'onde, en millisecondes — ce que le fichier contient.
    pub duree_onde_ms: u32,
    /// Vrai quand le `bgmName` du `gamedata` confirme la cue trouvée par son nom.
    pub confirme_par_hash: bool,
}

impl From<bande_son::PisteFilm> for BandeSon {
    fn from(p: bande_son::PisteFilm) -> Self {
        Self {
            cue: p.cue,
            awb_id: p.awb_id,
            codec: p.codec,
            frequence: p.frequence,
            canaux: p.canaux,
            duree_ms: p.duree_ms,
            duree_onde_ms: p.duree_onde_ms,
            confirme_par_hash: p.confirme_par_hash,
        }
    }
}

/// Ce que les tables de jeu disent d'un film.
///
/// Tous les champs sont facultatifs : `movie_playing_config` et `event_movie_config` ne
/// décrivent pas les mêmes colonnes, et un film absent des deux garde une fiche vide plutôt
/// qu'une fiche inventée.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gamedata {
    /// Fichier de jeu d'où vient la ligne (`movie_playing_config_1.02.28.cfg.bin`).
    pub source: String,
    /// Identifiant du film, tel que le jeu le hache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_id: Option<String>,
    /// Événement d'histoire qui déclenche le film.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Menu depuis lequel le film est joué.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_id: Option<String>,
    /// Identifiant de la légende associée.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption_id: Option<String>,
    /// Nom de la musique — en réalité le CRC32 du nom du film (cf. [`crate::bande_son`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bgm_name: Option<String>,
    /// Durée du fondu d'entrée, en secondes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fede_in_time: Option<f64>,
    /// Durée du fondu de sortie, en secondes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fede_out_time: Option<f64>,
    /// Générique joué par-dessus le film, quand il y en a un.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staffroll_data_name: Option<String>,
    /// Chemin des textes de sous-titres, `<LG>` restant à substituer par la langue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_text_path: Option<String>,
    /// Chemin des réglages de sous-titres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_setting_path: Option<String>,
}

impl Gamedata {
    /// Le `bgmName` interprété comme le hash qu'il est, quand il en porte un.
    ///
    /// `0x00000000` et `0xFFFFFFFF` sont les deux valeurs « pas de valeur » du jeu : les rendre
    /// ferait chercher une cue qui n'existe pas.
    #[must_use]
    pub fn bgm_hash(&self) -> Option<u32> {
        let brut = self.bgm_name.as_deref()?;
        let hex = brut
            .strip_prefix("0x")
            .or_else(|| brut.strip_prefix("0X"))?;
        let v = u32::from_str_radix(hex, 16).ok()?;
        (v != 0 && v != u32::MAX).then_some(v)
    }
}

/// La fiche d'un film.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Film {
    /// Chemin VFS complet du film.
    pub chemin: String,
    /// Radical du fichier (`ev01_00050`) — la clé de tout : jointure, cue, libellé.
    pub nom: String,
    /// Rubrique déduite du nom, convention du jeu.
    pub rubrique: String,
    /// Code de langue quand le nom en porte un, `null` sinon.
    pub langue: Option<String>,
    /// Taille du conteneur `.usm`, en octets.
    pub octets: u64,

    /// Message d'erreur si le film n'a pas pu être lu — les autres champs restent alors vides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erreur: Option<String>,

    /// Codec vidéo constaté (`h264`, `mpeg2`, `vp9`).
    pub codec: String,
    /// Vrai si un navigateur sait décoder ce codec.
    ///
    /// Faux pour les 20 MPEG-2 du corpus : leur proposer une balise `<video>` est un lecteur qui
    /// reste noir, pas une lecture.
    pub lisible_navigateur: bool,
    /// Largeur en pixels.
    pub largeur: u32,
    /// Hauteur en pixels.
    pub hauteur: u32,
    /// Nombre d'images réellement présentes dans le conteneur.
    pub images: u32,
    /// Nombre d'images que l'en-tête annonce.
    pub total_images_declare: u32,
    /// Cadence en images par seconde, `null` si l'en-tête ne la déclare pas.
    pub cadence: Option<f64>,
    /// Durée en secondes, déduite des images réelles et de la cadence.
    pub duree: Option<f64>,
    /// Total des octets vidéo, hors en-têtes de bloc et bourrage.
    pub octets_video: u64,
    /// Vrai si le conteneur était chiffré par l'enveloppe CRI.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dechiffre: bool,
    /// Nom du fichier tel que l'encodeur l'a inscrit, quand le conteneur le déclare.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom_origine: Option<String>,

    /// Pistes sonores portées par le conteneur — vide pour 95 films sur 97.
    pub audio: Vec<PisteInterne>,
    /// Bande-son externe résolue dans `anime_stream`, quand le conteneur est muet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bande_son: Option<BandeSon>,
    /// Nombre de blocs de sous-titres du conteneur, quand il y en a.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sous_titres: Option<u32>,

    /// Type MIME du conteneur web produit par le remux — seulement en fiche [`complet`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conteneur: Option<String>,
    /// Taille du conteneur web produit, en octets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conteneur_octets: Option<u64>,
    /// Nombre d'images-clés — ce sur quoi un lecteur peut se repositionner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cles: Option<u32>,
    /// Part du fichier économisée par le remux, en pourcentage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_remux: Option<f64>,
    /// Raison pour laquelle aucun conteneur web n'est possible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remux_impossible: Option<String>,

    /// Ce que les tables de jeu disent du film.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamedata: Option<Gamedata>,
}

impl Film {
    /// Fiche minimale d'un film illisible : ce qu'on sait de lui sans l'ouvrir.
    fn en_erreur(chemin: &str, octets: u64, erreur: String) -> Self {
        let nom = usm::radical_de(chemin).to_string();
        Self {
            rubrique: usm::rubrique_de(&nom),
            langue: usm::langue_de(&nom).map(str::to_string),
            chemin: chemin.to_string(),
            nom,
            octets,
            erreur: Some(erreur),
            codec: CodecVideo::Inconnu.nom().to_string(),
            lisible_navigateur: false,
            largeur: 0,
            hauteur: 0,
            images: 0,
            total_images_declare: 0,
            cadence: None,
            duree: None,
            octets_video: 0,
            dechiffre: false,
            nom_origine: None,
            audio: Vec::new(),
            bande_son: None,
            sous_titres: None,
            conteneur: None,
            conteneur_octets: None,
            cles: None,
            gain_remux: None,
            remux_impossible: None,
            gamedata: None,
        }
    }

    /// Vrai si le film a une bande-son, d'où qu'elle vienne.
    #[must_use]
    pub fn a_du_son(&self) -> bool {
        !self.audio.is_empty() || self.bande_son.is_some()
    }
}

/// Une langue de la table du jeu, code et nom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Langue {
    /// Code tel qu'il apparaît dans les noms de fichiers (`JP`, `fr`…).
    pub code: String,
    /// Nom en français.
    pub nom: String,
}

/// Le catalogue complet, tel que le consomment la page `/videos` et la page Cinéma.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalogue {
    /// Empreinte du corpus au moment de la construction — cf. [`empreinte`].
    ///
    /// C'est elle qui permet à un serveur de savoir que son catalogue en cache décrit encore le
    /// jeu installé : sans elle, une mise à jour laissait publier l'ancien inventaire
    /// indéfiniment.
    pub empreinte: String,
    /// Les films, triés par chemin.
    pub films: Vec<Film>,
    /// Les rubriques présentes, triées — de quoi bâtir un filtre sans le deviner.
    pub rubriques: Vec<String>,
    /// Les neuf langues du jeu.
    pub langues: Vec<Langue>,
}

/// Ce que le `gamedata` dit des films, indexé par chemin logique (`common/movie/x.usm`).
pub type Jointure = BTreeMap<String, Gamedata>;

/// Clé de jointure d'un chemin VFS : `data/common/movie/x.usm` → `common/movie/x.usm`.
#[must_use]
pub fn cle_jointure(chemin: &str) -> &str {
    chemin.strip_prefix("data/").unwrap_or(chemin)
}

/// Construit la jointure depuis `movie_playing_config` et `event_movie_config`.
///
/// Ces deux tables RDBN portent le `moviePath` de chaque cinématique, avec sa musique, ses
/// fondus et le chemin de ses sous-titres. Absentes ou illisibles, la jointure reste vide — le
/// catalogue est alors dégradé, pas faux.
#[must_use]
pub fn jointure_gamedata(vfs: &Vfs) -> Jointure {
    let mut out = Jointure::new();
    let chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains("gamedata/movie/movie_playing_config")
                || p.contains("gamedata/event/event_movie_config")
        })
        .collect();

    for chemin in chemins {
        let Ok(octets) = vfs.read(&chemin) else {
            continue;
        };
        let Some(root) = nie_formats::cfgbin::rdbn_to_iecode_json(&octets) else {
            continue;
        };
        let Some(listes) = root.get("lists").and_then(Value::as_array) else {
            continue;
        };
        let source = usm::nom_fichier_de(&chemin).to_string();
        for liste in listes {
            let Some(lignes) = liste.get("values").and_then(Value::as_array) else {
                continue;
            };
            for ligne in lignes {
                let Some(mp) = ligne.get("moviePath").and_then(Value::as_str) else {
                    continue;
                };
                if !mp.ends_with(".usm") {
                    continue;
                }
                let texte =
                    |champ: &str| ligne.get(champ).and_then(Value::as_str).map(str::to_string);
                out.entry(mp.to_string()).or_insert_with(|| Gamedata {
                    source: source.clone(),
                    movie_id: texte("movieId"),
                    event_id: texte("eventId"),
                    menu_id: texte("menuId"),
                    caption_id: texte("captionId"),
                    bgm_name: texte("bgmName"),
                    fede_in_time: ligne.get("fedeInTime").and_then(Value::as_f64),
                    fede_out_time: ligne.get("fedeOutTime").and_then(Value::as_f64),
                    staffroll_data_name: texte("staffrollDataName"),
                    subtitle_text_path: texte("subtitleTextPath"),
                    subtitle_setting_path: texte("subtitleSettingPath"),
                });
            }
        }
    }
    out
}

/// Les chemins des films d'un dossier, triés.
#[must_use]
pub fn chemins_films(vfs: &Vfs, prefixe: &str) -> Vec<String> {
    let mut chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.starts_with(prefixe) && p.ends_with(".usm"))
        .collect();
    chemins.sort();
    chemins
}

/// Empreinte du corpus : nombre de films et volume total, `"97:3712345678"`.
///
/// Se calcule sur le seul index VFS, sans lire un octet de film — c'est ce qui permet de
/// vérifier la fraîcheur d'un catalogue en cache à chaque requête sans le reconstruire.
#[must_use]
pub fn empreinte(vfs: &Vfs, prefixe: &str) -> String {
    let (n, octets) = vfs
        .iter()
        .filter(|(p, _)| p.starts_with(prefixe) && p.ends_with(".usm"))
        .fold((0u64, 0u64), |(n, o), (_, e)| {
            (n + 1, o + u64::from(e.file_size))
        });
    format!("{n}:{octets}")
}

/// Complète un nom court en chemin VFS. `ev01_00050` → `data/common/movie/ev01_00050.usm`.
///
/// Un chemin déjà complet est rendu tel quel, à l'exception du préfixe `data/` qui est ajouté
/// s'il manque : c'est la forme que le VFS attend, et celle qu'une URL n'écrit pas toujours.
#[must_use]
pub fn resoudre(vfs: &Vfs, entree: &str) -> String {
    if entree.contains('/') {
        return if entree.starts_with("data/") {
            entree.to_string()
        } else {
            format!("data/{entree}")
        };
    }
    let nom = entree.strip_suffix(".usm").unwrap_or(entree);
    for base in [DOSSIER_FILMS, "data/dx11/movie"] {
        let cand = format!("{base}/{nom}.usm");
        if vfs.is_readable(&cand) {
            return cand;
        }
    }
    format!("{DOSSIER_FILMS}/{nom}.usm")
}

/// Fiche d'un film **sans retenir une seule image** — le profil du catalogue.
///
/// La bande-son externe n'est cherchée que si le conteneur est muet, et ne coûte que la lecture
/// de la cue sheet (35 Kio), jamais celle de l'AWB (654 Mo).
#[must_use]
pub fn apercu(vfs: &Vfs, chemin: &str, jointure: Option<&Jointure>) -> Film {
    let brut = match vfs.read(chemin) {
        Ok(b) => b,
        Err(e) => return Film::en_erreur(chemin, 0, format!("lecture VFS : {e}")),
    };
    let taille = brut.len() as u64;
    let nom_fichier = usm::nom_fichier_de(chemin);
    match usm::inspecter(&brut, nom_fichier) {
        Err(e) => Film::en_erreur(chemin, taille, e.to_string()),
        Ok(a) => {
            let mut f = depuis_apercu(chemin, taille, &a);
            greffer(vfs, &mut f, jointure);
            f
        }
    }
}

/// Fiche d'un film **avec la mesure du remux** — le profil de la fiche détaillée.
///
/// Démultiplexe puis remuxe : c'est la seule façon honnête de chiffrer ce que le conteneur USM
/// coûtait, et de lire les dimensions dans le bitstream plutôt que dans l'en-tête.
#[must_use]
pub fn complet(vfs: &Vfs, chemin: &str, jointure: Option<&Jointure>) -> Film {
    let brut = match vfs.read(chemin) {
        Ok(b) => b,
        Err(e) => return Film::en_erreur(chemin, 0, format!("lecture VFS : {e}")),
    };
    let taille = brut.len() as u64;
    let nom_fichier = usm::nom_fichier_de(chemin);
    match usm::demuxer_nomme(&brut, nom_fichier) {
        Err(e) => Film::en_erreur(chemin, taille, e.to_string()),
        Ok(u) => fiche_de_usm(vfs, chemin, taille, &u, jointure),
    }
}

/// Champs communs aux deux profondeurs.
fn base(chemin: &str, taille: u64, entete: &usm::EnteteVideo, codec: CodecVideo) -> Film {
    let nom = usm::radical_de(chemin).to_string();
    Film {
        rubrique: usm::rubrique_de(&nom),
        langue: usm::langue_de(&nom).map(str::to_string),
        chemin: chemin.to_string(),
        nom,
        octets: taille,
        erreur: None,
        codec: codec.nom().to_string(),
        lisible_navigateur: codec.lisible_par_navigateur(),
        // L'en-tête déclare la taille codée et la taille d'affichage ; c'est la seconde qui
        // décrit le film tel qu'il se regarde (1920×1088 codé pour 1920×1080 affiché).
        largeur: entete.largeur_affichee.max(entete.largeur),
        hauteur: entete.hauteur_affichee.max(entete.hauteur),
        images: 0,
        total_images_declare: entete.total_images,
        cadence: entete.images_par_seconde(),
        duree: None,
        octets_video: 0,
        dechiffre: false,
        nom_origine: None,
        audio: Vec::new(),
        bande_son: None,
        sous_titres: None,
        conteneur: None,
        conteneur_octets: None,
        cles: None,
        gain_remux: None,
        remux_impossible: None,
        gamedata: None,
    }
}

/// Piste de conteneur → fiche, en gardant la taille (que l'aperçu conserve) plutôt que les
/// octets (qu'il ne retient pas).
fn pistes(source: &[usm::PisteAudio]) -> Vec<PisteInterne> {
    source
        .iter()
        .map(|p| PisteInterne {
            canal: p.canal,
            codec: p.codec.nom().to_string(),
            frequence: p.frequence,
            canaux: p.canaux,
            octets: p.taille,
        })
        .collect()
}

fn depuis_apercu(chemin: &str, taille: u64, a: &Apercu) -> Film {
    let mut f = base(chemin, taille, &a.entete, a.codec);
    f.images = a.images;
    f.octets_video = a.octets_video;
    f.dechiffre = a.dechiffre;
    f.nom_origine = a.nom.clone();
    f.duree = a.duree();
    f.audio = pistes(&a.pistes);
    f
}

/// Fiche d'un film **déjà démultiplexé** — pour ne pas relire ce qu'on tient en main.
///
/// C'est ce que [`complet`] appelle après son démultiplexage ; `niers video export`, qui a déjà
/// l'`Usm` sous la main, l'appelle directement plutôt que de rouvrir le fichier.
#[must_use]
pub fn fiche_de_usm(
    vfs: &Vfs,
    chemin: &str,
    taille: u64,
    u: &Usm,
    jointure: Option<&Jointure>,
) -> Film {
    let mut f = depuis_usm(chemin, taille, u);
    greffer(vfs, &mut f, jointure);
    f
}

fn depuis_usm(chemin: &str, taille: u64, u: &Usm) -> Film {
    let mut f = base(chemin, taille, &u.entete, u.codec);
    f.images = u32::try_from(u.images.len()).unwrap_or(u32::MAX);
    f.octets_video = u.octets_video;
    f.dechiffre = u.dechiffre;
    f.nom_origine = u.nom.clone();
    f.duree = u.duree();
    f.audio = pistes(&u.pistes);
    if !u.sous_titres.is_empty() {
        f.sous_titres = Some(u32::try_from(u.sous_titres.len()).unwrap_or(u32::MAX));
    }
    match u.en_conteneur_web() {
        Ok(c) => {
            let produit = c.octets.len() as u64;
            f.conteneur = Some(c.mime.to_string());
            f.conteneur_octets = Some(produit);
            f.cles = Some(c.cles);
            // Les dimensions du bitstream priment sur celles de l'en-tête : le SPS dit ce que le
            // décodeur verra vraiment.
            f.largeur = c.largeur;
            f.hauteur = c.hauteur;
            if taille > 0 {
                let gain = 100.0 - (produit as f64 * 100.0 / taille as f64);
                f.gain_remux = Some((gain * 100.0).round() / 100.0);
            }
        }
        Err(e) => f.remux_impossible = Some(e.to_string()),
    }
    f
}

/// Greffe la jointure `gamedata` puis la bande-son externe.
///
/// L'ordre compte : le `bgmName` du `gamedata` sert à **confirmer** la cue trouvée par son nom.
/// Les façades qui appelaient `piste_de_film(.., None)` ne l'exerçaient jamais — la confirmation
/// existait dans le code sans jamais s'appliquer.
fn greffer(vfs: &Vfs, f: &mut Film, jointure: Option<&Jointure>) {
    if let Some(j) = jointure {
        f.gamedata = j.get(cle_jointure(&f.chemin)).cloned();
    }
    if f.audio.is_empty() {
        let bgm = f.gamedata.as_ref().and_then(Gamedata::bgm_hash);
        f.bande_son = bande_son::piste_de_film(vfs, &f.nom, f.duree, bgm).map(BandeSon::from);
    }
}

/// Construit le catalogue complet d'un dossier de films.
///
/// `profond` demande la mesure du remux pour chaque film : sur les 97 du jeu, cela signifie
/// démultiplexer 3,7 Gio. À réserver à une génération hors ligne.
#[must_use]
pub fn catalogue(vfs: &Vfs, prefixe: &str, profond: bool) -> Catalogue {
    let jointure = jointure_gamedata(vfs);
    let films: Vec<Film> = chemins_films(vfs, prefixe)
        .iter()
        .map(|c| {
            if profond {
                complet(vfs, c, Some(&jointure))
            } else {
                apercu(vfs, c, Some(&jointure))
            }
        })
        .collect();

    let mut rubriques: Vec<String> = films.iter().map(|f| f.rubrique.clone()).collect();
    rubriques.sort();
    rubriques.dedup();

    Catalogue {
        empreinte: empreinte(vfs, prefixe),
        films,
        rubriques,
        langues: usm::LANGUES
            .iter()
            .map(|(code, nom)| Langue {
                code: (*code).to_string(),
                nom: (*nom).to_string(),
            })
            .collect(),
    }
}

/// La bande-son d'un film, en WAV, d'où qu'elle vienne.
///
/// Une piste de conteneur est décodée depuis ses octets ; une piste externe est lue dans
/// `anime_stream.awb` par le seul intervalle qui la porte. Rend `Err` quand le film n'a aucune
/// bande-son identifiable — ce qui est le cas de 65 des 97 films, et doit se dire au lieu de se
/// combler par le son d'un autre.
///
/// # Erreurs
///
/// Remonte l'absence de piste, l'échec de lecture de l'archive et l'échec de décodage.
pub fn wav_bande_son(vfs: &Vfs, cache_dir: &Path, film: &Film) -> Result<Vec<u8>, String> {
    if !film.audio.is_empty() {
        let brut = vfs.read(&film.chemin).map_err(|e| e.to_string())?;
        let u = usm::demuxer_nomme(&brut, usm::nom_fichier_de(&film.chemin))
            .map_err(|e| e.to_string())?;
        let piste = u
            .pistes
            .first()
            .ok_or_else(|| "conteneur sans piste".to_string())?;
        return nie_formats::cri_audio::decode_to_wav(&piste.octets).map_err(|e| e.to_string());
    }
    let bs = film
        .bande_son
        .as_ref()
        .ok_or_else(|| format!("{} n'a aucune bande-son identifiable", film.nom))?;
    bande_son::wav_de_la_cue(vfs, cache_dir, bs.awb_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cle_de_jointure_retire_le_prefixe_data() {
        assert_eq!(
            cle_jointure("data/common/movie/ev01_00050.usm"),
            "common/movie/ev01_00050.usm"
        );
        assert_eq!(cle_jointure("common/movie/x.usm"), "common/movie/x.usm");
    }

    #[test]
    fn bgm_hash_ignore_les_valeurs_vides() {
        let vide = Gamedata {
            bgm_name: Some("0x00000000".into()),
            ..Gamedata::default()
        };
        assert_eq!(vide.bgm_hash(), None);
        let absent = Gamedata {
            bgm_name: Some("0xFFFFFFFF".into()),
            ..Gamedata::default()
        };
        assert_eq!(absent.bgm_hash(), None);
        let vrai = Gamedata {
            bgm_name: Some("0xD0750D09".into()),
            ..Gamedata::default()
        };
        assert_eq!(vrai.bgm_hash(), Some(0xD075_0D09));
    }

    #[test]
    fn bgm_name_est_le_crc32_du_nom_du_film() {
        // Le lien mesuré sur le réel : ce « nom de musique » est le nom du film, haché.
        let g = Gamedata {
            bgm_name: Some("0xD0750D09".into()),
            ..Gamedata::default()
        };
        assert_eq!(g.bgm_hash(), Some(bande_son::hash_de_cue("ev01_00050")));
    }

    #[test]
    fn une_fiche_en_erreur_garde_ce_qui_se_lit_dans_le_nom() {
        let f = Film::en_erreur("data/common/movie/ev01_00050.usm", 42, "illisible".into());
        assert_eq!(f.nom, "ev01_00050");
        assert_eq!(f.rubrique, "Chapitre 01");
        assert_eq!(f.octets, 42);
        assert!(!f.a_du_son());
        assert_eq!(f.erreur.as_deref(), Some("illisible"));
    }

    #[test]
    fn le_json_est_en_camel_case_et_omet_le_vide() {
        let f = Film::en_erreur("data/common/movie/L5logo.usm", 1, "x".into());
        let v = serde_json::to_value(&f).expect("sérialisable");
        assert!(
            v.get("lisibleNavigateur").is_some(),
            "les champs passent en camelCase"
        );
        assert!(v.get("totalImagesDeclare").is_some());
        assert!(v.get("bandeSon").is_none(), "un champ vide ne s'écrit pas");
        assert!(
            v.get("dechiffre").is_none(),
            "un booléen faux ne s'écrit pas"
        );
    }
}
