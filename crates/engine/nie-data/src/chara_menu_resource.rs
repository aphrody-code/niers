//! `CharaMenuResource` — port du noeud `chara_menu_resource_*.cfg.bin` (format T2B `entries`).
//!
//! ## Rôle
//!
//! Templates de chemins d'icônes de personnage pour les menus. Le fichier définit comment
//! résoudre les chemins d'icônes d'un perso :
//! - `INFO_0` : template par défaut, chemins avec placeholders `<charaID>` / `<skillID>`.
//! - `INFO_1+` : overrides par personnage (chemins de fichiers spécifiques, sans placeholder).
//!
//! Couvre `200_icon/10_icon_chr` (face, uniform, coach, aura_fs, aura_armed, aura_mixi,
//! aura_soul), `200_icon/16_icon_minichr`, `200_icon/25_icon_win06_chr`, `220_img/soul_effect`.
//!
//! ## Vérité terrain
//!
//! - Parser TS : `packages/inagle/src/parsers/chara-menu-resource-config.ts` (`parseContent`
//!   l.100-160, helpers `toHex`/`cleanPath`/`extractPaths` l.62-76, `PATH_FIELDS` l.82-98).
//! - Données : VFS `data/common/gamedata/menu/chara_menu_resource_0.00.00.cfg.bin` (fichier
//!   unique) : 1 `BASE_PATH` (`#/menu/`) + 92 entrées `INFO` (1 template + 91 overrides).
//!
//! ## Port 1:1 d'inagle (quirks préservés)
//!
//! - `extractPaths` ne garde que les variables `String` (les `Int` intercalés sont **sautés**)
//!   puis les **tasse** dans l'ordre vers les champs `PATH_FIELDS[0..n]`. Conséquence (fidèle à
//!   inagle) : un override où seules les positions 14/15/16 sont des chaînes voit ces 3 chemins
//!   atterrir dans `face_small`/`face_large`/`mini_chr` — l'alignement sémantique n'est **pas**
//!   reconstruit, on porte le comportement tel quel (voir `INFO_91` = `0x68B85907`).
//! - `cleanPath` retire `.g4tx` (première occurrence, comme `String.replace` JS → `replacen(.,1)`).
//! - `resourceId` = `toHex(premier var Int)` (signé→non-signé, 8 hexa majuscules `0xXXXXXXXX`).
//! - Le template n'est retenu que si `INFO_0` contient un placeholder (`isTemplate`), conforme
//!   à la garde `i === 0 && isTemplate` d'inagle.

use alloc::string::String;
use alloc::vec::Vec;
use serde_json::Value;

use crate::cfgbin::Node;
use crate::hash::HashId;

/// Nom de l'entrée racine listant le chemin de base (`BASE_PATH_LIST_BEG`, suffixé `_0` par iecode).
pub const BASE_PATH_LIST_NAME: &str = "CHARA_MENU_RESOURCE_BASE_PATH_LIST_BEG_0";
/// Nom de l'entrée racine listant les templates/overrides (`INFO_LIST_BEG`, suffixé `_0`).
pub const INFO_LIST_NAME: &str = "CHARA_MENU_RESOURCE_INFO_LIST_BEG_0";

/// Nombre de champs de chemin d'un template/override (`PATH_FIELDS` d'inagle, l.82-98).
pub const PATH_FIELD_COUNT: usize = 15;

/// Noms des 15 champs de chemin, dans l'ordre d'extraction (référence `PATH_FIELDS` inagle).
/// L'index `j` de cette table correspond au `j`-ième champ de [`CharaResourcePaths`].
pub const PATH_FIELDS: [&str; PATH_FIELD_COUNT] = [
    "faceSmall",
    "faceLarge",
    "miniChr",
    "winIcon",
    "coachSmall",
    "coachLarge",
    "auraFsSmall",
    "auraFsLarge",
    "auraArmedSmall",
    "auraArmedLarge",
    "auraMixiSmall",
    "auraMixiLarge",
    "auraSoulSmall",
    "auraSoulLarge",
    "soulEffect",
];

/// Les 15 chemins d'un template ou override, dans l'ordre des chaînes extraites.
///
/// Chaque champ est `Some` si une chaîne a été extraite à cette position, `None` sinon (un
/// override ne fournit en général que les premiers champs). Les chemins sont **nettoyés**
/// (`.g4tx` retiré) et peuvent contenir des placeholders `<charaID>` / `<skillID>` (template).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaResourcePaths {
    /// `200_icon/10_icon_chr/<charaID>` — petite icône de visage.
    pub face_small: Option<String>,
    /// `200_icon/10_icon_chr/<charaID>_l` — grande icône de visage.
    pub face_large: Option<String>,
    /// `200_icon/16_icon_minichr/<charaID>_dot` — mini-sprite « dot ».
    pub mini_chr: Option<String>,
    /// `200_icon/25_icon_win06_chr/icon_<charaID>` — icône d'écran de victoire.
    pub win_icon: Option<String>,
    /// `200_icon/10_icon_chr/coach/<charaID>` — petite icône d'entraîneur.
    pub coach_small: Option<String>,
    /// `200_icon/10_icon_chr/coach/<charaID>_l` — grande icône d'entraîneur.
    pub coach_large: Option<String>,
    /// `200_icon/10_icon_chr/aura_fs/<charaID>` — petite icône aura « fs ».
    pub aura_fs_small: Option<String>,
    /// `200_icon/10_icon_chr/aura_fs/<charaID>_l` — grande icône aura « fs ».
    pub aura_fs_large: Option<String>,
    /// `200_icon/10_icon_chr/aura_armed/<charaID>` — petite icône aura « armed ».
    pub aura_armed_small: Option<String>,
    /// `200_icon/10_icon_chr/aura_armed/<charaID>_l` — grande icône aura « armed ».
    pub aura_armed_large: Option<String>,
    /// `200_icon/10_icon_chr/aura_mixi/<charaID>` — petite icône aura « mixi ».
    pub aura_mixi_small: Option<String>,
    /// `200_icon/10_icon_chr/aura_mixi/<charaID>_l` — grande icône aura « mixi ».
    pub aura_mixi_large: Option<String>,
    /// `200_icon/10_icon_chr/aura_soul/<charaID>` — petite icône aura « soul ».
    pub aura_soul_small: Option<String>,
    /// `200_icon/10_icon_chr/aura_soul/<charaID>_l` — grande icône aura « soul ».
    pub aura_soul_large: Option<String>,
    /// `220_img/soul_effect/ef_<skillID>` — effet d'âme (utilise `<skillID>`).
    pub soul_effect: Option<String>,
}

impl CharaResourcePaths {
    /// Construit depuis les chemins extraits (ordonnés) : assigne `paths[j]` au `j`-ième champ
    /// pour `j < min(paths.len(), 15)`, port 1:1 de la boucle inagle `pathMap[PATH_FIELDS[j]]`.
    #[must_use]
    fn from_extracted(paths: &[String]) -> Self {
        let mut p = CharaResourcePaths::default();
        for (j, path) in paths.iter().take(PATH_FIELD_COUNT).enumerate() {
            p.set_by_index(j, path.clone());
        }
        p
    }

    /// Assigne le `j`-ième champ (ordre [`PATH_FIELDS`]). Au-delà de 14, sans effet.
    fn set_by_index(&mut self, j: usize, value: String) {
        match j {
            0 => self.face_small = Some(value),
            1 => self.face_large = Some(value),
            2 => self.mini_chr = Some(value),
            3 => self.win_icon = Some(value),
            4 => self.coach_small = Some(value),
            5 => self.coach_large = Some(value),
            6 => self.aura_fs_small = Some(value),
            7 => self.aura_fs_large = Some(value),
            8 => self.aura_armed_small = Some(value),
            9 => self.aura_armed_large = Some(value),
            10 => self.aura_mixi_small = Some(value),
            11 => self.aura_mixi_large = Some(value),
            12 => self.aura_soul_small = Some(value),
            13 => self.aura_soul_large = Some(value),
            14 => self.soul_effect = Some(value),
            _ => {}
        }
    }

    /// Accès au `j`-ième champ (ordre [`PATH_FIELDS`]). `None` si `j >= 15` ou champ absent.
    #[must_use]
    pub fn by_index(&self, j: usize) -> Option<&str> {
        let f = match j {
            0 => &self.face_small,
            1 => &self.face_large,
            2 => &self.mini_chr,
            3 => &self.win_icon,
            4 => &self.coach_small,
            5 => &self.coach_large,
            6 => &self.aura_fs_small,
            7 => &self.aura_fs_large,
            8 => &self.aura_armed_small,
            9 => &self.aura_armed_large,
            10 => &self.aura_mixi_small,
            11 => &self.aura_mixi_large,
            12 => &self.aura_soul_small,
            13 => &self.aura_soul_large,
            14 => &self.soul_effect,
            _ => return None,
        };
        f.as_deref()
    }

    /// Nombre de champs renseignés (chaînes extraites pour cette entrée).
    #[must_use]
    pub fn filled_count(&self) -> usize {
        (0..PATH_FIELD_COUNT)
            .filter(|&j| self.by_index(j).is_some())
            .count()
    }
}

/// Un override `INFO_1+` (ou le template si placé en liste) : ID de ressource + chemins.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaResourceOverride {
    /// `resourceId` — `toHex(premier var Int)` (hash 32 bits, ex. `0x686BE87E`).
    pub resource_id: HashId,
    /// `true` si au moins un chemin contient un placeholder `<charaID>` / `<skillID>`.
    pub is_template: bool,
    /// Chemins extraits (tassés dans l'ordre des chaînes), nettoyés de `.g4tx`.
    pub paths: CharaResourcePaths,
}

/// Le fichier `chara_menu_resource` porté : chemin de base + template + overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaMenuResourceDatabase {
    /// `basePath` brut (non nettoyé), ex. `#/menu/`.
    pub base_path: String,
    /// Template par défaut (`INFO_0` si placeholder), avec ses 15 chemins à placeholders.
    pub template: Option<CharaResourcePaths>,
    /// Overrides par personnage (`INFO_1+`), dans l'ordre du fichier.
    pub overrides: Vec<CharaResourceOverride>,
}

impl CharaMenuResourceDatabase {
    /// Cherche un override par son `resourceId` (équivalent du `byId` map d'inagle).
    #[must_use]
    pub fn get(&self, resource_id: HashId) -> Option<&CharaResourceOverride> {
        self.overrides.iter().find(|o| o.resource_id == resource_id)
    }
}

/// Nettoie un chemin : retire la **première** occurrence de `.g4tx` (sémantique JS
/// `String.replace(str, str)`, port via `replacen(_, _, 1)`).
fn clean_path(s: &str) -> String {
    s.replacen(".g4tx", "", 1)
}

/// Extrait les chemins (chaînes nettoyées) d'un noeud : ne garde que les variables `String`,
/// dans l'ordre, en sautant les `Int` (port 1:1 d'`extractPaths`, l.72-76).
fn extract_paths(node: Node<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..node.var_count() {
        if let Some(v) = node.var(i)
            && v.ty == "String"
        {
            out.push(clean_path(v.value));
        }
    }
    out
}

/// Renvoie `toHex(premier var Int)` du noeud, ou `None` si aucune variable `Int`.
/// (Le fallback `INFO_${i}` d'inagle ne survient pas : chaque `INFO` réel commence par un `Int`.)
fn first_int_hash(node: Node<'_>) -> Option<HashId> {
    for i in 0..node.var_count() {
        if let Some(v) = node.var(i)
            && v.ty == "Int"
        {
            return Some(v.as_hash());
        }
    }
    None
}

/// `true` si un chemin contient un placeholder `<charaID>` ou `<skillID>` (`isTemplate` inagle).
fn paths_have_placeholder(paths: &[String]) -> bool {
    paths
        .iter()
        .any(|p| p.contains("<charaID>") || p.contains("<skillID>"))
}

/// Parse un `chara_menu_resource_*.cfg.bin.json` (forme iecode `{ entries: [...] }`) — port 1:1
/// d'inagle `parseContent` (chara-menu-resource-config.ts l.100-160).
#[must_use]
pub fn parse_chara_menu_resource(root: &Value) -> CharaMenuResourceDatabase {
    let mut db = CharaMenuResourceDatabase::default();

    let Some(entries) = root.get("entries").and_then(Value::as_array) else {
        return db;
    };

    for entry in entries {
        let entry = Node::new(entry);

        // Chemin de base : 1re variable String d'un enfant de la liste BASE_PATH.
        if entry.name() == BASE_PATH_LIST_NAME {
            for child in entry.children() {
                for i in 0..child.var_count() {
                    if let Some(v) = child.var(i)
                        && v.ty == "String"
                    {
                        db.base_path = String::from(v.value);
                        break;
                    }
                }
            }
        }

        // Templates / overrides : enfants de la liste INFO.
        if entry.name() == INFO_LIST_NAME {
            for (i, child) in entry.children().into_iter().enumerate() {
                let paths = extract_paths(child);
                let resource_id = first_int_hash(child).unwrap_or(HashId::ZERO);
                let is_template = paths_have_placeholder(&paths);
                let path_map = CharaResourcePaths::from_extracted(&paths);

                if i == 0 && is_template {
                    db.template = Some(path_map);
                } else {
                    db.overrides.push(CharaResourceOverride {
                        resource_id,
                        is_template,
                        paths: path_map,
                    });
                }
            }
        }
    }

    db
}
