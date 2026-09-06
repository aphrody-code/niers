//! Parseur **GLB** (glTF binaire 2.0) minimal — extrait la géométrie (positions/normales/UV/indices)
//! **et les textures** (PNG embarqués) des primitives de mesh. Cible : les GLB produits par
//! `nie_formats::assemble::to_glb_embedded` (positions déjà en espace monde) ; on ignore donc les
//! transforms de nœuds. La chaîne matériau→texture est résolue
//! (`primitive.material → materials[].baseColorTexture → textures[].source → images[]`).

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Une texture décodée en RGBA8 (atlas du modèle : corps, visage, uniforme…).
#[derive(Clone)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Une primitive de mesh : triangles indexés, positions + normales + UV (espace monde),
/// et l'indice de sa texture dans [`Model::textures`] (s'il y en a une).
#[derive(Clone)]
pub struct Primitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub texture: Option<usize>,
}

/// Modèle GLB chargé : toutes les primitives + les atlas de textures décodés.
#[derive(Clone)]
pub struct Model {
    pub primitives: Vec<Primitive>,
    pub textures: Vec<Texture>,
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Décode un PNG (n'importe quel type de couleur) en RGBA8.
fn decode_png(bytes: &[u8]) -> Result<Texture> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().context("png read_info")?;
    let bufsz = reader
        .output_buffer_size()
        .context("png output_buffer_size (overflow)")?;
    let mut buf = vec![0u8; bufsz];
    let info = reader.next_frame(&mut buf).context("png next_frame")?;
    let (w, h) = (info.width, info.height);
    let n = (w as usize) * (h as usize);
    let mut rgba = vec![0u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(&buf[..n * 4]),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4] = buf[i * 3];
                rgba[i * 4 + 1] = buf[i * 3 + 1];
                rgba[i * 4 + 2] = buf[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                let g = buf[i];
                rgba[i * 4] = g;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = g;
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                let g = buf[i * 2];
                rgba[i * 4] = g;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = g;
                rgba[i * 4 + 3] = buf[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => bail!("PNG indexé non normalisé"),
    }
    Ok(Texture {
        width: w,
        height: h,
        rgba,
    })
}

/// `primitive.material → materials[m].pbr.baseColorTexture.index → textures[t].source` (indice image).
fn material_image(root: &Value, mat_idx: usize) -> Option<usize> {
    let m = root["materials"].as_array()?.get(mat_idx)?;
    let t = m["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64()? as usize;
    let src = root["textures"].as_array()?.get(t)?["source"].as_u64()? as usize;
    Some(src)
}

/// Parse un buffer GLB complet (géométrie + textures embarquées).
///
/// # Errors
/// Si le magic n'est pas « glTF », si les chunks JSON/BIN manquent, ou si le glTF est malformé.
pub fn parse(data: &[u8]) -> Result<Model> {
    if data.len() < 12 || &data[0..4] != b"glTF" {
        bail!("pas un GLB (magic 'glTF' absent)");
    }
    // En-tête 12 o, puis chunks {len u32, type u32, data}.
    let mut json: Option<&[u8]> = None;
    let mut bin: Option<&[u8]> = None;
    let mut off = 12usize;
    while off + 8 <= data.len() {
        let clen = rd_u32(data, off) as usize;
        let ctype = rd_u32(data, off + 4);
        let start = off + 8;
        let end = start.checked_add(clen).context("chunk hors limites")?;
        if end > data.len() {
            bail!("chunk GLB hors limites");
        }
        match ctype {
            0x4E4F_534A => json = Some(&data[start..end]), // "JSON"
            0x004E_4942 => bin = Some(&data[start..end]),  // "BIN\0"
            _ => {}
        }
        off = end;
    }
    let json = json.context("chunk JSON absent")?;
    let bin = bin.context("chunk BIN absent")?;
    let root: Value = serde_json::from_slice(json).context("JSON glTF invalide")?;

    let accessors = root["accessors"].as_array().context("accessors absents")?;
    let views = root["bufferViews"]
        .as_array()
        .context("bufferViews absents")?;

    // Lit un accessor scalaire/vecteur en f32 (composantes consécutives).
    let read_floats = |acc_idx: usize, ncomp: usize| -> Result<Vec<f32>> {
        let acc = accessors.get(acc_idx).context("accessor hors limites")?;
        let bv = views
            .get(acc["bufferView"].as_u64().context("bufferView")? as usize)
            .context("bufferView hors limites")?;
        let comp_ty = acc["componentType"].as_u64().context("componentType")?;
        let count = acc["count"].as_u64().context("count")? as usize;
        let view_start = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let view_end = view_start
            .checked_add(bv["byteLength"].as_u64().context("byteLength")? as usize)
            .context("bufferView déborde")?;
        let base = view_start
            .checked_add(acc["byteOffset"].as_u64().unwrap_or(0) as usize)
            .context("offset accessor déborde")?;
        let comp_sz = match comp_ty {
            5126 => 4,
            5125 => 4,
            5123 => 2,
            5121 => 1,
            other => bail!("componentType {other} non géré"),
        };
        let stride = bv["byteStride"]
            .as_u64()
            .map(|s| s as usize)
            .unwrap_or(ncomp * comp_sz);
        let element_size = ncomp * comp_sz;
        let end = if count == 0 {
            base
        } else {
            (count - 1)
                .checked_mul(stride)
                .and_then(|n| base.checked_add(n))
                .and_then(|n| n.checked_add(element_size))
                .context("taille accessor déborde")?
        };
        if stride < element_size || end > view_end || view_end > bin.len() {
            bail!("accessor hors du bufferView ou du chunk BIN");
        }
        let mut out = Vec::with_capacity(count * ncomp);
        for i in 0..count {
            let p = base + i * stride;
            for c in 0..ncomp {
                let q = p + c * comp_sz;
                let v = match comp_ty {
                    5126 => f32::from_le_bytes([bin[q], bin[q + 1], bin[q + 2], bin[q + 3]]),
                    5125 => rd_u32(bin, q) as f32,
                    5123 => u16::from_le_bytes([bin[q], bin[q + 1]]) as f32,
                    5121 => f32::from(bin[q]),
                    _ => unreachable!(),
                };
                out.push(v);
            }
        }
        Ok(out)
    };

    // Décode les atlas PNG embarqués (indexés par numéro d'image glTF).
    let mut textures = Vec::new();
    if let Some(imgs) = root["images"].as_array() {
        for im in imgs {
            let bvi = im["bufferView"].as_u64().context("image bufferView")? as usize;
            let bv = views.get(bvi).context("image bufferView hors limites")?;
            let bo = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
            let bl = bv["byteLength"].as_u64().context("image byteLength")? as usize;
            let end = bo.checked_add(bl).context("image déborde")?;
            textures.push(decode_png(
                bin.get(bo..end).context("image hors du chunk BIN")?,
            )?);
        }
    }

    let mut primitives = Vec::new();
    let empty = Vec::new();
    for mesh in root["meshes"].as_array().unwrap_or(&empty) {
        for prim in mesh["primitives"].as_array().unwrap_or(&empty) {
            let attrs = &prim["attributes"];
            let Some(pos_acc) = attrs["POSITION"].as_u64() else {
                continue;
            };
            let pf = read_floats(pos_acc as usize, 3)?;
            let positions: Vec<[f32; 3]> = pf.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            let normals: Vec<[f32; 3]> = match attrs["NORMAL"].as_u64() {
                Some(a) => read_floats(a as usize, 3)?
                    .chunks_exact(3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect(),
                None => Vec::new(),
            };
            let uv: Vec<[f32; 2]> = match attrs["TEXCOORD_0"].as_u64() {
                Some(a) => read_floats(a as usize, 2)?
                    .chunks_exact(2)
                    .map(|c| [c[0], c[1]])
                    .collect(),
                None => Vec::new(),
            };
            let indices: Vec<u32> = match prim["indices"].as_u64() {
                Some(a) => read_floats(a as usize, 1)?
                    .iter()
                    .map(|&f| f as u32)
                    .collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let texture = prim["material"]
                .as_u64()
                .and_then(|mi| material_image(&root, mi as usize))
                .filter(|&src| src < textures.len());
            if indices.iter().any(|&i| i as usize >= positions.len()) {
                bail!("indice de sommet hors limites");
            }
            primitives.push(Primitive {
                positions,
                normals,
                uv,
                indices,
                texture,
            });
        }
    }
    if primitives.is_empty() {
        bail!("aucune primitive de mesh dans le GLB");
    }
    Ok(Model {
        primitives,
        textures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: Value) -> Vec<u8> {
        let mut json = serde_json::to_vec(&root).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut data = b"glTF".to_vec();
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&((12 + 8 + json.len() + 8 + 12) as u32).to_le_bytes());
        data.extend_from_slice(&(json.len() as u32).to_le_bytes());
        data.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        data.extend(json);
        data.extend_from_slice(&12u32.to_le_bytes());
        data.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        data.extend_from_slice(&[0; 12]);
        data
    }

    #[test]
    fn rejette_accessors_et_images_hors_limites_sans_panique() {
        let root = serde_json::json!({
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3"}],
            "bufferViews": [{"byteLength": 12}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}]
        });
        assert!(parse(&fixture(root.clone())).is_ok());
        for (pointer, value) in [
            ("/meshes/0/primitives/0/attributes/POSITION", 99),
            ("/accessors/0/bufferView", 99),
            ("/accessors/0/count", 2),
            ("/bufferViews/0/byteLength", 4),
        ] {
            let mut invalid = root.clone();
            *invalid.pointer_mut(pointer).unwrap() = serde_json::json!(value);
            assert!(parse(&fixture(invalid)).is_err(), "{pointer}");
        }
        let mut invalid = root;
        invalid["images"] = serde_json::json!([{"bufferView": 99}]);
        assert!(parse(&fixture(invalid)).is_err());
    }

    #[test]
    fn rejette_non_glb() {
        assert!(parse(b"pas un glb").is_err());
        assert!(parse(b"glTF").is_err()); // magic mais trop court
    }
}
