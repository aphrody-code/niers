//! Installation d'un mod par **patch d'octets** du `cpk_list.cfg.bin`, sans réencodage.
//!
//! # Pourquoi
//!
//! [`crate::pack::pack_mod`] passe par `cfgbin::encode_t2b`, dont l'aller-retour n'est pas
//! fidèle : sur le `cpk_list` du jeu, `decode → encode` rend **27 octets de moins** et plus de
//! 5,4 millions d'octets différents, *sans aucune modification*. Le jeu refuse alors le fichier
//! (« Code d'erreur E-02000000 — Échec de la lecture des fichiers de jeu »), et cela vaut même
//! pour un mod dont le contenu est rigoureusement identique au vanilla : le fautif est le
//! réencodage, pas le mod.
//!
//! L'enveloppe AES, elle, est fidèle : `decrypt → encrypt` rend les octets d'origine. On peut
//! donc déchiffrer, **modifier quelques octets en place**, et rechiffrer.
//!
//! # Ce qu'il suffit de changer
//!
//! Dans le T2B, une entrée de fichier porte `[dir, nom, ?, cpk, …]`. Une valeur de type
//! `String` est un **offset i32** dans la table de chaînes, et `-1` s'y lit comme la chaîne
//! vide. Or « `cpk` vide » est exactement ce qui désigne un fichier **hors paquet** (loose),
//! chargé depuis le disque plutôt que depuis un `.cpk`.
//!
//! Rendre un fichier moddable revient donc à écrire `-1` sur 4 octets. La taille du fichier ne
//! bouge pas, aucun offset ne se déplace, la table de chaînes est intacte : le reste du
//! `cpk_list` est préservé **bit pour bit**.

use std::collections::BTreeMap;

/// Valeur d'offset de chaîne signifiant « chaîne vide » dans le T2B.
const CHAINE_VIDE: i32 = -1;
/// Index de la variable portant le `.cpk` conteneur dans une entrée de fichier.
const VAR_CPK: usize = 3;
/// Nombre minimal de variables pour qu'une entrée décrive un fichier.
const VARS_MIN: usize = 5;

/// Ce qu'un patch a changé.
#[derive(Debug, Default, Clone)]
pub struct PatchReport {
    /// Chemins passés en entrée qui ont été rendus *loose*.
    pub rendus_loose: Vec<String>,
    /// Chemins demandés mais absents du `cpk_list`.
    pub introuvables: Vec<String>,
    /// Chemins déjà *loose* avant le patch (rien à faire).
    pub deja_loose: Vec<String>,
    /// Nombre d'octets réellement modifiés dans le clair.
    pub octets_modifies: usize,
}

/// Emplacement binaire d'une entrée de fichier du `cpk_list`.
struct Entree {
    /// Chemin complet `dir + nom`, tel que l'emploie le VFS.
    chemin: String,
    /// Offset absolu, dans le clair, de la variable `cpk` (4 octets).
    offset_cpk: usize,
    /// `true` si l'entrée est déjà *loose*.
    deja_loose: bool,
}

/// Parcourt le T2B **en clair** et relève, pour chaque entrée de fichier, l'offset binaire de sa
/// variable `cpk`.
///
/// Suit exactement la disposition lue par `cfgbin::parse_t2b` : en-tête de 16 octets, puis pour
/// chaque entrée `crc:u32`, `param_count:u8`, les types (2 bits chacun, 4 par octet), un
/// alignement à 4, puis `param_count` valeurs de 4 octets.
fn relever_entrees(clair: &[u8]) -> Result<Vec<Entree>, String> {
    if clair.len() < 16 {
        return Err("cpk_list trop court".to_string());
    }
    let entries_count = i32::from_le_bytes(clair[0..4].try_into().unwrap());
    let st_off = i32::from_le_bytes(clair[4..8].try_into().unwrap());
    let st_len = i32::from_le_bytes(clair[8..12].try_into().unwrap());
    let st_count = i32::from_le_bytes(clair[12..16].try_into().unwrap());
    if entries_count < 0 || st_off < 0 || st_len < 0 || st_count < 0 {
        return Err("en-tête T2B invalide (fichier chiffré ou corrompu ?)".to_string());
    }
    let (st_off, st_len, st_count) = (st_off as usize, st_len as usize, st_count as usize);
    let st_end = st_off
        .checked_add(st_len)
        .ok_or("débordement table de chaînes")?;
    if st_off < 16 || st_end > clair.len() {
        return Err("table de chaînes hors limites".to_string());
    }

    // Table de chaînes : offset → texte. Même lecture que le parseur.
    let mut strings: BTreeMap<i32, String> = BTreeMap::new();
    {
        let (mut pos, mut count) = (0usize, 0usize);
        while pos < st_len && count < st_count {
            let start = st_off + pos;
            let slice = &clair[start..st_end];
            let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            let s = String::from_utf8_lossy(&slice[..nul]).into_owned();
            let len = s.len();
            strings.insert(
                i32::try_from(pos).map_err(|_| "offset de chaîne hors i32".to_string())?,
                s,
            );
            pos += len + 1;
            count += 1;
        }
    }
    let lire_chaine = |off: i32| -> Option<String> {
        if off == CHAINE_VIDE {
            Some(String::new())
        } else {
            strings.get(&off).cloned()
        }
    };

    let mut out = Vec::new();
    let mut pos = 16usize;
    let buf_len = st_off;
    for _ in 0..entries_count {
        if pos + 5 > buf_len {
            break;
        }
        let param_count = clair[pos + 4] as usize;
        pos += 5;

        let type_bytes = param_count.div_ceil(4);
        let mut types = Vec::with_capacity(param_count);
        for t in 0..type_bytes {
            if pos + t >= buf_len {
                break;
            }
            let tb = clair[pos + t];
            for k in 0..4 {
                if types.len() < param_count {
                    types.push((tb >> (2 * k)) & 3);
                }
            }
        }
        pos += type_bytes;

        let total_header = 5 + type_bytes;
        if !total_header.is_multiple_of(4) {
            pos += 4 - (total_header % 4);
        }

        let base = pos;
        if pos + param_count * 4 > buf_len {
            break;
        }
        pos += param_count * 4;

        // Seules les entrées de fichier nous intéressent : dir + nom en chaînes, cpk en chaîne.
        if param_count < VARS_MIN
            || types.first() != Some(&0)
            || types.get(1) != Some(&0)
            || types.get(VAR_CPK) != Some(&0)
        {
            continue;
        }
        let val = |i: usize| {
            i32::from_le_bytes(clair[base + i * 4..base + i * 4 + 4].try_into().unwrap())
        };
        let (Some(dir), Some(nom)) = (lire_chaine(val(0)), lire_chaine(val(1))) else {
            continue;
        };
        let off_cpk = val(VAR_CPK);
        out.push(Entree {
            chemin: format!("{dir}{nom}"),
            offset_cpk: base + VAR_CPK * 4,
            deja_loose: lire_chaine(off_cpk).is_some_and(|s| s.is_empty()),
        });
    }
    Ok(out)
}

/// Rend *loose* les `chemins` donnés, dans un `cpk_list` **déjà déchiffré**.
///
/// Ne modifie que 4 octets par chemin. La longueur du tampon est inchangée.
///
/// # Errors
/// Si l'en-tête T2B est illisible.
pub fn patcher_clair(clair: &mut [u8], chemins: &[String]) -> Result<PatchReport, String> {
    let entrees = relever_entrees(clair)?;
    let index: BTreeMap<&str, &Entree> = entrees.iter().map(|e| (e.chemin.as_str(), e)).collect();

    let mut rapport = PatchReport::default();
    for c in chemins {
        // Le manifeste porte des chemins à séparateurs `/` ; le cpk_list aussi.
        let cle = c.replace('\\', "/");
        match index.get(cle.as_str()) {
            None => rapport.introuvables.push(cle),
            Some(e) if e.deja_loose => rapport.deja_loose.push(cle),
            Some(e) => {
                clair[e.offset_cpk..e.offset_cpk + 4].copy_from_slice(&CHAINE_VIDE.to_le_bytes());
                rapport.octets_modifies += 4;
                rapport.rendus_loose.push(cle);
            }
        }
    }
    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un T2B minimal : une entrée de fichier à 5 variables `[dir, nom, int, cpk, int]`.
    fn t2b_minimal() -> Vec<u8> {
        let mut strings = Vec::new();
        let off_dir = 0i32;
        strings.extend_from_slice(b"data/x/\0");
        let off_nom = i32::try_from(strings.len()).unwrap();
        strings.extend_from_slice(b"a.bin\0");
        let off_cpk = i32::try_from(strings.len()).unwrap();
        strings.extend_from_slice(b"z.cpk\0");

        // 1 entrée, 5 params → type_bytes = 2, total_header = 7 → padding 1 → base = 16 + 8.
        let mut corps = Vec::new();
        corps.extend_from_slice(&0u32.to_le_bytes()); // crc
        corps.push(5); // param_count
        // 2 bits par type, param k aux bits 2k..2k+1 : [str, str, int, str] → param2=1 en bits 4-5.
        corps.push(0b0001_0000); // types 0,0,1,0
        corps.push(0b0000_0001); // type 1 (int) pour le 5e
        corps.push(0); // padding
        for v in [off_dir, off_nom, 7, off_cpk, 42] {
            corps.extend_from_slice(&v.to_le_bytes());
        }

        let st_off = 16 + corps.len();
        let mut out = Vec::new();
        out.extend_from_slice(&1i32.to_le_bytes());
        out.extend_from_slice(&i32::try_from(st_off).unwrap().to_le_bytes());
        out.extend_from_slice(&i32::try_from(strings.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&3i32.to_le_bytes());
        out.extend_from_slice(&corps);
        out.extend_from_slice(&strings);
        out
    }

    #[test]
    fn rend_loose_sans_changer_la_taille() {
        let mut buf = t2b_minimal();
        let avant = buf.clone();
        let r = patcher_clair(&mut buf, &["data/x/a.bin".to_string()]).expect("patch");
        assert_eq!(r.rendus_loose, ["data/x/a.bin"]);
        assert_eq!(r.octets_modifies, 4);
        assert_eq!(buf.len(), avant.len(), "la taille ne doit pas bouger");
        let diff = buf.iter().zip(avant.iter()).filter(|(a, b)| a != b).count();
        assert_eq!(diff, 4, "exactement 4 octets doivent changer");
        // Et le fichier reste lisible, avec un cpk désormais vide.
        let cfg = nie_formats::cfgbin::cfgbin_parse(&buf).expect("reparse");
        let e = &cfg.entries[0];
        assert!(
            matches!(&e.variables[VAR_CPK], nie_formats::cfgbin::Value::String(s) if s.is_empty())
        );
    }

    #[test]
    fn signale_les_chemins_absents() {
        let mut buf = t2b_minimal();
        let r = patcher_clair(&mut buf, &["data/x/absent.bin".to_string()]).expect("patch");
        assert_eq!(r.introuvables, ["data/x/absent.bin"]);
        assert_eq!(r.octets_modifies, 0);
    }

    #[test]
    fn un_deuxieme_patch_ne_change_plus_rien() {
        let mut buf = t2b_minimal();
        patcher_clair(&mut buf, &["data/x/a.bin".to_string()]).expect("patch 1");
        let apres_un = buf.clone();
        let r = patcher_clair(&mut buf, &["data/x/a.bin".to_string()]).expect("patch 2");
        assert_eq!(r.deja_loose, ["data/x/a.bin"]);
        assert_eq!(buf, apres_un, "idempotent");
    }
}
