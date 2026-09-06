//! Catalogue et lecture piste-à-piste des banques audio Criware du VFS.
//!
//! Une banque IEVR n'est pas un fichier audio : `waza_stream.acb` décrit 1 512 pistes, et les
//! octets vivent dans un AWB frère qui atteint 1,25 Gio. Décoder « le fichier » n'a donc pas de
//! sens — il faut choisir une piste. Les façades qui ne savaient pas le faire rendaient toutes la
//! même : la plus volumineuse.
//!
//! Ce module tient les deux moitiés de cette opération, au-dessus de
//! [`nie_formats::cri_audio`] :
//!
//! 1. **cataloguer** ([`cues`]) — noms, durées, codec, fréquence, sans jamais ouvrir l'AWB quand
//!    un ACB suffit (0,10 Gio d'ACB contre 7,49 Gio d'AWB à l'échelle du jeu) ;
//! 2. **résoudre puis décoder** ([`decoder_cue`]) — trouver la banque d'octets (embarquée,
//!    autonome ou frère externe), y localiser la piste par son cue-id AFS2, la rendre en WAV.
//!
//! Le rang dans l'AWB et le cue-id AFS2 ne sont PAS interchangeables : ils coïncident sur la
//! plupart des banques et pas sur toutes, et les confondre fait jouer la mauvaise piste sans
//! jamais lever d'erreur. Tout ce qui traverse une façade ici est un cue-id.

use nie_formats::cri_audio;
use nie_formats::vfs::Vfs;

/// Une piste jouable d'une banque, telle qu'une interface doit la présenter.
#[derive(Debug, Clone)]
pub struct Cue {
    /// Nom donné par la banque, ex. `ev74_00840_me`. Vide si la banque ne nomme pas la piste.
    pub name: String,
    /// Cue-id AFS2 de la forme d'onde — l'identifiant à repasser à [`decoder_cue`].
    ///
    /// `None` quand la chaîne de résolution de l'ACB n'aboutit pas à une forme d'onde : la piste
    /// est cataloguée (elle existe) mais n'est pas adressable, et ne doit pas être proposée à la
    /// lecture.
    pub awb_id: Option<u16>,
    /// Durée annoncée par la banque, en millisecondes. `0` si inconnue.
    pub length_ms: u32,
    /// Codec de la forme d'onde, en clair (`HCA`, `ADX`…), vide si non résolu.
    pub codec: String,
    /// Fréquence d'échantillonnage en Hz, `None` si non résolue.
    pub sample_rate: Option<u32>,
    /// Nombre de canaux, `None` si non résolu.
    pub channels: Option<u8>,
    /// Taille des octets de la piste dans la banque, `None` si l'AWB n'a pas été ouvert.
    pub size: Option<u32>,
}

/// Provenance des octets d'une banque — ce que l'interface affiche pour expliquer d'où vient le son.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAwb {
    /// Le fichier interrogé EST l'AWB.
    Autonome,
    /// L'AWB est embarqué dans l'ACB (colonne `AwbFile`).
    Embarquee,
    /// L'AWB est un fichier frère du VFS, dont le chemin est donné.
    Externe(String),
}

/// Nom lisible d'un `EncodeType` de `WaveformTable`.
///
/// Les valeurs viennent des banques réelles du jeu ; une valeur inconnue est rendue telle quelle
/// plutôt que devinée.
#[must_use]
fn nom_codec(encode_type: Option<u8>) -> String {
    match encode_type {
        Some(0 | 3) => "ADX".to_string(),
        Some(2) => "HCA".to_string(),
        Some(n) => format!("type {n}"),
        None => String::new(),
    }
}

/// Chemin de l'AWB frère d'un ACB : même chemin, extension `.awb`.
///
/// L'ACB ne porte que le HASH du nom de son AWB externe (`StreamAwbHash`), jamais le nom. Dans
/// IEVR le frère porte systématiquement le même radical — vérifié sur les banques du jeu, et
/// c'est déjà l'hypothèse du service HTTP.
#[must_use]
pub fn awb_frere(path: &str) -> Option<String> {
    let tronc = path.strip_suffix(".acb")?;
    Some(format!("{tronc}.awb"))
}

/// Où vivent les octets d'une banque, et combien ils pèsent — **sans jamais lire un AWB externe**.
///
/// C'est ce que doit appeler un appelant qui LISTE. [`resoudre_awb`] charge la banque : sur un
/// AWB externe de 1,25 Gio, cataloguer 1 512 pistes coûterait alors un gigaoctet de disque par
/// sélection dans une interface, pour une colonne « taille ». Ici la taille de l'externe vient de
/// l'index du VFS, qui la connaît déjà.
///
/// Rend aussi les octets quand ils sont DÉJÀ en main (banque autonome, AWB embarqué) : les
/// relire serait un second coût pour rien.
///
/// `None` = aucune banque atteignable (ni AFS2 direct, ni embarqué, ni frère servable).
#[must_use]
pub fn localiser_awb(
    vfs: &Vfs,
    path: &str,
    raw: &[u8],
) -> Option<(SourceAwb, Option<Vec<u8>>, u64)> {
    if raw.starts_with(b"AFS2") {
        return Some((SourceAwb::Autonome, None, raw.len() as u64));
    }
    let info = cri_audio::acb_parse(raw).ok()?;
    if !info.embedded_awb.is_empty() {
        let taille = info.embedded_awb.len() as u64;
        return Some((SourceAwb::Embarquee, Some(info.embedded_awb), taille));
    }
    let frere = awb_frere(path)?;
    // `is_readable` en plus de `find` : l'index vient du JEU et déclare des fichiers « loose »
    // qui n'existent pas forcément sur une installation donnée. Annoncer une banque jouable que
    // `read` refusera ensuite est pire que de n'en annoncer aucune.
    let entry = vfs.find(&frere)?;
    if !vfs.is_readable(&frere) {
        return None;
    }
    Some((SourceAwb::Externe(frere), None, u64::from(entry.file_size)))
}

/// Trouve les octets AWB qui portent le son de `path`, et dit d'où ils viennent.
///
/// `raw` = contenu de `path` déjà lu. Charge réellement la banque — c'est la forme utile au
/// DÉCODAGE. Pour cataloguer, préférer [`localiser_awb`], qui ne lit pas un AWB externe.
///
/// Rend `None` quand aucune banque n'est atteignable (ni AFS2 direct, ni AWB embarqué, ni frère
/// lisible dans le VFS).
#[must_use]
pub fn resoudre_awb(vfs: &Vfs, path: &str, raw: &[u8]) -> Option<(Vec<u8>, SourceAwb)> {
    match localiser_awb(vfs, path, raw)? {
        (SourceAwb::Autonome, _, _) => Some((raw.to_vec(), SourceAwb::Autonome)),
        (SourceAwb::Embarquee, Some(bytes), _) => Some((bytes, SourceAwb::Embarquee)),
        // Un embarqué sans octets ne se produit pas (`localiser_awb` les rend toujours), mais le
        // type l'autorise : ne rien inventer plutôt que déballer.
        (SourceAwb::Embarquee, None, _) => None,
        (SourceAwb::Externe(frere), _, _) => {
            let bytes = vfs.read(&frere).ok()?;
            Some((bytes, SourceAwb::Externe(frere)))
        }
    }
}

/// Catalogue les pistes d'une banque.
///
/// Deux entrées possibles, selon ce que `raw` contient :
///
/// * **ACB** — le catalogue vient de la banque elle-même (noms, durées, codec). `awb` sert
///   uniquement à renseigner la taille de chaque piste, et peut être `None` : cataloguer 1 512
///   pistes ne doit pas exiger de charger un gigaoctet.
/// * **AWB** brut — aucun nom n'existe (l'AFS2 ne nomme rien), seules les entrées indexées par
///   cue-id sont rendues, avec leur taille.
///
/// Rend une liste vide plutôt qu'une erreur quand le fichier n'est ni l'un ni l'autre : un
/// appelant qui liste ne doit pas devoir distinguer « pas une banque » de « banque vide ».
#[must_use]
pub fn cues(raw: &[u8], awb: Option<&[u8]>) -> Vec<Cue> {
    // Tailles par cue-id, quand la banque d'octets est disponible.
    let tailles = awb.and_then(|a| cri_audio::Awb::parse(a).ok()).map(|a| {
        a.entries
            .iter()
            .map(|e| (e.cue_id, e.size))
            .collect::<std::collections::HashMap<u32, u32>>()
    });

    if raw.starts_with(b"@UTF")
        && let Ok(liste) = cri_audio::acb_cues(raw)
    {
        return liste
            .into_iter()
            .map(|c| Cue {
                size: c
                    .awb_id
                    .and_then(|id| tailles.as_ref()?.get(&u32::from(id)).copied()),
                name: c.name,
                awb_id: c.awb_id,
                length_ms: c.length_ms,
                codec: nom_codec(c.encode_type),
                sample_rate: c.sample_rate,
                channels: c.channels,
            })
            .collect();
    }

    if raw.starts_with(b"AFS2")
        && let Ok(a) = cri_audio::Awb::parse(raw)
    {
        return a
            .entries
            .iter()
            .map(|e| Cue {
                name: String::new(),
                // Un cue-id AFS2 tient sur 16 bits ; une banque qui déborde n'est pas adressable
                // par la voie nommée, et on ne prétend pas le contraire.
                awb_id: u16::try_from(e.cue_id).ok(),
                length_ms: 0,
                codec: String::new(),
                sample_rate: None,
                channels: None,
                size: Some(e.size),
            })
            .collect();
    }

    Vec::new()
}

/// Décode UNE piste d'une banque en WAV PCM 16 bits, désignée par son cue-id AFS2.
///
/// `awb` = octets rendus par [`resoudre_awb`]. L'erreur distingue « la banque ne contient pas ce
/// cue-id » d'un échec de décodage : la première est une erreur d'appelant, la seconde un
/// problème de contenu.
pub fn decoder_cue(awb: &[u8], awb_id: u16) -> Result<Vec<u8>, String> {
    let banque = cri_audio::Awb::parse(awb).map_err(|e| format!("AWB illisible : {e}"))?;
    let rang = banque.index_of_id(awb_id).ok_or_else(|| {
        format!(
            "cue-id {awb_id} absent de la banque ({} entrées)",
            banque.entries.len()
        )
    })?;
    cri_audio::decode_awb_entry(awb, Some(rang))
}

/// Nom de fichier proposé pour une piste : celui du cue, jamais celui de la banque.
///
/// Cinq pistes tirées de `waza_stream.acb` proposeraient sinon toutes `waza_stream.wav`, chacune
/// recouvrant la précédente. À défaut d'un nom dans la banque, le radical suivi du cue-id donne
/// au moins un nom DISTINCT. Le résultat est restreint à `[A-Za-z0-9_-]` : un nom de cue vient du
/// jeu, mais il finit dans un système de fichiers.
#[must_use]
pub fn nom_de_fichier(path: &str, cue: &Cue) -> String {
    let base = if cue.name.is_empty() {
        let radical = path
            .rsplit('/')
            .next()
            .and_then(|n| n.split('.').next())
            .unwrap_or("audio");
        format!("{radical}_{}", cue.awb_id.unwrap_or(0))
    } else {
        cue.name.clone()
    };
    let sain: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{sain}.wav")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_frere_dun_acb_porte_le_meme_radical() {
        assert_eq!(
            awb_frere("data/sound/waza_stream.acb").as_deref(),
            Some("data/sound/waza_stream.awb")
        );
        // Un AWB n'a pas de frère : c'est déjà la banque.
        assert_eq!(awb_frere("data/sound/waza_stream.awb"), None);
    }

    #[test]
    fn un_cue_sans_nom_reste_distinct() {
        let anonyme = Cue {
            name: String::new(),
            awb_id: Some(1461),
            length_ms: 0,
            codec: String::new(),
            sample_rate: None,
            channels: None,
            size: None,
        };
        assert_eq!(
            nom_de_fichier("data/sound/waza_stream.acb", &anonyme),
            "waza_stream_1461.wav"
        );

        let nomme = Cue {
            name: "ev74_00840_me".to_string(),
            ..anonyme.clone()
        };
        assert_eq!(
            nom_de_fichier("data/sound/waza_stream.acb", &nomme),
            "ev74_00840_me.wav"
        );

        // Un nom exotique ne traverse pas tel quel vers le disque.
        let sale = Cue {
            name: "a/b:c*d".to_string(),
            ..anonyme
        };
        assert_eq!(nom_de_fichier("x.acb", &sale), "a_b_c_d.wav");
    }

    #[test]
    fn un_fichier_qui_nest_pas_une_banque_ne_catalogue_rien() {
        assert!(cues(b"pas une banque", None).is_empty());
    }

    #[test]
    fn les_codecs_connus_sont_nommes_les_autres_rendus_tels_quels() {
        assert_eq!(nom_codec(Some(2)), "HCA");
        assert_eq!(nom_codec(Some(0)), "ADX");
        assert_eq!(nom_codec(Some(9)), "type 9");
        assert_eq!(nom_codec(None), "");
    }
}
