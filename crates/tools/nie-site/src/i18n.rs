//! Les trois langues d'Aphrody — français, anglais, japonais.
//!
//! ## Pourquoi le segment d'URL et pas le sous-domaine ni le paramètre
//!
//! Trois formes existent pour servir un site en plusieurs langues : le sous-domaine
//! (`en.aphrody.com`), le segment (`/en/…`) et le paramètre (`?lang=en`). La dernière est à
//! écarter — un paramètre n'est pas une page distincte pour un robot, et deux langues finissent
//! par se disputer la même URL canonique. Le sous-domaine marcherait, mais il coûte un
//! certificat, une entrée DNS et un vhost **par langue**, alors que la mesure du 2026-09-05
//! montre déjà dix hôtes servis par un seul bloc nginx et six d'entre eux en 502.
//!
//! Le segment ne coûte rien de tout cela : une seule origine, un seul certificat, un seul
//! service. C'est la forme retenue.
//!
//! ## Pourquoi le français est à la racine
//!
//! `/` est en français, `/en/…` en anglais, `/ja/…` en japonais. Le français n'a **pas** de
//! préfixe : lui en donner un obligerait soit à rediriger `/` vers `/fr/` (une redirection sur
//! chaque première visite), soit à servir le même contenu à deux URL — c'est-à-dire à créer le
//! doublon que le `canonical` existe pour éviter. Le préfixe `/fr/` est donc accepté en entrée
//! et **redirigé** vers la racine (cf. [`Langue::separer`] et la route de redirection), pour que
//! l'URL devinée par un visiteur mène quelque part au lieu de rendre 404.
//!
//! ## Pourquoi `ja` et jamais `jp`
//!
//! `hreflang` attend un code de **langue** ISO 639-1, éventuellement suivi d'un code de **pays**
//! ISO 3166-1. Le japonais est `ja` ; `jp` est le code *pays* du Japon et n'est pas une langue
//! valide. Un `hreflang="jp"` est ignoré en silence — la pire des erreurs, puisque rien ne la
//! signale. Le gisement nomme ses colonnes `name_ja`, ce qui tombe juste.

use std::fmt;

/// Une des trois langues servies par Aphrody.
///
/// L'ordre de la variante n'a aucune signification de préséance : c'est [`Langue::TOUTES`] qui
/// fixe l'ordre d'émission des balises `hreflang`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Langue {
    /// Français — la langue du projet, servie à la racine, sans préfixe.
    #[default]
    Fr,
    /// Anglais — servi sous `/en/`.
    En,
    /// Japonais — servi sous `/ja/`, la langue d'origine du jeu.
    Ja,
}

impl Langue {
    /// Les trois langues, dans l'ordre où elles sont émises dans le `<head>` et le plan du site.
    pub const TOUTES: [Self; 3] = [Self::Fr, Self::En, Self::Ja];

    /// Le code ISO 639-1, tel qu'il apparaît dans `<html lang>` et `hreflang`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }

    /// La locale Open Graph (`og:locale`), qui exige la forme `langue_PAYS`.
    #[must_use]
    pub const fn og_locale(self) -> &'static str {
        match self {
            Self::Fr => "fr_FR",
            Self::En => "en_US",
            Self::Ja => "ja_JP",
        }
    }

    /// Le préfixe d'URL, **vide** pour le français.
    ///
    /// Se concatène directement à une route nue : `format!("{}{}", langue.prefixe(), "/textures")`
    /// rend `/textures`, `/en/textures` ou `/ja/textures`.
    #[must_use]
    pub const fn prefixe(self) -> &'static str {
        match self {
            Self::Fr => "",
            Self::En => "/en",
            Self::Ja => "/ja",
        }
    }

    /// Le nom de la langue **dans cette langue** — pour le sélecteur de langue.
    #[must_use]
    pub const fn nom_natif(self) -> &'static str {
        match self {
            Self::Fr => "Français",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }

    /// La colonne du gisement qui porte le nom d'une entité dans cette langue.
    ///
    /// Les tables `inagle_*` portent `name_fr`, `name_en` et `name_ja` ; mesuré le 2026-09-05,
    /// 6 103 des 6 168 personnages les ont toutes les trois.
    #[must_use]
    pub const fn colonne_nom(self) -> &'static str {
        match self {
            Self::Fr => "name_fr",
            Self::En => "name_en",
            Self::Ja => "name_ja",
        }
    }

    /// Reconnaît un code de langue, insensible à la casse et tolérant à une étiquette régionale.
    ///
    /// `fr`, `FR`, `fr-BE`, `fr_CA` rendent tous [`Langue::Fr`]. `jp` rend `None` : c'est un code
    /// pays, et l'accepter reviendrait à fabriquer une URL que `hreflang` refuse.
    #[must_use]
    pub fn depuis_code(code: &str) -> Option<Self> {
        let base = code
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match base.as_str() {
            "fr" => Some(Self::Fr),
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }

    /// Sépare un chemin en (langue, route nue), et dit si la forme demandée doit être redirigée.
    ///
    /// La route nue commence toujours par `/` et ne porte jamais le préfixe de langue, de sorte
    /// que le reste du code raisonne sur une seule forme.
    ///
    /// ```
    /// use nie_site::i18n::{Langue, Demande};
    /// assert_eq!(Langue::separer("/en/textures"), Demande { langue: Langue::En, route: "/textures".into(), rediriger: false });
    /// assert_eq!(Langue::separer("/textures"),    Demande { langue: Langue::Fr, route: "/textures".into(), rediriger: false });
    /// // `/fr/…` est compris, mais renvoyé vers la forme canonique.
    /// assert_eq!(Langue::separer("/fr/textures"), Demande { langue: Langue::Fr, route: "/textures".into(), rediriger: true });
    /// ```
    #[must_use]
    pub fn separer(chemin: &str) -> Demande {
        let sans_pente = chemin.strip_prefix('/').unwrap_or(chemin);
        let (tete, reste) = match sans_pente.split_once('/') {
            Some((t, r)) => (t, format!("/{r}")),
            None => (sans_pente, String::from("/")),
        };
        match tete {
            "en" => Demande {
                langue: Self::En,
                route: reste,
                rediriger: false,
            },
            "ja" => Demande {
                langue: Self::Ja,
                route: reste,
                rediriger: false,
            },
            // Compris parce qu'un visiteur peut le deviner ; redirigé parce que la forme
            // canonique du français est la racine.
            "fr" => Demande {
                langue: Self::Fr,
                route: reste,
                rediriger: true,
            },
            _ => Demande {
                langue: Self::Fr,
                route: if chemin.is_empty() {
                    String::from("/")
                } else {
                    chemin.to_owned()
                },
                rediriger: false,
            },
        }
    }

    /// L'URL absolue de `route` dans cette langue.
    #[must_use]
    pub fn url(self, origine: &str, route: &str) -> String {
        let route = if route == "/" { "" } else { route };
        format!("{}{}{}", origine, self.prefixe(), route)
    }

    /// Négocie la langue depuis un en-tête `Accept-Language`, facteurs `q` compris.
    ///
    /// Sert à **suggérer**, jamais à rediriger : une redirection sur `Accept-Language` enferme
    /// un visiteur dans la langue de son navigateur et empêche un robot — qui annonce rarement
    /// autre chose que `en` — de voir les autres versions. Le choix reste dans l'URL.
    #[must_use]
    pub fn negocier(accept_language: &str) -> Self {
        let mut meilleure = (0.0_f32, Self::Fr);
        let mut trouve = false;
        for morceau in accept_language.split(',') {
            let mut parts = morceau.split(';');
            let etiquette = parts.next().unwrap_or("").trim();
            let q = parts
                .find_map(|p| p.trim().strip_prefix("q=").and_then(|v| v.parse().ok()))
                .unwrap_or(1.0_f32);
            if let Some(langue) = Self::depuis_code(etiquette)
                && q > meilleure.0
            {
                meilleure = (q, langue);
                trouve = true;
            }
        }
        if trouve { meilleure.1 } else { Self::Fr }
    }
}

impl fmt::Display for Langue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Ce qu'un chemin demandé dit de la langue et de la route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demande {
    /// La langue déduite du préfixe, français par défaut.
    pub langue: Langue,
    /// La route sans son préfixe de langue, commençant par `/`.
    pub route: String,
    /// Vrai quand la forme demandée n'est pas canonique et doit recevoir un 301.
    pub rediriger: bool,
}

/// Un lien `hreflang` : le code annoncé et l'URL absolue visée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alternative {
    /// Le code `hreflang` (`fr`, `en`, `ja`, ou `x-default`).
    pub hreflang: &'static str,
    /// L'URL absolue de cette version.
    pub url: String,
}

/// Les quatre liens `hreflang` d'une route : les trois langues, puis `x-default`.
///
/// La réciprocité est structurelle et non déclarative : chaque page émet **le même bloc**,
/// calculé depuis la même route nue. C'est ce qui évite l'erreur classique — une page qui
/// pointe ses traductions sans qu'elles la pointent en retour, auquel cas le groupe entier est
/// ignoré.
///
/// `x-default` vise la racine française : c'est la version servie à qui n'entre dans aucune des
/// trois cases, pas une quatrième langue.
#[must_use]
pub fn alternatives(origine: &str, route: &str) -> Vec<Alternative> {
    let mut liens: Vec<Alternative> = Langue::TOUTES
        .iter()
        .map(|l| Alternative {
            hreflang: l.code(),
            url: l.url(origine, route),
        })
        .collect();
    liens.push(Alternative {
        hreflang: "x-default",
        url: Langue::Fr.url(origine, route),
    });
    liens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_et_locales() {
        assert_eq!(Langue::Fr.code(), "fr");
        assert_eq!(Langue::Ja.code(), "ja");
        assert_eq!(Langue::Ja.og_locale(), "ja_JP");
        // Le code pays du Japon n'est pas une langue : l'accepter fabriquerait un hreflang mort.
        assert_eq!(Langue::depuis_code("jp"), None);
        assert_eq!(Langue::depuis_code("JA"), Some(Langue::Ja));
        assert_eq!(Langue::depuis_code("fr-BE"), Some(Langue::Fr));
        assert_eq!(Langue::depuis_code("en_US"), Some(Langue::En));
    }

    #[test]
    fn le_francais_n_a_pas_de_prefixe() {
        assert_eq!(Langue::Fr.prefixe(), "");
        assert_eq!(
            Langue::Fr.url("https://aphrody.com", "/"),
            "https://aphrody.com"
        );
        assert_eq!(
            Langue::En.url("https://aphrody.com", "/textures"),
            "https://aphrody.com/en/textures"
        );
        assert_eq!(
            Langue::Ja.url("https://aphrody.com", "/"),
            "https://aphrody.com/ja"
        );
    }

    #[test]
    fn separation_du_prefixe() {
        let d = Langue::separer("/ja/modeles/chara");
        assert_eq!(d.langue, Langue::Ja);
        assert_eq!(d.route, "/modeles/chara");
        assert!(!d.rediriger);

        let d = Langue::separer("/textures");
        assert_eq!(d.langue, Langue::Fr);
        assert_eq!(d.route, "/textures");

        let d = Langue::separer("/");
        assert_eq!(d.langue, Langue::Fr);
        assert_eq!(d.route, "/");

        // `/en` seul est l'accueil anglais, pas une route nommée « en ».
        let d = Langue::separer("/en");
        assert_eq!(d.langue, Langue::En);
        assert_eq!(d.route, "/");

        // `/fr/…` est compris mais pas canonique.
        let d = Langue::separer("/fr/sons");
        assert_eq!(d.langue, Langue::Fr);
        assert_eq!(d.route, "/sons");
        assert!(d.rediriger);
    }

    #[test]
    fn une_route_qui_commence_par_les_memes_lettres_n_est_pas_une_langue() {
        // « enemy » commence par « en » : découper sur les lettres et non sur le segment
        // enverrait cette route en anglais avec une route nue tronquée.
        let d = Langue::separer("/enemy/x");
        assert_eq!(d.langue, Langue::Fr);
        assert_eq!(d.route, "/enemy/x");
    }

    #[test]
    fn negociation_respecte_les_facteurs_q() {
        assert_eq!(Langue::negocier("ja,en;q=0.8,fr;q=0.5"), Langue::Ja);
        assert_eq!(Langue::negocier("en-GB;q=0.9,ja;q=0.2"), Langue::En);
        // Une langue que le site ne sert pas ne doit pas l'emporter.
        assert_eq!(Langue::negocier("de,es;q=0.9"), Langue::Fr);
        assert_eq!(Langue::negocier(""), Langue::Fr);
    }

    #[test]
    fn les_alternatives_sont_reciproques_et_completes() {
        let liens = alternatives("https://aphrody.com", "/textures");
        assert_eq!(liens.len(), 4, "trois langues plus x-default");
        let codes: Vec<_> = liens.iter().map(|l| l.hreflang).collect();
        assert_eq!(codes, ["fr", "en", "ja", "x-default"]);
        assert_eq!(liens[0].url, "https://aphrody.com/textures");
        assert_eq!(liens[1].url, "https://aphrody.com/en/textures");
        assert_eq!(liens[2].url, "https://aphrody.com/ja/textures");
        // x-default et fr visent la MEME url : c'est voulu, la racine est la version de repli.
        assert_eq!(liens[3].url, liens[0].url);

        // Réciprocité : partir de n'importe quelle langue rend le même groupe.
        for langue in Langue::TOUTES {
            let depuis = Langue::separer(&format!("{}/textures", langue.prefixe()));
            assert_eq!(alternatives("https://aphrody.com", &depuis.route), liens);
        }
    }
}
