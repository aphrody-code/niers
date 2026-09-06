//! Les 33 captures de référence de `data/menu/` — la transposition typée de
//! `data/menu/manifest.json` (`schema_version` 1).
//!
//! `data/menu/` n'est PAS une dépendance de cette crate : c'est un dossier local (assets
//! © LEVEL-5, jamais poussés). Ce module en porte l'**inventaire** (fichier, écran, écran
//! canonique, sous-écran visuel, confiance) pour que `nie-ui` puisse citer une capture par une
//! constante Rust plutôt que par une chaîne recopiée, et pour que les tests prouvent, quand le
//! dossier est là, que cet inventaire est identique au manifeste — entrée par entrée.
//!
//! Quand le dossier est absent, les tests l'annoncent (`GOLDEN SAUTE`), jamais un vert muet.

use std::path::{Path, PathBuf};

/// La variable d'environnement qui désigne le dossier des captures, lue AVANT tout chemin
/// déduit du dépôt (cf. `CLAUDE.md` : une garde sur un chemin codé en dur saute toujours).
pub const CAPTURES_ENV: &str = "NIE_MENU_CAPTURES";

/// Les dimensions de chaque capture, en pixels (`manifest.json` → `image_dimensions`).
pub const IMAGE_SIZE: (u32, u32) = (2560, 1440);

/// Comment le manifeste a identifié l'écran d'une capture (`confidence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Confidence {
    /// L'écran exact est documenté (`documented_exact`).
    DocumentedExact,
    /// La famille d'écrans est documentée, le sous-écran vient du visuel (`documented_family`).
    DocumentedFamily,
    /// Identifié par un fichier de configuration et le visuel (`config_and_visual`).
    ConfigAndVisual,
    /// Identifié par le dictionnaire de textes et le visuel (`dictionary_and_visual`).
    DictionaryAndVisual,
}

impl Confidence {
    /// La chaîne exacte du manifeste.
    #[must_use]
    pub const fn manifest_name(self) -> &'static str {
        match self {
            Self::DocumentedExact => "documented_exact",
            Self::DocumentedFamily => "documented_family",
            Self::ConfigAndVisual => "config_and_visual",
            Self::DictionaryAndVisual => "dictionary_and_visual",
        }
    }

    /// Depuis la chaîne du manifeste.
    #[must_use]
    pub fn from_manifest_name(s: &str) -> Option<Self> {
        [
            Self::DocumentedExact,
            Self::DocumentedFamily,
            Self::ConfigAndVisual,
            Self::DictionaryAndVisual,
        ]
        .into_iter()
        .find(|c| c.manifest_name() == s)
    }
}

/// Les 14 écrans canoniques distincts du manifeste (`canonical_screen`) — les familles que
/// `nie-game --menu <canonical_screen> --runtime` sait monter (`runtime_matrix.results`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CanonicalScreen {
    /// `ability_learning_board_menu` — l'arbre de compétences d'un joueur.
    AbilityLearningBoardMenu,
    /// `advent_calendar_menu` — le calendrier d'événements.
    AdventCalendarMenu,
    /// `camera_option_menu` — les Options.
    CameraOptionMenu,
    /// `camera_option_menu_shortcut` — la configuration des touches.
    CameraOptionMenuShortcut,
    /// `chara_bank_menu` — la Banque (filtres, effectif, fiche joueur).
    CharaBankMenu,
    /// `chronicle_mode_top_menu` — le mode Chronique (accueil et carte).
    ChronicleModeTopMenu,
    /// `gallery_menu` — la galerie de trophées.
    GalleryMenu,
    /// `kizuna_town_avatar_menu` — l'éditeur d'avatar.
    KizunaTownAvatarMenu,
    /// `main_menu` — le menu principal.
    MainMenu,
    /// `pause_menu` — le menu pause (contrôles).
    PauseMenu,
    /// `players_universe_menu` — l'univers des joueurs.
    PlayersUniverseMenu,
    /// `shop_menu` — le Marché (et le centre commercial Chronique).
    ShopMenu,
    /// `soccer_formation_menu` — la sélection de formation.
    SoccerFormationMenu,
    /// `story_mode_top_menu` — le mode Histoire.
    StoryModeTopMenu,
}

impl CanonicalScreen {
    /// Les 14 valeurs, dans l'ordre alphabétique du manifeste (`runtime_matrix.results`).
    pub const ALL: [Self; 14] = [
        Self::AbilityLearningBoardMenu,
        Self::AdventCalendarMenu,
        Self::CameraOptionMenu,
        Self::CameraOptionMenuShortcut,
        Self::CharaBankMenu,
        Self::ChronicleModeTopMenu,
        Self::GalleryMenu,
        Self::KizunaTownAvatarMenu,
        Self::MainMenu,
        Self::PauseMenu,
        Self::PlayersUniverseMenu,
        Self::ShopMenu,
        Self::SoccerFormationMenu,
        Self::StoryModeTopMenu,
    ];

    /// La chaîne exacte du manifeste (`canonical_screen`), qui est aussi le nom passé à
    /// `nie-game --menu`.
    #[must_use]
    pub const fn manifest_name(self) -> &'static str {
        match self {
            Self::AbilityLearningBoardMenu => "ability_learning_board_menu",
            Self::AdventCalendarMenu => "advent_calendar_menu",
            Self::CameraOptionMenu => "camera_option_menu",
            Self::CameraOptionMenuShortcut => "camera_option_menu_shortcut",
            Self::CharaBankMenu => "chara_bank_menu",
            Self::ChronicleModeTopMenu => "chronicle_mode_top_menu",
            Self::GalleryMenu => "gallery_menu",
            Self::KizunaTownAvatarMenu => "kizuna_town_avatar_menu",
            Self::MainMenu => "main_menu",
            Self::PauseMenu => "pause_menu",
            Self::PlayersUniverseMenu => "players_universe_menu",
            Self::ShopMenu => "shop_menu",
            Self::SoccerFormationMenu => "soccer_formation_menu",
            Self::StoryModeTopMenu => "story_mode_top_menu",
        }
    }

    /// Depuis la chaîne du manifeste.
    #[must_use]
    pub fn from_manifest_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.manifest_name() == s)
    }
}

/// Une capture de référence — une entrée de `manifest.json` → `entries[]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    /// Le nom du fichier PNG dans `data/menu/` (`file`).
    pub file: &'static str,
    /// L'identifiant lisible de l'écran (`screen`) — le radical du fichier.
    pub screen: &'static str,
    /// L'écran canonique (`canonical_screen`).
    pub canonical_screen: CanonicalScreen,
    /// Le sous-écran visuel quand le réglage exact n'est pas recouvrable (`visual_subscreen`).
    pub visual_subscreen: Option<&'static str>,
    /// La confiance de l'identification (`confidence`).
    pub confidence: Confidence,
}

macro_rules! capture {
    ($file:literal, $screen:literal, $canon:ident, $sub:expr, $conf:ident) => {
        Capture {
            file: $file,
            screen: $screen,
            canonical_screen: CanonicalScreen::$canon,
            visual_subscreen: $sub,
            confidence: Confidence::$conf,
        }
    };
}

/// Les 33 captures, dans l'ordre exact du manifeste (`entries[]`).
pub const CAPTURES: [Capture; 33] = [
    capture!("filters_elements.png", "filters_elements", CharaBankMenu, Some("filter_elements"), DocumentedFamily),
    capture!("filters_position.png", "filters_position", CharaBankMenu, Some("filter_position"), DocumentedFamily),
    capture!("filters_rarity.png", "filters_rarity", CharaBankMenu, Some("filter_rarity"), DocumentedFamily),
    capture!("filters_appearance.png", "filters_appearance", CharaBankMenu, Some("filter_appearance"), DocumentedFamily),
    capture!("filters_foot.png", "filters_foot", CharaBankMenu, Some("filter_foot"), DocumentedFamily),
    capture!("filters_bonus.png", "filters_bonus", CharaBankMenu, Some("filter_bonus"), DocumentedFamily),
    capture!("filters_team_role.png", "filters_team_role", CharaBankMenu, Some("filter_team_role"), DocumentedFamily),
    capture!("filters_team.png", "filters_team", CharaBankMenu, Some("filter_team"), DocumentedFamily),
    capture!("formation_select.png", "formation_select", SoccerFormationMenu, None, DocumentedFamily),
    capture!("formation_presets.png", "formation_presets", SoccerFormationMenu, Some("formation_preset_selector"), DocumentedFamily),
    capture!("character_detail_hamano.png", "character_detail_hamano", CharaBankMenu, Some("character_detail"), DocumentedFamily),
    capture!("chronicle_map.png", "chronicle_map", ChronicleModeTopMenu, Some("chronicle_map"), DocumentedFamily),
    capture!("main_menu.png", "main_menu", MainMenu, None, DocumentedExact),
    capture!("options.png", "options", CameraOptionMenu, None, ConfigAndVisual),
    capture!("controls.png", "controls", CameraOptionMenuShortcut, Some("controller_settings"), ConfigAndVisual),
    capture!("trophy_gallery.png", "trophy_gallery", GalleryMenu, None, DocumentedFamily),
    capture!("pause_controls.png", "pause_controls", PauseMenu, None, DictionaryAndVisual),
    capture!("story_mode.png", "story_mode", StoryModeTopMenu, None, DocumentedExact),
    capture!("avatar_edit_top.png", "avatar_edit_top", KizunaTownAvatarMenu, Some("avatar_edit_root"), DocumentedFamily),
    capture!("avatar_edit_style.png", "avatar_edit_style", KizunaTownAvatarMenu, Some("chara_edit_style"), DocumentedFamily),
    capture!("avatar_edit_hair.png", "avatar_edit_hair", KizunaTownAvatarMenu, Some("chara_edit_hair"), DocumentedFamily),
    capture!("avatar_edit_clothes.png", "avatar_edit_clothes", KizunaTownAvatarMenu, Some("chara_edit_clothes"), DocumentedFamily),
    capture!("avatar_edit_stats.png", "avatar_edit_stats", KizunaTownAvatarMenu, Some("chara_edit_stats"), DocumentedFamily),
    capture!("avatar_edit_name.png", "avatar_edit_name", KizunaTownAvatarMenu, Some("chara_edit_name"), DocumentedFamily),
    capture!("main_menu_alt.png", "main_menu_alt", MainMenu, None, DocumentedExact),
    capture!("event_calendar.png", "event_calendar", AdventCalendarMenu, None, DocumentedExact),
    capture!("shop.png", "shop", ShopMenu, None, DocumentedExact),
    capture!("chronicle_shop.png", "chronicle_shop", ShopMenu, Some("chronicle_shop"), DocumentedFamily),
    capture!("chronicle_mode.png", "chronicle_mode", ChronicleModeTopMenu, None, DocumentedExact),
    capture!("player_universe.png", "player_universe", PlayersUniverseMenu, None, DocumentedExact),
    capture!("player_roster.png", "player_roster", CharaBankMenu, Some("character_roster"), DocumentedFamily),
    capture!("player_skill_tree.png", "player_skill_tree", AbilityLearningBoardMenu, None, DocumentedExact),
    capture!("bank_character_detail.png", "bank_character_detail", CharaBankMenu, Some("character_detail"), DocumentedFamily),
];

/// La capture nommée par son `screen`, ou `None`.
#[must_use]
pub fn find(screen: &str) -> Option<Capture> {
    CAPTURES.into_iter().find(|c| c.screen == screen)
}

/// Les captures d'un écran canonique, dans l'ordre du manifeste.
pub fn of_screen(screen: CanonicalScreen) -> impl Iterator<Item = Capture> {
    CAPTURES.into_iter().filter(move |c| c.canonical_screen == screen)
}

/// Le dossier des captures : `NIE_MENU_CAPTURES` s'il est posé et non vide, sinon
/// `<dépôt>/data/menu` résolu depuis CETTE crate (trois `ancestors()` au-dessus de
/// `crates/engine/nie-ui`, comme [`crate::css::game_tokens_css_path`]).
#[must_use]
pub fn captures_dir() -> PathBuf {
    if let Some(v) = std::env::var_os(CAPTURES_ENV)
        && !v.is_empty()
    {
        return PathBuf::from(v);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| Path::new("."))
        .join("data/menu")
}

/// Le chemin de `manifest.json` dans [`captures_dir`].
#[must_use]
pub fn manifest_path() -> PathBuf {
    captures_dir().join("manifest.json")
}

/// [`captures_dir`] si le manifeste y existe, sinon `None` — la garde des tests qui lisent le
/// disque (elle ne dit rien : c'est à l'appelant d'annoncer le saut).
#[must_use]
pub fn captures_dir_if_present() -> Option<PathBuf> {
    let dir = captures_dir();
    dir.join("manifest.json").is_file().then_some(dir)
}

/// Lit la largeur et la hauteur d'un PNG depuis ses 24 premiers octets (signature + IHDR),
/// sans dépendance — c'est tout ce qu'un test de dimensions a besoin de savoir.
///
/// # Erreurs
/// Si la signature PNG ou le chunk `IHDR` n'est pas là où le format l'impose.
pub fn png_size(header: &[u8]) -> Result<(u32, u32), String> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if header.len() < 24 {
        return Err(format!("{} octets : en-tête PNG tronqué (24 attendus)", header.len()));
    }
    if header[..8] != SIGNATURE {
        return Err("signature PNG absente".into());
    }
    if &header[12..16] != b"IHDR" {
        return Err("le premier chunk n'est pas IHDR".into());
    }
    let be = |i: usize| u32::from_be_bytes([header[i], header[i + 1], header[i + 2], header[i + 3]]);
    Ok((be(16), be(20)))
}

#[cfg(test)]
mod tests {
    use super::{
        CAPTURES, CanonicalScreen, Capture, Confidence, IMAGE_SIZE, captures_dir_if_present, find,
        manifest_path, of_screen, png_size,
    };
    use std::collections::BTreeSet;

    fn skip(reason: &str) {
        let m = format!("GOLDEN SAUTE — {reason} : {}", manifest_path().display());
        eprintln!("{m}");
        println!("{m}");
    }

    #[test]
    fn les_33_fichiers_et_ecrans_sont_uniques() {
        let fichiers: BTreeSet<&str> = CAPTURES.iter().map(|c| c.file).collect();
        let ecrans: BTreeSet<&str> = CAPTURES.iter().map(|c| c.screen).collect();
        assert_eq!(fichiers.len(), 33);
        assert_eq!(ecrans.len(), 33);
        for c in CAPTURES {
            assert_eq!(c.file, format!("{}.png", c.screen), "{} : file ≠ screen.png", c.screen);
        }
    }

    #[test]
    fn les_14_ecrans_canoniques_sont_tous_captures_et_nommes_sans_doublon() {
        let noms: BTreeSet<&str> = CanonicalScreen::ALL.iter().map(|c| c.manifest_name()).collect();
        assert_eq!(noms.len(), 14);
        for s in CanonicalScreen::ALL {
            assert_eq!(CanonicalScreen::from_manifest_name(s.manifest_name()), Some(s));
            assert!(of_screen(s).next().is_some(), "{} : aucune capture", s.manifest_name());
        }
        let distincts: BTreeSet<CanonicalScreen> = CAPTURES.iter().map(|c| c.canonical_screen).collect();
        assert_eq!(distincts.len(), 14);
        for c in [
            Confidence::DocumentedExact,
            Confidence::DocumentedFamily,
            Confidence::ConfigAndVisual,
            Confidence::DictionaryAndVisual,
        ] {
            assert_eq!(Confidence::from_manifest_name(c.manifest_name()), Some(c));
        }
    }

    #[test]
    fn find_et_of_screen_retrouvent_le_menu_principal() {
        assert_eq!(find("main_menu").map(|c| c.file), Some("main_menu.png"));
        assert_eq!(find("inconnu"), None);
        let principal: Vec<&str> = of_screen(CanonicalScreen::MainMenu).map(|c| c.screen).collect();
        assert_eq!(principal, ["main_menu", "main_menu_alt"]);
    }

    #[test]
    fn png_size_lit_ihdr_et_refuse_le_reste() {
        let mut h = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        h.extend_from_slice(b"IHDR");
        h.extend_from_slice(&2560u32.to_be_bytes());
        h.extend_from_slice(&1440u32.to_be_bytes());
        assert_eq!(png_size(&h), Ok((2560, 1440)));
        assert!(png_size(&h[..20]).is_err());
        let mut faux = h.clone();
        faux[0] = 0;
        assert!(png_size(&faux).is_err());
        let mut pas_ihdr = h;
        pas_ihdr[12..16].copy_from_slice(b"IDAT");
        assert!(png_size(&pas_ihdr).is_err());
    }

    /// (a) Le manifeste local, entrée par entrée, doit être identique à `CAPTURES`.
    #[test]
    fn le_manifeste_local_est_identique_aux_captures_typees() {
        let Some(dir) = captures_dir_if_present() else {
            skip("manifest.json absent, l'inventaire typé n'est pas confronté au fichier");
            return;
        };
        let texte = std::fs::read_to_string(dir.join("manifest.json")).expect("manifest lisible");
        let json: serde_json::Value = serde_json::from_str(&texte).expect("manifest JSON");
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["image_dimensions"], serde_json::json!([IMAGE_SIZE.0, IMAGE_SIZE.1]));
        let entries = json["entries"].as_array().expect("entries[]");
        assert_eq!(entries.len(), CAPTURES.len(), "le manifeste n'a plus 33 entrées");
        for (i, (e, c)) in entries.iter().zip(CAPTURES.iter()).enumerate() {
            let lu = Capture {
                file: Box::leak(e["file"].as_str().expect("file").to_owned().into_boxed_str()),
                screen: Box::leak(e["screen"].as_str().expect("screen").to_owned().into_boxed_str()),
                canonical_screen: CanonicalScreen::from_manifest_name(
                    e["canonical_screen"].as_str().expect("canonical_screen"),
                )
                .unwrap_or_else(|| panic!("entrée {i} : canonical_screen inconnu {}", e["canonical_screen"])),
                visual_subscreen: e["visual_subscreen"]
                    .as_str()
                    .map(|s| &*Box::leak(s.to_owned().into_boxed_str())),
                confidence: Confidence::from_manifest_name(e["confidence"].as_str().expect("confidence"))
                    .unwrap_or_else(|| panic!("entrée {i} : confidence inconnue {}", e["confidence"])),
            };
            assert_eq!(&lu, c, "entrée {i} ({}) diverge du manifeste", c.file);
        }
    }

    /// (b) Chaque PNG existe et son IHDR dit 2560×1440.
    #[test]
    fn chaque_png_existe_en_2560x1440() {
        let Some(dir) = captures_dir_if_present() else {
            skip("captures absentes, les dimensions ne sont pas vérifiées");
            return;
        };
        let mut vus = 0;
        for c in CAPTURES {
            let chemin = dir.join(c.file);
            let octets = std::fs::read(&chemin).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
            let taille = png_size(&octets[..octets.len().min(24)])
                .unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
            assert_eq!(taille, IMAGE_SIZE, "{} n'est pas en 2560×1440", c.file);
            vus += 1;
        }
        assert_eq!(vus, 33);
    }

    /// (c) Chaque écran canonique est une famille que le manifeste documente : soit une racine
    /// Lua (`lua_analysis.documented_roots`), soit une famille montée par
    /// `nie-game --menu` (`runtime_matrix.results[].screen`).
    #[test]
    fn chaque_ecran_canonique_est_documente_par_le_manifeste() {
        let Some(dir) = captures_dir_if_present() else {
            skip("manifest.json absent, la couverture des écrans canoniques n'est pas vérifiée");
            return;
        };
        let texte = std::fs::read_to_string(dir.join("manifest.json")).expect("manifest lisible");
        let json: serde_json::Value = serde_json::from_str(&texte).expect("manifest JSON");
        let mut documentes: BTreeSet<String> = BTreeSet::new();
        for r in json["lua_analysis"]["documented_roots"].as_array().expect("documented_roots") {
            documentes.insert(r.as_str().expect("racine").to_owned());
        }
        for r in json["runtime_matrix"]["results"].as_array().expect("runtime_matrix.results") {
            documentes.insert(r["screen"].as_str().expect("screen").to_owned());
        }
        assert!(documentes.len() >= 14, "le manifeste documente {} familles", documentes.len());
        for s in CanonicalScreen::ALL {
            assert!(
                documentes.contains(s.manifest_name()),
                "{} : ni racine Lua documentée ni famille montée par nie-game",
                s.manifest_name()
            );
        }
    }
}
