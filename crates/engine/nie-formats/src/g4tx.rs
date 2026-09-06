//! Lecteur de conteneurs de textures G4TX (Level-5 « Graphics 4 Texture »).
//!
//! Port Rust de `IECODE.Core/Formats/Level5/G4txParser.cs` (`ParseTextures` L141,
//! calcul des offsets de tables L146-150, `G4txHeader` L17, `G4txEntry` L45,
//! `G4txSubEntry` L61). Recoupé octet par octet contre deux fichiers RÉELS :
//!
//! - `font_def.g4tx`  : `texture_count=1`, `total_count=1`, `sub_texture_count=0`,
//!   texture nommée « font », payload **DDS** (4096×2048) — PAS de NXTCH.
//! - `gaiji_game.g4tx`: `texture_count=1`, `total_count=126`, `sub_texture_count=125`,
//!   texture nommée « gaiji_game », payload **DDS** (908×856) — 125 régions d'atlas.
//!   Ces comptes **suivent les mises à jour du jeu** : ne pas les figer dans un test, les relire.
//!
//! ## Réalité terrain : DDS, pas NXTCH
//!
//! Les deux seuls .g4tx présents sur le VPS portent un payload **DDS** (`"DDS "` à
//! `nxtch_base`), pas le chunk Switch NXTCH décrit par la doc. Le parseur lit donc les
//! dimensions depuis l'en-tête DDS quand il est présent, sinon depuis les champs
//! `width`/`height` de l'entrée (offset +0x18). Le déswizzle NXTCH vit dans
//! [`crate::nxtch`] (aucun fichier NXTCH réel disponible ici pour le valider).
//!
//! ## Disposition des tables (toutes vérifiées sur les fixtures)
//!
//! ```text
//! entry_offset    = 0x60
//! sub_entry_offset = entry_offset + texture_count * 0x30
//! hash_offset     = align16(sub_entry_offset + sub_texture_count * 0x18)
//! id_offset       = hash_offset + total_count * 4
//! string_offset   = align4(id_offset + total_count)
//! nxtch_base      = align16(header_size + table_size)
//! ```
//!
//! Compatible `no_std + alloc`.

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::FormatError;

/// Magic « G4TX » lu en little-endian (`0x58543447`).
pub const G4TX_MAGIC: u32 = 0x5854_3447;
/// Magic « G4TX » en octets.
pub const G4TX_MAGIC_BYTES: [u8; 4] = *b"G4TX";

const HEADER_SIZE: usize = 0x60;
const ENTRY_SIZE: usize = 0x30;
const SUB_ENTRY_SIZE: usize = 0x18;

/// Magic DDS (`"DDS "`, LE `0x20534444`).
const DDS_MAGIC: u32 = 0x2053_4444;

/// En-tête G4TX (0x60 octets). Champs vérifiés sur fichiers réels.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4txHeader {
    /// Taille de l'en-tête (`0x60`).
    pub header_size: u16,
    /// Type de fichier (`0x65` observé).
    pub file_type: u16,
    /// Taille de la table (sert au calcul de `nxtch_base`).
    pub table_size: u32,
    /// Nombre de textures principales (entrées de 0x30 octets).
    pub texture_count: u16,
    /// Nombre total d'entrées (textures + régions d'atlas) — dimensionne hash/id/string tables.
    pub total_count: u16,
    /// Nombre de sous-textures (régions d'atlas, entrées de 0x18 octets).
    pub sub_texture_count: u8,
    /// Taille des données texture (champ d'en-tête, non utilisé pour le slicing).
    pub texture_data_size: u32,
}

/// Entrée de texture principale (0x30 octets).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4txEntry {
    /// Offset du chunk (relatif à `nxtch_base`).
    pub nxtch_offset: u32,
    /// Taille du chunk de texture.
    pub nxtch_size: u32,
    /// Largeur (champ d'entrée +0x18).
    pub width: i16,
    /// Hauteur (champ d'entrée +0x1A).
    pub height: i16,
}

/// Région d'atlas (sous-texture, 0x18 octets).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4txSubEntry {
    /// Index de la texture principale parente.
    pub entry_id: i16,
    /// Coin X de la région.
    pub x: i16,
    /// Coin Y de la région.
    pub y: i16,
    /// Largeur de la région.
    pub width: i16,
    /// Hauteur de la région.
    pub height: i16,
}

/// Région d'atlas résolue (avec id + nom).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4txSubTexture {
    /// Identifiant (octet de la table d'ids).
    pub id: u8,
    /// Nom résolu depuis la table de chaînes.
    pub name: String,
    /// Coin X.
    pub x: i16,
    /// Coin Y.
    pub y: i16,
    /// Largeur.
    pub width: i16,
    /// Hauteur.
    pub height: i16,
}

/// Texture G4TX décodée.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4txTexture {
    /// Identifiant (octet de la table d'ids).
    pub id: u8,
    /// Nom de la texture (null-term ASCII résolu).
    pub name: String,
    /// Largeur effective (DDS si présent, sinon champ d'entrée).
    pub width: i32,
    /// Hauteur effective.
    pub height: i32,
    /// Vrai si le payload commence par le magic DDS.
    pub is_dds: bool,
    /// Offset absolu du payload dans le tampon (`nxtch_base + entry.nxtch_offset`).
    pub data_offset: usize,
    /// Taille déclarée du payload (`entry.nxtch_size`).
    pub data_size: usize,
    /// Régions d'atlas rattachées à cette texture.
    pub sub_textures: Vec<G4txSubTexture>,
}

/// Conteneur G4TX parsé.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4tx {
    /// En-tête.
    pub header: G4txHeader,
    /// Textures principales (avec leurs régions d'atlas).
    pub textures: Vec<G4txTexture>,
}

/// Vrai si `data` commence par le magic G4TX.
#[must_use]
pub fn is_g4tx(data: &[u8]) -> bool {
    data.starts_with(&G4TX_MAGIC_BYTES)
}

/// `true` si une texture principale est un **dummy** (placeholder) à ne PAS dessiner :
/// minuscule (≤ 4 px de côté, ex. les `4×4` de Level-5) ou nom contenant `dmy`.
#[must_use]
fn is_dummy_texture(tex: &G4txTexture) -> bool {
    if tex.width <= 4 && tex.height <= 4 {
        return true;
    }
    let n = tex.name.as_bytes();
    let needle = b"dmy";
    n.len() >= needle.len()
        && n.windows(needle.len())
            .any(|w| w.iter().zip(needle).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

/// Sélectionne la texture principale **réelle** d'un atlas g4tx pour un objet-menu.
///
/// Le compositeur naïf prenait la 1ʳᵉ texture DDS, souvent un **dummy 4×4** (`*_dmy*`) placé en
/// tête de l'atlas → sprite invisible. Stratégie, alignée sur la résolution par nom d'iecode
/// (`MenuPngService.DecodeNamedTexture`) :
/// 1. texture principale DDS dont le nom == `basename` du conteneur (insensible à la casse) ;
/// 2. sinon, plus grande texture principale DDS par aire, en **écartant les dummies** ;
/// 3. sinon, plus grande texture DDS quelconque (tout est dummy/minuscule) ;
/// 4. sinon `None` (aucune texture DDS décodable).
///
/// `basename` est le nom de fichier du `.g4tx` sans extension (ex. `title02_07`).
#[must_use]
pub fn select_main_texture<'a>(g4tx: &'a G4tx, basename: &str) -> Option<&'a G4txTexture> {
    // 1. Nom exact == basename (insensible casse), DDS.
    if let Some(t) = g4tx
        .textures
        .iter()
        .find(|t| t.is_dds && t.name.eq_ignore_ascii_case(basename))
    {
        return Some(t);
    }

    // 2. Plus grande texture DDS non-dummy par aire.
    let by_area = |t: &&G4txTexture| (t.width as i64) * (t.height as i64);
    if let Some(t) = g4tx
        .textures
        .iter()
        .filter(|t| t.is_dds && !is_dummy_texture(t))
        .max_by_key(by_area)
    {
        return Some(t);
    }

    // 3. Plus grande texture DDS quelconque (dernier recours : tout est dummy).
    g4tx.textures
        .iter()
        .filter(|t| t.is_dds)
        .max_by_key(by_area)
}

/// Cherche une **région d'atlas** par son nom dans tout le conteneur.
///
/// C'est la primitive « render-from-runtime » : `funcLuaMenuCommand(SetIconSprite, obj,
/// CRC32(chemin_g4tx), CRC32(nom_région), …)` désigne un sprite par `(chemin g4tx, nom de région)` ;
/// une fois le g4tx chargé, ce helper résout `nom_région` → la sous-texture `(x, y, w, h)` à
/// rogner dans la texture principale (ex. `gtxt_rarity01_05` dans `icon_rarity.g4tx`).
///
/// Renvoie la texture porteuse **et** la région (la région seule ne dit pas dans quelle texture
/// principale rogner). Comparaison de nom insensible à la casse, comme [`select_main_texture`].
#[must_use]
pub fn find_sub_texture<'a>(
    g4tx: &'a G4tx,
    region_name: &str,
) -> Option<(&'a G4txTexture, &'a G4txSubTexture)> {
    g4tx.textures.iter().find_map(|t| {
        t.sub_textures
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(region_name))
            .map(|s| (t, s))
    })
}

/// Ce qu'un nom désigne dans un conteneur G4TX.
///
/// Un même nom d'« icône » recouvre deux réalités que le conteneur ne distingue pas de l'extérieur :
/// soit une **texture entière** du conteneur (payload DDS autonome), soit un **rectangle** dans une
/// texture porteuse. `avatar01_13.g4tx` porte les deux à quelques octets d'écart —
/// `edit_bar_icon14_off` y est une sous-texture 56×56, `edit_bar_icon02_off` une texture entière
/// 56×56. Ne chercher que dans les sous-textures fait disparaître silencieusement la moitié des
/// icônes, qui sont alors blitées en atlas entier.
#[derive(Debug, Clone, Copy)]
pub enum NamedTarget<'a> {
    /// Le nom est celui d'une texture du conteneur : rien à rogner.
    Texture(&'a G4txTexture),
    /// Le nom est celui d'une région : rogner `sub` dans `texture`.
    Region {
        /// Texture porteuse à décoder.
        texture: &'a G4txTexture,
        /// Rectangle à rogner dedans.
        sub: &'a G4txSubTexture,
    },
}

/// Résout un nom en sa cible, texture entière ou région.
///
/// L'ordre — texture d'abord, région ensuite — est celui de
/// [`crate::g4tx_decode::decode_named_to_rgba`] : les deux doivent désigner le même pixel, sans
/// quoi les métadonnées d'un rendu ne décriraient pas ce que le décodeur produit.
#[must_use]
pub fn find_named<'a>(g4tx: &'a G4tx, nom: &str) -> Option<NamedTarget<'a>> {
    if let Some(tex) = g4tx
        .textures
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(nom))
    {
        return Some(NamedTarget::Texture(tex));
    }
    find_sub_texture(g4tx, nom).map(|(texture, sub)| NamedTarget::Region { texture, sub })
}

impl G4tx {
    /// Région d'atlas par nom — cf. [`find_sub_texture`].
    #[must_use]
    pub fn region(&self, region_name: &str) -> Option<(&G4txTexture, &G4txSubTexture)> {
        find_sub_texture(self, region_name)
    }

    /// Cible d'un nom, texture entière ou région — cf. [`find_named`].
    #[must_use]
    pub fn named(&self, nom: &str) -> Option<NamedTarget<'_>> {
        find_named(self, nom)
    }

    /// Rectangle `(x, y, w, h)` d'un nom, **qu'il désigne une région ou une texture entière**.
    ///
    /// Complète [`G4tx::region_rect`], qui ne répond que pour les sous-textures. Pour une texture
    /// entière le rectangle est `(0, 0, largeur, hauteur)` de cette texture — la texture à décoder
    /// est alors celle que rend [`G4tx::named`], pas la première du conteneur.
    #[must_use]
    pub fn named_rect(&self, nom: &str) -> Option<(i16, i16, i16, i16)> {
        match self.named(nom)? {
            NamedTarget::Texture(t) => Some((
                0,
                0,
                i16::try_from(t.width).ok()?,
                i16::try_from(t.height).ok()?,
            )),
            NamedTarget::Region { sub, .. } => Some((sub.x, sub.y, sub.width, sub.height)),
        }
    }

    /// Rectangle `(x, y, w, h)` (en pixels) d'une région d'atlas par nom, ou `None` si absente.
    ///
    /// Ne valide PAS que le rect tient dans la texture (le décodeur DDS s'en charge) : renvoie les
    /// coordonnées brutes telles que stockées, prêtes à être passées au compositeur de rendu.
    #[must_use]
    pub fn region_rect(&self, region_name: &str) -> Option<(i16, i16, i16, i16)> {
        self.region(region_name)
            .map(|(_, s)| (s.x, s.y, s.width, s.height))
    }
}

/// Suffixes des textures **techniques** d'un conteneur : elles accompagnent la couleur de base
/// sans jamais la remplacer.
///
/// `line` est un trait de contour 8×8, `msk` un masque de teinte, `oc` une occlusion, `sp`/`spm`
/// des cartes spéculaires. Une sélection « la plus grande non vide » les choisit à tort : sur
/// `hairF001M.g4tx`, `hair_10spm` fait 256×128 contre 64×32 pour la couleur de base `hair_10`.
const SUFFIXES_TECHNIQUES: [&str; 5] = ["line", "msk", "oc", "spm", "sp"];

/// Les planches de **couleur de base** d'un conteneur, du plus grand au plus petit.
///
/// Un conteneur de visage en porte souvent plusieurs : les yeux et les pupilles sont latéralisés
/// (`eye_L_00` et `eye_R_00`), la peau ne l'est pas (`face_00` seul). Chacune a son masque
/// compagnon `<nom>msk`, de mêmes dimensions.
#[must_use]
pub fn base_color_texture_names(data: &[u8]) -> alloc::vec::Vec<alloc::string::String> {
    let Ok(conteneur) = parse(data) else {
        return alloc::vec::Vec::new();
    };
    let mut retenues: alloc::vec::Vec<&G4txTexture> = conteneur
        .textures
        .iter()
        .filter(|t| {
            !t.name.is_empty()
                && !SUFFIXES_TECHNIQUES.iter().any(|s| t.name.ends_with(s))
                && t.width > 0
                && t.height > 0
        })
        .collect();
    retenues.sort_by_key(|t| core::cmp::Reverse((t.width as i64) * (t.height as i64)));
    retenues.iter().map(|t| t.name.clone()).collect()
}

/// Nom de la texture **couleur de base** d'un conteneur G4TX, s'il en porte une.
///
/// Rend la plus grande texture dont le nom ne finit par aucun suffixe technique. Les conteneurs
/// de l'éditeur d'avatar ne nomment pas leur couleur de base d'après le matériau qui la
/// consomme — `hairF001M.g4tx` porte `hair_10` alors que le matériau du G4MD s'appelle
/// `hairF_10` — donc la résolution par nom de matériau échoue et il faut choisir dans le
/// conteneur.
#[must_use]
pub fn base_color_texture_name(data: &[u8]) -> Option<alloc::string::String> {
    let conteneur = parse(data).ok()?;
    conteneur
        .textures
        .iter()
        .filter(|t| {
            !t.name.is_empty()
                && !SUFFIXES_TECHNIQUES.iter().any(|s| t.name.ends_with(s))
                && t.width > 0
                && t.height > 0
        })
        .max_by_key(|t| (t.width as i64) * (t.height as i64))
        .map(|t| t.name.clone())
}

/// Parse un conteneur G4TX.
///
/// Le payload de chaque texture n'a PAS besoin d'être présent dans `data` : si le slice
/// payload est hors limites (fichier tronqué), on renvoie quand même nom/dimensions/id/régions
/// (`is_dds = false`, dimensions issues des champs d'entrée).
///
/// # Erreurs
///
/// - [`FormatError::TooShort`] si `data` est plus court que l'en-tête.
/// - [`FormatError::BadMagic`] si le magic n'est pas G4TX.
/// - [`FormatError::Corrupt`] si une table déborde du tampon.
pub fn parse(data: &[u8]) -> Result<G4tx, FormatError> {
    if data.len() < HEADER_SIZE {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: HEADER_SIZE,
        });
    }
    if !is_g4tx(data) {
        return Err(FormatError::BadMagic { format: "G4TX" });
    }

    let header = parse_header(data)?;
    let tex_count = header.texture_count as usize;
    let total_count = header.total_count as usize;
    let sub_count = header.sub_texture_count as usize;

    // Offsets de tables (port exact de ParseTextures L146-150).
    let entry_offset = HEADER_SIZE;
    let sub_entry_offset = entry_offset + tex_count * ENTRY_SIZE;
    let hash_offset = align(sub_entry_offset + sub_count * SUB_ENTRY_SIZE, 16);
    let id_offset = hash_offset + total_count * 4;
    let string_offset = align(id_offset + total_count, 4);

    // Table d'ids : 1 octet par entrée (total_count octets).
    let ids = data
        .get(id_offset..id_offset + total_count)
        .ok_or(FormatError::Corrupt("G4TX : table d'ids hors limites"))?;

    // Offsets de chaînes : i16 par entrée à string_offset.
    let mut string_offsets = Vec::with_capacity(total_count);
    for i in 0..total_count {
        string_offsets.push(read_i16_le(data, string_offset + i * 2)?);
    }

    let nxtch_base = align(header.header_size as usize + header.table_size as usize, 16);

    // Sous-entrées (régions d'atlas).
    let mut sub_entries = Vec::with_capacity(sub_count);
    for i in 0..sub_count {
        let pos = sub_entry_offset + i * SUB_ENTRY_SIZE;
        sub_entries.push(G4txSubEntry {
            entry_id: read_i16_le(data, pos)?,
            // +0x02 inconnu
            x: read_i16_le(data, pos + 4)?,
            y: read_i16_le(data, pos + 6)?,
            width: read_i16_le(data, pos + 8)?,
            height: read_i16_le(data, pos + 10)?,
        });
    }

    let mut textures = Vec::with_capacity(tex_count);
    for i in 0..tex_count {
        let pos = entry_offset + i * ENTRY_SIZE;
        // Layout de G4txEntry : nxtch_offset @+0x04, nxtch_size @+0x08, width @+0x18, height @+0x1A.
        let entry = G4txEntry {
            nxtch_offset: read_u32_le(data, pos + 4)?,
            nxtch_size: read_u32_le(data, pos + 8)?,
            width: read_i16_le(data, pos + 0x18)?,
            height: read_i16_le(data, pos + 0x1A)?,
        };

        let name = read_name(data, string_offset, string_offsets[i])?;
        let data_offset = nxtch_base + entry.nxtch_offset as usize;
        let data_size = entry.nxtch_size as usize;

        // Dimensions : si le payload est présent ET commence par DDS, on lit l'en-tête DDS ;
        // sinon on retombe sur les champs d'entrée (fichier tronqué ou NXTCH).
        let (is_dds, width, height) = match data.get(data_offset..data_offset + 4) {
            Some(m) if u32::from_le_bytes([m[0], m[1], m[2], m[3]]) == DDS_MAGIC => {
                // DDS_HEADER : height @+0x0C, width @+0x10.
                let h = read_i32_le(data, data_offset + 0x0C)?;
                let w = read_i32_le(data, data_offset + 0x10)?;
                (true, w, h)
            }
            _ => (false, entry.width as i32, entry.height as i32),
        };

        // Régions d'atlas rattachées à cette texture (entry_id == i).
        // Les ids/noms des sous-textures suivent les textures principales : index absolu
        // = texture_count + (rang de la sous-entrée dans la liste globale).
        let mut sub_textures = Vec::new();
        for (sub_idx, sub) in sub_entries.iter().enumerate() {
            if sub.entry_id as usize == i {
                let absolute_id = tex_count + sub_idx;
                let sub_name = string_offsets.get(absolute_id).copied().map_or_else(
                    || Ok(String::new()),
                    |so| read_name(data, string_offset, so),
                )?;
                let id = ids.get(absolute_id).copied().unwrap_or(0);
                sub_textures.push(G4txSubTexture {
                    id,
                    name: sub_name,
                    x: sub.x,
                    y: sub.y,
                    width: sub.width,
                    height: sub.height,
                });
            }
        }

        textures.push(G4txTexture {
            id: ids.get(i).copied().unwrap_or(0),
            name,
            width,
            height,
            is_dds,
            data_offset,
            data_size,
            sub_textures,
        });
    }

    Ok(G4tx { header, textures })
}

fn parse_header(data: &[u8]) -> Result<G4txHeader, FormatError> {
    Ok(G4txHeader {
        header_size: read_u16_le(data, 4)?,
        file_type: read_u16_le(data, 6)?,
        // Unknown1 @0x08 (4 octets), TableSize @0x0C.
        table_size: read_u32_le(data, 0x0C)?,
        texture_count: read_u16_le(data, 0x20)?,
        total_count: read_u16_le(data, 0x22)?,
        // Unknown2 @0x24 (1 octet), SubTextureCount @0x25.
        sub_texture_count: data[0x25],
        // TextureDataSize @0x2C.
        texture_data_size: read_u32_le(data, 0x2C)?,
    })
}

fn read_name(data: &[u8], string_offset: usize, rel: i16) -> Result<String, FormatError> {
    let pos = string_offset
        .checked_add(rel as usize)
        .ok_or(FormatError::Corrupt("G4TX : overflow offset de nom"))?;
    Ok(read_cstr(data, pos))
}

#[inline]
const fn align(v: usize, a: usize) -> usize {
    (v + (a - 1)) & !(a - 1)
}

fn read_cstr(data: &[u8], abs: usize) -> String {
    let slice = data.get(abs..).unwrap_or(&[]);
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn read_u16_le(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let b: [u8; 2] = data
        .get(off..off + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4TX : lecture u16 hors limites"))?;
    Ok(u16::from_le_bytes(b))
}

fn read_i16_le(data: &[u8], off: usize) -> Result<i16, FormatError> {
    read_u16_le(data, off).map(|v| v as i16)
}

fn read_u32_le(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let b: [u8; 4] = data
        .get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(FormatError::Corrupt("G4TX : lecture u32 hors limites"))?;
    Ok(u32::from_le_bytes(b))
}

fn read_i32_le(data: &[u8], off: usize) -> Result<i32, FormatError> {
    read_u32_le(data, off).map(|v| v as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixtures réelles tronquées au début du payload (header + tables), copiées de
    // /home/ubuntu/rg/iecode/re/menu/extracted/fonts/. Le payload DDS volumineux est coupé :
    // on valide nom/id/dimensions(entrée)/régions, pas les pixels.
    #[cfg(feature = "real-fixtures")]
    const FONT_DEF_HEAD: &[u8] = include_bytes!("../tests/fixtures/font_def.g4tx.head");
    #[cfg(feature = "real-fixtures")]
    const GAIJI_HEAD: &[u8] = include_bytes!("../tests/fixtures/gaiji_game.g4tx.head");

    #[test]
    fn is_g4tx_detection() {
        assert!(is_g4tx(b"G4TX...."));
        assert!(!is_g4tx(b"g4tx"));
        assert!(!is_g4tx(b""));
    }

    fn tex(name: &str, w: i32, h: i32, is_dds: bool) -> G4txTexture {
        G4txTexture {
            id: 0,
            name: String::from(name),
            width: w,
            height: h,
            is_dds,
            data_offset: 0,
            data_size: 0,
            sub_textures: Vec::new(),
        }
    }

    /// `select_main_texture` : priorité au nom == basename, sinon plus grande non-dummy.
    #[test]
    fn select_main_texture_prefers_basename_then_largest_non_dummy() {
        // Cas title02_07-like : dummies 4×4 en tête, vraie texture nommée comme le conteneur.
        let g = G4tx {
            header: G4txHeader {
                header_size: 0x60,
                file_type: 0x65,
                table_size: 0,
                texture_count: 3,
                total_count: 3,
                sub_texture_count: 0,
                texture_data_size: 0,
            },
            textures: alloc::vec![
                tex("num_victory_dmy01", 4, 4, true),
                tex("num_victory_dmy02", 4, 4, true),
                tex("title02_07", 312, 104, true),
            ],
        };
        // 1) nom exact == basename → la vraie texture, jamais le dummy en tête.
        let t = select_main_texture(&g, "title02_07").expect("texture");
        assert_eq!(t.name, "title02_07");
        assert_eq!((t.width, t.height), (312, 104));
        // 2) basename inconnu → plus grande non-dummy (même résultat ici).
        let t2 = select_main_texture(&g, "inconnu").expect("texture");
        assert_eq!(t2.name, "title02_07");
    }

    /// Dummies seuls → dernier recours = la plus grande DDS quelconque (jamais `None` à tort).
    #[test]
    fn select_main_texture_all_dummy_falls_back() {
        let g = G4tx {
            header: G4txHeader {
                header_size: 0x60,
                file_type: 0x65,
                table_size: 0,
                texture_count: 2,
                total_count: 2,
                sub_texture_count: 0,
                texture_data_size: 0,
            },
            textures: alloc::vec![tex("a_dmy01", 4, 4, true), tex("b_dmy02", 4, 4, false)],
        };
        let t = select_main_texture(&g, "x").expect("fallback DDS");
        assert_eq!(t.name, "a_dmy01", "seule DDS, même dummy");
    }

    #[test]
    fn is_dummy_texture_detection() {
        assert!(is_dummy_texture(&tex("foo_dmy01", 100, 100, true)));
        assert!(is_dummy_texture(&tex("foo", 4, 4, true)));
        assert!(!is_dummy_texture(&tex("title02_08", 60, 52, true)));
    }

    /// Cas réel `avatar01_13.g4tx` : le conteneur mêle des icônes qui sont des **régions** et des
    /// icônes qui sont des **textures entières**. `region_rect` ne voit que les premières —
    /// `named_rect` doit répondre pour les deux, sinon l'icône est blitée en atlas complet.
    #[test]
    fn named_rect_couvre_region_et_texture_entiere() {
        let mut porteuse = tex("avatar01_13", 260, 44, true);
        porteuse.sub_textures = alloc::vec![sub("edit_bar_icon14_off", 300, 0, 56, 56)];
        let g = G4tx {
            header: G4txHeader {
                header_size: 0x60,
                file_type: 0x65,
                table_size: 0,
                texture_count: 2,
                total_count: 2,
                sub_texture_count: 1,
                texture_data_size: 0,
            },
            textures: alloc::vec![porteuse, tex("edit_bar_icon02_off", 56, 56, true)],
        };

        // Région : le rectangle est celui de la sous-texture, dans sa porteuse.
        assert_eq!(g.named_rect("edit_bar_icon14_off"), Some((300, 0, 56, 56)));
        assert_eq!(g.region_rect("edit_bar_icon14_off"), Some((300, 0, 56, 56)));

        // Texture entière : `region_rect` échoue (c'est son contrat), `named_rect` répond.
        assert_eq!(g.region_rect("edit_bar_icon02_off"), None);
        assert_eq!(g.named_rect("edit_bar_icon02_off"), Some((0, 0, 56, 56)));

        // Et la cible dit QUELLE texture décoder — la porteuse n'est pas la première venue.
        match g.named("edit_bar_icon02_off").expect("cible") {
            NamedTarget::Texture(t) => assert_eq!(t.name, "edit_bar_icon02_off"),
            NamedTarget::Region { .. } => panic!("attendu une texture entière"),
        }
        match g.named("edit_bar_icon14_off").expect("cible") {
            NamedTarget::Region { texture, sub } => {
                assert_eq!(texture.name, "avatar01_13");
                assert_eq!(sub.name, "edit_bar_icon14_off");
            }
            NamedTarget::Texture(_) => panic!("attendu une région"),
        }

        assert_eq!(g.named_rect("absent"), None);
    }

    fn sub(name: &str, x: i16, y: i16, w: i16, h: i16) -> G4txSubTexture {
        G4txSubTexture {
            id: 0,
            name: String::from(name),
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// `find_sub_texture` / `region` / `region_rect` : résout `nom de région` → `(texture, rect)`.
    /// Reproduit la structure d'`icon_rarity.g4tx` : une texture-atlas DDS portant les régions
    /// `gtxt_rarity01_*` désignées au runtime par `SetIconSprite(obj, CRC32(path), CRC32(region))`.
    #[test]
    fn region_lookup_resolves_atlas_subtexture() {
        let mut atlas = tex("icon_rarity", 512, 256, true);
        atlas.sub_textures = alloc::vec![
            sub("gtxt_rarity_dmy01", 0, 0, 4, 4),
            sub("gtxt_rarity01_05", 128, 64, 96, 32),
            sub("gtxt_rarity01_10", 224, 64, 96, 32),
        ];
        let g = G4tx {
            header: G4txHeader {
                header_size: 0x60,
                file_type: 0x65,
                table_size: 0,
                texture_count: 1,
                total_count: 4,
                sub_texture_count: 3,
                texture_data_size: 0,
            },
            textures: alloc::vec![atlas],
        };

        // Région présente → texture porteuse + rect exact.
        let (t, s) = g.region("gtxt_rarity01_05").expect("région présente");
        assert_eq!(t.name, "icon_rarity");
        assert_eq!((s.x, s.y, s.width, s.height), (128, 64, 96, 32));
        // Helper rect direct.
        assert_eq!(g.region_rect("gtxt_rarity01_10"), Some((224, 64, 96, 32)));
        // Insensible à la casse (comme select_main_texture).
        assert_eq!(g.region_rect("GTXT_RARITY01_05"), Some((128, 64, 96, 32)));
        // Région absente → None (et non panique).
        assert_eq!(g.region_rect("gtxt_rarity01_99"), None);
        assert!(find_sub_texture(&g, "inconnue").is_none());
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn font_def_header_golden() {
        let g = parse(FONT_DEF_HEAD).expect("parse font_def header");
        assert_eq!(g.header.header_size, 0x60);
        assert_eq!(g.header.file_type, 0x65);
        assert_eq!(g.header.table_size, 0x48);
        assert_eq!(g.header.texture_count, 1);
        assert_eq!(g.header.total_count, 1);
        assert_eq!(g.header.sub_texture_count, 0);
        assert_eq!(g.textures.len(), 1);
        // nxtch_base = align16(0x60 + 0x48) = align16(0xA8) = 0xB0.
        assert_eq!(g.textures[0].data_offset, 0xB0);
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn font_def_texture_named() {
        let g = parse(FONT_DEF_HEAD).unwrap();
        let t = &g.textures[0];
        // Nom résolu = "font" (vu via `strings`), id = 0xD0 (octet de la table d'ids).
        assert_eq!(t.name, "font");
        assert!(t.sub_textures.is_empty());
        // Payload tronqué dans la fixture : pas de DDS lisible → dimensions issues de l'entrée
        // (width/height @+0x18 = 4096×2048, vérifiées au xxd).
        assert!(!t.is_dds);
        assert_eq!(t.width, 4096);
        assert_eq!(t.height, 2048);
        assert_eq!(t.data_size, 0x02A0_0080);
    }

    #[cfg(feature = "real-fixtures")]
    #[test]
    fn gaiji_header_and_atlas_golden() {
        let g = parse(GAIJI_HEAD).expect("parse gaiji header");
        assert_eq!(g.header.texture_count, 1);
        assert_eq!(g.header.total_count, 118);
        assert_eq!(g.header.sub_texture_count, 117);
        assert_eq!(g.header.table_size, 0x1934);
        assert_eq!(g.textures.len(), 1);

        let t = &g.textures[0];
        // Nom et id vérifiés byte-à-byte : "gaiji_game", id=110.
        assert_eq!(t.name, "gaiji_game");
        assert_eq!(t.id, 110);
        // 117 régions d'atlas rattachées à la texture 0.
        assert_eq!(t.sub_textures.len(), 117);
        // Région 0 vérifiée byte-à-byte : "gaiji_system01", 128×128 à (0,0).
        let r0 = &t.sub_textures[0];
        assert_eq!(r0.name, "gaiji_system01");
        assert_eq!((r0.x, r0.y, r0.width, r0.height), (0, 0, 128, 128));
        // nxtch_base = align16(0x60 + 0x1934) = align16(0x1994) = 0x19A0.
        assert_eq!(t.data_offset, 0x19A0);
        assert_eq!(t.data_size, 0xB20B4);
    }

    #[test]
    fn rejette_petit_et_mauvais_magic() {
        assert!(matches!(
            parse(&[0u8; 8]),
            Err(FormatError::TooShort { .. })
        ));
        let mut buf = [0u8; HEADER_SIZE];
        buf[..4].copy_from_slice(b"XXXX");
        assert!(matches!(parse(&buf), Err(FormatError::BadMagic { .. })));
    }

    /// Forge un buffer G4TX minimal (texture_count=1, total_count=1, sub_texture_count=0)
    /// avec width=256, height=128 dans les champs d'entrée.
    /// Vérifie : header correctement parsé, 1 texture, dimensions issues des champs
    /// d'entrée (pas de payload DDS présent), nom "tex".
    #[test]
    fn forge_minimal_g4tx_buffer() {
        // Layout calculé :
        //   HEADER_SIZE = 0x60
        //   entry_offset = 0x60, entry_size = 0x30  → fin entrées = 0x90
        //   sub_entry_offset = 0x90 (sub_count=0)
        //   hash_offset = align16(0x90) = 0x90
        //   id_offset = 0x90 + 1*4 = 0x94
        //   string_offset = align4(0x94 + 1) = align4(0x95) = 0x98
        //   string offsets table : [0x98..0x9A] (1 × i16)
        //   nom "tex\0" démarre à 0x9A (rel_offset=0 depuis 0x98)
        //   nxtch_base = align16(header_size=0x60 + table_size=0x40) = align16(0xA0) = 0xA0
        //   Taille buffer = 0xA0

        const BUF_LEN: usize = 0xA0;
        let mut buf = [0u8; BUF_LEN];

        // magic
        buf[0..4].copy_from_slice(b"G4TX");
        // header_size = 0x60
        buf[4..6].copy_from_slice(&0x60u16.to_le_bytes());
        // file_type = 0x65
        buf[6..8].copy_from_slice(&0x65u16.to_le_bytes());
        // table_size = 0x40
        buf[0x0C..0x10].copy_from_slice(&0x40u32.to_le_bytes());
        // texture_count = 1
        buf[0x20..0x22].copy_from_slice(&1u16.to_le_bytes());
        // total_count = 1
        buf[0x22..0x24].copy_from_slice(&1u16.to_le_bytes());
        // sub_texture_count = 0
        buf[0x25] = 0;
        // texture_data_size = 0
        buf[0x2C..0x30].copy_from_slice(&0u32.to_le_bytes());

        // G4txEntry à entry_offset=0x60 :
        //   nxtch_offset @+0x04 = 0 (payload à nxtch_base+0 = 0xA0 = hors buffer)
        buf[0x64..0x68].copy_from_slice(&0u32.to_le_bytes());
        //   nxtch_size @+0x08 = 0
        buf[0x68..0x6C].copy_from_slice(&0u32.to_le_bytes());
        //   width @+0x18 = 256
        buf[0x78..0x7A].copy_from_slice(&256i16.to_le_bytes());
        //   height @+0x1A = 128
        buf[0x7A..0x7C].copy_from_slice(&128i16.to_le_bytes());

        // hash table @0x90 : 1 × u32 = 0 (hash non utilisé dans le parse actuel)
        buf[0x90..0x94].copy_from_slice(&0u32.to_le_bytes());
        // id table @0x94 : 1 × u8 = 42
        buf[0x94] = 42;
        // string offsets @0x98 : 1 × i16 = 2 (relatif à string_offset=0x98, pointe sur 0x9A)
        buf[0x98..0x9A].copy_from_slice(&2i16.to_le_bytes());
        // nom "tex\0" @0x9A
        buf[0x9A..0x9E].copy_from_slice(b"tex\0");

        let g = parse(&buf).expect("parse buffer forgé");
        assert_eq!(g.header.header_size, 0x60);
        assert_eq!(g.header.texture_count, 1);
        assert_eq!(g.header.total_count, 1);
        assert_eq!(g.header.sub_texture_count, 0);
        assert_eq!(g.textures.len(), 1);

        let t = &g.textures[0];
        assert_eq!(t.id, 42);
        assert_eq!(t.name, "tex");
        // Payload hors limites (nxtch_base=0xA0 = fin buffer) → is_dds=false, dims depuis entrée.
        assert!(!t.is_dds);
        assert_eq!(t.width, 256);
        assert_eq!(t.height, 128);
        assert!(t.sub_textures.is_empty());
    }
}
