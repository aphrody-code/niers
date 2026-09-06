//! Forge et décodage du paramètre `?q=` du site Inagle.
//!
//! Algorithme confirmé par recon :
//! ```text
//! encode_q(json_str) :
//!   bytes   = json_str.as_bytes()
//!   inverted = bytes.map(|b| !b & 0xFF)   // complément à 1 octet par octet
//!   b64url  = base64url_nopad(inverted)
//!   q       = percent_encode(b64url)
//! ```
//!
//! # Exemples
//!
//! ```
//! use nie_zukan::forge::{encode_q, decode_q};
//!
//! // Ancre Endou (c01000010) — valeur vérifiée en live contre zukan.inazuma.jp
//! let q = encode_q(r#"{"character_id":["c01000010"]}"#).unwrap();
//! assert_eq!(q, "hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC");
//!
//! // Round-trip
//! let json = decode_q(&q).unwrap();
//! assert_eq!(json, r#"{"character_id":["c01000010"]}"#);
//! ```

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// Jeu de caractères à percent-encoder : tout sauf alphanumérique + `-_~.`
/// (les caractères base64url `_` et `-` ne doivent PAS être encodés).
/// Le site utilise ces valeurs brutes dans l'URL sans encoder `_` ni `-`.
const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b':')
    .add(b'@')
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b';')
    .add(b'=')
    .add(b'%');

/// Encode un JSON en paramètre `?q=` Inagle.
///
/// L'entrée doit être un JSON UTF-8 valide (ex. `{"character_id":["c01000010"]}`).
/// La sortie est prête à être collée dans une URL (`?q=<sortie>`).
pub fn encode_q(json: &str) -> Result<String> {
    let bytes = json.as_bytes();
    let inverted: Vec<u8> = bytes.iter().map(|&b| !b).collect();
    let b64 = URL_SAFE_NO_PAD.encode(&inverted);
    // Percent-encoder en préservant les caractères base64url (`A-Za-z0-9_-`).
    // Le site envoie `_` et `-` non-encodés dans le paramètre `q=`.
    // Seuls les caractères comme `=` (padding base64 standard, absent en NO_PAD) seraient encodés.
    let encoded = utf8_percent_encode(&b64, QUERY_ENCODE_SET).to_string();
    Ok(encoded)
}

/// Décode un paramètre `?q=` Inagle en JSON UTF-8.
///
/// Accepte la forme URL-encodée ou déjà décodée.
pub fn decode_q(q: &str) -> Result<String> {
    // D'abord percent-decoder
    let decoded_url = percent_encoding::percent_decode_str(q)
        .decode_utf8()
        .context("percent-decode UTF-8")?;
    // Padding base64url : longueur doit être multiple de 4
    let padded = match decoded_url.len() % 4 {
        2 => format!("{decoded_url}=="),
        3 => format!("{decoded_url}="),
        _ => decoded_url.into_owned(),
    };
    let inverted = URL_SAFE_NO_PAD
        .decode(padded.trim_end_matches('='))
        .context("base64url decode")?;
    let original: Vec<u8> = inverted.iter().map(|&b| !b).collect();
    let json = String::from_utf8(original).context("UTF-8 decode")?;
    Ok(json)
}

/// Forge un `?q=` pour `chara_param` (filtre par `character_id`).
///
/// Utilise la clé `filter_chara_id_str` qui est la forme utilisée dans `chara_list`
/// pour les liens vers `chara_param`.
pub fn q_for_chara_param(character_id: &str) -> Result<String> {
    let json = format!(r#"{{"filter_chara_id_str":["{character_id}"]}}"#);
    encode_q(&json)
}

/// Forge un `?q=` pour `chara_model_view` (`character_id` direct).
pub fn q_for_model_view(character_id: &str) -> Result<String> {
    let json = format!(r#"{{"character_id":["{character_id}"]}}"#);
    encode_q(&json)
}

/// Forge un `?q=` pour les skills par catégorie.
///
/// Catégories connues : 1=Shoot, 2=Offense, 3=Defense, 4=Keeper
pub fn q_for_skill_category(category: u32) -> Result<String> {
    let json = format!(r#"{{"category_filter":[{category}]}}"#);
    encode_q(&json)
}

/// Forge un `?q=` pour les items par catégorie.
///
/// Catégories connues : 30=Shoes, 40=Misanga, 50=Pendant, 60=Special
pub fn q_for_item_category(category: u32) -> Result<String> {
    let json = format!(r#"{{"category_filter":[{category}]}}"#);
    encode_q(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ancre Endou : valeur vérifiée en live contre zukan.inazuma.jp
    #[test]
    fn encode_endou_anchor() {
        let q = encode_q(r#"{"character_id":["c01000010"]}"#).unwrap();
        assert_eq!(q, "hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC");
    }

    #[test]
    fn roundtrip_endou() {
        let original = r#"{"character_id":["c01000010"]}"#;
        let q = encode_q(original).unwrap();
        let decoded = decode_q(&q).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn roundtrip_filter_chara() {
        let original = r#"{"filter_chara_id_str":["c01000010"]}"#;
        let q = encode_q(original).unwrap();
        let decoded = decode_q(&q).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn roundtrip_category_filter() {
        let original = r#"{"category_filter":[30]}"#;
        let q = encode_q(original).unwrap();
        let decoded = decode_q(&q).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_url_encoded_q() {
        // q avec %3D (padding = sign url-encoded), extrait du HTML live
        let q = "hN2ZlpOLmo2gnJeejZ6glpugjIuN3cWk3ZzPzs_Pz8_Oz92igg%3D%3D";
        let decoded = decode_q(q).unwrap();
        assert_eq!(decoded, r#"{"filter_chara_id_str":["c01000010"]}"#);
    }

    #[test]
    fn q_for_chara_param_endou() {
        let q = q_for_chara_param("c01000010").unwrap();
        let decoded = decode_q(&q).unwrap();
        assert_eq!(decoded, r#"{"filter_chara_id_str":["c01000010"]}"#);
    }

    #[test]
    fn q_for_model_view_endou() {
        let q = q_for_model_view("c01000010").unwrap();
        assert_eq!(q, "hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC");
    }

    #[test]
    fn roundtrip_various_ids() {
        let ids = [
            "c01000010",
            "c01000020",
            "c01000100",
            "c02024750",
            "c05020110",
        ];
        for id in ids {
            let json = format!(r#"{{"character_id":["{id}"]}}"#);
            let q = encode_q(&json).unwrap();
            let decoded = decode_q(&q).unwrap();
            assert_eq!(decoded, json, "round-trip failed for id={id}");
        }
    }
}
