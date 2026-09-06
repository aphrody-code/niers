//! Atlas d'icônes du jeu → feuille de sprites exploitable sur le web.
//!
//! Les `.g4tx` d'interface ne sont pas des images simples : ce sont des **atlas**. Le conteneur
//! décrit lui-même ses régions ([`crate::g4tx::G4txSubTexture`] : nom + rectangle), et le jeu les
//! adresse au runtime par `SetIconSprite(obj, CRC32(chemin), CRC32(région))`. `gaiji_game.g4tx`
//! en compte 117.
//!
//! Ce module transpose cette description telle quelle vers les deux formes que le web attend :
//!
//! - **CSS** : une classe par région, `background-position` négatif sur l'atlas complet. C'est la
//!   forme la moins chère — une seule image en cache, aucun découpage, et le navigateur fait le
//!   reste.
//! - **SVG** : un `<symbol>` par région autour d'une `<image>` unique en `data:`. Le fichier est
//!   autonome (aucune ressource externe), donc collable dans une page ou importable dans React.
//!
//! Dans les deux cas les rectangles sont **recopiés**, jamais recalculés : ce sont ceux du jeu.

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::g4tx::G4tx;

/// Une région d'atlas prête à être écrite, avec son nom assaini.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    /// Nom d'origine de la région, tel qu'il figure dans le `.g4tx`.
    pub nom: String,
    /// Nom utilisable comme classe CSS / id SVG (voir [`assainir_nom`]).
    pub classe: String,
    /// Coin gauche dans l'atlas, en pixels.
    pub x: i32,
    /// Coin haut dans l'atlas, en pixels.
    pub y: i32,
    /// Largeur en pixels.
    pub largeur: i32,
    /// Hauteur en pixels.
    pub hauteur: i32,
}

/// Feuille de sprites extraite d'un atlas.
#[derive(Debug, Clone)]
pub struct SpriteSheet {
    /// Nom de la texture porteuse (sert de base aux noms de fichiers).
    pub nom: String,
    /// Largeur de l'atlas.
    pub largeur: i32,
    /// Hauteur de l'atlas.
    pub hauteur: i32,
    /// Régions, dans l'ordre du fichier.
    pub sprites: Vec<Sprite>,
}

/// Préfixe des classes CSS et des identifiants SVG générés.
pub const PREFIXE: &str = "nie";

/// Comment la feuille CSS pose l'atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCss {
    /// `background-image` — restitue les pixels de l'atlas tels quels.
    Image,
    /// `mask-image` + `background-color: currentColor` — l'icône prend la couleur du texte.
    ///
    /// C'est ce qui permet à une icône du jeu de suivre le thème, comme une icône vectorielle.
    /// Suppose un atlas aplati en masque (seul l'alpha compte).
    Masque,
}

/// Assainit un nom de région pour en faire un identifiant CSS/SVG valide.
///
/// Les noms du jeu (`gtxt_rarity01_05`) sont déjà propres, mais rien ne le garantit pour tout
/// l'atlas : tout caractère hors `[A-Za-z0-9_-]` devient `-`, et un nom commençant par un chiffre
/// est préfixé, un identifiant CSS ne pouvant pas débuter par un chiffre.
#[must_use]
pub fn assainir_nom(nom: &str) -> String {
    let mut s = String::with_capacity(nom.len());
    for c in nom.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            s.push(c.to_ascii_lowercase());
        } else {
            s.push('-');
        }
    }
    if s.is_empty() {
        return "sprite".to_string();
    }
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, 's');
    }
    s
}

/// Extrait la feuille de sprites d'un atlas déjà parsé.
///
/// `index_texture` désigne la texture porteuse (0 pour la principale). Rend `None` si l'index
/// n'existe pas. Une texture **sans** région rend une feuille vide : ce n'est pas une erreur, la
/// plupart des `.g4tx` sont des images simples.
#[must_use]
pub fn depuis_g4tx(g4tx: &G4tx, index_texture: usize) -> Option<SpriteSheet> {
    let t = g4tx.textures.get(index_texture)?;
    let sprites = t
        .sub_textures
        .iter()
        .map(|s| Sprite {
            nom: s.name.clone(),
            classe: assainir_nom(&s.name),
            x: i32::from(s.x),
            y: i32::from(s.y),
            largeur: i32::from(s.width),
            hauteur: i32::from(s.height),
        })
        .collect();
    Some(SpriteSheet {
        nom: t.name.clone(),
        largeur: t.width,
        hauteur: t.height,
        sprites,
    })
}

impl SpriteSheet {
    /// Nombre de régions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    /// `true` si l'atlas ne décrit aucune région (image simple).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    /// Génère la feuille CSS en mode image, `image_url` étant l'URL de l'atlas (chemin relatif
    /// ou `data:`).
    ///
    /// Le sélecteur de base porte l'image ; chaque classe de région ne porte que sa taille et sa
    /// position. Un élément s'écrit donc `<i class="nie-sprite nie-gtxt_rarity01_05"></i>`.
    #[must_use]
    pub fn vers_css(&self, image_url: &str) -> String {
        self.vers_css_mode(image_url, ModeCss::Image)
    }

    /// Génère la feuille CSS dans le mode demandé.
    ///
    /// En mode [`ModeCss::Masque`], l'atlas sert de `mask-image` et la couleur vient de
    /// `currentColor` : les icônes du jeu se comportent alors comme des icônes vectorielles —
    /// elles suivent la couleur du texte, donc le thème, en clair comme en sombre. L'atlas doit
    /// avoir été aplati en masque au préalable (alpha conservé, couleur ignorée).
    #[must_use]
    pub fn vers_css_mode(&self, image_url: &str, mode: ModeCss) -> String {
        let url = echapper_url(image_url);
        let mut css = String::new();
        css.push_str(&format!(
            "/* {} — {} région(s), atlas {}×{}. Généré par niers. */\n",
            self.nom,
            self.sprites.len(),
            self.largeur,
            self.hauteur
        ));

        // `background-size` / `mask-size` sur le sélecteur de base : sans lui, mettre l'atlas à
        // l'échelle décale toutes les régions, et les positions ci-dessous deviennent fausses.
        match mode {
            ModeCss::Image => css.push_str(&format!(
                ".{PREFIXE}-sprite {{\n  display: inline-block;\n  background-image: url(\"{url}\");\n  \
                 background-repeat: no-repeat;\n  background-size: {}px {}px;\n  \
                 image-rendering: pixelated;\n}}\n\n",
                self.largeur, self.hauteur
            )),
            ModeCss::Masque => css.push_str(&format!(
                ".{PREFIXE}-sprite {{\n  display: inline-block;\n  background-color: currentColor;\n  \
                 -webkit-mask-image: url(\"{url}\");\n  mask-image: url(\"{url}\");\n  \
                 -webkit-mask-repeat: no-repeat;\n  mask-repeat: no-repeat;\n  \
                 -webkit-mask-size: {0}px {1}px;\n  mask-size: {0}px {1}px;\n}}\n\n",
                self.largeur, self.hauteur
            )),
        }

        for s in &self.sprites {
            let (x, y) = (-s.x, -s.y);
            match mode {
                ModeCss::Image => css.push_str(&format!(
                    ".{PREFIXE}-{} {{ width: {}px; height: {}px; background-position: {x}px {y}px; }}\n",
                    s.classe, s.largeur, s.hauteur
                )),
                ModeCss::Masque => css.push_str(&format!(
                    ".{PREFIXE}-{} {{ width: {}px; height: {}px; \
                     -webkit-mask-position: {x}px {y}px; mask-position: {x}px {y}px; }}\n",
                    s.classe, s.largeur, s.hauteur
                )),
            }
        }
        css
    }

    /// Génère un SVG autonome : une `<image>` unique et un `<symbol>` par région.
    ///
    /// L'atlas est embarqué en `data:` — le fichier se suffit à lui-même, ce qui est la condition
    /// pour qu'il traverse une page web, un bundle ou l'explorateur sans perdre ses pixels.
    /// L'emploi côté page : `<svg><use href="feuille.svg#nie-gtxt_rarity01_05"/></svg>`.
    #[must_use]
    pub fn vers_svg(&self, image_data_uri: &str) -> String {
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
             width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            self.largeur, self.hauteur, self.largeur, self.hauteur
        ));
        svg.push_str(&format!(
            "  <title>{} — {} région(s)</title>\n  <defs>\n",
            echapper_xml(&self.nom),
            self.sprites.len()
        ));
        svg.push_str(&format!(
            "    <image id=\"{PREFIXE}-atlas\" width=\"{}\" height=\"{}\" href=\"{}\" \
             style=\"image-rendering: pixelated\"/>\n",
            self.largeur, self.hauteur, image_data_uri
        ));
        for s in &self.sprites {
            svg.push_str(&format!(
                "    <symbol id=\"{PREFIXE}-{}\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\">\
                 <use href=\"#{PREFIXE}-atlas\"/></symbol>\n",
                s.classe, s.x, s.y, s.largeur, s.hauteur, s.largeur, s.hauteur
            ));
        }
        svg.push_str("  </defs>\n");
        // Sans élément rendu, un SVG ouvert seul paraît vide : on affiche l'atlas entier.
        svg.push_str(&format!("  <use href=\"#{PREFIXE}-atlas\"/>\n</svg>\n"));
        svg
    }

    /// Génère le manifeste JSON des régions — la même information, pour du code.
    ///
    /// Utile à `nie-explorer` et au web : indexer par nom sans reparser le CSS.
    #[must_use]
    pub fn vers_json(&self) -> String {
        let mut j = String::new();
        j.push_str(&format!(
            "{{\n  \"nom\": \"{}\",\n  \"largeur\": {},\n  \"hauteur\": {},\n  \"sprites\": [\n",
            echapper_json(&self.nom),
            self.largeur,
            self.hauteur
        ));
        for (i, s) in self.sprites.iter().enumerate() {
            j.push_str(&format!(
                "    {{ \"nom\": \"{}\", \"classe\": \"{}\", \"x\": {}, \"y\": {}, \"largeur\": {}, \"hauteur\": {} }}{}\n",
                echapper_json(&s.nom),
                s.classe,
                s.x,
                s.y,
                s.largeur,
                s.hauteur,
                if i + 1 == self.sprites.len() { "" } else { "," }
            ));
        }
        j.push_str("  ]\n}\n");
        j
    }
}

/// Échappe les guillemets et parenthèses d'une URL placée dans `url("…")`.
fn echapper_url(u: &str) -> String {
    u.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Échappe le texte inséré dans un nœud XML.
fn echapper_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Échappe une chaîne JSON.
fn echapper_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Construit une URI `data:` pour un atlas encodé.
#[must_use]
pub fn data_uri(octets: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", base64(octets))
}

/// Encode en base64 standard (avec bourrage) — évite d'ajouter une dépendance pour trois lignes.
fn base64(entree: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(entree.len().div_ceil(3) * 4);
    for c in entree.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g4tx::{G4txSubTexture, G4txTexture};

    fn atlas_exemple() -> SpriteSheet {
        SpriteSheet {
            nom: "gaiji_game".to_string(),
            largeur: 512,
            hauteur: 256,
            sprites: alloc::vec![
                Sprite {
                    nom: "gtxt_rarity01_05".into(),
                    classe: "gtxt_rarity01_05".into(),
                    x: 128,
                    y: 64,
                    largeur: 96,
                    hauteur: 32
                },
                Sprite {
                    nom: "icon 01".into(),
                    classe: "icon-01".into(),
                    x: 0,
                    y: 0,
                    largeur: 32,
                    hauteur: 32
                },
            ],
        }
    }

    #[test]
    fn les_regions_du_g4tx_deviennent_des_sprites() {
        let mut t = G4txTexture {
            id: 0,
            name: "gaiji_game".into(),
            width: 512,
            height: 256,
            is_dds: true,
            data_offset: 0,
            data_size: 0,
            sub_textures: Vec::new(),
        };
        t.sub_textures.push(G4txSubTexture {
            id: 1,
            name: "gtxt_rarity01_05".into(),
            x: 128,
            y: 64,
            width: 96,
            height: 32,
        });
        let entete = crate::g4tx::G4txHeader {
            header_size: 0x60,
            file_type: 0x65,
            table_size: 0,
            texture_count: 1,
            total_count: 2,
            sub_texture_count: 1,
            texture_data_size: 0,
        };
        let g = G4tx {
            header: entete,
            textures: alloc::vec![t],
        };

        let f = depuis_g4tx(&g, 0).expect("texture 0");
        assert_eq!(f.nom, "gaiji_game");
        assert_eq!(f.len(), 1);
        assert_eq!(f.sprites[0].x, 128);
        assert_eq!(f.sprites[0].largeur, 96);
        assert!(depuis_g4tx(&g, 9).is_none(), "index hors bornes");
    }

    #[test]
    fn le_css_positionne_en_negatif() {
        let css = atlas_exemple().vers_css("gaiji_game.webp");
        assert!(css.contains("background-image: url(\"gaiji_game.webp\")"));
        // Le décalage CSS est l'opposé du coin de la région : c'est la règle des sprites.
        assert!(
            css.contains(".nie-gtxt_rarity01_05 { width: 96px; height: 32px; background-position: -128px -64px; }"),
            "{css}"
        );
        assert!(
            css.contains("image-rendering: pixelated"),
            "les icônes ne doivent pas être lissées"
        );
    }

    /// Sans `background-size`, mettre l'atlas à l'échelle décale toutes les régions et les
    /// positions calculées deviennent fausses. Le sélecteur de base doit donc le porter.
    #[test]
    fn le_css_fixe_la_taille_de_l_atlas() {
        let css = atlas_exemple().vers_css("a.webp");
        assert!(css.contains("background-size: 512px 256px;"), "{css}");
    }

    #[test]
    fn le_mode_masque_teinte_par_currentcolor() {
        let css = atlas_exemple().vers_css_mode("a.webp", ModeCss::Masque);
        assert!(css.contains("background-color: currentColor;"), "{css}");
        assert!(css.contains("mask-image: url(\"a.webp\")"));
        assert!(
            css.contains("-webkit-mask-image: url(\"a.webp\")"),
            "préfixe WebKit requis"
        );
        assert!(css.contains("mask-size: 512px 256px;"));
        assert!(
            css.contains(
                ".nie-gtxt_rarity01_05 { width: 96px; height: 32px; \
                          -webkit-mask-position: -128px -64px; mask-position: -128px -64px; }"
            ),
            "{css}"
        );
        // En mode masque, aucune image de fond : la couleur seule remplit la silhouette.
        assert!(
            !css.contains("background-image"),
            "le mode masque ne pose pas d'image de fond"
        );
    }

    #[test]
    fn le_svg_est_autonome_et_declare_un_symbole_par_region() {
        let svg = atlas_exemple().vers_svg("data:image/png;base64,AAAA");
        assert!(svg.starts_with("<svg xmlns="));
        assert!(
            svg.contains("href=\"data:image/png;base64,AAAA\""),
            "atlas embarqué"
        );
        assert!(svg.contains("<symbol id=\"nie-gtxt_rarity01_05\" viewBox=\"128 64 96 32\""));
        assert_eq!(svg.matches("<symbol").count(), 2, "un symbole par région");
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn le_json_liste_les_regions() {
        let j = atlas_exemple().vers_json();
        assert!(j.contains("\"nom\": \"gaiji_game\""));
        assert!(j.contains("\"classe\": \"gtxt_rarity01_05\""));
        // Pas de virgule après le dernier élément, sinon le JSON est invalide.
        assert!(!j.contains("},\n  ]"));
        let v: serde_json::Value = serde_json::from_str(&j).expect("JSON valide");
        assert_eq!(v["sprites"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn les_noms_deviennent_des_identifiants_valides() {
        assert_eq!(assainir_nom("gtxt_rarity01_05"), "gtxt_rarity01_05");
        assert_eq!(assainir_nom("icon 01"), "icon-01");
        assert_eq!(assainir_nom("A/B.c"), "a-b-c");
        assert_eq!(
            assainir_nom("01_debut"),
            "s01_debut",
            "un id CSS ne commence pas par un chiffre"
        );
        assert_eq!(assainir_nom(""), "sprite");
        assert_eq!(assainir_nom("MAJ"), "maj", "les classes sont en minuscules");
    }

    #[test]
    fn base64_est_conforme() {
        // Vecteurs de la RFC 4648.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert!(data_uri(b"foo", "image/png").starts_with("data:image/png;base64,Zm9v"));
    }
}
