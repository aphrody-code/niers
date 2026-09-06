//! Indexation de tout le savoir caméra dans la base de connaissance `var/niers.sqlite`.
//!
//! Remplit les tables `cam_*` créées par la migration `nie_index::CAMERA_SCHEMA` :
//!
//! | Source | Ce qui est indexé |
//! |---|---|
//! | [`index_map`] | contrôleurs RTTI, dispatchers Lua, symboles RE, commandes d'entrée, caméras nommées |
//! | [`index_binary_params`] | les noms de paramètres caméra de `.rdata`, classés par domaine |
//! | [`index_assets`] | les fichiers de données caméra du VFS (présence, taille, sha256, format) |
//! | [`index_soccer_config`] | les 11 listes de `soccer_camera_config` |
//! | [`index_property`] | les presets de contrôleur, valeurs déclarées **et** effectives |
//! | [`index_anims`] | les 1 215 `.g4cm` : fichiers, objets, canaux, et les échantillons sur demande |
//!
//! ## Idempotence
//!
//! Chaque passe purge ce qu'elle a produit précédemment (par `cam_source` ou par asset)
//! avant de réinsérer : réindexer deux fois de suite donne le même contenu, et réindexer
//! après une mise à jour du jeu remplace proprement l'ancien état.

use std::collections::BTreeMap;
use std::path::Path;

use nie_index::rusqlite::{Connection, params};

use crate::config::SoccerCameraConfig;
use crate::g4cm::{self, Track};
use crate::map;
use crate::model::CtrlKind;
use crate::property::{ParamValue, PropertySet};

/// Erreurs d'indexation.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// Échec SQLite.
    #[error("sqlite : {0}")]
    Sqlite(#[from] nie_index::rusqlite::Error),
    /// Échec d'ouverture de la base.
    #[error("base de connaissance : {0}")]
    Index(#[from] nie_index::IndexError),
    /// Échec de décodage d'un fichier caméra.
    #[error("{path} : {source}")]
    Decode {
        /// Fichier en cause.
        path: String,
        /// Cause.
        source: crate::CameraError,
    },
}

/// Résultat d'indexation.
pub type Result<T> = core::result::Result<T, DbError>;

/// Compteurs d'une passe d'indexation, pour rendre compte de ce qui a été écrit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexStats {
    /// Contrôleurs RTTI.
    pub ctrl_classes: usize,
    /// Dispatchers `funcLua*`.
    pub dispatchers: usize,
    /// Symboles RE.
    pub re_symbols: usize,
    /// Noms de paramètres.
    pub params: usize,
    /// Commandes d'entrée + caméras nommées.
    pub symbols: usize,
    /// Fichiers de données référencés.
    pub assets: usize,
    /// Lignes de `soccer_camera_config`.
    pub config_rows: usize,
    /// Presets de contrôleur.
    pub presets: usize,
    /// Paramètres de preset (déclarés + hérités).
    pub preset_params: usize,
    /// Animations `.g4cm`.
    pub anims: usize,
    /// Canaux d'animation.
    pub channels: usize,
    /// Échantillons de keyframes.
    pub samples: usize,
}

impl IndexStats {
    /// Somme des deux passes.
    #[must_use]
    pub fn merged(self, other: IndexStats) -> IndexStats {
        IndexStats {
            ctrl_classes: self.ctrl_classes + other.ctrl_classes,
            dispatchers: self.dispatchers + other.dispatchers,
            re_symbols: self.re_symbols + other.re_symbols,
            params: self.params + other.params,
            symbols: self.symbols + other.symbols,
            assets: self.assets + other.assets,
            config_rows: self.config_rows + other.config_rows,
            presets: self.presets + other.presets,
            preset_params: self.preset_params + other.preset_params,
            anims: self.anims + other.anims,
            channels: self.channels + other.channels,
            samples: self.samples + other.samples,
        }
    }
}

/// Ouvre la base et applique la migration caméra.
///
/// # Errors
/// [`DbError::Index`] si la base ne peut pas être ouverte ou la migration appliquée.
pub fn open(path: impl AsRef<Path>) -> Result<nie_index::Db> {
    let db = nie_index::Db::open(path)?;
    db.init_camera()?;
    Ok(db)
}

/// Enregistre (ou retrouve) une source d'indexation et rend son identifiant.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
pub fn upsert_source(
    conn: &Connection,
    kind: &str,
    label: &str,
    sha256: Option<&str>,
    size: Option<u64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO cam_source(kind, label, sha256, size) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(kind, label) DO UPDATE SET
             sha256 = excluded.sha256,
             size = excluded.size,
             indexed_at = datetime('now')",
        params![kind, label, sha256, size.map(|s| s as i64)],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM cam_source WHERE kind = ?1 AND label = ?2",
        params![kind, label],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Indexe la carte statique du reverse : contrôleurs, dispatchers, symboles, listes.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
pub fn index_map(conn: &Connection, source_id: i64) -> Result<IndexStats> {
    let mut st = IndexStats::default();

    conn.execute(
        "DELETE FROM cam_ctrl_class WHERE source_id = ?1",
        params![source_id],
    )?;
    // Deux passes : les classes d'abord sans parent (la colonne `base` se référence
    // elle-même), puis on renseigne le parent une fois toutes les lignes présentes.
    for k in CtrlKind::ALL {
        let short = format!("{k:?}");
        let depth = {
            let (mut d, mut c) = (0i64, k);
            while let Some(b) = c.base() {
                c = b;
                d += 1;
            }
            d
        };
        conn.execute(
            "INSERT INTO cam_ctrl_class(source_id, cpp_name, short_name, depth, ported)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(cpp_name) DO UPDATE SET
                 source_id = excluded.source_id, short_name = excluded.short_name,
                 depth = excluded.depth, ported = excluded.ported",
            params![
                source_id,
                k.cpp_name(),
                short,
                depth,
                i64::from(k.is_ported())
            ],
        )?;
        st.ctrl_classes += 1;
    }
    for k in CtrlKind::ALL {
        conn.execute(
            "UPDATE cam_ctrl_class SET base = ?2 WHERE cpp_name = ?1",
            params![k.cpp_name(), k.base().map(CtrlKind::cpp_name)],
        )?;
    }

    conn.execute(
        "DELETE FROM cam_dispatcher WHERE source_id = ?1",
        params![source_id],
    )?;
    for d in map::DISPATCHERS {
        conn.execute(
            "INSERT INTO cam_dispatcher(source_id, name, table_va, cmd_count, is_camera)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![
                source_id,
                d.name,
                d.table_va as i64,
                i64::from(d.count),
                i64::from(d.name == map::CAMERA_DISPATCHER.name)
            ],
        )?;
        st.dispatchers += 1;
    }

    conn.execute(
        "DELETE FROM cam_re_symbol WHERE source_id = ?1",
        params![source_id],
    )?;
    let symbols: [(&str, u64, &str, &str); 7] = [
        (
            "funcLuaCameraCommand.string",
            map::CAMERA_DISPATCHER_NAME_VA,
            "string",
            "chaîne du nom exposé à Lua",
        ),
        (
            "funcLuaCameraCommand.entry",
            map::CAMERA_DISPATCHER_ENTRY_VA,
            "entry",
            "lua_CFunction du dispatcher caméra",
        ),
        (
            "funcLuaCameraCommand.alt",
            map::CAMERA_DISPATCHER_ALT_VA,
            "entry",
            "variante interne, même table",
        ),
        (
            "funcLua.dispatch",
            map::DISPATCH_ROUTINE_VA,
            "routine",
            "recherche dichotomique partagée par les 15 dispatchers",
        ),
        (
            "funcLua.pool",
            map::FUNCLUA_POOL_VA,
            "table",
            "réservoir global des commandes (non segmenté par dispatcher)",
        ),
        (
            "g4.loader",
            map::G4_LOADER_VA,
            "loader",
            "loader générique des conteneurs G4 — fixe la formule d'offsets du G4CM",
        ),
        (
            "g4.magic_table",
            map::G4_MAGIC_TABLE_VA,
            "table",
            "G4MT G4MA G4TP G4CM G4VS G4LA G4BA",
        ),
    ];
    for (name, va, kind, note) in symbols {
        let off = map::va_to_file_offset(va);
        conn.execute(
            "INSERT INTO cam_re_symbol(source_id, name, va, file_offset, kind, note)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                source_id,
                name,
                va as i64,
                off.map(|o| o as i64),
                kind,
                note
            ],
        )?;
        st.re_symbols += 1;
    }

    conn.execute(
        "DELETE FROM cam_symbol_list WHERE source_id = ?1",
        params![source_id],
    )?;
    for c in map::INPUT_COMMANDS {
        conn.execute(
            "INSERT INTO cam_symbol_list(source_id, kind, name) VALUES(?1, 'input_command', ?2)",
            params![source_id, c],
        )?;
        st.symbols += 1;
    }
    for c in map::SCENE_CAMERAS {
        conn.execute(
            "INSERT INTO cam_symbol_list(source_id, kind, name) VALUES(?1, 'scene_camera', ?2)",
            params![source_id, c],
        )?;
        st.symbols += 1;
    }
    Ok(st)
}

/// Classe un nom de paramètre caméra dans un domaine.
///
/// Heuristique lexicale assumée : elle sert à naviguer (`WHERE domain = 'shake'`), pas à
/// établir un fait. Les règles suivent les familles observées dans `.rdata`.
#[must_use]
pub fn classify_param(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("shake") {
        "shake"
    } else if n.contains("goalnet") || n.contains("goal") {
        "goal"
    } else if n.contains("fade") || n.contains("clip") || n.contains("near") || n.contains("far") {
        "fade"
    } else if n.contains("posteffect") {
        "posteffect"
    } else if n.contains("touch")
        || n.contains("mouse")
        || n.contains("pad")
        || n.contains("keyboard")
        || n.contains("speed")
        || n.contains("reverse")
        || n.contains("revrse")
        || n.contains("adjust")
    {
        "input"
    } else if n.contains("photo") || n.contains("selfie") || n.contains("screenshot") {
        "photo"
    } else if n.contains("popup") {
        "hud"
    } else if n.contains("chase") {
        "chase"
    } else if n.contains("shoot") {
        "shoot"
    } else if n.contains("coachai") || n.contains("training") {
        "coach"
    } else if n.contains("summon") || n.contains("zone") || n.contains("scramble") {
        "production"
    } else if n.contains("event") || n.contains("ev") && n.starts_with("ev") {
        "event"
    } else if n.contains("soccer") || n.contains("battle") {
        "match"
    } else {
        "autre"
    }
}

/// Extrait de `nie.exe` les noms de paramètres caméra et les indexe.
///
/// Balaie les chaînes ASCII imprimables du binaire, retient celles qui contiennent
/// `amera`, écarte les phrases (espaces) et les chemins, puis classe par domaine.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
pub fn index_binary_params(conn: &Connection, source_id: i64, exe: &[u8]) -> Result<IndexStats> {
    conn.execute(
        "DELETE FROM cam_param WHERE source_id = ?1",
        params![source_id],
    )?;
    let mut st = IndexStats::default();
    let mut stmt = conn.prepare(
        "INSERT INTO cam_param(source_id, name, va, section, domain) VALUES(?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_id, name) DO NOTHING",
    )?;

    let mut start: Option<usize> = None;
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for i in 0..=exe.len() {
        let printable = i < exe.len() && (0x20..0x7F).contains(&exe[i]);
        match (printable, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                start = None;
                if i - s < 4 || i - s > 96 {
                    continue;
                }
                let Ok(text) = core::str::from_utf8(&exe[s..i]) else {
                    continue;
                };
                if !text.contains("amera") || text.contains(' ') || text.contains('/') {
                    continue;
                }
                if !seen.insert(text) {
                    continue;
                }
                // L'offset fichier remonte vers une VA seulement pour les sections mappées.
                let va = section_of_offset(s);
                stmt.execute(params![
                    source_id,
                    text,
                    va.map(|(v, _)| v as i64),
                    va.map(|(_, sec)| sec),
                    classify_param(text)
                ])?;
                st.params += 1;
            }
            _ => {}
        }
    }
    Ok(st)
}

/// Section et VA correspondant à un offset fichier de `nie.exe` (build cartographié).
fn section_of_offset(off: usize) -> Option<(u64, &'static str)> {
    const SECTIONS: [(&str, u64, u64, u64); 4] = [
        (".text", 0x1_4000_1000, 0x400, 0x186_A800),
        (".rdata", 0x1_4186_C000, 0x186_AC00, 0x43_2200),
        (".data", 0x1_41C9_F000, 0x1C9_CE00, 0x24_D400),
        (".rsrc", 0x1_4278_8000, 0x201_8A00, 0xF200),
    ];
    let off = off as u64;
    for (name, va, raw, size) in SECTIONS {
        if off >= raw && off < raw + size {
            return Some((va + (off - raw), name));
        }
    }
    None
}

/// Enregistre un fichier de données caméra et rend son `cam_asset.id`.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
pub fn upsert_asset(
    conn: &Connection,
    source_id: i64,
    path: &str,
    role: Option<&str>,
    bytes: Option<&[u8]>,
) -> Result<i64> {
    let format = bytes.and_then(detect_format);
    conn.execute(
        "INSERT INTO cam_asset(source_id, path, role, present, size, sha256, format)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(source_id, path) DO UPDATE SET
             role = COALESCE(excluded.role, cam_asset.role),
             present = excluded.present, size = excluded.size,
             sha256 = excluded.sha256, format = excluded.format",
        params![
            source_id,
            path,
            role,
            i64::from(bytes.is_some()),
            bytes.map(|b| b.len() as i64),
            bytes.map(sha256_hex),
            format
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM cam_asset WHERE source_id = ?1 AND path = ?2",
        params![source_id, path],
        |r| r.get(0),
    )?;
    Ok(id)
}

fn detect_format(bytes: &[u8]) -> Option<&'static str> {
    if g4cm::is_g4cm(bytes) {
        return Some("G4CM");
    }
    if nie_formats::cfgbin::is_rdbn(bytes) {
        return Some("RDBN");
    }
    if bytes.len() >= 16 {
        return Some("T2B");
    }
    None
}

/// SHA-256 hexadécimal d'un tampon.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        use core::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Indexe un `soccer_camera_config` déjà décodé.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
#[expect(
    clippy::too_many_lines,
    reason = "onze listes à écrire, chacune avec ses colonnes ; découper nuirait à la lisibilité"
)]
pub fn index_soccer_config(
    conn: &Connection,
    asset_id: i64,
    cfg: &SoccerCameraConfig,
) -> Result<IndexStats> {
    let mut st = IndexStats::default();
    for t in [
        "cam_soccer_data",
        "cam_soccer_ref",
        "cam_goalnet",
        "cam_aerial",
        "cam_aerial_map",
        "cam_dir",
        "cam_fixpos_data",
        "cam_cinematic_data",
        "cam_cinematic_situation",
    ] {
        conn.execute(
            &format!("DELETE FROM {t} WHERE asset_id = ?1"),
            params![asset_id],
        )?;
    }

    for (i, d) in cfg.camera_data.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_soccer_data(
                asset_id, row_idx, no, length, length_min, length_max,
                rot_x, rot_x_min, rot_x_max, rot_y, rot_y_min, rot_y_max,
                offence_offset_rot_y, defence_offset_rot_y, fov,
                ref_offset_x, ref_offset_y, ref_offset_z,
                off_ref_offset_x, off_ref_offset_y, off_ref_offset_z,
                def_ref_offset_x, def_ref_offset_y, def_ref_offset_z,
                ref_offset_length, move_interp_rate, rot_interp_rate, zoom_interp_rate)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,
                    ?19,?20,?21,?22,?23,?24,?25,?26,?27,?28)",
            params![
                asset_id,
                i as i64,
                i64::from(d.no),
                f64::from(d.length),
                f64::from(d.length_min),
                f64::from(d.length_max),
                f64::from(d.rot_x),
                f64::from(d.rot_x_min),
                f64::from(d.rot_x_max),
                f64::from(d.rot_y),
                f64::from(d.rot_y_min),
                f64::from(d.rot_y_max),
                f64::from(d.offence_offset_rot_y),
                f64::from(d.defence_offset_rot_y),
                f64::from(d.fov),
                f64::from(d.ref_offset[0]),
                f64::from(d.ref_offset[1]),
                f64::from(d.ref_offset[2]),
                f64::from(d.offence_ref_offset[0]),
                f64::from(d.offence_ref_offset[1]),
                f64::from(d.offence_ref_offset[2]),
                f64::from(d.defence_ref_offset[0]),
                f64::from(d.defence_ref_offset[1]),
                f64::from(d.defence_ref_offset[2]),
                f64::from(d.ref_offset_length),
                f64::from(d.move_interp_rate),
                f64::from(d.rot_interp_rate),
                f64::from(d.zoom_interp_rate)
            ],
        )?;
        st.config_rows += 1;
    }

    for (list, refs) in [
        ("m_soccerCameraInfoList", &cfg.cameras),
        ("m_soccerFixPosCameraInfoList", &cfg.fix_pos),
        ("m_cinematicCameraInfoList", &cfg.cinematic),
    ] {
        for (i, r) in refs.iter().enumerate() {
            conn.execute(
                "INSERT INTO cam_soccer_ref(asset_id, list_name, row_idx, cam_id,
                                            slice_offset, slice_count)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    asset_id,
                    list,
                    i as i64,
                    i64::from(r.id),
                    i64::from(r.slice[0]),
                    i64::from(r.slice[1])
                ],
            )?;
            st.config_rows += 1;
        }
    }

    for (i, g) in cfg.goalnet.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_goalnet(asset_id, row_idx, cam_id, pos_x, pos_y, pos_z,
                 ref_offset_x, ref_offset_y, ref_offset_z, fov, chase_max_speed,
                 not_follow_after_bouncing, fixed_ref_x, fixed_ref_y, fixed_ref_z,
                 init_ref_goal_line)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                asset_id,
                i as i64,
                i64::from(g.id),
                f64::from(g.cam_pos[0]),
                f64::from(g.cam_pos[1]),
                f64::from(g.cam_pos[2]),
                f64::from(g.ref_offset[0]),
                f64::from(g.ref_offset[1]),
                f64::from(g.ref_offset[2]),
                f64::from(g.fov),
                f64::from(g.chase_max_speed),
                i64::from(g.not_follow_after_bouncing),
                i64::from(g.fixed_ref[0]),
                i64::from(g.fixed_ref[1]),
                i64::from(g.fixed_ref[2]),
                i64::from(g.init_ref_goal_line)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, a) in cfg.aerial.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_aerial(asset_id, row_idx, cam_id, cam_length, pos_x, pos_y, pos_z,
                 rot_x_start, rot_x_end, rot_y_start, rot_y_end)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                asset_id,
                i as i64,
                i64::from(a.id),
                f64::from(a.cam_length),
                f64::from(a.cam_pos[0]),
                f64::from(a.cam_pos[1]),
                f64::from(a.cam_pos[2]),
                f64::from(a.rot_x.0),
                f64::from(a.rot_x.1),
                f64::from(a.rot_y.0),
                f64::from(a.rot_y.1)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, m) in cfg.aerial_map.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_aerial_map(asset_id, row_idx, map_id, aerial_cam_info_id,
                                        light_overwrite_id)
             VALUES(?1,?2,?3,?4,?5)",
            params![
                asset_id,
                i as i64,
                i64::from(m.id),
                i64::from(m.aerial_cam_info_id),
                i64::from(m.light_overwrite_id)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, d) in cfg.dir_cameras.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_dir(asset_id, row_idx, dir_cam_id, hor_cam_id, vert_cam_id)
             VALUES(?1,?2,?3,?4,?5)",
            params![
                asset_id,
                i as i64,
                i64::from(d.dir_cam_id),
                i64::from(d.hor_cam_id),
                i64::from(d.vert_cam_id)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, f) in cfg.fix_pos_data.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_fixpos_data(asset_id, row_idx, cam_id,
                 ref_offset_x, ref_offset_y, ref_offset_z,
                 cam_pos_offset_x, cam_pos_offset_y, cam_pos_offset_z,
                 move_vec_offset_x, move_vec_offset_y, move_vec_offset_z,
                 cam_roll, fov, offset_length, offset_time, condition_area_radius,
                 enable_interp, move_ref_pos_only, move_type, curvature)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![
                asset_id,
                i as i64,
                i64::from(f.id),
                f64::from(f.ref_offset[0]),
                f64::from(f.ref_offset[1]),
                f64::from(f.ref_offset[2]),
                f64::from(f.cam_pos_offset[0]),
                f64::from(f.cam_pos_offset[1]),
                f64::from(f.cam_pos_offset[2]),
                f64::from(f.move_vec_offset[0]),
                f64::from(f.move_vec_offset[1]),
                f64::from(f.move_vec_offset[2]),
                f64::from(f.cam_roll),
                f64::from(f.fov),
                f64::from(f.offset_length),
                f64::from(f.offset_time),
                f64::from(f.condition_area_radius),
                i64::from(f.enable_interp),
                i64::from(f.move_ref_pos_only),
                i64::from(f.move_type),
                f64::from(f.curvature)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, c) in cfg.cinematic_data.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_cinematic_data(asset_id, row_idx, weight, change_recast,
                 chase_camera_id, fix_camera_id)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                asset_id,
                i as i64,
                i64::from(c.weight),
                f64::from(c.change_recast),
                i64::from(c.chase_camera_id),
                i64::from(c.fix_camera_id)
            ],
        )?;
        st.config_rows += 1;
    }

    for (i, s) in cfg.cinematic_situations.iter().enumerate() {
        conn.execute(
            "INSERT INTO cam_cinematic_situation(asset_id, row_idx, situation_type,
                 slice_offset, slice_count)
             VALUES(?1,?2,?3,?4,?5)",
            params![
                asset_id,
                i as i64,
                i64::from(s.situation_type),
                i64::from(s.slice[0]),
                i64::from(s.slice[1])
            ],
        )?;
        st.config_rows += 1;
    }

    Ok(st)
}

/// Colonnes typées d'un paramètre de preset : `(ty, v_int, v_f0, v_f1, v_f2, v_text)`.
type ParamColumns<'a> = (
    &'static str,
    Option<i64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<&'a str>,
);

fn bind_param(
    conn: &Connection,
    preset_id: i64,
    name: &str,
    v: &ParamValue,
    inherited: bool,
    from: Option<&str>,
) -> Result<()> {
    let (ty, vi, f0, f1, f2, text): ParamColumns<'_> = match v {
        ParamValue::Int(i) => ("int", Some(i64::from(*i)), None, None, None, None),
        ParamValue::Float(f) => ("float", None, Some(f64::from(*f)), None, None, None),
        ParamValue::Vec3(v) => (
            "vec3",
            None,
            Some(f64::from(v[0])),
            Some(f64::from(v[1])),
            Some(f64::from(v[2])),
            None,
        ),
        ParamValue::Text(t) => ("text", None, None, None, None, Some(t.as_str())),
    };
    conn.execute(
        "INSERT INTO cam_preset_param(preset_id, name, ty, v_int, v_f0, v_f1, v_f2, v_text,
                                      inherited, from_preset)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(preset_id, name, inherited) DO UPDATE SET
             ty = excluded.ty, v_int = excluded.v_int, v_f0 = excluded.v_f0,
             v_f1 = excluded.v_f1, v_f2 = excluded.v_f2, v_text = excluded.v_text,
             from_preset = excluded.from_preset",
        params![
            preset_id,
            name,
            ty,
            vi,
            f0,
            f1,
            f2,
            text,
            i64::from(inherited),
            from
        ],
    )?;
    Ok(())
}

/// Indexe un jeu de presets de contrôleur, valeurs déclarées **et** effectives.
///
/// # Errors
/// [`DbError::Sqlite`] en cas d'échec SQL.
pub fn index_property(
    conn: &Connection,
    asset_id: i64,
    context: &str,
    set: &PropertySet,
) -> Result<IndexStats> {
    let mut st = IndexStats::default();
    conn.execute(
        "DELETE FROM cam_preset_param WHERE preset_id IN
             (SELECT id FROM cam_preset WHERE asset_id = ?1)",
        params![asset_id],
    )?;
    conn.execute(
        "DELETE FROM cam_preset WHERE asset_id = ?1",
        params![asset_id],
    )?;

    for (name, preset) in &set.presets {
        conn.execute(
            "INSERT INTO cam_preset(asset_id, name, parent, context) VALUES(?1,?2,?3,?4)",
            params![asset_id, name, preset.parent.as_deref(), context],
        )?;
        let preset_id = conn.last_insert_rowid();
        st.presets += 1;

        for (k, v) in &preset.params {
            bind_param(conn, preset_id, k, v, false, None)?;
            st.preset_params += 1;
        }
        // Valeurs effectives : celles qui ne sont pas déjà déclarées localement.
        let effective: BTreeMap<String, ParamValue> = set.resolve(name);
        for (k, v) in &effective {
            if preset.params.contains_key(k) {
                continue;
            }
            let origin = find_origin(set, name, k);
            bind_param(conn, preset_id, k, v, true, origin.as_deref())?;
            st.preset_params += 1;
        }
    }
    Ok(st)
}

/// Remonte la chaîne d'héritage pour dire d'où vient un paramètre hérité.
fn find_origin(set: &PropertySet, start: &str, param: &str) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut cur = set.presets.get(start)?.parent.clone();
    while let Some(name) = cur {
        if seen.contains(&name) {
            return None;
        }
        let p = set.presets.get(&name)?;
        if p.params.contains_key(param) {
            return Some(name);
        }
        seen.push(name);
        cur = p.parent.clone();
    }
    None
}

/// Indexe une animation `.g4cm`.
///
/// `with_samples` déclenche l'insertion de **chaque** échantillon de keyframe
/// (`cam_anim_sample`) : c'est ce qui fait grossir la base, à n'activer que si on veut
/// requêter les courbes en SQL.
///
/// # Errors
/// [`DbError::Decode`] si le fichier ne se décode pas, [`DbError::Sqlite`] en cas d'échec SQL.
pub fn index_anim(
    conn: &Connection,
    source_id: i64,
    path: &str,
    bytes: &[u8],
    with_samples: bool,
) -> Result<IndexStats> {
    // `g4cm::decode` vit désormais dans `nie-formats` et rend une `FormatError` ; `CameraError`
    // l'absorbe (variante `Format`), ce que `DbError::Decode` continue d'attendre.
    let anim = g4cm::decode(bytes).map_err(|e| DbError::Decode {
        path: path.to_string(),
        source: crate::CameraError::Format(e),
    })?;
    let roundtrip_ok = g4cm::encode(&anim).is_ok_and(|re| re == bytes);
    let mut st = IndexStats::default();

    let event_id = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix("_camera.g4cm"))
        .map(str::to_string);
    let (fmin, fmax) = anim.frame_range().map_or((None, None), |(a, b)| {
        (Some(i64::from(a)), Some(i64::from(b)))
    });
    let n_samples: usize = anim.channels.iter().map(|c| c.track.len()).sum();

    conn.execute(
        "DELETE FROM cam_anim WHERE source_id = ?1 AND path = ?2",
        params![source_id, path],
    )?;
    conn.execute(
        "INSERT INTO cam_anim(source_id, path, event_id, size, sha256, version, align,
             n_objects, n_channels, n_times, frame_min, frame_max, n_samples,
             decoded_ratio, roundtrip_ok)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            source_id,
            path,
            event_id,
            bytes.len() as i64,
            sha256_hex(bytes),
            i64::from(anim.header.type_id),
            i64::from(anim.header.align),
            anim.objects.len() as i64,
            anim.channels.len() as i64,
            anim.times.len() as i64,
            fmin,
            fmax,
            n_samples as i64,
            f64::from(anim.decoded_ratio()),
            i64::from(roundtrip_ok)
        ],
    )?;
    let anim_id = conn.last_insert_rowid();
    st.anims += 1;

    // Objets, puis canaux rattachés à leur objet quand c'est possible.
    let mut object_ids: Vec<(usize, usize, i64)> = Vec::new(); // (first, count, id)
    for (i, o) in anim.objects.iter().enumerate() {
        let clip = anim.clips.get(i);
        conn.execute(
            "INSERT INTO cam_anim_object(anim_id, obj_idx, name, first_channel, channel_count,
                 clip_start, clip_end, clip_flags)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                anim_id,
                i as i64,
                anim.name_of(i),
                i64::from(o.first_channel),
                i64::from(o.channel_count),
                clip.map(|c| i64::from(c.start)),
                clip.map(|c| i64::from(c.end)),
                clip.map(|c| i64::from(c.flags))
            ],
        )?;
        object_ids.push((
            o.first_channel as usize,
            o.channel_count as usize,
            conn.last_insert_rowid(),
        ));
    }

    let mut sample_stmt = conn.prepare(
        "INSERT INTO cam_anim_sample(channel_id, idx, frame, v_f32, v_raw)
         VALUES(?1,?2,?3,?4,?5)",
    )?;
    for (ci, c) in anim.channels.iter().enumerate() {
        let object_id = object_ids
            .iter()
            .find(|(first, count, _)| ci >= *first && ci < first + count)
            .map(|(_, _, id)| *id);
        let times = c.times(&anim);
        let (encoding, v_min, v_max) = match &c.track {
            Track::F32(v) => {
                let lo = v.iter().copied().fold(f32::INFINITY, f32::min);
                let hi = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                if v.is_empty() {
                    ("f32", None, None)
                } else {
                    ("f32", Some(f64::from(lo)), Some(f64::from(hi)))
                }
            }
            Track::Raw16(_) => ("raw16", None, None),
            Track::Raw8(_) => ("raw8", None, None),
        };
        conn.execute(
            "INSERT INTO cam_anim_channel(anim_id, object_id, chan_idx, kind_code, kind, mode,
                 encoding, elem_size, sample_count, time_index, value_offset,
                 frame_first, frame_last, v_min, v_max)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                anim_id,
                object_id,
                ci as i64,
                i64::from(c.kind.code()),
                c.kind.label(),
                i64::from(c.mode),
                encoding,
                c.track.elem_size() as i64,
                c.track.len() as i64,
                i64::from(c.time_index),
                i64::from(c.value_offset),
                times.first().map(|t| i64::from(*t)),
                times.last().map(|t| i64::from(*t)),
                v_min,
                v_max
            ],
        )?;
        let channel_id = conn.last_insert_rowid();
        st.channels += 1;

        if with_samples {
            for i in 0..c.track.len() {
                let frame = times.get(i).map(|t| i64::from(*t));
                let (vf, vr): (Option<f64>, Option<i64>) = match &c.track {
                    Track::F32(v) => (v.get(i).map(|x| f64::from(*x)), None),
                    Track::Raw16(v) => (None, v.get(i).map(|x| i64::from(*x))),
                    Track::Raw8(v) => (None, v.get(i).map(|x| i64::from(*x))),
                };
                sample_stmt.execute(params![channel_id, i as i64, frame, vf, vr])?;
                st.samples += 1;
            }
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GoalnetCameraInfo, IndexedRef, SoccerCameraInfoData};

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().expect("base mémoire");
        conn.execute_batch(nie_index::CAMERA_SCHEMA)
            .expect("migration");
        conn
    }

    #[test]
    fn migration_cree_tables_et_vues() {
        let conn = mem();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'cam\\_%' ESCAPE '\\'",
                [],
                |r| r.get(0),
            )
            .expect("compte");
        assert_eq!(n, 22, "22 tables cam_*");
        let v: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='view'",
                [],
                |r| r.get(0),
            )
            .expect("compte");
        assert_eq!(v, 5, "5 vues");
    }

    #[test]
    fn index_map_remplit_la_hierarchie() {
        let conn = mem();
        let src = upsert_source(&conn, "exe", "test", None, None).expect("source");
        let st = index_map(&conn, src).expect("index");
        assert_eq!(st.ctrl_classes, 23);
        assert_eq!(st.dispatchers, 15);

        // La racine n'a pas de parent, et ChaseSoccer descend bien de ChaseBase.
        let base: Option<String> = conn
            .query_row(
                "SELECT base FROM cam_ctrl_class WHERE cpp_name = 'game::CCameraCtrlChaseSoccer'",
                [],
                |r| r.get(0),
            )
            .expect("requête");
        assert_eq!(base.as_deref(), Some("game::CCameraCtrlChaseBase"));

        // La vue récursive doit couvrir toutes les classes.
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM v_cam_ctrl_hierarchy", [], |r| {
                r.get(0)
            })
            .expect("vue");
        assert_eq!(n, 23);

        // Le dispatcher caméra est marqué comme tel, avec ses 46 commandes.
        let (name, count): (String, i64) = conn
            .query_row(
                "SELECT name, cmd_count FROM cam_dispatcher WHERE is_camera = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("dispatcher caméra");
        assert_eq!(name, "funcLuaCameraCommand");
        assert_eq!(count, 46);
    }

    #[test]
    fn reindexation_est_idempotente() {
        let conn = mem();
        let src = upsert_source(&conn, "exe", "test", None, None).expect("source");
        index_map(&conn, src).expect("1re passe");
        index_map(&conn, src).expect("2e passe");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM cam_dispatcher", [], |r| r.get(0))
            .expect("compte");
        assert_eq!(n, 15, "pas de doublon après réindexation");
        let s: i64 = conn
            .query_row("SELECT COUNT(*) FROM cam_source", [], |r| r.get(0))
            .expect("compte");
        assert_eq!(s, 1);
    }

    #[test]
    fn config_soccer_et_vue_resolue() {
        let conn = mem();
        let src = upsert_source(&conn, "vfs", "test", None, None).expect("source");
        let asset = upsert_asset(
            &conn,
            src,
            "common/x/soccer_camera_config.cfg.bin",
            None,
            None,
        )
        .expect("asset");
        let cfg = SoccerCameraConfig {
            camera_data: vec![
                SoccerCameraInfoData {
                    no: 0,
                    length: 8.0,
                    fov: 45.0,
                    ..Default::default()
                },
                SoccerCameraInfoData {
                    no: 1,
                    length: 12.0,
                    fov: 50.0,
                    ..Default::default()
                },
            ],
            cameras: vec![IndexedRef {
                id: 0xDEAD_BEEF,
                slice: [1, 1],
            }],
            goalnet: vec![GoalnetCameraInfo {
                id: 0x596C_1326,
                cam_pos: [14.0, 2.5, 50.0],
                fov: 45.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let st = index_soccer_config(&conn, asset, &cfg).expect("index");
        assert_eq!(st.config_rows, 4);

        // La vue doit résoudre la tranche [1,1] → la 2ᵉ donnée.
        let (cam_id, length): (i64, f64) = conn
            .query_row(
                "SELECT cam_id, length FROM v_cam_soccer_resolved WHERE list_name = 'm_soccerCameraInfoList'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("vue");
        assert_eq!(cam_id, 0xDEAD_BEEF);
        assert!(
            (length - 12.0).abs() < 1e-6,
            "la tranche pointe la 2ᵉ ligne"
        );
    }

    #[test]
    fn classification_des_parametres() {
        assert_eq!(classify_param("shootCameraLargeShakeTime"), "shake");
        assert_eq!(classify_param("selfGoalnetCameraPosX"), "goal");
        assert_eq!(classify_param("defaultCameraFadeNear"), "fade");
        assert_eq!(classify_param("changeCameraPostEffectBloom"), "posteffect");
        assert_eq!(classify_param("CameraSpeedMouse"), "input");
        assert_eq!(classify_param("photoModeCameraParallelMoveVal"), "photo");
        assert_eq!(classify_param("damagePopupOffsetXfromCamera"), "hud");
        assert_eq!(classify_param("m_cameraFov"), "autre");
    }

    #[test]
    fn asset_calcule_sha256_et_format() {
        let conn = mem();
        let src = upsert_source(&conn, "vfs", "test", None, None).expect("source");
        let id =
            upsert_asset(&conn, src, "a/b.g4cm", Some("test"), Some(b"G4CM____")).expect("asset");
        let (present, fmt, sha): (i64, String, String) = conn
            .query_row(
                "SELECT present, format, sha256 FROM cam_asset WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("requête");
        assert_eq!(present, 1);
        assert_eq!(fmt, "G4CM");
        assert_eq!(sha.len(), 64);

        // Un asset absent est enregistré comme tel, sans écraser son rôle.
        let missing = upsert_asset(&conn, src, "a/c.cfg.bin", Some("rôle"), None).expect("asset");
        let (present, role): (i64, String) = conn
            .query_row(
                "SELECT present, role FROM cam_asset WHERE id = ?1",
                params![missing],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("requête");
        assert_eq!(present, 0);
        assert_eq!(role, "rôle");
    }
}
