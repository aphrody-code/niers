//! Recettes de **live modding** : un fichier texte décrit les patchs, `nie-mem` les applique au
//! process en cours.
//!
//! Le besoin : rejouer un lot de modifications après chaque lancement du jeu. Les adresses
//! bougent (allocation dynamique, ASLR), donc une recette s'exprime autant que possible en
//! **valeurs à remplacer** plutôt qu'en adresses — la valeur, elle, ne bouge pas.
//!
//! # Format
//!
//! Une directive par ligne ; `#` commence un commentaire ; les lignes vides sont ignorées.
//!
//! ```text
//! nom  Solaria-Zeus titulaire
//!
//! # Remplace toutes les occurrences d'un entier 32 bits par un autre.
//! u32  0x209B996D -> 0xD5ACAA9D
//!
//! # Idem, borné aux N premières occurrences rencontrées.
//! u32  0xBA4C2C21 -> 0xFF439369  max 1
//!
//! # Avec une garde de forme : ne remplace que si le voisin porte la valeur attendue. C'est ce
//! # qui distingue la structure visée d'une copie de la même valeur dans une table de données.
//! u32  0x209B996D -> 0xD5ACAA9D  si +0x04 == 0xE17C3465  max 1
//!
//! # Écrit des octets bruts à une adresse absolue…
//! at   0x0234DE4C76D8 = 9d aa ac d5
//!
//! # …ou relative au module principal (l'ASLR est résolue à l'application).
//! at   nie.exe+0xC72400 = 63 00 00 00
//! ```
//!
//! # Ce que la recette ne fait pas
//!
//! Elle n'invente aucune adresse et ne devine aucune structure : une règle `u32` qui ne trouve
//! rien le **dit** au lieu d'écrire ailleurs, et le rapport donne le compte exact d'occurrences
//! touchées. Chaque écriture est relue ; une relecture divergente est signalée comme un échec,
//! jamais tue.

use crate::{find_module_base, module_regions, read_exact, write_exact};

/// Une directive d'une recette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Regle {
    /// Remplace un entier 32 bits par un autre, partout (ou dans les `max` premières occurrences).
    RemplacerU32 {
        /// Valeur cherchée.
        de: u32,
        /// Valeur écrite.
        vers: u32,
        /// Plafond d'occurrences, `None` = toutes.
        max: Option<usize>,
        /// Garde de forme : `(offset relatif, valeur attendue)`. Une occurrence n'est retenue
        /// que si l'entier 32 bits situé à `occurrence + offset` vaut exactement cette valeur.
        ///
        /// Indispensable dès qu'un identifiant apparaît à la fois dans une table de données et
        /// dans la structure qu'on vise : `max 1` prendrait la première occurrence **de l'ordre
        /// de balayage**, pas la bonne. Le voisin sert de signature.
        garde: Option<(i64, u32)>,
    },
    /// Écrit des octets bruts à une adresse.
    Ecrire {
        /// Adresse, telle qu'écrite dans la recette (`0x…` ou `module+0xRVA`).
        adresse: String,
        /// Octets à écrire.
        octets: Vec<u8>,
    },
}

/// Une recette : un nom et des règles, dans l'ordre du fichier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recette {
    /// Nom libre, pour le rapport.
    pub nom: String,
    /// Les règles à appliquer, dans l'ordre.
    pub regles: Vec<Regle>,
}

/// Ce qu'une règle a produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultatRegle {
    /// La règle, telle que lue.
    pub regle: Regle,
    /// Occurrences trouvées (pour `RemplacerU32`) ; 1 pour une écriture directe réussie.
    pub trouvees: usize,
    /// Écritures effectivement appliquées **et relues conformes**.
    pub ecrites: usize,
    /// Adresses touchées, en hexadécimal.
    pub adresses: Vec<String>,
    /// Message d'échec, si la règle n'a pas pu s'appliquer.
    pub erreur: Option<String>,
}

/// Bilan de l'application d'une recette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    /// Nom de la recette.
    pub nom: String,
    /// Un résultat par règle.
    pub resultats: Vec<ResultatRegle>,
}

impl Rapport {
    /// Total d'écritures appliquées.
    #[must_use]
    pub fn total_ecrites(&self) -> usize {
        self.resultats.iter().map(|r| r.ecrites).sum()
    }
    /// Nombre de règles en échec (erreur, ou aucune occurrence trouvée).
    #[must_use]
    pub fn echecs(&self) -> usize {
        self.resultats
            .iter()
            .filter(|r| r.erreur.is_some() || r.trouvees == 0)
            .count()
    }
}

/// Parse un entier `0x…` ou décimal.
fn nombre(s: &str) -> Option<u32> {
    let t = s.trim();
    t.strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .or_else(|| t.parse::<u32>().ok())
}

/// Parse une suite d'octets hexadécimaux (`9d aa ac d5`, `9daaacd5`, `9d-aa-ac-d5`).
fn octets(s: &str) -> Result<Vec<u8>, String> {
    let hex: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return Err(format!("suite hex vide ou de longueur impaire : {s:?}"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("octet invalide à {i}"))
        })
        .collect()
}

/// Parse une recette depuis son texte.
///
/// # Errors
///
/// Rend la première ligne mal formée, avec son numéro — une recette à moitié comprise ne
/// s'applique pas.
pub fn parser(texte: &str) -> Result<Recette, String> {
    let mut r = Recette::default();
    for (n, ligne_brute) in texte.lines().enumerate() {
        let ligne = ligne_brute.split('#').next().unwrap_or("").trim();
        if ligne.is_empty() {
            continue;
        }
        let (mot, reste) = ligne.split_once(char::is_whitespace).unwrap_or((ligne, ""));
        let reste = reste.trim();
        match mot {
            "nom" => r.nom = reste.to_owned(),
            "u32" => {
                let (gauche, droite) = reste
                    .split_once("->")
                    .ok_or_else(|| format!("ligne {}: attendu « u32 <de> -> <vers> »", n + 1))?;

                // Suffixes optionnels. `max N` se lit EN PREMIER bien qu'il s'écrive en dernier :
                // sinon la valeur de la garde absorberait « max 1 » et deviendrait illisible.
                let mut droite = droite.trim();
                let mut max: Option<usize> = None;
                if let Some((avant, m)) = droite.rsplit_once("max") {
                    max = Some(
                        m.trim()
                            .parse::<usize>()
                            .map_err(|_| format!("ligne {}: « max » attend un entier", n + 1))?,
                    );
                    droite = avant.trim();
                }
                let mut garde = None;
                if let Some((avant, cond)) = droite.split_once(" si ") {
                    let (off_s, val_s) = cond.split_once("==").ok_or_else(|| {
                        format!("ligne {}: attendu « si <offset> == <valeur> »", n + 1)
                    })?;
                    // L'offset peut être négatif (le voisin de gauche fait aussi une signature).
                    let off_t = off_s.trim().trim_start_matches('+');
                    let (negatif, chiffres) = off_t
                        .strip_prefix('-')
                        .map_or((false, off_t), |reste| (true, reste));
                    let brut = nombre(chiffres)
                        .ok_or_else(|| format!("ligne {}: offset de garde illisible", n + 1))?;
                    let off = if negatif {
                        -i64::from(brut)
                    } else {
                        i64::from(brut)
                    };
                    let val = nombre(val_s)
                        .ok_or_else(|| format!("ligne {}: valeur de garde illisible", n + 1))?;
                    garde = Some((off, val));
                    droite = avant.trim();
                }
                let vers_s = droite;
                let de = nombre(gauche)
                    .ok_or_else(|| format!("ligne {}: valeur source illisible", n + 1))?;
                let vers = nombre(vers_s)
                    .ok_or_else(|| format!("ligne {}: valeur cible illisible", n + 1))?;
                r.regles.push(Regle::RemplacerU32 {
                    de,
                    vers,
                    max,
                    garde,
                });
            }
            "at" => {
                let (adresse, val) = reste.split_once('=').ok_or_else(|| {
                    format!("ligne {}: attendu « at <adresse> = <octets> »", n + 1)
                })?;
                let o = octets(val).map_err(|e| format!("ligne {}: {e}", n + 1))?;
                r.regles.push(Regle::Ecrire {
                    adresse: adresse.trim().to_owned(),
                    octets: o,
                });
            }
            autre => return Err(format!("ligne {}: directive inconnue « {autre} »", n + 1)),
        }
    }
    Ok(r)
}

/// Résout `0x…` ou `module+0xRVA` en adresse absolue dans `pid`.
fn resoudre(adresse: &str, pid: i32) -> Result<u64, String> {
    if let Some((module, rva)) = adresse.split_once('+') {
        let base = find_module_base(pid, module.trim())
            .ok_or_else(|| format!("module « {} » introuvable", module.trim()))?;
        let r = rva.trim();
        let off = r
            .strip_prefix("0x")
            .or_else(|| r.strip_prefix("0X"))
            .and_then(|h| u64::from_str_radix(h, 16).ok())
            .or_else(|| r.parse::<u64>().ok())
            .ok_or_else(|| format!("RVA illisible : {r}"))?;
        return Ok(base + off);
    }
    let t = adresse.trim();
    t.strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .and_then(|h| u64::from_str_radix(h, 16).ok())
        .or_else(|| t.parse::<u64>().ok())
        .ok_or_else(|| format!("adresse illisible : {adresse}"))
}

/// Écrit puis relit : une écriture n'est comptée que si la mémoire la porte réellement.
fn ecrire_et_verifier(pid: i32, addr: u64, octets: &[u8]) -> Result<(), String> {
    write_exact(pid, addr, octets).map_err(|e| e.to_string())?;
    let relu = read_exact(pid, addr, octets.len()).map_err(|e| e.to_string())?;
    if relu == octets {
        Ok(())
    } else {
        Err(format!("relecture divergente à 0x{addr:x}"))
    }
}

/// Applique une recette au process `pid`.
///
/// `module` sert au filtrage des régions à scanner : on balaye toutes les pages accessibles en
/// écriture du process (`all = true`), parce que les structures de jeu vivent dans le tas, pas
/// dans l'image du module.
///
/// Avec `a_blanc`, rien n'est écrit : le rapport dit ce qui **aurait** été touché. C'est le mode
/// à utiliser pour vérifier une recette avant de la lancer pour de bon.
#[must_use]
pub fn appliquer(pid: i32, module: &str, recette: &Recette, a_blanc: bool) -> Rapport {
    let mut resultats = Vec::with_capacity(recette.regles.len());

    for regle in &recette.regles {
        let mut res = ResultatRegle {
            regle: regle.clone(),
            trouvees: 0,
            ecrites: 0,
            adresses: Vec::new(),
            erreur: None,
        };
        match regle {
            Regle::RemplacerU32 {
                de,
                vers,
                max,
                garde,
            } => {
                let motif = de.to_le_bytes();
                let remplacement = vers.to_le_bytes();
                let plafond = max.unwrap_or(usize::MAX);
                'regions: for region in module_regions(pid, module, true) {
                    if !region.is_readable() || !region.is_writable() {
                        continue;
                    }
                    let Ok(buf) = read_exact(pid, region.start, region.size() as usize) else {
                        continue;
                    };
                    let mut i = 0usize;
                    while i + 4 <= buf.len() {
                        if buf[i..i + 4] != motif {
                            i += 4;
                            continue;
                        }
                        // Garde de forme : le voisin doit porter la valeur attendue, sinon
                        // l'occurrence est écartée — c'est ce qui distingue la structure visée
                        // d'une copie de la même valeur dans une table de données.
                        if let Some((off, attendu)) = garde {
                            let cible = i as i64 + off;
                            let ok = usize::try_from(cible).ok().is_some_and(|c| {
                                buf.get(c..c + 4)
                                    .and_then(|s| s.try_into().ok())
                                    .is_some_and(|o| u32::from_le_bytes(o) == *attendu)
                            });
                            if !ok {
                                i += 4;
                                continue;
                            }
                        }
                        let addr = region.start + i as u64;
                        res.trouvees += 1;
                        res.adresses.push(format!("0x{addr:x}"));
                        if !a_blanc {
                            match ecrire_et_verifier(pid, addr, &remplacement) {
                                Ok(()) => res.ecrites += 1,
                                Err(e) => res.erreur = Some(e),
                            }
                        }
                        if res.trouvees >= plafond {
                            break 'regions;
                        }
                        i += 4;
                    }
                }
                if res.trouvees == 0 {
                    res.erreur = Some(format!("aucune occurrence de 0x{de:08X}"));
                }
            }
            Regle::Ecrire { adresse, octets } => match resoudre(adresse, pid) {
                Ok(addr) => {
                    res.trouvees = 1;
                    res.adresses.push(format!("0x{addr:x}"));
                    if !a_blanc {
                        match ecrire_et_verifier(pid, addr, octets) {
                            Ok(()) => res.ecrites = 1,
                            Err(e) => res.erreur = Some(e),
                        }
                    }
                }
                Err(e) => res.erreur = Some(e),
            },
        }
        resultats.push(res);
    }

    Rapport {
        nom: recette.nom.clone(),
        resultats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_les_trois_directives() {
        let r = parser(
            "# entête\nnom Essai\n\nu32 0x11223344 -> 0x55667788\nu32 1 -> 2 max 3\nat nie.exe+0x10 = de ad be ef\n",
        )
        .expect("recette valide");
        assert_eq!(r.nom, "Essai");
        assert_eq!(r.regles.len(), 3);
        assert_eq!(
            r.regles[0],
            Regle::RemplacerU32 {
                de: 0x1122_3344,
                vers: 0x5566_7788,
                max: None,
                garde: None
            }
        );
        assert_eq!(
            r.regles[1],
            Regle::RemplacerU32 {
                de: 1,
                vers: 2,
                max: Some(3),
                garde: None
            }
        );
        assert_eq!(
            r.regles[2],
            Regle::Ecrire {
                adresse: "nie.exe+0x10".into(),
                octets: vec![0xDE, 0xAD, 0xBE, 0xEF]
            }
        );
    }

    #[test]
    fn une_ligne_mauvaise_arrete_le_parsing() {
        assert!(parser("u32 0x1").is_err(), "flèche manquante");
        assert!(parser("at 0x10").is_err(), "signe = manquant");
        assert!(parser("bidule 1").is_err(), "directive inconnue");
        assert!(parser("at 0x10 = abc").is_err(), "hex impair");
        // Le numéro de ligne est reporté, pour qu'une recette longue soit corrigeable.
        let e = parser("nom X\n\nu32 zz -> 1").unwrap_err();
        assert!(e.contains("ligne 3"), "message sans numéro de ligne : {e}");
    }

    #[test]
    fn commentaires_et_lignes_vides_ignores() {
        let r = parser("\n  \n# tout commentaire\nnom A  # en fin de ligne\n").expect("valide");
        assert_eq!(r.nom, "A");
        assert!(r.regles.is_empty());
    }

    #[test]
    fn octets_accepte_les_trois_ecritures() {
        assert_eq!(octets("de ad be ef").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(octets("deadbeef").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(octets("de-ad-be-ef").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(octets("").is_err());
    }

    #[test]
    fn rapport_compte_echecs_et_ecritures() {
        let rapport = Rapport {
            nom: "t".into(),
            resultats: vec![
                ResultatRegle {
                    regle: Regle::RemplacerU32 {
                        de: 1,
                        vers: 2,
                        max: None,
                        garde: None,
                    },
                    trouvees: 3,
                    ecrites: 3,
                    adresses: vec![],
                    erreur: None,
                },
                ResultatRegle {
                    regle: Regle::RemplacerU32 {
                        de: 9,
                        vers: 8,
                        max: None,
                        garde: None,
                    },
                    trouvees: 0,
                    ecrites: 0,
                    adresses: vec![],
                    erreur: Some("rien".into()),
                },
            ],
        };
        assert_eq!(rapport.total_ecrites(), 3);
        assert_eq!(rapport.echecs(), 1);
    }
}
