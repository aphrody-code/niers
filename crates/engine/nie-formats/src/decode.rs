//! Décodage générique « octets → JSON » : **la** table de dispatch du dépôt, partagée par la FFI
//! (`nie-ffi`) et la CLI (`niers decode`). Une famille ajoutée ici profite aux deux.
//!
//! Ordre de dispatch (le premier qui parse gagne) :
//! 1. `\x1bLua` (bytecode Lua 5.2) et `lip\0` — testés avant [`crate::detect`], qui ne les
//!    distingue pas ;
//! 2. `G4TX`, `G4MD`, `G4PK`/`G4PKM`, `RDBN`, `CPK ` — d'après le magic ;
//! 3. inconnu — les conteneurs Level-5 annexes au prédicat (`dxbc`, `g4mt`, `g4cm`, `g4la`,
//!    `g4ma`, `g4vs`, `col`), puis les conteneurs T2B dans l'ordre `objbin` → `cfgbin` →
//!    `mevbin`, qui partagent le même en-tête et ne se distinguent qu'au contenu.

extern crate alloc;

use alloc::vec::Vec;

use crate::FileFormat;

/// Résultat d'un décodage réussi : le JSON, et **le parseur qui l'a produit**.
///
/// Le nom vient du parseur, pas de la détection : les conteneurs T2B (`objbin`, `cfgbin` T2B,
/// `mevbin`) partagent le même en-tête et tombent tous dans `FileFormat::Unknown` — dire
/// « inconnu » d'un fichier qu'on vient de décoder n'apprend rien à l'appelant.
#[derive(Debug, Clone)]
pub struct Decoded {
    /// JSON UTF-8.
    pub json: Vec<u8>,
    /// Nom court du parseur ayant réussi (`"g4tx"`, `"cfg.bin"`, `"objbin"`…).
    pub format: &'static str,
}

/// Décode un tampon vers du JSON UTF-8, format auto-détecté.
///
/// Retourne `None` si le format n'est pas supporté ou si le parse échoue — jamais d'erreur
/// détaillée : les appelants (FFI, CLI, décodage en lot) veulent tous « ça a marché ou non ».
#[must_use]
pub fn decode(data: &[u8]) -> Option<Decoded> {
    fn done(json: Option<Vec<u8>>, format: &'static str) -> Option<Decoded> {
        json.map(|json| Decoded { json, format })
    }

    // Bytecode Lua 5.2 (`\x1bLua`) : le format des 1 197 `.lua.bin` du jeu — déclencheurs de
    // chapitre (`gamedata/phase/`), de quête (`gamedata/quest/`) et scripts de scène. Testé au
    // magic, avant `detect` qui ne le connaît pas. Le décodeur reste celui de `nie-lua`
    // (en-tête, prototypes imbriqués, constantes, instructions, locales, upvalues) : une seule
    // implémentation, atteignable depuis la FFI depuis que `nie-formats` en dépend.
    #[cfg(feature = "lua")]
    if data.len() >= 4 && data[..4] == *b"\x1bLua" {
        return done(
            nie_lua::bytecode::parse(data)
                .ok()
                .and_then(|c| serde_json::to_vec(&c).ok()),
            "lua-bytecode",
        );
    }

    // `lip\0` n'a pas de variante dans `FileFormat` : le tester au magic.
    if data.len() >= 4 && data[..4] == *b"lip\0" {
        return done(
            crate::lip::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "lip",
        );
    }

    match crate::detect(data) {
        FileFormat::G4tx => done(
            crate::g4tx::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4tx",
        ),
        FileFormat::G4md => done(
            crate::g4md::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4md",
        ),
        // Le layout 2D menu (G4SKM encapsulé) d'abord : `g4pk::parse` accepterait le fichier
        // mais rendrait une vue moins riche.
        FileFormat::G4pk => done(
            crate::g4pkm::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4pkm",
        )
        .or_else(|| {
            done(
                crate::g4pk::parse(data)
                    .ok()
                    .and_then(|v| serde_json::to_vec(&v).ok()),
                "g4pk",
            )
        }),
        FileFormat::CfgBin => done(
            crate::cfgbin::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "cfg.bin",
        ),
        FileFormat::Cpk => done(
            crate::cpk::parse_cpk(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "cpk",
        ),
        FileFormat::G4sk => done(
            crate::g4sk::parse_header(data)
                .ok()
                .map(|h| {
                    let os = crate::g4sk::parse_hierarchy(data, &h);
                    (h, os)
                })
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4sk",
        ),
        FileFormat::G4nv => done(
            crate::navm::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "navm",
        ),
        FileFormat::Awb => done(
            crate::cri_audio::Awb::parse(data).ok().and_then(|awb| {
                // Un AWB est un conteneur : exporter sa table de matières est utile même
                // lorsque ses entrées HCA sont chiffrées et nécessitent la sous-clé par banque.
                serde_json::to_vec(&serde_json::json!({
                    "subkey": awb.subkey,
                    "entries": awb.entries.iter().map(|entry| serde_json::json!({
                        "cue_id": entry.cue_id,
                        "offset": entry.offset,
                        "size": entry.size,
                    })).collect::<Vec<_>>(),
                }))
                .ok()
            }),
            "awb",
        ),
        // Un `G4MG` ne se décode PAS seul : sa géométrie n'a de sens qu'avec le `G4MD` frère,
        // qui porte la description des sous-maillages et des attributs de sommet. Le reconnaître
        // sans prétendre le décoder est la seule réponse honnête — le compter « non reconnu »
        // ferait croire à un format non porté.
        FileFormat::Unknown => decode_level5_annexe(data)
            .or_else(|| {
                done(
                    crate::objbin::parse(data)
                        .ok()
                        .and_then(|v| serde_json::to_vec(&v).ok()),
                    "objbin",
                )
            })
            .or_else(|| {
                done(
                    crate::cfgbin::parse_t2b(data)
                        .ok()
                        .and_then(|v| serde_json::to_vec(&v).ok()),
                    "cfg.bin (T2B)",
                )
            })
            .or_else(|| {
                done(
                    crate::mevbin::parse(data)
                        .ok()
                        .and_then(|v| serde_json::to_vec(&v).ok()),
                    "mevbin",
                )
            }),
        _ => None,
    }
}

/// Conteneurs Level-5 annexes que [`crate::detect`] ne distingue pas — il ne connaît que les
/// magics des grosses familles, ceux-ci tombent tous dans `Unknown`.
///
/// Chaque module expose son prédicat `is_*` : on le consulte avant de parser plutôt que
/// d'enchaîner des `parse()` qui échouent, pour que le coût reste celui d'une comparaison de
/// quatre octets. Essayés AVANT les conteneurs T2B : ceux-là n'ont pas de magic propre et
/// acceptent large, donc les laisser passer en premier volerait des fichiers à leur vrai
/// parseur.
fn decode_level5_annexe(data: &[u8]) -> Option<Decoded> {
    fn done(json: Option<Vec<u8>>, format: &'static str) -> Option<Decoded> {
        json.map(|json| Decoded { json, format })
    }
    // Les shaders du jeu (`.vfxo`/`.pfxo`/`.gfxo`/`.cfxo` — vertex/pixel/geometry/compute) sont
    // des conteneurs **DXBC**, pas un format Level-5 : leurs extensions les faisaient passer pour
    // des effets, et 2 497 fichiers comptaient comme « non reconnus » alors que le module `dxbc`
    // les parse depuis toujours.
    if crate::dxbc::is_dxbc(data) {
        return done(
            crate::dxbc::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "dxbc",
        );
    }
    if crate::g4mt::is_g4mt(data) {
        return done(
            crate::g4mt::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4mt",
        );
    }
    if crate::g4cm::is_g4cm(data) {
        return done(
            crate::g4cm::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4cm",
        );
    }
    if crate::g4la::is_g4la(data) {
        return done(
            crate::g4la::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4la",
        );
    }
    if crate::g4ma::is_g4ma(data) {
        return done(
            crate::g4ma::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4ma",
        );
    }
    if crate::g4vs::is_g4vs(data) {
        return done(
            crate::g4vs::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "g4vs",
        );
    }
    if crate::col::is_pxcl(data) {
        return done(
            crate::col::parse(data)
                .ok()
                .and_then(|v| serde_json::to_vec(&v).ok()),
            "col (PXCL)",
        );
    }
    None
}

/// Variante « JSON seul » de [`decode`], pour les appelants qui ignorent le format (FFI).
#[must_use]
pub fn to_json(data: &[u8]) -> Option<Vec<u8>> {
    decode(data).map(|d| d.json)
}

/// Nom court du format **détecté** (avant tout parse) : `"g4tx"`, `"cfg.bin"`, `"inconnu"`…
///
/// Pour nommer ce qui a réellement été décodé, utiliser [`Decoded::format`] : la détection
/// seule ne distingue pas les conteneurs T2B.
#[must_use]
pub fn format_name(data: &[u8]) -> &'static str {
    if data.len() >= 4 && data[..4] == *b"\x1bLua" {
        return "lua-bytecode";
    }
    if data.len() >= 4 && data[..4] == *b"lip\0" {
        return "lip";
    }
    match crate::detect(data) {
        FileFormat::G4tx => "g4tx",
        FileFormat::G4md => "g4md",
        FileFormat::G4mg => "g4mg",
        FileFormat::G4sk => "g4sk",
        FileFormat::G4pk => "g4pk",
        FileFormat::CfgBin => "cfg.bin",
        FileFormat::Cpk => "cpk",
        FileFormat::Awb => "awb",
        FileFormat::Unknown => "inconnu",
        _ => "autre",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tampon_vide_ou_bruit_ne_decode_pas() {
        assert!(to_json(&[]).is_none());
        assert!(to_json(&[0xFF; 8]).is_none());
    }

    #[test]
    fn le_magic_lip_est_teste_avant_la_detection() {
        // `detect` ne connaît pas `lip\0` : sans le test préalable, ce tampon partirait dans
        // la branche `Unknown` et tenterait les parseurs T2B.
        assert_eq!(format_name(b"lip\0suite"), "lip");
    }

    #[test]
    fn un_nom_de_format_est_toujours_rendu() {
        assert_eq!(format_name(&[]), "inconnu");
        assert!(!format_name(&[0x00; 32]).is_empty());
    }

    #[test]
    fn to_json_et_decode_disent_la_meme_chose() {
        // `to_json` ne doit rester qu'une vue de `decode`, sinon les deux chemins divergent.
        let bruit = [0xAB; 32];
        assert_eq!(to_json(&bruit).is_some(), decode(&bruit).is_some());
    }
}
