//! Sélecteur de chemins du dump — la syntaxe de filtre du jeu, portée telle quelle.
//!
//! [`crate::glob_match`] ne connaît que `*`. Les presets de dump, eux, s'écrivent avec quatre
//! constructions que ce module ajoute :
//!
//! | | |
//! |---|---|
//! | `a,b,c` | une liste : le chemin est retenu s'il matche **au moins un** motif |
//! | `!motif` | une exclusion : elle **prime** sur toutes les inclusions |
//! | `**` | traverse les `/` |
//! | `*`, `?` | ne traversent **pas** les `/` |
//!
//! Le matching est **ancré** (`^…$`) et insensible à la casse, et les `\` sont normalisés en `/`.
//! Ces règles ne sont pas un choix : ce sont celles de `DumpService.GlobToRegex` côté IECODE, et
//! les presets ont été écrits pour elles. En dévier ferait matcher 0 fichier — le piège que
//! `DumpPresets` documente en tête : *tous les globs doivent inclure le préfixe `data/`*.
//!
//! Le motif est compilé **une fois** ; l'ancien chemin le réinterprétait pour chacun des 255 308
//! chemins du VFS.

/// Un élément de motif, dans l'ordre où il doit être consommé.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Element {
    /// Texte littéral, déjà en minuscules.
    Litteral(String),
    /// `*` — n'importe quoi sauf `/`.
    Etoile,
    /// `**` — n'importe quoi, `/` compris.
    EtoileDouble,
    /// `?` — exactement un caractère, sauf `/`.
    Point,
}

/// Un motif compilé, avec son signe.
#[derive(Debug, Clone)]
struct Motif {
    /// `true` pour un motif préfixé `!`.
    negatif: bool,
    elements: Vec<Element>,
}

/// Filtre de chemins compilé.
#[derive(Debug, Clone, Default)]
pub struct Filtre {
    motifs: Vec<Motif>,
}

impl Filtre {
    /// Compile une spécification (`"data/dx11/**,data/chr/**,!**/movie/**"`).
    ///
    /// Une spécification vide, ou faite uniquement de séparateurs, rend un filtre qui accepte
    /// tout — c'est le comportement attendu quand aucun filtre n'est demandé.
    #[must_use]
    pub fn parse(spec: &str) -> Self {
        let motifs = spec
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|brut| {
                let (negatif, corps) = brut.strip_prefix('!').map_or((false, brut), |r| (true, r));
                Motif {
                    negatif,
                    elements: compiler(corps),
                }
            })
            .collect();
        Filtre { motifs }
    }

    /// `true` si aucun motif n'a été compilé (tout passe).
    #[must_use]
    pub fn est_vide(&self) -> bool {
        self.motifs.is_empty()
    }

    /// `true` si `chemin` est retenu.
    ///
    /// Les exclusions priment : un chemin qui matche un `!motif` est rejeté, même s'il matche
    /// par ailleurs une inclusion. En l'absence de toute inclusion, tout ce qui n'est pas exclu
    /// est retenu — c'est ce qui rend `"!**/debug/**"` utilisable seul.
    #[must_use]
    pub fn accepte(&self, chemin: &str) -> bool {
        if self.motifs.is_empty() {
            return true;
        }
        let normalise = chemin.replace('\\', "/").to_lowercase();

        if self
            .motifs
            .iter()
            .any(|m| m.negatif && correspond(&m.elements, &normalise))
        {
            return false;
        }
        let mut a_inclusion = false;
        for m in self.motifs.iter().filter(|m| !m.negatif) {
            a_inclusion = true;
            if correspond(&m.elements, &normalise) {
                return true;
            }
        }
        !a_inclusion
    }
}

/// Découpe un motif en éléments. `**` est reconnu avant `*`.
fn compiler(motif: &str) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut litteral = String::new();
    let mut chars = motif.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '?' => {
                if !litteral.is_empty() {
                    elements.push(Element::Litteral(core::mem::take(&mut litteral)));
                }
                if c == '?' {
                    elements.push(Element::Point);
                } else if chars.peek() == Some(&'*') {
                    chars.next();
                    elements.push(Element::EtoileDouble);
                } else {
                    elements.push(Element::Etoile);
                }
            }
            _ => litteral.extend(c.to_lowercase()),
        }
    }
    if !litteral.is_empty() {
        elements.push(Element::Litteral(litteral));
    }
    elements
}

/// Confronte les éléments au texte, en repartant en arrière sur les jokers.
///
/// Récursif sur les seuls jokers : la profondeur est le nombre de `*`/`**` du motif (au plus 2
/// dans les presets), pas la longueur du chemin.
fn correspond(elements: &[Element], texte: &str) -> bool {
    match elements.split_first() {
        None => texte.is_empty(),
        Some((Element::Litteral(l), reste)) => texte
            .strip_prefix(l.as_str())
            .is_some_and(|suite| correspond(reste, suite)),
        Some((Element::Point, reste)) => {
            let mut it = texte.chars();
            match it.next() {
                Some(c) if c != '/' => correspond(reste, it.as_str()),
                _ => false,
            }
        }
        Some((joker @ (Element::Etoile | Element::EtoileDouble), reste)) => {
            let traverse = *joker == Element::EtoileDouble;
            // Essai le plus court d'abord : le suffixe vide, puis un caractère de plus à chaque
            // tour. `*` s'arrête au premier `/`, `**` ne s'arrête pas.
            for (i, c) in texte.char_indices() {
                if correspond(reste, &texte[i..]) {
                    return true;
                }
                if !traverse && c == '/' {
                    return false;
                }
            }
            correspond(reste, "")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_etoile_ne_traverse_pas_les_barres() {
        let f = Filtre::parse("data/*/x.bin");
        assert!(f.accepte("data/chr/x.bin"));
        assert!(
            !f.accepte("data/chr/sub/x.bin"),
            "`*` ne doit pas franchir un /"
        );
    }

    #[test]
    fn deux_etoiles_traversent() {
        let f = Filtre::parse("data/dx11/**");
        assert!(f.accepte("data/dx11/a.g4tx"));
        assert!(f.accepte("data/dx11/chr/_face/01_ie1/c01001900/c01001900.g4tx"));
        assert!(!f.accepte("data/common/a.g4tx"));
    }

    #[test]
    fn la_liste_est_un_ou_logique() {
        let f = Filtre::parse("data/dx11/**,data/chr/**");
        assert!(f.accepte("data/dx11/a"));
        assert!(f.accepte("data/chr/b"));
        assert!(!f.accepte("data/common/c"));
    }

    #[test]
    fn l_exclusion_prime_sur_l_inclusion() {
        let f = Filtre::parse("data/**,!data/**/movie/**");
        assert!(f.accepte("data/common/x.bin"));
        assert!(
            !f.accepte("data/common/movie/intro.usm"),
            "l'exclusion doit gagner"
        );
    }

    #[test]
    fn une_specification_purement_negative_garde_le_reste() {
        let f = Filtre::parse("!**/debug/**");
        assert!(f.accepte("data/common/x.bin"));
        assert!(!f.accepte("data/common/debug/y.bin"));
    }

    #[test]
    fn le_matching_est_ancre_et_insensible_a_la_casse() {
        let f = Filtre::parse("data/dx11/**");
        // Ancré : un préfixe qui ne commence pas au début ne matche pas.
        assert!(!f.accepte("x/data/dx11/a"));
        // Les chemins du VFS mélangent les casses (`01_IE1` vs `01_ie1`).
        assert!(Filtre::parse("DATA/DX11/**").accepte("data/dx11/a"));
        assert!(f.accepte("DATA/DX11/A"));
    }

    #[test]
    fn les_antislashs_sont_normalises() {
        assert!(Filtre::parse("data/dx11/**").accepte("data\\dx11\\a.g4tx"));
    }

    #[test]
    fn le_point_d_interrogation_prend_un_caractere() {
        let f = Filtre::parse("data/a?.bin");
        assert!(f.accepte("data/ab.bin"));
        assert!(!f.accepte("data/abc.bin"));
        assert!(!f.accepte("data/a/.bin"), "`?` ne prend pas un /");
    }

    #[test]
    fn une_specification_vide_accepte_tout() {
        assert!(Filtre::parse("").accepte("n'importe quoi"));
        assert!(Filtre::parse("  ,  ").accepte("n'importe quoi"));
        assert!(Filtre::parse("").est_vide());
    }

    /// Le piège documenté par `DumpPresets` : sans le préfixe `data/`, un glob ne matche rien,
    /// puisque les chemins du `cpk_list` en sont tous préfixés.
    #[test]
    fn un_glob_sans_prefixe_data_ne_matche_rien() {
        let f = Filtre::parse("common/gamedata/**");
        assert!(!f.accepte("data/common/gamedata/skill/x.cfg.bin"));
        assert!(
            Filtre::parse("data/common/gamedata/**")
                .accepte("data/common/gamedata/skill/x.cfg.bin")
        );
    }
}
