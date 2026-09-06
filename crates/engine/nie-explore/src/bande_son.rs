//! Bande-son des cinématiques — **elle n'est pas dans le film**.
//!
//! ## Ce que la mesure a montré
//!
//! Sur les 97 `.usm` du jeu, **2 seulement** portent une piste sonore dans leur conteneur : les
//! deux logos d'ouverture. Les 95 autres sont muets — leur son vit dans une banque Criware
//! séparée, `data/common/sound_asset/anime_stream.acb` / `.awb`, que le moteur joue en parallèle
//! du film.
//!
//! ## Comment le lien se fait
//!
//! `movie_playing_config` donne pour chaque film un `bgmName`, présenté comme un hash. Vérifié
//! sur le réel :
//!
//! ```text
//! ev01_00050 → bgmName = 0xD0750D09 = CRC32("ev01_00050")
//! ```
//!
//! Ce n'est donc pas « le nom d'une musique » : c'est le **nom du film lui-même**, haché, qui
//! désigne une cue de `anime_stream`. La banque en contient 187, nommées d'après les films :
//! `ev01_00050`, plus les pistes séparées `ev01_00050_bgm`, `_se` (bruitages) et `_vc` (voix)
//! quand l'encodeur les a laissées à part.
//!
//! Le lien se fait donc par le nom, et le hash sert à le confirmer : les deux voies doivent
//! tomber sur le même film, sinon on ne joue rien plutôt que de jouer autre chose.
//!
//! ## La cue au nom nu n'est pas toujours la bonne
//!
//! On attendrait qu'elle soit le mixage complet. Mesuré : sur 73 cues résolues, **42 pointent une
//! forme d'onde bien plus longue que le film** — des bobines partagées. Profil sonore de l'entrée
//! visée par `ev01_00150` (film de 143 s) : silence de 0 à 170 s, audio de 170 à 305 s, silence,
//! puis un autre passage. La servir donnerait **143 secondes de silence**, ce qui ressemble
//! exactement à un bug de lecture.
//!
//! D'où le garde-fou de [`piste_de_film`] : une forme d'onde qui dépasse le film d'un facteur est
//! refusée, et le stem `_bgm` — qui, lui, commence bien à zéro (vérifié : 93,6 s de musique
//! pleine, RMS 7 000) — prend le relais. Aucune information de position n'existe dans la chaîne
//! `Cue → Synth → Waveform` : sans elle, on ne peut pas découper une bobine, seulement la
//! reconnaître et l'écarter.
//!
//! ## Pourquoi ce module lit l'AWB par morceaux
//!
//! `anime_stream.awb` pèse **654 Mo** et porte les 97 bandes-son ; une seule est demandée à la
//! fois. Le charger entier pour en extraire trois mégaoctets serait absurde, d'autant que la
//! table des matières AFS2 tient dans les premiers kilo-octets. [`wav_de_la_cue`] matérialise le
//! fichier une fois (le VFS l'extrait du CPK vers son cache), lit son en-tête, puis ne lit QUE
//! l'intervalle de l'entrée voulue.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use nie_formats::cri_audio;
use nie_formats::vfs::Vfs;

/// Cue sheet des bandes-son de cinématiques.
pub const BANQUE_ANIME: &str = "data/common/sound_asset/anime_stream.acb";

/// Archive des formes d'onde correspondantes.
pub const AWB_ANIME: &str = "data/common/sound_asset/anime_stream.awb";

/// Taille d'en-tête lue pour parser la table des matières AFS2.
///
/// Elle vaut `0x10 + n × 4 + (n + 1) × taille_offset` : 1 Mio couvre plus de 100 000 entrées, là
/// où la banque des cinématiques en compte 187.
const ENTETE_AWB: usize = 1024 * 1024;

/// Une bande-son de film, décrite sans avoir ouvert l'AWB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PisteFilm {
    /// Nom de la cue dans `anime_stream` — celui du film.
    pub cue: String,
    /// Identifiant AFS2 de la forme d'onde.
    pub awb_id: u16,
    /// Codec (`hca`, `adx`), tel que la banque le déclare.
    pub codec: String,
    /// Fréquence d'échantillonnage en Hz, `0` si la banque ne la donne pas.
    pub frequence: u32,
    /// Nombre de canaux, `0` si absent.
    pub canaux: u32,
    /// Durée du CUE en millisecondes — ce que le jeu joue.
    pub duree_ms: u32,
    /// Durée de la FORME D'ONDE en millisecondes — ce que le fichier contient.
    ///
    /// Les deux diffèrent souvent, et l'écart est ce qui permet de repérer une bobine partagée
    /// (cf. [`piste_de_film`]).
    pub duree_onde_ms: u32,
    /// Vrai si le `bgmName` du `gamedata` confirme la cue trouvée par son nom.
    pub confirme_par_hash: bool,
}

/// CRC32 standard (poly `0xEDB88320`, init/xorout `0xFFFFFFFF`) — le hash des `bgmName`.
///
/// C'est le même que celui des `cfg.bin` Level-5 ; il vit déjà dans `nie_formats::cfgbin`, on ne
/// le réécrit pas.
#[must_use]
pub fn hash_de_cue(nom: &str) -> u32 {
    nie_formats::cfgbin::crc32(nom.as_bytes())
}

/// Marge acceptée entre la durée d'une forme d'onde et celle du film.
///
/// Une piste propre dépasse le film de quelques secondes (queue de fondu) : `ev01_00050_bgm` fait
/// 96,6 s pour un film de 93,6 s. Une **bobine partagée** le dépasse d'un facteur : l'entrée AWB
/// n° 14 fait 402 s pour un film de 143 s, et son profil sonore le confirme — silence de 0 à
/// 170 s, audio de 170 à 305 s. La servir donnerait 143 secondes de silence. Le facteur 1,5 (plus
/// 5 s de garde pour les films très courts) sépare nettement les deux cas sur tout le corpus.
const FACTEUR_ONDE_MAX: f64 = 1.5;

/// Garde additive, en secondes, pour ne pas rejeter un logo de 4 s dont la piste en fait 8.
const MARGE_ONDE_S: f64 = 5.0;

/// Trouve la bande-son d'un film, **sans ouvrir l'AWB** — seule la cue sheet (35 Kio) est lue.
///
/// `radical` est le nom du film sans extension (`ev01_00050`). `duree_film_s` sert de garde-fou
/// (cf. [`FACTEUR_ONDE_MAX`]) : sans elle, la fonction accepte la première cue trouvée, ce qui
/// peut être une bobine partagée. `bgm_name` est le `bgmName` du `gamedata` quand on l'a : il ne
/// sert pas à chercher, mais à confirmer.
///
/// Rend `None` si la banque est absente, ne nomme aucune cue correspondante, ou n'en propose que
/// des bobines — un film sans bande-son identifiable doit le rester, pas se voir attribuer une
/// piste qui ne lui va pas.
#[must_use]
pub fn piste_de_film(
    vfs: &Vfs,
    radical: &str,
    duree_film_s: Option<f64>,
    bgm_name: Option<u32>,
) -> Option<PisteFilm> {
    let acb = vfs.read(BANQUE_ANIME).ok()?;
    let cues = cri_audio::acb_cues(&acb).ok()?;

    // Le nom NU d'abord — quand il porte une forme d'onde, c'est le mixage complet — puis les
    // stems.
    //
    // **Il faut exiger la forme d'onde, pas seulement le nom.** Sur `ev01_00050`, la cue au nom
    // nu EXISTE mais ne résout aucune forme d'onde, tandis que `ev01_00050_bgm` en porte une :
    // s'arrêter au premier nom trouvé rendait « pas de son » sur un film qui en a un. Mesuré sur
    // la banque : 187 cues nommées, 75 seulement portent une forme d'onde.
    let candidats = [
        radical.to_string(),
        format!("{radical}_bgm"),
        format!("{radical}_vc"),
        format!("{radical}_se"),
    ];
    let plafond_ms = duree_film_s
        .map(|d| ((d * FACTEUR_ONDE_MAX + MARGE_ONDE_S) * 1000.0) as u64)
        .unwrap_or(u64::MAX);

    let mut choisi = None;
    for nom in &candidats {
        let Some(c) = cues.iter().find(|c| c.name == *nom && c.awb_id.is_some()) else {
            continue;
        };
        let onde_ms = duree_onde_ms(c);
        if u64::from(onde_ms) > plafond_ms {
            continue;
        }
        choisi = Some((c, onde_ms));
        break;
    }
    let (cue, duree_onde_ms) = choisi?;
    let awb_id = cue.awb_id?;

    // Le hash du `gamedata` désigne la cue au nom NU. Quand on retombe sur un stem, il ne
    // confirme pas CE nom-là mais bien le film : c'est exactement ce qu'on veut vérifier —
    // que la banque et la table de lecture parlent du même film.
    let confirme_par_hash = bgm_name.is_some_and(|h| h == hash_de_cue(radical));
    Some(PisteFilm {
        cue: cue.name.clone(),
        awb_id,
        codec: match cue.encode_type {
            Some(2) => "hca".to_string(),
            Some(0 | 3) => "adx".to_string(),
            _ => "inconnu".to_string(),
        },
        frequence: cue.sample_rate.unwrap_or(0),
        canaux: u32::from(cue.channels.unwrap_or(0)),
        duree_ms: cue.length_ms,
        duree_onde_ms,
        confirme_par_hash,
    })
}

/// Durée de la forme d'onde d'une cue, en millisecondes, d'après son nombre d'échantillons.
///
/// `0` quand la banque ne les donne pas — auquel cas aucune garde n'est possible, et l'appelant
/// accepte la cue telle quelle.
fn duree_onde_ms(c: &cri_audio::AcbCue) -> u32 {
    match (c.num_samples, c.sample_rate) {
        (Some(n), Some(sr)) if sr > 0 => (u64::from(n) * 1000 / u64::from(sr)) as u32,
        _ => 0,
    }
}

/// Décode une cue de `anime_stream` en WAV PCM 16 bits, en ne lisant de l'AWB que son entrée.
///
/// `cache_dir` accueille la matérialisation de l'AWB (le VFS l'extrait du CPK une seule fois) ;
/// les appels suivants ne rouvrent que ce fichier.
///
/// # Erreurs
///
/// Rend un message si l'AWB est introuvable, si sa table des matières ne porte pas ce cue-id, ou
/// si le décodage HCA échoue.
pub fn wav_de_la_cue(vfs: &Vfs, cache_dir: &Path, awb_id: u16) -> Result<Vec<u8>, String> {
    let chemin = vfs
        .materialiser(AWB_ANIME, cache_dir)
        .map_err(|e| format!("{AWB_ANIME} indisponible : {e}"))?;
    let mut f = std::fs::File::open(&chemin).map_err(|e| format!("ouverture AWB : {e}"))?;
    let taille = f.metadata().map_err(|e| format!("taille AWB : {e}"))?.len();

    let mut entete = vec![0u8; ENTETE_AWB.min(taille as usize)];
    f.read_exact(&mut entete)
        .map_err(|e| format!("lecture de l'en-tête AWB : {e}"))?;
    let banque = cri_audio::Awb::parse_entete(&entete, taille)
        .map_err(|e| format!("table des matières AWB illisible : {e}"))?;

    let rang = banque.index_of_id(awb_id).ok_or_else(|| {
        format!(
            "cue-id {awb_id} absent de la banque ({} entrées)",
            banque.entries.len()
        )
    })?;
    let entree = &banque.entries[rang];
    if entree.size == 0 {
        return Err(format!("entrée {awb_id} vide dans la banque"));
    }

    f.seek(SeekFrom::Start(u64::from(entree.offset)))
        .map_err(|e| format!("positionnement dans l'AWB : {e}"))?;
    let mut octets = vec![0u8; entree.size as usize];
    f.read_exact(&mut octets)
        .map_err(|e| format!("lecture de l'entrée AWB : {e}"))?;

    // Les entrées des AWB IEVR sont précédées de quelques octets nuls avant le magic HCA/ADX —
    // même saut que `Awb::entry_bytes`, qui travaille lui sur le fichier entier.
    let debut = octets.iter().position(|&b| b != 0).unwrap_or(0);
    let charge = &octets[debut..];

    if cri_audio::is_hca(charge) {
        let (pcm, canaux, frequence) = cri_audio::hca_decode_to_pcm16(charge, banque.subkey)?;
        return Ok(cri_audio::encode_pcm16_wav(&pcm, canaux, frequence));
    }
    if cri_audio::is_adx(charge) {
        let pcm = cri_audio::adx_decode(charge).map_err(|e| format!("ADX : {e}"))?;
        return Ok(cri_audio::encode_pcm16_wav(
            &pcm.samples,
            pcm.channels,
            pcm.sample_rate,
        ));
    }
    Err("l'entrée n'est ni du HCA ni de l'ADX".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Combien de cinématiques gagnent réellement une bande-son — mesuré, pas supposé.
    ///
    /// Le chiffre compte : il dit à l'interface ce qu'elle peut promettre. Ce test l'imprime et
    /// exige seulement qu'il reste franchement majoritaire ; le figer à l'unité près casserait
    /// au premier correctif de jeu.
    #[test]
    fn couverture_reelle_des_bandes_son() {
        let racine = nie_formats::vfs::resolve_game_dir();
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(racine.join("data")).is_err() {
            eprintln!("SAUTÉ : VFS réel indisponible (corpus du jeu absent)");
            return;
        }
        let films: Vec<(String, String)> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.starts_with("data/common/movie") && p.ends_with(".usm"))
            .map(|p| {
                let r = nie_formats::usm::radical_de(&p).to_string();
                (p, r)
            })
            .collect();
        if films.is_empty() {
            eprintln!("SAUTÉ : aucune cinématique dans le VFS monté");
            return;
        }

        let acb = vfs.read(BANQUE_ANIME).expect("banque des cinématiques");
        let cues = cri_audio::acb_cues(&acb).expect("cues");

        let (mut sonores, mut ecartees, mut nommes_sans_onde, mut absents) = (0, 0, 0, 0);
        for (chemin, radical) in &films {
            // La durée du film est le garde-fou : sans elle on ne saurait pas distinguer une
            // piste d'une bobine. Elle coûte la lecture du conteneur, d'où un test lent.
            let duree = vfs
                .read(chemin)
                .ok()
                .and_then(|o| {
                    nie_formats::usm::inspecter(&o, nie_formats::usm::nom_fichier_de(chemin)).ok()
                })
                .and_then(|a| a.duree());
            let nomme = cues.iter().any(|c| c.name.starts_with(radical.as_str()));
            if piste_de_film(&vfs, radical, duree, None).is_some() {
                sonores += 1;
            } else if nomme && piste_de_film(&vfs, radical, None, None).is_some() {
                ecartees += 1;
                if ecartees <= 12 {
                    let candidats: Vec<String> = cues
                        .iter()
                        .filter(|c| c.name.starts_with(radical.as_str()) && c.awb_id.is_some())
                        .map(|c| format!("{}={} ms", c.name, duree_onde_ms(c)))
                        .collect();
                    eprintln!(
                        "  écarté : {radical} film={:.0} ms → {}",
                        duree.unwrap_or(0.0) * 1000.0,
                        candidats.join(" ")
                    );
                }
            } else if nomme {
                nommes_sans_onde += 1;
            } else {
                absents += 1;
            }
        }
        eprintln!(
            "bandes-son : {sonores} résolues, {ecartees} écartées (bobine partagée), \
             {nommes_sans_onde} nommées sans forme d'onde (cues de type séquence), \
             {absents} absentes de la banque (écrans-titres et logos), sur {} films",
            films.len()
        );
        // Le seuil vise le corpus qui compte : les cinématiques `ev*`. Les écrans-titres et les
        // logos ne déclarent aucune bande-son (`bgmName = 0x00000000`), et ne peuvent donc pas en
        // avoir. Seuil volontairement bas : il protège contre une résolution CASSÉE, pas contre une
        // évolution du jeu. Le chiffre du jour (30 résolues, 12 bobines écartées) est imprimé
        // ci-dessus ; le figer à l’unité près casserait au premier correctif de contenu.
        assert!(
            sonores >= 25,
            "seulement {sonores} films sonorisés — la résolution est cassée"
        );
    }

    /// Le lien film → cue est un CRC32 du nom du film, pas un identifiant arbitraire.
    ///
    /// Cette égalité est ce qui autorise à chercher par le NOM : sans elle, il faudrait un index
    /// hash → cue, et une erreur d'appariement jouerait le son d'un autre film sans le dire.
    #[test]
    fn le_bgm_name_est_le_crc32_du_nom_du_film() {
        assert_eq!(hash_de_cue("ev01_00050"), 0xD075_0D09);
        assert_eq!(hash_de_cue("ev01_00150"), 0xD1B7_673E);
        assert_eq!(hash_de_cue("ev01_00200"), 0xAE86_2D22);
        // Les pistes séparées ont bien un autre hash — ce ne sont pas elles que `bgmName` désigne.
        assert_ne!(hash_de_cue("ev01_00050_bgm"), 0xD075_0D09);
    }

    /// Sur le VRAI jeu : la cue existe, son hash confirme, et son entrée se décode en WAV sans
    /// charger les 654 Mo de la banque.
    #[test]
    fn bande_son_reelle_d_une_cinematique() {
        let racine = nie_formats::vfs::resolve_game_dir();
        let mut vfs = nie_formats::vfs::Vfs::new();
        if vfs.init(racine.join("data")).is_err() {
            eprintln!("SAUTÉ : VFS réel indisponible (corpus du jeu absent)");
            return;
        }
        if vfs.read(BANQUE_ANIME).is_err() {
            eprintln!("SAUTÉ : {BANQUE_ANIME} absent du VFS monté");
            return;
        }

        let piste = piste_de_film(&vfs, "ev01_00050", Some(93.55), Some(0xD075_0D09))
            .expect("bande-son de ev01_00050");
        // La cue au nom NU existe mais ne porte aucune forme d'onde : c'est le stem `_bgm` qui
        // en a une. La résolution doit donc désigner un nom DÉRIVÉ du film, pas exactement le
        // sien — et surtout pas celui d'un autre film.
        assert!(
            piste.cue.starts_with("ev01_00050"),
            "cue inattendue : {}",
            piste.cue
        );
        assert!(
            piste.confirme_par_hash,
            "le bgmName du gamedata doit confirmer le film"
        );
        assert_eq!(piste.codec, "hca");
        assert!(
            piste.frequence >= 8000,
            "fréquence douteuse : {}",
            piste.frequence
        );

        // Un film qui n'existe pas ne doit se voir attribuer AUCUNE piste.
        assert!(piste_de_film(&vfs, "film_qui_n_existe_pas", None, None).is_none());

        let cache = std::env::temp_dir().join("niers-bande-son-test");
        let wav = wav_de_la_cue(&vfs, &cache, piste.awb_id).expect("décodage WAV");
        assert_eq!(&wav[..4], b"RIFF", "en-tête WAV attendu");
        assert_eq!(&wav[8..12], b"WAVE");
        // Une bande-son de 93 s en 16 bits stéréo pèse des mégaoctets : un WAV d'en-tête seul
        // (44 octets) passerait un test de magic mais pas celui-ci.
        assert!(
            wav.len() > 1_000_000,
            "WAV trop court : {} octets",
            wav.len()
        );
    }
}
