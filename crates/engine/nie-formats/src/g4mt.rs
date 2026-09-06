//! Parseur **G4MT** — données de **matériaux / motion** Level-5 (`.g4mt`), présentes en standalone
//! (`common/chr/**/*.g4mt`, `common/event/**/*.g4mt`) et comme sous-table des archives `.g4pk`.
//!
//! En-tête commun Level-5 (cf. [`crate::level5`]), **validé byte sur 63 `.g4mt` réels** du VFS :
//! magic `G4MT` 63/63 + invariant `header_size + data_size == file_size` 63/63 (`header_size`=0x40,
//! `type_id`=0x68, `align`=16).
//!
//! Le corps (animation squelettique) est décodé **structurellement** par [`Motion::parse`] : en-tête
//! étendu (offsets de section CALCULÉS depuis les champs d'en-tête, jamais scannés) → table de clips
//! (un `.g4mt` en contient couramment plusieurs dizaines, ex. `c000101_p250.g4mt` = 37 clips dont un
//! à 383 frames) → table de cibles (hash CRC32 du nom d'os, résolu contre un G4SK réel) → canaux
//! typés à encodage variable, échantillonnés par interpolation keyframe (LERP/SLERP/STEP). Reversé
//! et validé croisé contre une implémentation Python indépendante
//! (`plugins/niers-blender/g4mt_probe.py`/`g4mt_motion.py`, submodule tiers) sur des fichiers réels du VFS.

use crate::FormatError;
use crate::level5::{self, Level5Header};

/// Magic « G4MT » en little-endian.
const MAGIC: u32 = 0x544D_3447;
/// Taille de l'en-tête fixe Level-5 pour ce format.
const HEADER_LEN: usize = 0x40;

/// Fichier G4MT parsé : en-tête commun + taille fichier.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct G4mt {
    pub header: Level5Header,
    pub file_size: usize,
}

impl G4mt {
    /// Invariant structurel : `header_size + data_size == file_size`.
    #[must_use]
    pub fn is_size_consistent(&self) -> bool {
        self.header.is_size_consistent(self.file_size)
    }
}

/// `true` si les 4 premiers octets sont le magic « G4MT ».
#[must_use]
pub fn is_g4mt(data: &[u8]) -> bool {
    level5::read_u32_le(data, 0).is_ok_and(|m| m == MAGIC)
}

/// Parse l'en-tête d'un `.g4mt` (corps non interprété : cf. [`Motion::parse`] pour l'animation).
///
/// # Errors
/// [`FormatError::TooShort`] si < 0x40 octets, [`FormatError::BadMagic`] si le magic ≠ « G4MT ».
pub fn parse(data: &[u8]) -> Result<G4mt, FormatError> {
    if data.len() < HEADER_LEN {
        return Err(FormatError::TooShort {
            got: data.len(),
            need: HEADER_LEN,
        });
    }
    let header = level5::parse_header(data, MAGIC, "G4MT")?;
    Ok(G4mt {
        header,
        file_size: data.len(),
    })
}

// ============================================================================
// Décodage STRUCTUREL du corps (animation squelettique). Layout reversé (offsets de section
// tous calculés depuis des champs d'en-tête, PAS des constantes scannées) :
//
//   0x0A u16 header_words     (header_size == header_words*4, cf. Level5Header::align)
//   0x20 u16 clip_count
//   0x22 u16 target_count
//   0x24 u16 target_info_units
//   0x26 u16 channel_units
//   0x28 u16[6] section_units  (scale, clip_hash, target_hash, clip_name, key, data)
//   0x36 u8  offset_shift
//
// offset(scale|clip_hash|target_hash|clip_name) = (header_words + section_units[i]) * 4
// offset(target_info|channel)                   = (header_words + (units << offset_shift)) * 4
// offset(key|data)                               = (header_words + (units << offset_shift*2)) * 4
//
// Ce doublement de shift explique pourquoi un offset FIXE (l'ancien scan à 0x144/0xA4/0xD00,
// calé sur un seul fichier "walk" 60 frames) n'est valide que par accident : il dérive dès que
// le nombre de clips/cibles/canaux change la taille des tables précédentes. Table de clips à
// `header_size` (16 o/entrée) ; cibles = hash CRC32 (même algo que [`crate::cfgbin::crc32`]) du
// nom d'os, résolues contre un G4SK réel via [`resolve_targets`].
// ============================================================================

extern crate alloc;
use alloc::{string::String, vec::Vec};

fn u8_at(d: &[u8], o: usize) -> Option<u8> {
    d.get(o).copied()
}
fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn f32_at(d: &[u8], o: usize) -> Option<f32> {
    d.get(o..o + 4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn i8_at(d: &[u8], o: usize) -> Option<f32> {
    u8_at(d, o).map(|v| f32::from(v as i8))
}
fn i16_at(d: &[u8], o: usize) -> Option<f32> {
    d.get(o..o + 2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])))
}

/// Un clip nommé d'un conteneur G4MT. Un fichier en contient couramment plusieurs (cf. module doc).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clip {
    pub name: String,
    /// Hash CRC32 du nom de clip (même algo que [`crate::cfgbin::crc32`]).
    pub crc32: u32,
    pub start_frame: u16,
    pub end_frame: u16,
    /// Bit 0 = clip additif (nécessite une pose de base ; non géré par [`Motion::sample_rotation`]).
    pub flags: u8,
    pub fps: u8,
    target_info_start: u32,
    target_info_count: u16,
}

impl Clip {
    /// Nombre de frames du clip, bornes incluses.
    #[must_use]
    pub fn frame_count(&self) -> u32 {
        u32::from(self.end_frame) - u32::from(self.start_frame) + 1
    }
    /// `true` si le clip est additif (superposé à une pose de base, non supportée ici).
    #[must_use]
    pub fn is_additive(&self) -> bool {
        self.flags & 1 != 0
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct RawTargetInfo {
    target_index: u16,
    channel_start: u32,
    channel_count: u8,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct RawChannel {
    /// Type Level-5 : `1/2/3` = scale x/y/z, `9` = rotation (quaternion), `10/11/12` = translation x/y/z.
    channel_type: u8,
    codec: u8,
    /// `false` = interpolé (LERP scale/translation, SLERP rotation) ; `true` = maintien (STEP).
    step: bool,
    variant: u8,
    n_comp: u8,
    stride: u8,
    scale_index: u8,
    key_start: u32,
    key_count: u32,
    data_offset: u32,
}

/// Animation squelettique décodée d'un conteneur G4MT/G4MA/G4TP : clips + cibles + canaux, prête à
/// échantillonner à n'importe quelle frame (interpolation keyframe, pas d'hypothèse « 1 sample =
/// 1 frame »).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Motion {
    pub header: Level5Header,
    pub file_size: usize,
    pub clips: Vec<Clip>,
    /// Hash CRC32 du nom d'os de chaque cible, dans l'ordre de la table `targets`. `target_index`
    /// (cf. [`Motion::target_indices`]) indexe ce vecteur. Résoudre via [`resolve_targets`].
    pub target_hashes: Vec<u32>,
    target_infos: Vec<RawTargetInfo>,
    channels: Vec<RawChannel>,
    scales: Vec<f32>,
    keys: Vec<u16>,
    data_offset: usize,
}

fn find_name_table(data: &[u8], start: usize, count: usize) -> Option<(usize, Vec<String>)> {
    if count == 0 {
        return Some((start, Vec::new()));
    }
    let search_end = (start + (0x100).max(count * 8)).min(data.len());
    let mut base = start;
    while base + count * 2 <= search_end {
        let mut names = Vec::with_capacity(count);
        let mut valid = true;
        for i in 0..count {
            let Some(off) = u16_at(data, base + i * 2) else {
                valid = false;
                break;
            };
            let off = off as usize;
            let absolute = base + off;
            if off < count * 2 || absolute >= data.len() {
                valid = false;
                break;
            }
            match read_utf8_cstr(data, absolute) {
                Some(name) => names.push(name),
                None => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            return Some((base, names));
        }
        base += 2;
    }
    None
}

fn read_utf8_cstr(data: &[u8], start: usize) -> Option<String> {
    let rel_end = data.get(start..)?.iter().position(|&b| b == 0)?;
    let end = start + rel_end;
    let s = core::str::from_utf8(&data[start..end]).ok()?;
    if s.is_empty() || s.chars().any(|c| (c as u32) < 0x20) {
        return None;
    }
    Some(String::from(s))
}

fn normalize_quat(q: [f32; 4]) -> [f32; 4] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n <= 1e-9 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
    }
}

fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let a = normalize_quat(a);
    let mut b = normalize_quat(b);
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        return normalize_quat([
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3] + (b[3] - a[3]) * t,
        ]);
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_theta = theta.sin();
    let left = ((1.0 - t) * theta).sin() / sin_theta;
    let right = (t * theta).sin() / sin_theta;
    [
        a[0] * left + b[0] * right,
        a[1] * left + b[1] * right,
        a[2] * left + b[2] * right,
        a[3] * left + b[3] * right,
    ]
}

fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

impl Motion {
    /// Décode structurellement un conteneur G4MT/G4MA/G4TP. `None` si la structure ne correspond
    /// pas (offsets hors limites, magic inattendu — utiliser [`parse`] pour distinguer « pas un
    /// G4MT » d'« un G4MT dont le corps est corrompu »).
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn parse(data: &[u8]) -> Option<Motion> {
        let header = level5::parse_header(data, MAGIC, "G4MT").ok()?;
        let header_words = u16_at(data, 0x0A)? as usize;
        if header_words * 4 != header.header_size as usize {
            return None;
        }
        let clip_count = u16_at(data, 0x20)? as usize;
        let target_count = u16_at(data, 0x22)? as usize;
        let target_info_units = u16_at(data, 0x24)? as usize;
        let channel_units = u16_at(data, 0x26)? as usize;
        let mut section_units = [0usize; 6];
        for (i, slot) in section_units.iter_mut().enumerate() {
            *slot = u16_at(data, 0x28 + i * 2)? as usize;
        }
        let offset_shift = u8_at(data, 0x36)?;

        let scale_offset = (header_words + section_units[0]) * 4;
        let clip_hash_offset = (header_words + section_units[1]) * 4;
        let target_hash_offset = (header_words + section_units[2]) * 4;
        let name_meta_offset = (header_words + section_units[3]) * 4;
        let target_info_offset = (header_words + (target_info_units << offset_shift)) * 4;
        let channel_offset = (header_words + (channel_units << offset_shift)) * 4;
        let key_offset = (header_words + (section_units[4] << (offset_shift * 2))) * 4;
        let data_offset = (header_words + (section_units[5] << (offset_shift * 2))) * 4;

        // Table de clips : 16 o/entrée @ header_size = <HHHHBBBBI>.
        let clip_rows_offset = header.header_size as usize;
        struct ClipRow {
            start_frame: u16,
            end_frame: u16,
            ti_start_lo: u16,
            ti_count: u16,
            flags: u8,
            fps: u8,
            ti_start_hi: u8,
        }
        let mut clip_rows = Vec::with_capacity(clip_count);
        for i in 0..clip_count {
            let o = clip_rows_offset + i * 0x10;
            clip_rows.push(ClipRow {
                start_frame: u16_at(data, o)?,
                end_frame: u16_at(data, o + 2)?,
                ti_start_lo: u16_at(data, o + 4)?,
                ti_count: u16_at(data, o + 6)?,
                flags: u8_at(data, o + 8)?,
                fps: u8_at(data, o + 9)?,
                ti_start_hi: u8_at(data, o + 10)?,
            });
        }

        // Échelles f32 partagées, entre scale_offset et clip_hash_offset.
        let scale_count = clip_hash_offset.checked_sub(scale_offset)? / 4;
        let mut scales = Vec::with_capacity(scale_count);
        for i in 0..scale_count {
            scales.push(f32_at(data, scale_offset + i * 4)?);
        }

        let mut clip_hashes = Vec::with_capacity(clip_count);
        for i in 0..clip_count {
            clip_hashes.push(u32_at(data, clip_hash_offset + i * 4)?);
        }
        let mut target_hashes = Vec::with_capacity(target_count);
        for i in 0..target_count {
            target_hashes.push(u32_at(data, target_hash_offset + i * 4)?);
        }

        let (_, clip_names) = find_name_table(data, name_meta_offset, clip_count)?;

        let clips: Vec<Clip> = clip_rows
            .into_iter()
            .zip(clip_hashes)
            .zip(clip_names)
            .map(|((row, hash), name)| Clip {
                name,
                crc32: hash,
                start_frame: row.start_frame,
                end_frame: row.end_frame,
                flags: row.flags,
                fps: row.fps,
                target_info_start: u32::from(row.ti_start_lo) + (u32::from(row.ti_start_hi) << 16),
                target_info_count: row.ti_count,
            })
            .collect();

        let target_info_count = clips
            .iter()
            .map(|c| c.target_info_start + u32::from(c.target_info_count))
            .max()
            .unwrap_or(0) as usize;
        let mut target_infos = Vec::with_capacity(target_info_count);
        for i in 0..target_info_count {
            let o = target_info_offset + i * 8;
            let target_index = u16_at(data, o)?;
            if target_index as usize >= target_count {
                return None;
            }
            let channel_start_low = u32::from(u16_at(data, o + 2)?);
            let channel_count = u8_at(data, o + 4)?;
            let channel_start_high = u32::from(u8_at(data, o + 5)?);
            target_infos.push(RawTargetInfo {
                target_index,
                channel_start: channel_start_low + (channel_start_high << 16),
                channel_count,
            });
        }

        let channel_count = target_infos
            .iter()
            .map(|t| t.channel_start + u32::from(t.channel_count))
            .max()
            .unwrap_or(0) as usize;
        let mut channels = Vec::with_capacity(channel_count);
        for i in 0..channel_count {
            let o = channel_offset + i * 20;
            let encoding = data.get(o..o + 8)?;
            channels.push(RawChannel {
                channel_type: encoding[0],
                codec: encoding[1],
                step: encoding[2] == 0,
                variant: encoding[3],
                n_comp: encoding[4],
                stride: encoding[5],
                scale_index: encoding[6],
                key_start: u32_at(data, o + 8)?,
                data_offset: u32_at(data, o + 12)?,
                key_count: u32_at(data, o + 16)?,
            });
        }

        let key_count = channels
            .iter()
            .map(|c| c.key_start + c.key_count)
            .max()
            .unwrap_or(0) as usize;
        let mut keys = Vec::with_capacity(key_count);
        for i in 0..key_count {
            keys.push(u16_at(data, key_offset + i * 2)?);
        }

        Some(Motion {
            header,
            file_size: data.len(),
            clips,
            target_hashes,
            target_infos,
            channels,
            scales,
            keys,
            data_offset,
        })
    }

    /// Index des cibles animées par ce clip (dédupliqués, ordre de la table). Indexe
    /// [`Self::target_hashes`] / le résultat de [`resolve_targets`].
    #[must_use]
    pub fn target_indices(&self, clip: &Clip) -> Vec<u16> {
        let start = clip.target_info_start as usize;
        let end = start + clip.target_info_count as usize;
        let mut out: Vec<u16> = self
            .target_infos
            .get(start..end)
            .into_iter()
            .flatten()
            .map(|t| t.target_index)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    fn rotation_channel(&self, clip: &Clip, target_index: u16) -> Option<&RawChannel> {
        let start = clip.target_info_start as usize;
        let end = start + clip.target_info_count as usize;
        let info = self
            .target_infos
            .get(start..end)?
            .iter()
            .find(|t| t.target_index == target_index)?;
        let cs = info.channel_start as usize;
        let ce = cs + info.channel_count as usize;
        self.channels
            .get(cs..ce)?
            .iter()
            .find(|c| c.channel_type == 9)
    }

    fn decode_key(&self, data: &[u8], channel: &RawChannel, key_index: u32) -> Option<[f32; 4]> {
        let scale = self
            .scales
            .get(channel.scale_index as usize)
            .copied()
            .unwrap_or(1.0);
        let base = self.data_offset
            + channel.data_offset as usize
            + key_index as usize * channel.stride as usize;
        let n = (channel.n_comp as usize).min(4);
        let mut out = [0.0f32; 4];
        for (c, slot) in out.iter_mut().enumerate().take(n) {
            *slot = match (channel.codec, channel.variant) {
                (1, 1) => i8_at(data, base + c)?,
                (1, 2) => i16_at(data, base + c * 2)?,
                (1, 4) => f32_at(data, base + c * 4)?,
                (2, 1) => f32::from(u8_at(data, base + c)?) * scale / 256.0,
                (2, 2) => f32::from(u16_at(data, base + c * 2)?) * scale / 65536.0,
                (3, 1) => i8_at(data, base + c)? * scale / 128.0,
                (3, 2) => i16_at(data, base + c * 2)? * scale / 32768.0,
                _ => return None,
            };
        }
        Some(out)
    }

    fn sample_channel(&self, data: &[u8], channel: &RawChannel, frame: f32) -> Option<[f32; 4]> {
        let ks = channel.key_start as usize;
        let ke = ks + channel.key_count as usize;
        let keys = self.keys.get(ks..ke)?;
        if keys.is_empty() {
            return None;
        }
        let right = keys.partition_point(|&k| f32::from(k) <= frame);
        if right == 0 {
            return self.decode_key(data, channel, 0);
        }
        let left = (right - 1).min(keys.len() - 1);
        if left == keys.len() - 1 || channel.step {
            return self.decode_key(data, channel, left as u32);
        }
        let span = f32::from(keys[left + 1]) - f32::from(keys[left]);
        let t = if span > 0.0 {
            (frame - f32::from(keys[left])) / span
        } else {
            0.0
        };
        let a = self.decode_key(data, channel, left as u32)?;
        let b = self.decode_key(data, channel, left as u32 + 1)?;
        Some(if channel.channel_type == 9 {
            slerp(a, b, t)
        } else {
            lerp4(a, b, t)
        })
    }

    /// Échantillonne le quaternion de rotation (xyzw normalisé) d'une cible animée à la frame
    /// donnée, avec interpolation SLERP entre clés voisines (ou maintien si le canal est en mode
    /// STEP). `data` = les octets bruts du `.g4mt` (déjà utilisés pour [`Self::parse`]).
    /// `target_index` = une valeur renvoyée par [`Self::target_indices`].
    #[must_use]
    pub fn sample_rotation(
        &self,
        data: &[u8],
        clip: &Clip,
        target_index: u16,
        frame: f32,
    ) -> Option<[f32; 4]> {
        let channel = self.rotation_channel(clip, target_index)?;
        self.sample_channel(data, channel, frame)
            .map(normalize_quat)
    }

    /// Échantillonne tous les canaux TRS d'un os, en conservant la pose de repos pour
    /// les composantes absentes. Les clips additifs sont refusés car ils exigent une base.
    #[must_use]
    pub fn sample_local_trs(
        &self,
        data: &[u8],
        clip: &Clip,
        target_index: u16,
        frame: f32,
        rest: crate::g4sk::LocalTrs,
    ) -> Option<crate::g4sk::LocalTrs> {
        if clip.is_additive() || !frame.is_finite() {
            return None;
        }
        let start = clip.target_info_start as usize;
        let end = start + clip.target_info_count as usize;
        let info = self
            .target_infos
            .get(start..end)?
            .iter()
            .find(|t| t.target_index == target_index)?;
        let cs = info.channel_start as usize;
        let ce = cs + info.channel_count as usize;
        let mut pose = rest;
        for channel in self.channels.get(cs..ce)? {
            let value = self.sample_channel(data, channel, frame)?;
            if !value.iter().all(|v| v.is_finite()) {
                return None;
            }
            match channel.channel_type {
                1..=3 => pose.scale[channel.channel_type as usize - 1] = value[0],
                9 => pose.quat = normalize_quat(value),
                10..=12 => pose.translation[channel.channel_type as usize - 10] = value[0],
                _ => {}
            }
        }
        Some(pose)
    }
}

/// Résout chaque hash de cible ([`Motion::target_hashes`]) contre une liste de noms d'os (ordre =
/// index de squelette), via le même CRC32 que [`crate::cfgbin::crc32`]. `bone_names[i]` doit être
/// le nom de l'os d'index `i` (ex. `G4skBones::bones[i].name`).
#[must_use]
pub fn resolve_targets(target_hashes: &[u32], bone_names: &[&str]) -> Vec<Option<usize>> {
    target_hashes
        .iter()
        .map(|&hash| {
            bone_names
                .iter()
                .position(|name| crate::cfgbin::crc32(name.as_bytes()) == hash)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_synthetique() {
        let mut buf = [0u8; HEADER_LEN];
        buf[0..4].copy_from_slice(b"G4MT");
        buf[4..6].copy_from_slice(&0x0040u16.to_le_bytes());
        buf[6..8].copy_from_slice(&0x0068u16.to_le_bytes());
        buf[10..12].copy_from_slice(&0x0010u16.to_le_bytes());
        buf[12..16].copy_from_slice(&0u32.to_le_bytes());
        let g = parse(&buf).expect("parse");
        assert_eq!(g.header.magic, MAGIC);
        assert_eq!(g.header.header_size, 0x40);
        assert_eq!(g.header.type_id, 0x68);
        assert!(g.is_size_consistent());
    }

    #[test]
    fn rejette_magic_et_court() {
        assert!(matches!(
            parse(&[0u8; HEADER_LEN]),
            Err(FormatError::BadMagic { .. })
        ));
        assert!(matches!(parse(b"G4MT"), Err(FormatError::TooShort { .. })));
        assert!(is_g4mt(b"G4MT____"));
        assert!(!is_g4mt(b"G4CM"));
    }

    /// Golden sur de VRAIS `.g4mt` du VFS (matériaux chr / motion d'événement).
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_g4mt_reels() {
        for (bytes, size) in [
            (
                include_bytes!("../tests/fixtures/g4mt/small.g4mt").as_slice(),
                2176usize,
            ),
            (
                include_bytes!("../tests/fixtures/g4mt/med.g4mt").as_slice(),
                41280usize,
            ),
        ] {
            let g = parse(bytes).expect("g4mt réel");
            assert_eq!(&g.header.magic.to_le_bytes(), b"G4MT");
            assert_eq!(g.header.header_size, 64);
            assert_eq!(g.header.type_id, 0x68);
            assert_eq!(g.file_size, size);
            assert!(g.is_size_consistent());
        }
    }

    /// Un `Motion` synthétique minimal (1 clip, 1 cible, 1 canal de rotation à 2 clés) : construit
    /// à la main en respectant le layout structurel (chaque section à un offset absolu fixe,
    /// `offset_shift=0` pour que `units == offset/4 - header_words` partout), décodé et
    /// échantillonné à mi-chemin.
    ///
    /// Layout : header(0x00..0x40) clip_rows(0x40..0x50) scales(∅) clip_hash(0x50..0x54)
    /// target_hash(0x54..0x58) clip_names(0x58..0x60) target_info(0x60..0x68) channel(0x68..0x7C)
    /// keys(0x7C..0x80) data(0x80..0x90).
    #[test]
    fn motion_synthetique_un_canal_rotation() {
        const HEADER_WORDS: u16 = 0x10; // 0x40 / 4
        const OFF_CLIP_HASH: usize = 0x50;
        const OFF_TARGET_HASH: usize = 0x54;
        const OFF_CLIP_NAMES: usize = 0x58;
        const OFF_TARGET_INFO: usize = 0x60;
        const OFF_CHANNEL: usize = 0x68;
        const OFF_KEYS: usize = 0x7C;
        const OFF_DATA: usize = 0x80;

        let mut buf = alloc::vec![0u8; OFF_DATA + 16];
        buf[0..4].copy_from_slice(b"G4MT");
        buf[4..6].copy_from_slice(&0x40u16.to_le_bytes()); // header_size
        buf[6..8].copy_from_slice(&0x68u16.to_le_bytes()); // type_id
        buf[10..12].copy_from_slice(&HEADER_WORDS.to_le_bytes());
        buf[0x20..0x22].copy_from_slice(&1u16.to_le_bytes()); // clip_count
        buf[0x22..0x24].copy_from_slice(&1u16.to_le_bytes()); // target_count
        let units = |off: usize| -> u16 { (off / 4 - HEADER_WORDS as usize) as u16 };
        buf[0x24..0x26].copy_from_slice(&units(OFF_TARGET_INFO).to_le_bytes());
        buf[0x26..0x28].copy_from_slice(&units(OFF_CHANNEL).to_le_bytes());
        buf[0x28..0x2A].copy_from_slice(&units(OFF_CLIP_HASH).to_le_bytes()); // scale (vide)
        buf[0x2A..0x2C].copy_from_slice(&units(OFF_CLIP_HASH).to_le_bytes());
        buf[0x2C..0x2E].copy_from_slice(&units(OFF_TARGET_HASH).to_le_bytes());
        buf[0x2E..0x30].copy_from_slice(&units(OFF_CLIP_NAMES).to_le_bytes());
        buf[0x30..0x32].copy_from_slice(&units(OFF_KEYS).to_le_bytes());
        buf[0x32..0x34].copy_from_slice(&units(OFF_DATA).to_le_bytes());
        buf[0x36] = 0; // offset_shift

        // clip row @0x40 : start=0 end=1 ti_start=0 ti_count=1 flags=0 fps=60.
        buf[0x40..0x42].copy_from_slice(&0u16.to_le_bytes());
        buf[0x42..0x44].copy_from_slice(&1u16.to_le_bytes());
        buf[0x44..0x46].copy_from_slice(&0u16.to_le_bytes());
        buf[0x46..0x48].copy_from_slice(&1u16.to_le_bytes());
        buf[0x48] = 0;
        buf[0x49] = 60;

        buf[OFF_CLIP_HASH..OFF_CLIP_HASH + 4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let target_hash = crate::cfgbin::crc32(b"root");
        buf[OFF_TARGET_HASH..OFF_TARGET_HASH + 4].copy_from_slice(&target_hash.to_le_bytes());
        // Table de noms de clip : offset[0]=2 (relatif à OFF_CLIP_NAMES) → cstr "clip".
        buf[OFF_CLIP_NAMES..OFF_CLIP_NAMES + 2].copy_from_slice(&2u16.to_le_bytes());
        buf[OFF_CLIP_NAMES + 2..OFF_CLIP_NAMES + 7].copy_from_slice(b"clip\0");

        // target_info @OFF_TARGET_INFO : target_index=0 channel_start=0 channel_count=1 reserved.
        buf[OFF_TARGET_INFO..OFF_TARGET_INFO + 2].copy_from_slice(&0u16.to_le_bytes());
        buf[OFF_TARGET_INFO + 2..OFF_TARGET_INFO + 4].copy_from_slice(&0u16.to_le_bytes());
        buf[OFF_TARGET_INFO + 4] = 1;
        buf[OFF_TARGET_INFO + 5] = 0;

        // channel @OFF_CHANNEL : type=9(rotation) codec=1 step=1(non-step) variant=2(i16)
        // n_comp=4 stride=8 scale_idx=0 pad=0, key_start=0 data_offset=0 key_count=2.
        buf[OFF_CHANNEL..OFF_CHANNEL + 8].copy_from_slice(&[9, 1, 1, 2, 4, 8, 0, 0]);
        buf[OFF_CHANNEL + 8..OFF_CHANNEL + 12].copy_from_slice(&0u32.to_le_bytes());
        buf[OFF_CHANNEL + 12..OFF_CHANNEL + 16].copy_from_slice(&0u32.to_le_bytes());
        buf[OFF_CHANNEL + 16..OFF_CHANNEL + 20].copy_from_slice(&2u32.to_le_bytes());

        // keys @OFF_KEYS : frames 0 et 1.
        buf[OFF_KEYS..OFF_KEYS + 2].copy_from_slice(&0u16.to_le_bytes());
        buf[OFF_KEYS + 2..OFF_KEYS + 4].copy_from_slice(&1u16.to_le_bytes());

        // data @OFF_DATA : clé 0 = identité (0,0,0,1) ; clé 1 = 180° autour de X.
        for (i, v) in [0i16, 0, 0, 32767].into_iter().enumerate() {
            buf[OFF_DATA + i * 2..OFF_DATA + i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        let half = (32767.0f32 * core::f32::consts::FRAC_1_SQRT_2) as i16;
        for (i, v) in [half, 0, 0, half].into_iter().enumerate() {
            buf[OFF_DATA + 8 + i * 2..OFF_DATA + 8 + i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }

        let mut motion = Motion::parse(&buf).expect("motion synthétique");
        assert_eq!(motion.clips.len(), 1);
        assert_eq!(motion.target_hashes, alloc::vec![target_hash]);
        let clip = &motion.clips[0];
        assert_eq!(clip.frame_count(), 2);
        let targets = motion.target_indices(clip);
        assert_eq!(targets, alloc::vec![0]);

        let bones = ["root"];
        let resolved = resolve_targets(&motion.target_hashes, &bones);
        assert_eq!(resolved, alloc::vec![Some(0)]);

        let q0 = motion.sample_rotation(&buf, clip, 0, 0.0).expect("q0");
        assert!((q0[3] - 1.0).abs() < 0.01, "q0 ≈ identité : {q0:?}");
        let q_mid = motion.sample_rotation(&buf, clip, 0, 0.5).expect("q_mid");
        let n =
            (q_mid[0] * q_mid[0] + q_mid[1] * q_mid[1] + q_mid[2] * q_mid[2] + q_mid[3] * q_mid[3])
                .sqrt();
        assert!((n - 1.0).abs() < 0.01, "q_mid normalisé : {n}");

        let rest = crate::g4sk::LocalTrs {
            scale: [2.0, 3.0, 4.0],
            quat: [0.0, 0.0, 0.0, 1.0],
            translation: [5.0, 6.0, 7.0],
        };
        let local = motion.sample_local_trs(&buf, clip, 0, 0.5, rest).unwrap();
        assert_eq!(local.quat, q_mid);
        assert_eq!(local.translation, rest.translation);
        assert_eq!(local.scale, rest.scale);
        assert!(
            motion
                .sample_local_trs(&buf, clip, 0, f32::NAN, rest)
                .is_none()
        );

        // Un canal scalaire ne normalise pas sa valeur comme un quaternion ; les axes
        // non animés restent ceux de repos, y compris pour translation et échelle.
        motion.channels[0].channel_type = 11;
        motion.channels[0].variant = 4;
        motion.channels[0].n_comp = 1;
        motion.channels[0].stride = 4;
        buf[OFF_DATA..OFF_DATA + 4].copy_from_slice(&(-2.0f32).to_le_bytes());
        buf[OFF_DATA + 4..OFF_DATA + 8].copy_from_slice(&6.0f32.to_le_bytes());
        let clip = &motion.clips[0];
        let local = motion.sample_local_trs(&buf, clip, 0, 0.5, rest).unwrap();
        assert_eq!(local.translation[0], 5.0);
        assert_eq!(local.translation[1], 2.0);
        assert_eq!(local.translation[2], 7.0);
        assert_eq!(local.quat, rest.quat);
        motion.channels[0].channel_type = 3;
        let local = motion
            .sample_local_trs(&buf, &motion.clips[0], 0, 1.0, rest)
            .unwrap();
        assert_eq!(&local.scale[..2], &[2.0, 3.0]);
        assert_eq!(local.scale[2], 6.0);
        motion.clips[0].flags = 1;
        assert!(
            motion
                .sample_local_trs(&buf, &motion.clips[0], 0, 0.0, rest)
                .is_none()
        );
    }

    /// Golden croisé contre l'implémentation Python indépendante (`plugins/niers-blender`, submodule)
    /// sur le VRAI conteneur multi-clips `c000101_p250.g4mt` (37 clips, jusqu'à 383 frames) + son
    /// G4SK compagnon — le cas exact que l'ancien scan à offsets fixes ne pouvait pas décoder.
    #[cfg(feature = "real-fixtures")]
    #[test]
    fn golden_g4mt_motion_multiclip_reel() {
        let data = include_bytes!("../tests/fixtures/g4mt/long_multiclip.g4mt").as_slice();
        let sk = include_bytes!("../tests/fixtures/g4mt/long_multiclip.g4sk").as_slice();
        let motion = Motion::parse(data).expect("motion réelle");
        assert_eq!(motion.clips.len(), 37);
        assert_eq!(motion.target_hashes.len(), 156);

        let header = crate::g4sk::parse_header(sk).expect("g4sk header");
        let bones = crate::g4sk::parse_hierarchy(sk, &header);
        let bone_names: Vec<&str> = bones.bones.iter().map(|b| b.name.as_str()).collect();
        let resolved = resolve_targets(&motion.target_hashes, &bone_names);
        assert!(
            resolved.iter().filter(|r| r.is_some()).count() > 100,
            "la majorité des cibles doit se résoudre contre le G4SK réel"
        );

        // Clip 6 = "立ち会話控えめ1L", 383 frames (0..=382) — le cas long que l'ancien parseur ratait.
        let clip = &motion.clips[6];
        assert_eq!(clip.frame_count(), 383);

        // Chaque quaternion échantillonné sur toute la plage doit rester normalisé (validation
        // structurelle : bon offset de données ⇒ décodage plausible, pas des octets aléatoires).
        let targets = motion.target_indices(clip);
        assert!(!targets.is_empty());
        let mut sampled = 0usize;
        for &t in targets.iter().take(20) {
            for frame in [0.0f32, 100.0, 200.0, 382.0] {
                if let Some(q) = motion.sample_rotation(data, clip, t, frame) {
                    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                    assert!(
                        (n - 1.0).abs() < 0.01,
                        "quaternion non-unitaire cible={t} frame={frame} n={n}"
                    );
                    sampled += 1;
                }
            }
        }
        assert!(
            sampled > 20,
            "trop peu de rotations résolues sur le clip long ({sampled})"
        );
    }
}
