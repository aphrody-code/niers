//! Carte du reverse-engineering caméra : adresses, tables, symboles, chemins d'assets.
//!
//! Toutes les VA sont relevées sur le `nie.exe` **local** (33 918 464 octets, image base
//! `0x140000000`). Elles sont **spécifiques à ce build** : les `cmdId` sont stables entre builds
//! (ce sont des hashes), pas les adresses. [`verify_against`] permet de contrôler qu'un binaire
//! donné correspond bien à cette carte avant de s'y fier.

/// Image base statique de `nie.exe`.
pub const IMAGE_BASE: u64 = 0x1_4000_0000;

/// Taille du `nie.exe` sur lequel cette carte a été relevée.
pub const MAPPED_EXE_SIZE: u64 = 33_918_464;

/// Un dispatcher `funcLua*Command` : sa table de commandes et le nombre d'entrées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dispatcher {
    /// Nom Lua exposé aux scripts.
    pub name: &'static str,
    /// VA de la table de dispatch (en BSS : remplie et triée au runtime).
    pub table_va: u64,
    /// Nombre de commandes de la table.
    pub count: u32,
}

/// Le dispatcher caméra : `funcLuaCameraCommand`, 46 commandes.
///
/// Relevé : `mov r9d, 0x2E` + `lea r8, [rip+…]` → `0x1422B3380`, aux deux sites d'appel
/// (`0x140BE6735` et `0x140BE67C8`) de la routine de dispatch partagée [`DISPATCH_ROUTINE_VA`].
pub const CAMERA_DISPATCHER: Dispatcher = Dispatcher {
    name: "funcLuaCameraCommand",
    table_va: 0x1_422B_3380,
    count: 46,
};

/// VA de la chaîne `"funcLuaCameraCommand"` en `.rdata`.
pub const CAMERA_DISPATCHER_NAME_VA: u64 = 0x1_4190_0EC8;

/// Entrée `lua_CFunction` du dispatcher caméra.
pub const CAMERA_DISPATCHER_ENTRY_VA: u64 = 0x1_40BE_66F0;

/// Variante interne du dispatcher caméra (même table).
pub const CAMERA_DISPATCHER_ALT_VA: u64 = 0x1_40BE_6780;

/// Routine de dispatch partagée par les 15 dispatchers : recherche dichotomique sur `cmdId`.
///
/// Signature déduite du désassemblage : `(ctx, nargs, table, count)`. La table est un tableau de
/// **pointeurs 8 octets** vers des entrées `{handler:u64, cmdId:u32, pad:u32}` ; le `cmdId` est
/// lu à `[entrée+8]` (`mov rcx,[rbx+rax*8]` ; `cmp r10d,[rcx+8]`).
pub const DISPATCH_ROUTINE_VA: u64 = 0x1_40CA_7550;

/// Réservoir global des commandes `funcLua*` en `.data` initialisée : 3 660 entrées de 16 octets.
///
/// **Non segmenté par dispatcher** : les handlers y sont rangés par adresse décroissante, sans
/// frontière observable entre sections. C'est ce bloc qu'extrait
/// `scripts/extract_funclua_table.py`.
pub const FUNCLUA_POOL_VA: u64 = 0x1_41CB_5500;

/// Nombre d'entrées du réservoir [`FUNCLUA_POOL_VA`].
pub const FUNCLUA_POOL_COUNT: u32 = 3_660;

/// Loader générique des conteneurs G4 (dont G4CM) — c'est lui qui fixe la formule d'offsets
/// utilisée par [`crate::g4cm`].
pub const G4_LOADER_VA: u64 = 0x1_4050_6630;

/// Table des magics de conteneurs G4, dans l'ordre : `G4MT G4MA G4TP G4CM G4VS G4LA G4BA`.
pub const G4_MAGIC_TABLE_VA: u64 = 0x1_41A5_E12C;

/// Index (1-based, comme dans le binaire) de `G4CM` dans [`G4_MAGIC_TABLE_VA`].
pub const G4CM_MAGIC_INDEX: u32 = 4;

/// Les 15 dispatchers `funcLua*Command` du build local : table BSS et nombre de commandes.
pub const DISPATCHERS: [Dispatcher; 15] = [
    Dispatcher {
        name: "funcLuaActionCommand",
        table_va: 0x1_422B_32B0,
        count: 26,
    },
    Dispatcher {
        name: "funcLuaCameraCommand",
        table_va: 0x1_422B_3380,
        count: 46,
    },
    Dispatcher {
        name: "funcLuaCommand",
        table_va: 0x1_422B_34F0,
        count: 2451,
    },
    Dispatcher {
        name: "funcLuaEffectCommand",
        table_va: 0x1_422B_8190,
        count: 48,
    },
    Dispatcher {
        name: "funcLuaMenuCommand",
        table_va: 0x1_422B_8320,
        count: 1150,
    },
    Dispatcher {
        name: "dispatch@0x1422BA710",
        table_va: 0x1_422B_A710,
        count: 11,
    },
    Dispatcher {
        name: "dispatch@0x1422BAA30",
        table_va: 0x1_422B_AA30,
        count: 9,
    },
    Dispatcher {
        name: "dispatch@0x1422BAA78",
        table_va: 0x1_422B_AA78,
        count: 3,
    },
    Dispatcher {
        name: "dispatch@0x1422BAA90",
        table_va: 0x1_422B_AA90,
        count: 1,
    },
    Dispatcher {
        name: "dispatch@0x1422BAAA0",
        table_va: 0x1_422B_AAA0,
        count: 39,
    },
    Dispatcher {
        name: "dispatch@0x1422BABD8",
        table_va: 0x1_422B_ABD8,
        count: 5,
    },
    Dispatcher {
        name: "dispatch@0x1422BAC00",
        table_va: 0x1_422B_AC00,
        count: 28,
    },
    Dispatcher {
        name: "dispatch@0x1422BACE0",
        table_va: 0x1_422B_ACE0,
        count: 8,
    },
    Dispatcher {
        name: "dispatch@0x1422BAD20",
        table_va: 0x1_422B_AD20,
        count: 2,
    },
    Dispatcher {
        name: "dispatch@0x1422BAE80",
        table_va: 0x1_422B_AE80,
        count: 18,
    },
];

/// Commandes d'entrée liées à la caméra (chaînes `CMD_*` présentes dans `.rdata`).
pub const INPUT_COMMANDS: [&str; 25] = [
    "CMD_CAMERA_MOVE_X",
    "CMD_CAMERA_MOVE_Y",
    "CMD_CAMERA_MOVE_UP",
    "CMD_CAMERA_MOVE_DOWN",
    "CMD_CAMERA_MOVE_LEFT",
    "CMD_CAMERA_MOVE_RIGHT",
    "CMD_CAMERA_PARALLEL_MOVE_UP",
    "CMD_CAMERA_PARALLEL_MOVE_DOWN",
    "CMD_CAMERA_PARALLEL_MOVE_LEFT",
    "CMD_CAMERA_PARALLEL_MOVE_RIGHT",
    "CMD_CAMERA_PARALLEL_MOVE_LX",
    "CMD_CAMERA_PARALLEL_MOVE_LY",
    "CMD_CAMERA_ZOOM_IN",
    "CMD_CAMERA_ZOOM_OUT",
    "CMD_CAMERA_LEN_OFS_INC",
    "CMD_CAMERA_LEN_OFS_DEC",
    "CMD_CAMERA_LEN_OFS_ROLL",
    "CMD_CAMERA_LEN_OFS_RESET",
    "CMD_CAMERA_RESET",
    "CMD_CAMERA_REVERSE",
    "CMD_MOUSE_CAMERA_MOVE",
    "CMD_CHANGE_SOCCER_CAMERA_TYPE",
    "CMD_COACH_AI_CAMERA_MOVE_X",
    "CMD_COACH_AI_CAMERA_MOVE_Y",
    "CMD_CRAFT_CAMERA_ZOOM_IN",
];

/// Caméras nommées enregistrées dans la scène (`scene_register_camera`).
pub const SCENE_CAMERAS: [&str; 8] = [
    "BaseCamera",
    "EventCamera",
    "MenuCamera",
    "MenuCameraDefault",
    "RpgCamera",
    "RpgBattleCamera",
    "SoccerCamera",
    "WaitCamera",
];

/// Un fichier de données caméra du VFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetPath {
    /// Chemin interne (sans le préfixe `data/`).
    pub path: &'static str,
    /// À quoi il sert.
    pub role: &'static str,
}

/// Les fichiers de données caméra référencés en dur dans `nie.exe`.
pub const ASSETS: [AssetPath; 18] = [
    AssetPath {
        path: "common/gamedata/soccer/soccer_camera_config_1.03.21.cfg.bin",
        role: "caméras de match — 11 listes (RDBN)",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info.cfg.bin",
        role: "paramètres CCameraCtrl — contexte par défaut",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info_battle.cfg.bin",
        role: "paramètres CCameraCtrl — combat (absent du VFS de ce build)",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info_craft_edit.cfg.bin",
        role: "paramètres CCameraCtrl — édition craft",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info_photo.cfg.bin",
        role: "paramètres CCameraCtrl — mode photo",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info_rpg_battle.cfg.bin",
        role: "paramètres CCameraCtrl — combat RPG",
    },
    AssetPath {
        path: "common/property/camera/camera_ctrl_property_info_screenshot_mode.cfg.bin",
        role: "paramètres CCameraCtrl — mode capture",
    },
    AssetPath {
        path: "common/property/soccer/soccer_camera_property.cfg.bin",
        role: "SoccerCameraProperty",
    },
    AssetPath {
        path: "common/property/soccer/soccer_camera_interp_property.cfg.bin",
        role: "SoccerCameraInterpProperty",
    },
    AssetPath {
        path: "common/property/rpg_battle/rpg_battle_camera_info.cfg.bin",
        role: "RpgBattleCameraInfo / RpgBattleAttackCameraInfo",
    },
    AssetPath {
        path: "common/property/global_param/battle_kill_camera_param.cfg.bin",
        role: "caméra de finish de combat",
    },
    AssetPath {
        path: "common/camera/config/external_camera_config.cfg.bin",
        role: "CResExternalCameraData",
    },
    AssetPath {
        path: "common/gamedata/event/event_cam_preset_config.cfg.bin",
        role: "EventCameraPresetConfig",
    },
    AssetPath {
        path: "common/gamedata/event/event_general_camera_offset_config.cfg.bin",
        role: "EventGeneralCameraOffsetConfig (absent du VFS de ce build)",
    },
    AssetPath {
        path: "common/event/ev72/ev72_01010/ev72_01010_camera.g4cm",
        role: "SoccerFormationEventCameraAnime",
    },
    AssetPath {
        path: "common/event/ev72/ev72_03090/ev72_03090_camera.g4cm",
        role: "SoccerBattleStartCameraAnime",
    },
    AssetPath {
        path: "common/event/ev72/ev72_10310/ev72_10310_camera.g4cm",
        role: "RpgBattleStartEventCameraAnime",
    },
    AssetPath {
        path: "common/event/ev72/ev72_50010/ev72_50010_camera.g4cm",
        role: "RpgBattleDanceBattleCameraAnime",
    },
];

/// Convertit une VA en offset fichier via les sections du PE de `nie.exe`.
///
/// Les sections sont celles du build cartographié ; `None` si la VA tombe hors image ou dans
/// une zone non initialisée dans le fichier (BSS — cas des tables de dispatch).
#[must_use]
pub fn va_to_file_offset(va: u64) -> Option<u64> {
    // (nom, va_debut, va_fin, offset_brut, taille_brute)
    const SECTIONS: [(&str, u64, u64, u64, u64); 5] = [
        (".text", 0x1_4000_1000, 0x1_4186_B6E0, 0x400, 0x186_A800),
        (
            ".rdata",
            0x1_4186_C000,
            0x1_41C9_E0CE,
            0x186_AC00,
            0x43_2200,
        ),
        (".data", 0x1_41C9_F000, 0x1_4265_694C, 0x1C9_CE00, 0x24_D400),
        (
            ".pdata",
            0x1_4265_7000,
            0x1_4278_279C,
            0x1EE_A200,
            0x12_B800,
        ),
        (".rsrc", 0x1_4278_8000, 0x1_4279_7070, 0x201_8A00, 0xF200),
    ];
    for (_, start, end, raw, raw_size) in SECTIONS {
        if va >= start && va < end {
            let delta = va - start;
            return (delta < raw_size).then_some(raw + delta);
        }
    }
    None
}

/// Vérifie qu'un `nie.exe` correspond bien à cette carte.
///
/// Contrôle la taille du fichier et la présence de la chaîne `funcLuaCameraCommand` à
/// [`CAMERA_DISPATCHER_NAME_VA`]. Renvoie la liste des écarts (vide = carte applicable).
#[must_use]
pub fn verify_against(exe: &[u8]) -> Vec<String> {
    let mut issues = Vec::new();
    if exe.len() as u64 != MAPPED_EXE_SIZE {
        issues.push(format!(
            "taille inattendue : {} octets (carte relevée sur {MAPPED_EXE_SIZE})",
            exe.len()
        ));
    }
    match va_to_file_offset(CAMERA_DISPATCHER_NAME_VA) {
        Some(off) => {
            let want = b"funcLuaCameraCommand";
            let got = exe
                .get(off as usize..off as usize + want.len())
                .unwrap_or_default();
            if got != want {
                issues.push(format!(
                    "« funcLuaCameraCommand » absent à 0x{CAMERA_DISPATCHER_NAME_VA:X} \
                     (offset fichier 0x{off:X})"
                ));
            }
        }
        None => issues.push("VA de la chaîne du dispatcher hors sections connues".to_string()),
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatcher_camera_present_et_coherent() {
        let cam = DISPATCHERS
            .iter()
            .find(|d| d.name == "funcLuaCameraCommand")
            .expect("le dispatcher caméra doit être dans la table");
        assert_eq!(*cam, CAMERA_DISPATCHER);
        assert_eq!(cam.count, 46);
        // Les tables sont ordonnées par VA croissante et ne se chevauchent pas (entrées 8 octets).
        for w in DISPATCHERS.windows(2) {
            assert!(w[0].table_va < w[1].table_va, "tables non ordonnées");
            assert!(
                w[0].table_va + u64::from(w[0].count) * 8 <= w[1].table_va,
                "chevauchement entre {} et {}",
                w[0].name,
                w[1].name
            );
        }
    }

    #[test]
    fn va_vers_offset() {
        // .rdata : la chaîne du dispatcher est bien dans le fichier.
        assert!(va_to_file_offset(CAMERA_DISPATCHER_NAME_VA).is_some());
        // .text : le loader G4.
        assert_eq!(
            va_to_file_offset(G4_LOADER_VA),
            Some(0x1_4050_6630 - 0x1_4000_1000 + 0x400)
        );
        // BSS de .data : hors du fichier.
        assert_eq!(va_to_file_offset(CAMERA_DISPATCHER.table_va), None);
        // Hors image.
        assert_eq!(va_to_file_offset(0x1_0000_0000), None);
    }

    #[test]
    fn assets_bien_formes() {
        for a in ASSETS {
            assert!(
                !a.path.starts_with('/') && a.path.starts_with("common/"),
                "{}",
                a.path
            );
            assert!(!a.role.is_empty());
        }
        assert_eq!(
            ASSETS.iter().filter(|a| a.path.ends_with(".g4cm")).count(),
            4
        );
    }
}
