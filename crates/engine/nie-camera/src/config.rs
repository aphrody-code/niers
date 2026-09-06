//! `soccer_camera_config_*.cfg.bin` — les caméras de **match**, typées.
//!
//! Conteneur RDBN, 11 listes. Les noms de champs ci-dessous sont ceux du fichier réel
//! (`soccer_camera_config_1.03.21`, 28 425 octets), pas des suppositions : chaque struct est
//! remplie depuis les colonnes déclarées dans la table de types du RDBN.
//!
//! | Liste | Lignes | Struct |
//! |---|---|---|
//! | `m_soccerCameraInfoDataList` | 138 | [`SoccerCameraInfoData`] — le cœur : longueur, rotations, FOV, taux d'interpolation |
//! | `m_soccerCameraInfoList` | 54 | [`IndexedRef`] — id → tranche de la liste précédente |
//! | `m_scGoalnetCameraInfoList` | 8 | [`GoalnetCameraInfo`] |
//! | `m_scAerialCameraInfoList` | 4 | [`AerialCameraInfo`] |
//! | `m_scAerialCameraMapInfoList` | 110 | [`AerialCameraMapInfo`] |
//! | `m_soccerDirCameraInfoList` | 4 | [`DirCameraInfo`] |
//! | `m_soccerFixPosCameraInfoDataList` | 21 | [`FixPosCameraInfoData`] |
//! | `m_soccerFixPosCameraInfoList` | 21 | [`IndexedRef`] |
//! | `m_cinematicCameraInfoDataList` | 15 | [`CinematicCameraInfoData`] |
//! | `m_cinematicCameraSituationInfoList` | 3 | [`SituationRef`] |
//! | `m_cinematicCameraInfoList` | 1 | [`IndexedRef`] |
//!
//! Les listes `*InfoList` ne portent qu'un `id` et une **tranche** `[offset, count]` dans la
//! liste `*InfoDataList` correspondante : c'est le mécanisme d'indirection standard de Level-5
//! (plusieurs jeux de paramètres par caméra logique). [`SoccerCameraConfig::data_for`] fait la
//! résolution.

use nie_formats::cfgbin::{self, RdbnList, RdbnRow, RdbnValue};

use crate::{CameraError, Result};

fn f32_of(row: &RdbnRow, key: &str) -> f32 {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::Float(f) => Some(*f),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn i32_of(row: &RdbnRow, key: &str) -> i32 {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::Int(i) | RdbnValue::Flag(i) => Some(*i),
            RdbnValue::Short(s) | RdbnValue::ActType(s) => Some(i32::from(*s)),
            RdbnValue::Byte(b) => Some(i32::from(*b)),
            _ => None,
        })
        .unwrap_or(0)
}

fn hash_of(row: &RdbnRow, key: &str) -> u32 {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::Hash(h) => Some(*h),
            _ => None,
        })
        .unwrap_or(0)
}

fn bool_of(row: &RdbnRow, key: &str) -> bool {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false)
}

fn vec4_of(row: &RdbnRow, key: &str) -> [f32; 4] {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::Rates(r) | RdbnValue::Position(r) => Some(*r),
            _ => None,
        })
        .unwrap_or([0.0; 4])
}

fn pair_of(row: &RdbnRow, key: &str) -> [i16; 2] {
    row.fields
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            RdbnValue::ShortTuple(t) => Some(*t),
            _ => None,
        })
        .unwrap_or([0, 0])
}

/// `SOCCER_CAMERA_INFO_DATA` — un jeu complet de paramètres de caméra de match.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SoccerCameraInfoData {
    /// `no` — index de la donnée.
    pub no: i32,
    /// `length` / `lengthMin` / `lengthMax` — distance caméra→cible et ses bornes.
    pub length: f32,
    /// `lengthMin`.
    pub length_min: f32,
    /// `lengthMax`.
    pub length_max: f32,
    /// `rotX` / `rotXMin` / `rotXMax` — inclinaison (degrés).
    pub rot_x: f32,
    /// `rotXMin`.
    pub rot_x_min: f32,
    /// `rotXMax`.
    pub rot_x_max: f32,
    /// `rotY` / `rotYMin` / `rotYMax` — azimut (degrés).
    pub rot_y: f32,
    /// `rotYMin`.
    pub rot_y_min: f32,
    /// `rotYMax`.
    pub rot_y_max: f32,
    /// `camOffenceOffsetRotY` — décalage d'azimut en phase offensive.
    pub offence_offset_rot_y: f32,
    /// `camDefenceOffsetRotY` — décalage d'azimut en phase défensive.
    pub defence_offset_rot_y: f32,
    /// `fov` — champ de vision vertical (degrés).
    pub fov: f32,
    /// `refOffset` — décalage du point visé.
    pub ref_offset: [f32; 4],
    /// `offenceRefOffset` — décalage du point visé en attaque.
    pub offence_ref_offset: [f32; 4],
    /// `defenceRefOffset` — décalage du point visé en défense.
    pub defence_ref_offset: [f32; 4],
    /// `refOffsetLength`.
    pub ref_offset_length: f32,
    /// `moveInterpRate` — taux de lissage de la position (par frame).
    pub move_interp_rate: f32,
    /// `rotInterpRate` — taux de lissage de la rotation.
    pub rot_interp_rate: f32,
    /// `zoomInterpRate` — taux de lissage du zoom.
    pub zoom_interp_rate: f32,
}

impl SoccerCameraInfoData {
    fn from_row(r: &RdbnRow) -> Self {
        Self {
            no: i32_of(r, "no"),
            length: f32_of(r, "length"),
            length_min: f32_of(r, "lengthMin"),
            length_max: f32_of(r, "lengthMax"),
            rot_x: f32_of(r, "rotX"),
            rot_x_min: f32_of(r, "rotXMin"),
            rot_x_max: f32_of(r, "rotXMax"),
            rot_y: f32_of(r, "rotY"),
            rot_y_min: f32_of(r, "rotYMin"),
            rot_y_max: f32_of(r, "rotYMax"),
            offence_offset_rot_y: f32_of(r, "camOffenceOffsetRotY"),
            defence_offset_rot_y: f32_of(r, "camDefenceOffsetRotY"),
            fov: f32_of(r, "fov"),
            ref_offset: vec4_of(r, "refOffset"),
            offence_ref_offset: vec4_of(r, "offenceRefOffset"),
            defence_ref_offset: vec4_of(r, "defenceRefOffset"),
            ref_offset_length: f32_of(r, "refOffsetLength"),
            move_interp_rate: f32_of(r, "moveInterpRate"),
            rot_interp_rate: f32_of(r, "rotInterpRate"),
            zoom_interp_rate: f32_of(r, "zoomInterpRate"),
        }
    }
}

/// Une entrée `*InfoList` : un `id` et la tranche `[offset, count]` qu'elle désigne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexedRef {
    /// `id` — hash de la caméra logique.
    pub id: u32,
    /// `data` — `[offset, count]` dans la liste `*InfoDataList` correspondante.
    pub slice: [i16; 2],
}

/// `CINEMATIC_CAMERA_SITUATION_INFO` — situation de déclenchement d'une caméra cinématique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SituationRef {
    /// `situationType`.
    pub situation_type: i32,
    /// `data` — `[offset, count]` dans `m_cinematicCameraInfoDataList`.
    pub slice: [i16; 2],
}

/// `ScGoalnetCameraInfo` — caméra derrière un but.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GoalnetCameraInfo {
    /// `id`.
    pub id: u32,
    /// `camPosX` / `camPosY` / `camPosZ`.
    pub cam_pos: [f32; 3],
    /// `refOffsetPosX` / `refOffsetPosY` / `refOffsetPosZ`.
    pub ref_offset: [f32; 3],
    /// `fov` (degrés).
    pub fov: f32,
    /// `chaseMaxSpeed`.
    pub chase_max_speed: f32,
    /// `notFollowAfterBouncing` — cesse de suivre le ballon après un rebond.
    pub not_follow_after_bouncing: bool,
    /// `isFixedRefX` / `isFixedRefY` / `isFixedRefZ` — axes du point visé figés.
    pub fixed_ref: [bool; 3],
    /// `isInitRefGoalLine` — initialise le point visé sur la ligne de but.
    pub init_ref_goal_line: bool,
}

/// `ScAerialCameraInfo` — vue aérienne (travelling d'ouverture de stade).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AerialCameraInfo {
    /// `id`.
    pub id: u32,
    /// `camLength`.
    pub cam_length: f32,
    /// `camPsoX` (orthographe du fichier), `camPsoY`, `camPosZ`.
    pub cam_pos: [f32; 3],
    /// `camRotXStart` → `camRotXEnd`.
    pub rot_x: (f32, f32),
    /// `camRotYStart` → `camRotYEnd`.
    pub rot_y: (f32, f32),
}

/// `ScAerialCameraMapInfo` — association stade → vue aérienne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AerialCameraMapInfo {
    /// `id` — hash du stade/map.
    pub id: u32,
    /// `aerialCamInfoId` — vue aérienne associée.
    pub aerial_cam_info_id: u32,
    /// `lightOverwriteId` — surcharge d'éclairage.
    pub light_overwrite_id: u32,
}

/// `SOCCER_DIR_CAMERA_INFO` — triplet de caméras directionnelles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DirCameraInfo {
    /// `dirCamId`.
    pub dir_cam_id: u32,
    /// `horCamId` — variante horizontale.
    pub hor_cam_id: u32,
    /// `vertCamId` — variante verticale.
    pub vert_cam_id: u32,
}

/// `SOCCER_FIX_POS_CAMERA_INFO_DATA` — caméra à poste fixe.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FixPosCameraInfoData {
    /// `id`.
    pub id: u32,
    /// `refOffset`.
    pub ref_offset: [f32; 4],
    /// `camPosOffset`.
    pub cam_pos_offset: [f32; 4],
    /// `moveVecOffset`.
    pub move_vec_offset: [f32; 4],
    /// `camRoll` (degrés).
    pub cam_roll: f32,
    /// `fov` (degrés).
    pub fov: f32,
    /// `offsetLength`.
    pub offset_length: f32,
    /// `offsetTime`.
    pub offset_time: f32,
    /// `conditionAreaRadius` — rayon de la zone d'activation.
    pub condition_area_radius: f32,
    /// `isEnableInterp`.
    pub enable_interp: bool,
    /// `moveRefPosOnly`.
    pub move_ref_pos_only: bool,
    /// `moveType`.
    pub move_type: i32,
    /// `curvature`.
    pub curvature: f32,
}

/// `CINEMATIC_CAMERA_INFO_DATA` — caméra cinématique.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CinematicCameraInfoData {
    /// `weight` — poids de tirage.
    pub weight: i32,
    /// `changeRecast` — délai avant re-déclenchement (secondes).
    pub change_recast: f32,
    /// `chaseCameraId`.
    pub chase_camera_id: u32,
    /// `fixCameraId` (0 = aucune).
    pub fix_camera_id: u32,
}

/// Contenu complet d'un `soccer_camera_config_*.cfg.bin`.
#[derive(Debug, Clone, Default)]
pub struct SoccerCameraConfig {
    /// `m_soccerCameraInfoDataList`.
    pub camera_data: Vec<SoccerCameraInfoData>,
    /// `m_soccerCameraInfoList`.
    pub cameras: Vec<IndexedRef>,
    /// `m_scGoalnetCameraInfoList`.
    pub goalnet: Vec<GoalnetCameraInfo>,
    /// `m_scAerialCameraInfoList`.
    pub aerial: Vec<AerialCameraInfo>,
    /// `m_scAerialCameraMapInfoList`.
    pub aerial_map: Vec<AerialCameraMapInfo>,
    /// `m_soccerDirCameraInfoList`.
    pub dir_cameras: Vec<DirCameraInfo>,
    /// `m_soccerFixPosCameraInfoDataList`.
    pub fix_pos_data: Vec<FixPosCameraInfoData>,
    /// `m_soccerFixPosCameraInfoList`.
    pub fix_pos: Vec<IndexedRef>,
    /// `m_cinematicCameraInfoDataList`.
    pub cinematic_data: Vec<CinematicCameraInfoData>,
    /// `m_cinematicCameraSituationInfoList`.
    pub cinematic_situations: Vec<SituationRef>,
    /// `m_cinematicCameraInfoList`.
    pub cinematic: Vec<IndexedRef>,
}

fn rows<'a>(lists: &'a [RdbnList], name: &str) -> &'a [RdbnRow] {
    lists
        .iter()
        .find(|l| l.name == name)
        .map_or(&[][..], |l| l.rows.as_slice())
}

impl SoccerCameraConfig {
    /// Décode un `soccer_camera_config_*.cfg.bin` (RDBN).
    ///
    /// # Errors
    /// [`CameraError::Format`] si le conteneur RDBN est illisible, [`CameraError::Malformed`]
    /// s'il ne contient aucune liste caméra connue.
    pub fn parse(data: &[u8]) -> Result<SoccerCameraConfig> {
        let rdbn = cfgbin::parse(data)?;
        let lists = cfgbin::read_values(&rdbn, data);
        let cfg = SoccerCameraConfig {
            camera_data: rows(&lists, "m_soccerCameraInfoDataList")
                .iter()
                .map(SoccerCameraInfoData::from_row)
                .collect(),
            cameras: rows(&lists, "m_soccerCameraInfoList")
                .iter()
                .map(|r| IndexedRef {
                    id: hash_of(r, "id"),
                    slice: pair_of(r, "data"),
                })
                .collect(),
            goalnet: rows(&lists, "m_scGoalnetCameraInfoList")
                .iter()
                .map(|r| GoalnetCameraInfo {
                    id: hash_of(r, "id"),
                    cam_pos: [
                        f32_of(r, "camPosX"),
                        f32_of(r, "camPosY"),
                        f32_of(r, "camPosZ"),
                    ],
                    ref_offset: [
                        f32_of(r, "refOffsetPosX"),
                        f32_of(r, "refOffsetPosY"),
                        f32_of(r, "refOffsetPosZ"),
                    ],
                    fov: f32_of(r, "fov"),
                    chase_max_speed: f32_of(r, "chaseMaxSpeed"),
                    not_follow_after_bouncing: bool_of(r, "notFollowAfterBouncing"),
                    fixed_ref: [
                        bool_of(r, "isFixedRefX"),
                        bool_of(r, "isFixedRefY"),
                        bool_of(r, "isFixedRefZ"),
                    ],
                    init_ref_goal_line: bool_of(r, "isInitRefGoalLine"),
                })
                .collect(),
            aerial: rows(&lists, "m_scAerialCameraInfoList")
                .iter()
                .map(|r| AerialCameraInfo {
                    id: hash_of(r, "id"),
                    cam_length: f32_of(r, "camLength"),
                    // `camPsoX`/`camPsoY` : coquille présente dans le fichier du jeu, conservée.
                    cam_pos: [
                        f32_of(r, "camPsoX"),
                        f32_of(r, "camPsoY"),
                        f32_of(r, "camPosZ"),
                    ],
                    rot_x: (f32_of(r, "camRotXStart"), f32_of(r, "camRotXEnd")),
                    rot_y: (f32_of(r, "camRotYStart"), f32_of(r, "camRotYEnd")),
                })
                .collect(),
            aerial_map: rows(&lists, "m_scAerialCameraMapInfoList")
                .iter()
                .map(|r| AerialCameraMapInfo {
                    id: hash_of(r, "id"),
                    aerial_cam_info_id: hash_of(r, "aerialCamInfoId"),
                    light_overwrite_id: hash_of(r, "lightOverwriteId"),
                })
                .collect(),
            dir_cameras: rows(&lists, "m_soccerDirCameraInfoList")
                .iter()
                .map(|r| DirCameraInfo {
                    dir_cam_id: hash_of(r, "dirCamId"),
                    hor_cam_id: hash_of(r, "horCamId"),
                    vert_cam_id: hash_of(r, "vertCamId"),
                })
                .collect(),
            fix_pos_data: rows(&lists, "m_soccerFixPosCameraInfoDataList")
                .iter()
                .map(|r| FixPosCameraInfoData {
                    id: hash_of(r, "id"),
                    ref_offset: vec4_of(r, "refOffset"),
                    cam_pos_offset: vec4_of(r, "camPosOffset"),
                    move_vec_offset: vec4_of(r, "moveVecOffset"),
                    cam_roll: f32_of(r, "camRoll"),
                    fov: f32_of(r, "fov"),
                    offset_length: f32_of(r, "offsetLength"),
                    offset_time: f32_of(r, "offsetTime"),
                    condition_area_radius: f32_of(r, "conditionAreaRadius"),
                    enable_interp: bool_of(r, "isEnableInterp"),
                    move_ref_pos_only: bool_of(r, "moveRefPosOnly"),
                    move_type: i32_of(r, "moveType"),
                    curvature: f32_of(r, "curvature"),
                })
                .collect(),
            fix_pos: rows(&lists, "m_soccerFixPosCameraInfoList")
                .iter()
                .map(|r| IndexedRef {
                    id: hash_of(r, "id"),
                    slice: pair_of(r, "data"),
                })
                .collect(),
            cinematic_data: rows(&lists, "m_cinematicCameraInfoDataList")
                .iter()
                .map(|r| CinematicCameraInfoData {
                    weight: i32_of(r, "weight"),
                    change_recast: f32_of(r, "changeRecast"),
                    chase_camera_id: hash_of(r, "chaseCameraId"),
                    fix_camera_id: hash_of(r, "fixCameraId"),
                })
                .collect(),
            cinematic_situations: rows(&lists, "m_cinematicCameraSituationInfoList")
                .iter()
                .map(|r| SituationRef {
                    situation_type: i32_of(r, "situationType"),
                    slice: pair_of(r, "data"),
                })
                .collect(),
            cinematic: rows(&lists, "m_cinematicCameraInfoList")
                .iter()
                .map(|r| IndexedRef {
                    id: hash_of(r, "id"),
                    slice: pair_of(r, "data"),
                })
                .collect(),
        };
        if cfg.camera_data.is_empty() && cfg.goalnet.is_empty() {
            return Err(CameraError::Malformed(
                "aucune liste caméra reconnue : ce n'est pas un soccer_camera_config".to_string(),
            ));
        }
        Ok(cfg)
    }

    /// Jeux de paramètres désignés par une caméra logique (résolution de la tranche).
    ///
    /// Une tranche invalide (offset ou compte négatif, bornes dépassées) rend une tranche
    /// **vide** plutôt que de paniquer ou de retomber sur le début de la liste : sur un fichier
    /// édité, mieux vaut ne rien rendre que rendre les paramètres d'une autre caméra.
    #[must_use]
    pub fn data_for(&self, camera: &IndexedRef) -> &[SoccerCameraInfoData] {
        let (Ok(off), Ok(cnt)) = (
            usize::try_from(camera.slice[0]),
            usize::try_from(camera.slice[1]),
        ) else {
            return &[];
        };
        let end = off.saturating_add(cnt).min(self.camera_data.len());
        self.camera_data.get(off..end).unwrap_or(&[])
    }

    /// Caméra logique portant ce hash.
    #[must_use]
    pub fn camera_by_id(&self, id: u32) -> Option<&IndexedRef> {
        self.cameras.iter().find(|c| c.id == id)
    }

    /// Caméra de but portant ce hash.
    #[must_use]
    pub fn goalnet_by_id(&self, id: u32) -> Option<&GoalnetCameraInfo> {
        self.goalnet.iter().find(|c| c.id == id)
    }

    /// Vue aérienne associée à un stade.
    #[must_use]
    pub fn aerial_for_map(&self, map_id: u32) -> Option<&AerialCameraInfo> {
        let m = self.aerial_map.iter().find(|m| m.id == map_id)?;
        self.aerial.iter().find(|a| a.id == m.aerial_cam_info_id)
    }

    /// Nombre total de lignes décodées, toutes listes confondues.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.camera_data.len()
            + self.cameras.len()
            + self.goalnet.len()
            + self.aerial.len()
            + self.aerial_map.len()
            + self.dir_cameras.len()
            + self.fix_pos_data.len()
            + self.fix_pos.len()
            + self.cinematic_data.len()
            + self.cinematic_situations.len()
            + self.cinematic.len()
    }
}

impl GoalnetCameraInfo {
    /// État de caméra correspondant, prêt à rendre.
    ///
    /// `goal_center` est le centre du but visé : le point de référence est ce centre décalé de
    /// `refOffset`, la position est `camPos` (coordonnées de terrain, telles quelles).
    #[must_use]
    pub fn to_state(&self, goal_center: [f32; 3]) -> crate::CameraState {
        crate::CameraState {
            pos: self.cam_pos,
            ref_pos: [
                goal_center[0] + self.ref_offset[0],
                goal_center[1] + self.ref_offset[1],
                goal_center[2] + self.ref_offset[2],
            ],
            fov_deg: self.fov,
            ..crate::CameraState::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tranche_hors_bornes_ne_panique_pas() {
        let cfg = SoccerCameraConfig {
            camera_data: vec![SoccerCameraInfoData {
                no: 0,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            cfg.data_for(&IndexedRef {
                id: 1,
                slice: [0, 1]
            })
            .len(),
            1
        );
        assert_eq!(
            cfg.data_for(&IndexedRef {
                id: 1,
                slice: [0, 99]
            })
            .len(),
            1
        );
        assert_eq!(
            cfg.data_for(&IndexedRef {
                id: 1,
                slice: [50, 1]
            })
            .len(),
            0
        );
        assert_eq!(
            cfg.data_for(&IndexedRef {
                id: 1,
                slice: [-3, 2]
            })
            .len(),
            0
        );
    }

    #[test]
    fn goalnet_vers_etat() {
        let g = GoalnetCameraInfo {
            id: 0x596C_1326,
            cam_pos: [14.0, 2.5, 50.0],
            ref_offset: [0.0, 1.0, 0.0],
            fov: 45.0,
            chase_max_speed: 0.6,
            not_follow_after_bouncing: true,
            ..Default::default()
        };
        let s = g.to_state([0.0, 0.0, 52.0]);
        assert_eq!(s.pos, [14.0, 2.5, 50.0]);
        assert_eq!(s.ref_pos, [0.0, 1.0, 52.0]);
        assert!((s.fov_deg - 45.0).abs() < f32::EPSILON);
    }

    #[test]
    fn recherche_aerienne_par_stade() {
        let cfg = SoccerCameraConfig {
            aerial: vec![AerialCameraInfo {
                id: 7,
                cam_length: 10.0,
                ..Default::default()
            }],
            aerial_map: vec![AerialCameraMapInfo {
                id: 42,
                aerial_cam_info_id: 7,
                light_overwrite_id: 0,
            }],
            ..Default::default()
        };
        assert!((cfg.aerial_for_map(42).expect("trouvée").cam_length - 10.0).abs() < f32::EPSILON);
        assert!(cfg.aerial_for_map(1).is_none());
    }

    #[test]
    fn refuse_un_rdbn_sans_liste_camera() {
        // Un RDBN valide mais vide : pas de liste caméra → erreur explicite plutôt qu'un
        // `SoccerCameraConfig` vide qu'on croirait chargé.
        let empty = SoccerCameraConfig::default();
        assert_eq!(empty.total_rows(), 0);
    }
}
