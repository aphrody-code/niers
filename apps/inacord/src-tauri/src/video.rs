//! Lecture native des cinématiques du jeu — catalogue, métadonnées, flux vidéo.
//!
//! ## Ce qui a changé, et pourquoi
//!
//! L'aperçu vidéo passait par `ffmpeg` en sous-processus, puis renvoyait le MP4 **en base64**
//! avec un plafond de 40 Mo. Trois murs :
//!
//! 1. `ffmpeg` n'est pas installé ici — l'aperçu échouait sur `échec de lancement de ffmpeg` ;
//! 2. 40 Mo, quand une cinématique de chapitre pèse jusqu'à 300 Mo : la moitié du corpus était
//!    hors de portée par construction ;
//! 3. le base64 gonfle de 33 % et interdit le `seek` — un `<video src="data:…">` doit tout
//!    charger avant de jouer la première image.
//!
//! Ici, [`nie_formats::usm`] démultiplexe, puis [`nie_formats::mp4`] (H.264) ou
//! [`nie_formats::webm`] (VP9) remuxe, en pur Rust et sans réencodage. Le résultat est servi par
//! le protocole `nievideo://` avec **support des requêtes `Range`** : le `<video>` de la webview
//! ne charge que l'intervalle dont il a besoin, ce qui rend le déplacement dans la timeline
//! instantané quelle que soit la taille du film.
//!
//! ## Ce que le protocole expose
//!
//! | URL | Contenu |
//! |-----|---------|
//! | `nievideo://localhost/<chemin VFS>` | la piste vidéo : MP4 si H.264, WebM si VP9 |
//! | `nievideo://localhost/<chemin VFS>?track=audio` | la bande-son décodée, en WAV |
//!
//! La bande-son est un flux **séparé** parce qu'elle est en HCA Criware : aucun conteneur MP4
//! ne la transporte, et l'encoder en AAC demanderait un encodeur C et dégraderait une piste
//! qu'on vient de décoder sans perte. Le lecteur les resynchronise (cf. `VideoPlayer.tsx`).

use std::collections::HashMap;
use std::sync::Mutex;

use nie_formats::usm::{self, langue_de, nom_fichier_de, radical_de, rubrique_de};
use nie_formats::vfs::Vfs;
use serde::{Deserialize, Serialize};

/// Budget mémoire du cache vidéo, en octets. Deux cinématiques de chapitre y tiennent.
const BUDGET_CACHE: usize = 768 * 1024 * 1024;

/// Nombre maximal de films gardés simultanément.
const ENTREES_CACHE: usize = 4;

/// Cache mémoire des flux produits (MP4 ou WAV), par clé d'URL.
///
/// Deux usages le rendent indispensable :
///
/// * le lecteur émet une requête `Range` par saut dans la timeline — sans cache, chaque saut
///   redémultiplexerait le conteneur entier ;
/// * la page Cinéma prévisualise au survol, et l'aller-retour entre deux cartes voisines
///   rejouerait le même travail à chaque passage.
///
/// D'où un petit LRU plutôt qu'une entrée unique : borné par [`ENTREES_CACHE`] **et** par
/// [`BUDGET_CACHE`], parce qu'un film de chapitre pèse à lui seul 300 Mo.
#[derive(Default)]
pub struct CacheVideo(pub Mutex<Vec<(String, &'static str, Vec<u8>)>>);

impl CacheVideo {
    /// Rend **la tranche demandée** d'un flux déjà produit, avec son type MIME et la taille
    /// totale, et remet l'entrée en tête (usage le plus récent).
    ///
    /// La tranche, et pas tout le flux : un `<video>` émet une requête `Range` par saut et par
    /// remplissage de tampon. Cloner les 300 Mo à chaque fois faisait monter la mémoire de
    /// travail de l'explorateur à 15 Go — l'allocateur de Windows garde ces blocs. Ici on ne
    /// copie que ce qui part sur le fil.
    pub fn tranche(&self, cle: &str, plage: Option<(u64, u64)>) -> Option<(&'static str, Vec<u8>, u64)> {
        let mut g = self.0.lock().ok()?;
        let i = g.iter().position(|(k, _, _)| k == cle)?;
        let entree = g.remove(i);
        let total = entree.2.len() as u64;
        let morceau = decouper(&entree.2, plage);
        let mime = entree.1;
        g.push(entree);
        Some((mime, morceau, total))
    }

    /// Range des octets produits, en évinçant les plus anciens si les bornes sont dépassées.
    ///
    /// Prend la propriété du tampon : le ranger ne doit pas coûter une copie de 300 Mo.
    pub fn ranger(&self, cle: String, mime: &'static str, octets: Vec<u8>) {
        let Ok(mut g) = self.0.lock() else { return };
        g.retain(|(k, _, _)| *k != cle);
        g.push((cle, mime, octets));
        while g.len() > ENTREES_CACHE
            || (g.len() > 1 && g.iter().map(|(_, _, v)| v.len()).sum::<usize>() > BUDGET_CACHE)
        {
            g.remove(0);
        }
    }
}

/// Extrait `plage` d'un tampon, bornes comprises. `None` rend le tampon entier.
pub fn decouper(octets: &[u8], plage: Option<(u64, u64)>) -> Vec<u8> {
    match plage {
        None => octets.to_vec(),
        Some((debut, fin)) => {
            if octets.is_empty() {
                return Vec::new();
            }
            let debut = (debut as usize).min(octets.len() - 1);
            let fin = (fin as usize).min(octets.len() - 1).max(debut);
            octets[debut..=fin].to_vec()
        }
    }
}

/// Piste sonore d'un film.
///
/// Elle vient de deux endroits, et c'est le fait marquant du corpus : **2 films sur 97 seulement**
/// portent leur son dans leur propre conteneur (les deux logos). Pour tous les autres, il vit
/// dans la banque `anime_stream`, à côté — cf. [`nie_explore::bande_son`].
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PisteAudioDto {
    /// Numéro de canal (toujours `0` pour une piste externe).
    pub canal: u8,
    /// Codec détecté (`hca`, `adx`).
    pub codec: String,
    /// Fréquence d'échantillonnage en Hz.
    pub frequence: u32,
    /// Nombre de canaux.
    pub canaux: u32,
    /// Taille du flux brut, en octets — `0` pour une piste externe (connue seulement à l'ouverture
    /// de la banque, qui pèse 654 Mo).
    pub octets: u32,
    /// D'où vient la piste : `conteneur` (dans le `.usm`) ou le nom de la cue de `anime_stream`.
    pub source: String,
}

/// Une entrée du catalogue. Les champs issus du démultiplexage sont `None` tant que
/// [`video_info`] n'a pas été appelé sur ce film — le catalogue s'ouvre instantanément et se
/// complète à mesure que les cartes deviennent visibles.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct FilmDto {
    /// Chemin VFS complet.
    pub chemin: String,
    /// Radical du nom de fichier (`ev01_00050`).
    pub nom: String,
    /// Rubrique d'affichage — une rubrique = une rangée.
    pub rubrique: String,
    /// Code de langue (`fr`, `JP`…) quand le nom en porte un.
    pub langue: Option<String>,
    /// Taille du conteneur USM, en octets.
    pub octets: u32,
    /// Codec vidéo (`h264`, `mpeg2`), une fois le film inspecté.
    pub codec: Option<String>,
    /// Le navigateur sait-il décoder ce codec ?
    pub lisible: Option<bool>,
    /// Largeur en pixels.
    pub largeur: Option<u32>,
    /// Hauteur en pixels.
    pub hauteur: Option<u32>,
    /// Nombre d'images démultiplexées.
    pub images: Option<u32>,
    /// Cadence en images par seconde.
    pub cadence: Option<f64>,
    /// Durée en secondes.
    pub duree: Option<f64>,
    /// Pistes sonores.
    pub audio: Vec<PisteAudioDto>,
    /// Le conteneur était-il enveloppé par le XOR CRI ?
    pub chiffre: Option<bool>,
    /// Nom du fichier source chez l'encodeur, tel qu'inscrit dans le conteneur.
    pub nom_origine: Option<String>,
    /// Musique de fond déclarée par le `gamedata` (hash).
    pub bgm: Option<String>,
    /// Chemin du `.cfg.bin` de texte des sous-titres, quand il y en a un.
    pub sous_titres: Option<String>,
}

/// Le catalogue complet renvoyé au frontend.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogueVideoDto {
    /// Films, triés par chemin.
    pub films: Vec<FilmDto>,
    /// Rubriques distinctes, dans l'ordre d'affichage.
    pub rubriques: Vec<String>,
}

/// Complète une fiche avec ce que l'inspection révèle.
fn completer(f: &mut FilmDto, u: &usm::Apercu) {
    f.codec = Some(u.codec.nom().to_string());
    f.lisible = Some(u.codec.lisible_par_navigateur());
    f.largeur = Some(u.entete.largeur_affichee.max(u.entete.largeur));
    f.hauteur = Some(u.entete.hauteur_affichee.max(u.entete.hauteur));
    f.images = Some(u.images);
    f.cadence = u.entete.images_par_seconde();
    f.duree = u.duree();
    f.chiffre = Some(u.dechiffre);
    f.nom_origine = u.nom.clone();
    f.audio = u
        .pistes
        .iter()
        .map(|p| PisteAudioDto {
            canal: p.canal,
            codec: p.codec.nom().to_string(),
            frequence: p.frequence,
            canaux: p.canaux,
            octets: p.taille as u32,
            source: "conteneur".to_string(),
        })
        .collect();
}

/// Ajoute la bande-son EXTERNE d'un film à sa fiche, quand son conteneur n'en porte pas.
///
/// Ne lit que la cue sheet (35 Kio), jamais l'archive de 654 Mo : le catalogue doit rester
/// instantané. Le film garde `audio` vide s'il n'a de son nulle part — l'interface le dit alors,
/// au lieu de monter un `<audio>` qui échouerait.
fn completer_bande_son(f: &mut FilmDto, vfs: &Vfs) {
    if !f.audio.is_empty() {
        return;
    }
    // La durée du film sert de garde-fou : sans elle, une bobine partagée passerait pour la
    // bande-son du film et jouerait le son de quelqu'un d'autre, ou du silence.
    let Some(p) = nie_explore::bande_son::piste_de_film(vfs, &f.nom, f.duree, None) else { return };
    f.audio.push(PisteAudioDto {
        canal: 0,
        codec: p.codec,
        frequence: p.frequence,
        canaux: p.canaux,
        octets: 0,
        source: p.cue,
    });
}

/// Fiche « rapide » : ce que l'index du VFS suffit à dire, sans lire un octet du conteneur.
fn fiche_rapide(chemin: &str, octets: u32) -> FilmDto {
    let rad = radical_de(chemin);
    FilmDto {
        chemin: chemin.to_string(),
        nom: rad.to_string(),
        rubrique: rubrique_de(rad),
        langue: langue_de(rad).map(str::to_string),
        octets,
        codec: None,
        lisible: None,
        largeur: None,
        hauteur: None,
        images: None,
        cadence: None,
        duree: None,
        audio: Vec::new(),
        chiffre: None,
        nom_origine: None,
        bgm: None,
        sous_titres: None,
    }
}

/// Ce que le `gamedata` dit de chaque film, indexé par `moviePath` (`common/movie/x.usm`).
fn jointure(vfs: &Vfs) -> HashMap<String, (Option<String>, Option<String>)> {
    use serde_json::Value;
    let mut out = HashMap::new();
    let chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains("gamedata/movie/movie_playing_config")
                || p.contains("gamedata/event/event_movie_config")
        })
        .collect();

    for chemin in chemins {
        let Ok(octets) = vfs.read(&chemin) else { continue };
        let Some(root) = nie_formats::cfgbin::rdbn_to_iecode_json(&octets) else { continue };
        let Some(listes) = root.get("lists").and_then(Value::as_array) else { continue };
        for liste in listes {
            let Some(lignes) = liste.get("values").and_then(Value::as_array) else { continue };
            for ligne in lignes {
                let Some(mp) = ligne.get("moviePath").and_then(Value::as_str) else { continue };
                if !mp.ends_with(".usm") {
                    continue;
                }
                let bgm = ligne
                    .get("bgmName")
                    .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string));
                let st = ligne
                    .get("subtitleTextPath")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty() && *s != "0xFFFFFFFF")
                    .map(str::to_string);
                out.entry(mp.to_string()).or_insert((bgm, st));
            }
        }
    }
    out
}

/// Construit le catalogue **sans** démultiplexer : instantané, complété ensuite par
/// [`info_film`] au fil de l'affichage.
///
/// # Erreurs
///
/// Aucune : un VFS vide rend un catalogue vide.
pub fn catalogue(vfs: &Vfs) -> CatalogueVideoDto {
    let mut entrees: Vec<(String, u32)> = vfs
        .iter()
        .filter(|(p, _)| p.starts_with("data/common/movie") && p.ends_with(".usm"))
        .map(|(p, e)| (p.to_string(), e.file_size))
        .collect();
    entrees.sort_by(|a, b| a.0.cmp(&b.0));

    let liens = jointure(vfs);
    let films: Vec<FilmDto> = entrees
        .into_iter()
        .map(|(chemin, octets)| {
            let mut f = fiche_rapide(&chemin, octets);
            let cle = chemin.strip_prefix("data/").unwrap_or(&chemin);
            if let Some((bgm, st)) = liens.get(cle) {
                f.bgm = bgm.clone();
                f.sous_titres = st.clone();
            }
            f
        })
        .collect();

    // Les chapitres d'abord dans leur ordre naturel, puis les rubriques nommées.
    let mut rubriques: Vec<String> = films.iter().map(|f| f.rubrique.clone()).collect();
    rubriques.sort();
    rubriques.dedup();
    CatalogueVideoDto { films, rubriques }
}

/// Démultiplexe un film et rend sa fiche complète.
///
/// # Erreurs
///
/// Chemin absent du VFS, ou conteneur qui ne se démultiplexe pas (même déchiffré).
pub fn info_film(vfs: &Vfs, chemin: &str) -> Result<FilmDto, String> {
    let brut = vfs.read(chemin).map_err(|e| e.to_string())?;
    // `inspecter` et NON `demuxer_nomme` : la fiche n'a besoin d'aucune image. Retenir le film
    // entier coûtait jusqu'à 312 Mo et 38 081 allocations par appel — mesuré, l'explorateur
    // montait à 15 Go de mémoire de travail en enrichissant son catalogue.
    let u = usm::inspecter(&brut, nom_fichier_de(chemin)).map_err(|e| e.to_string())?;
    let mut f = fiche_rapide(chemin, brut.len() as u32);
    completer(&mut f, &u);
    completer_bande_son(&mut f, vfs);
    Ok(f)
}

/// Emballe la piste vidéo d'un `.usm` dans son conteneur web, **sans réencodage ni processus
/// externe**. Rend `(type MIME, octets)` : H.264 → MP4, VP9 → WebM.
///
/// # Erreurs
///
/// Conteneur illisible, ou codec que le navigateur ne décode pas (MPEG-2) : le message le dit
/// explicitement plutôt que de produire un fichier que rien n'ouvrira.
pub fn flux_web_depuis_usm(octets: &[u8], nom: &str) -> Result<(&'static str, Vec<u8>), String> {
    let u = usm::demuxer_nomme(octets, nom).map_err(|e| e.to_string())?;
    if u.images.is_empty() {
        return Err("aucun flux vidéo dans ce fichier".to_string());
    }
    if !u.codec.lisible_par_navigateur() {
        return Err(format!(
            "codec {} : aucun navigateur ne le décode — utilisez Extraire pour obtenir le flux \
             élémentaire .{}",
            u.codec.nom(),
            u.codec.extension()
        ));
    }
    u.en_conteneur_web().map(|c| (c.mime, c.octets)).map_err(|e| e.to_string())
}

/// Même chose, quand seul le contenu importe (aperçu base64 borné).
///
/// # Erreurs
///
/// Voir [`flux_web_depuis_usm`].
pub fn mp4_depuis_usm(octets: &[u8], nom: &str) -> Result<Vec<u8>, String> {
    flux_web_depuis_usm(octets, nom).map(|(_, o)| o)
}

/// Décode la bande-son d'un film en WAV, d'où qu'elle vienne.
///
/// D'abord la piste du conteneur (les deux logos), sinon la cue de `anime_stream` qui porte le
/// nom du film. C'est ce second chemin qui donne du son aux cinématiques : **95 des 97 `.usm`
/// sont muets**, leur bande-son vit dans une banque Criware à côté.
///
/// # Erreurs
///
/// Conteneur illisible, film sans son ni dans le conteneur ni dans la banque, ou décodage HCA
/// refusé. Le message dit LEQUEL des deux chemins a échoué.
pub fn wav_bande_son(
    vfs: &Vfs,
    cache_dir: &std::path::Path,
    chemin: &str,
    octets: &[u8],
) -> Result<Vec<u8>, String> {
    let u = usm::demuxer_nomme(octets, nom_fichier_de(chemin)).map_err(|e| e.to_string())?;
    if let Some(piste) = u.pistes.first() {
        return nie_formats::cri_audio::decode_to_wav(&piste.octets);
    }
    let radical = radical_de(chemin);
    let externe = nie_explore::bande_son::piste_de_film(vfs, radical, u.duree(), None).ok_or_else(|| {
        format!("« {radical} » n'a de bande-son ni dans son conteneur ni dans anime_stream")
    })?;
    nie_explore::bande_son::wav_de_la_cue(vfs, cache_dir, externe.awb_id)
}

// Pas de module de tests ici : `cargo test` dans `src-tauri` ne DÉMARRE pas sur cette machine
// (`STATUS_ENTRYPOINT_NOT_FOUND` avant le premier test, cf. CLAUDE.md « Pièges d'environnement »),
// donc un test écrit ici ne serait jamais exécuté — un faux vert. Les conventions de nommage que
// ce module consomme sont testées à leur source, dans `nie_formats::usm` (`cargo test -p
// nie-formats --lib usm::`), là où elles tournent vraiment.
