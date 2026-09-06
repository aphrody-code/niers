//! `niers mode` — catalogue des **modes de jeu** : écrans, calques, objets, assets et scripts.
//!
//! ## Ce qu'est un « mode » ici
//!
//! Une entrée du menu principal (une tuile de `mode_base01_atl`, cf. `mainmenu90_01.g4tx`).
//! Le jeu ne stocke nulle part la liste en clair : le script `main_menu` désigne ses onglets par
//! un `TAB_TYPE` **entier**, et l'include ne porte aucune chaîne exploitable. La liste des modes
//! est donc **éditoriale** — [`MODES`] ci-dessous — mais chaque entrée est adossée à des écrans
//! `*_setting.cfg.bin` qui existent réellement dans le VFS ; l'agrégation, elle, est mécanique.
//!
//! Le cas `victory_road` montre pourquoi une liste curatée est nécessaire : ses assets vivent
//! sous **quatre** orthographes (`victory_road`, `victory_load`, `victory_lode`, `vroad`)
//! qu'aucune règle de préfixe ne relierait automatiquement.
//!
//! ## Agrégation
//!
//! Pour chaque mode : écrans (par préfixe) → calques (`MENU_LAYER_INFO`) → objbin → `g4pkm` /
//! `g4tx` référencés, plus les scripts Lua de même préfixe. Tout est écrit dans `mode`,
//! `mode_screen` et `mode_asset`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::objbin;
use nie_formats::vfs::Vfs;
use nie_lua::bytecode;
use serde_json::Value as Json;

/// Définition éditoriale d'un mode.
pub struct ModeDef {
    /// Identifiant stable, utilisable en URL (`victory-road`).
    pub slug: &'static str,
    /// Nom de repli, si le jeu ne fournit pas de libellé pour ce mode.
    pub label: &'static str,
    /// Préfixes de noms d'écran/script qui appartiennent à ce mode.
    pub prefixes: &'static [&'static str],
    /// Région de l'atlas `mode_base01_atl` qui porte l'icône, si identifiée.
    pub icon_region: Option<&'static str>,
    /// Hash `menu_text` du libellé officiel — le nom que le JEU affiche, résolu à l'indexation
    /// dans les trois locales plutôt que recopié ici.
    pub text_hash: Option<u32>,
    /// Vrai si le jeu énumère lui-même ce mode dans ses réglages audio (cf. [`MODES`]).
    pub official: bool,
    /// Ce que les fichiers permettent d'affirmer sur l'état du mode.
    pub note: &'static str,
    /// Sous-chaîne qui identifie, dans les chaînes de `nie.exe`, les clés de message du mode
    /// (`vroad_message_*`, `sysmes_vroad_err_*`…). Les tables de texte ne portent que leur
    /// CRC-32 : sans ce motif, ces messages restent introuvables. `None` = mode dont les
    /// messages ne sont pas nommés à part.
    pub key_pattern: Option<&'static str>,
}

/// Les modes, chacun adossé à des écrans réels du VFS.
///
/// **Les cinq modes marqués `official` ne sont pas un choix éditorial** : le jeu les énumère
/// lui-même dans `menu_text`, via trois familles de réglages concordantes — « BGM Volume (X) »,
/// « Character Voice Volume (X) » et « Power List Display (X) ». Cette liste a corrigé la
/// première version de ce fichier : `Competition Mode` y manquait, et le mode nommé
/// « Kizuna Station » y était confondu avec le lieu « Bond Town » (FR « Ville Kizuna »), qui est
/// un libellé distinct.
///
/// Les autres entrées sont des écrans utilitaires du menu principal — utiles à cataloguer, mais
/// que le jeu ne compte pas parmi ses modes.
///
/// `icon_region` n'est renseignée que pour les tuiles identifiées **visuellement** sur une
/// capture du menu ; les autres restent `None` plutôt que devinées.
pub const MODES: &[ModeDef] = &[
    ModeDef {
        slug: "victory-road",
        label: "Victory Road",
        prefixes: &[
            "victory_road",
            "victory_load",
            "victory_lode",
            "fake_vroad",
            "vroad_",
            "fade_menu_encount_victory_road",
        ],
        icon_region: Some("mode_base04"),
        text_hash: Some(0x80cd_176b),
        official: true,
        note: "Tournoi en ligne en trois phases (inscription, qualifications, classement final). \
               Les ecrans `fake_vroad_*` sont des MAQUETTES posees sous soccer99_*, sans texture \
               propre ; le mode lui-meme ne l'est pas : ses assets vivent sous \
               `menu/75_vroad/` (vroad01..vroad50) et ses 28 ecrans couvrent entree, tournoi \
               final, classement, recompenses, region, photo et notifications. \
               `VictoryRoad` est l'orthographe canonique cote code — `nie.exe` porte \
               BGMVolVictoryRoad / SEVolVictoryRoad / VoiceVolVictoryRoad et 152 symboles \
               *VictoryRoad* (machines a etats, menus, erreurs reseau `sysmes_vroad_err_*`) ; \
               `VictoryLoad` n'y figure PAS. `victory_load`, `victory_lode` et `vroad` ne sont \
               que des variantes cote assets.",
        key_pattern: Some("vroad"),
    },
    ModeDef {
        slug: "competition",
        label: "Mode Compétition",
        prefixes: &[],
        icon_region: None,
        text_hash: Some(0x6e14_cca7),
        official: true,
        note: "Nomme par `menu_text`, mais AUCUN ecran ne porte ce nom dans le VFS, et le \
               binaire n'a PAS de cle de reglage a son nom : `nie.exe` porte BGMVol/SEVol/\
               VoiceVol pour Chronicle, KizunaStation, Story et VictoryRoad — pas pour lui. \
               Comme les modes en ligne (`lobby`, `ranked`, `bot_match`, tous absents), son \
               contenu n'est pas dans les fichiers installes.",
        key_pattern: None,
    },
    ModeDef {
        slug: "story",
        label: "Histoire",
        prefixes: &["story_mode"],
        icon_region: None,
        text_hash: Some(0x76db_0fff),
        official: true,
        note: "Ecran story_mode_top_menu.",
        key_pattern: Some("story_mode"),
    },
    ModeDef {
        slug: "chronicle",
        label: "Mode Chronique",
        prefixes: &["chronicle_mode"],
        icon_region: Some("mode_base07"),
        text_hash: Some(0xce37_875a),
        official: true,
        note: "Ecrans chronicle_mode_top_menu et chronicle_mode_soccer_vs_menu ; \
               images dediees sous 220_img/ev_chronicle_img (943 fichiers).",
        key_pattern: Some("chronicle"),
    },
    ModeDef {
        slug: "kizuna-station",
        label: "Station Kizuna",
        prefixes: &["kizuna_town"],
        icon_region: None,
        text_hash: Some(0x126c_915e),
        official: true,
        note: "Le MODE s'appelle « Station Kizuna » ; le LIEU qu'il ouvre est « Ville Kizuna » \
               (EN Bond Town), un libelle distinct. Ses ecrans portent le prefixe kizuna_town.",
        key_pattern: Some("kizuna"),
    },
    ModeDef {
        slug: "chara-edit",
        label: "Éditeur d'avatar",
        prefixes: &["chara_edit"],
        icon_region: None,
        text_hash: None,
        official: false,
        note: "Editeur de personnage joueur (creation d'avatar). 42 ecrans `chara_edit_*_setting` \
               (menu racine, modele, liste, recette, parts par categorie, 14 grilles de couleur \
               10x4/12x5/13x5) et 51 scripts `chara_edit_*.lua`. Ses assets d'interface vivent \
               sous `menu/161_avatar/` (avatar01..avatar03) ; ses modeles et textures de parts \
               sous `chr/_face/20_EDIT/`. Le catalogue de donnees, lui, est adosse a \
               `chara_edit_<ver>.cfg.bin` — cf. `niers avatar`. Aucun libelle de mode ne lui est \
               attribue dans `menu_text` : ce n'est pas une tuile du menu principal mais un \
               editeur ouvert depuis un autre mode, d'ou `official: false` et `text_hash: None`.",
        key_pattern: Some("chara_edit"),
    },
    ModeDef {
        slug: "soccer",
        label: "Match",
        prefixes: &["soccer_top_menu", "soccer_game_mode"],
        icon_region: Some("mode_base03"),
        text_hash: Some(0x848d_75db),
        official: false,
        note: "Entree des matchs (crampons + ballon sur la tuile). Le jeu ne le compte pas \
               parmi les modes de ses reglages audio.",
        key_pattern: None,
    },
    ModeDef {
        slug: "bb-stadium",
        label: "BB Stadium",
        prefixes: &["bb_stadium"],
        icon_region: Some("mode_base10"),
        text_hash: None,
        official: false,
        note: "Tuile au logo `BB`.",
        key_pattern: Some("bb_stadium"),
    },
    ModeDef {
        slug: "play-guide",
        label: "Guide de jeu",
        prefixes: &["play_guide"],
        icon_region: Some("mode_base05"),
        text_hash: None,
        official: false,
        note: "Tuile au livre marque d'un point d'exclamation.",
        key_pattern: Some("play_guide"),
    },
    ModeDef {
        slug: "setting",
        label: "Paramètres",
        prefixes: &["setting_top_menu"],
        icon_region: Some("mode_base06"),
        text_hash: Some(0x82c9_a2b3),
        official: false,
        note: "Tuile a l'engrenage.",
        key_pattern: None,
    },
    ModeDef {
        slug: "information",
        label: "Informations",
        prefixes: &["information_top_menu", "information_"],
        icon_region: Some("mode_base09"),
        text_hash: Some(0x1796_88e8),
        official: false,
        note: "Tuile au `i`.",
        key_pattern: None,
    },
    ModeDef {
        slug: "team-dock",
        label: "Équipe",
        prefixes: &["team_dock"],
        icon_region: None,
        text_hash: Some(0x7aae_281e),
        official: false,
        note: "Ecran commun de gestion d'equipe.",
        key_pattern: Some("team_dock"),
    },
];

/// Retourne les modes auxquels un écran est rattaché par ses préfixes réels.
///
/// Une liste vide est un résultat utile : le catalogue contient aussi des écrans communs ou
/// utilitaires qui ne sont pas des tuiles de mode. Plusieurs résultats signalent au contraire
/// un recouvrement éditorial qui doit rester visible dans l'audit.
fn classify_screen(stem: &str) -> Vec<&'static str> {
    MODES
        .iter()
        .filter(|def| matches(def, stem))
        .map(|def| def.slug)
        .collect()
}

/// Audite le rattachement de chaque définition `*_menu_setting.cfg.bin` du VFS aux modes.
///
/// Le rendu et `mode index` consomment les mêmes fichiers, mais leurs anciennes sorties ne
/// permettaient pas de vérifier leur couverture ensemble. Cette sortie est volontairement
/// indépendante de SQLite : elle peut être comparée directement au JSON `--menu-matrix` de
/// `nie-game` sans recréer un catalogue intermédiaire.
pub fn menu_coverage_json(vfs: &Vfs) -> Json {
    let mut screens: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, _) in vfs.iter() {
        if !path.starts_with("data/common/gamedata/menu/cfg/")
            || !path.ends_with("_setting.cfg.bin")
        {
            continue;
        }
        let Some(stem) = path
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix("_setting.cfg.bin"))
        else {
            continue;
        };
        screens
            .entry(stem.to_string())
            .or_default()
            .push(path.to_string());
    }

    let mut mode_counts: BTreeMap<&str, usize> = MODES.iter().map(|def| (def.slug, 0)).collect();
    let mut classified = 0usize;
    let mut overlapping = Vec::new();
    let mut unclassified = Vec::new();
    let mut duplicates = Vec::new();

    for (screen, paths) in &screens {
        if paths.len() > 1 {
            duplicates.push(serde_json::json!({
                "screen": screen,
                "paths": paths,
            }));
        }
        let modes = classify_screen(screen);
        if modes.is_empty() {
            unclassified.push(serde_json::json!({
                "screen": screen,
                "paths": paths,
            }));
        } else {
            classified += 1;
            for mode in &modes {
                *mode_counts
                    .get_mut(mode)
                    .expect("classify_screen only returns MODES slugs") += 1;
            }
            if modes.len() > 1 {
                overlapping.push(serde_json::json!({
                    "screen": screen,
                    "modes": modes,
                    "paths": paths,
                }));
            }
        }
    }

    let modes = MODES
        .iter()
        .map(|def| {
            serde_json::json!({
                "slug": def.slug,
                "official": def.official,
                "screens": mode_counts[def.slug],
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema": "niers.menu.coverage/v1",
        "settings": {
            "unique": screens.len(),
            "classified": classified,
            "unclassified": unclassified.len(),
            "overlapping": overlapping.len(),
            "duplicateStems": duplicates.len(),
        },
        "modes": modes,
        "unclassifiedScreens": unclassified,
        "overlappingScreens": overlapping,
        "duplicateScreens": duplicates,
    })
}

/// Locales dont on résout le libellé officiel.
const LOCALES: [&str; 3] = ["fr", "en", "ja"];

/// Charge `menu_text` d'une locale : `hash` → libellé.
///
/// Le fichier porte `TEXT_INFO_BEGIN > TEXT_INFO [hash, 0, texte]`. Absence = locale non
/// installee, ce qui n'est pas une erreur : le catalogue retombe sur le libellé de repli.
fn charger_menu_text(vfs: &Vfs, locale: &str) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    let path = format!("data/common/text/{locale}/menu_text.cfg.bin");
    let Ok(bytes) = vfs.read(&path) else {
        return out;
    };
    let Ok(file) = cfgbin::parse_t2b(&bytes) else {
        return out;
    };
    walk(&file.entries, &mut |e: &CfgEntry| {
        if !e.name.starts_with("TEXT_INFO") || e.name.contains("BEGIN") || e.name.contains("END") {
            return;
        }
        let hash = e.variables.iter().find_map(|v| match v {
            Value::Int(i) => Some(u32::from_ne_bytes(i.to_ne_bytes())),
            _ => None,
        });
        let texte = e.variables.iter().rev().find_map(|v| match v {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        });
        if let (Some(h), Some(t)) = (hash, texte) {
            out.insert(h, t);
        }
    });
    out
}

/// Ce qui a été trouvé pour un mode.
#[derive(Default)]
pub struct ModeFacts {
    /// Écrans `*_setting.cfg.bin` (stem → chemin VFS).
    pub screens: BTreeMap<String, String>,
    /// Calques déclarés par ces écrans.
    pub layers: BTreeSet<String>,
    /// Objbin résolus depuis les calques.
    pub objbins: BTreeSet<String>,
    /// `g4pkm` référencés par ces objbin.
    pub g4pkm: BTreeSet<String>,
    /// `g4tx` référencés (SETUP ou paramètre de composant).
    pub g4tx: BTreeSet<String>,
    /// Types de composants rencontrés (noms de classes RTTI).
    pub components: BTreeSet<String>,
    /// Scripts Lua de même préfixe.
    pub lua: BTreeSet<String>,
    /// Nombre d'éléments focusables cumulés.
    pub focus: usize,
    /// Slots de texte des composants `MenuTextSetting` : `(objet, slot, hash)`.
    ///
    /// Le hash se résout dans `menu_text` ; beaucoup pointent des guides de boutons
    /// (`<CMD_BACK|10>`), ce qui est une donnée en soi — c'est l'UI de l'écran.
    pub text_slots: BTreeSet<(String, String, u32)>,
}

fn first_string(e: &CfgEntry) -> Option<&str> {
    e.variables.iter().find_map(|v| match v {
        Value::String(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    })
}

fn walk<'a>(entries: &'a [CfgEntry], f: &mut impl FnMut(&'a CfgEntry)) {
    for e in entries {
        f(e);
        walk(&e.children, f);
    }
}

/// Vrai si `stem` relève d'un des préfixes du mode.
fn matches(def: &ModeDef, stem: &str) -> bool {
    def.prefixes.iter().any(|p| stem.starts_with(p))
}

/// Récolte les faits d'un mode depuis le VFS.
pub fn collect(vfs: &Vfs, def: &ModeDef) -> ModeFacts {
    let mut facts = ModeFacts::default();

    // Index des chemins utiles, figé avant lecture (`iter` emprunte le VFS).
    let mut cfg_paths: Vec<String> = Vec::new();
    let mut obj_paths: BTreeMap<String, String> = BTreeMap::new();
    for (path, _) in vfs.iter() {
        if path.starts_with("data/common/gamedata/menu/cfg/") && path.ends_with("_setting.cfg.bin")
        {
            cfg_paths.push(path.to_string());
        } else if path.starts_with("data/common/gamedata/menu/obj/") && path.ends_with(".objbin") {
            if let Some(stem) = path
                .rsplit('/')
                .next()
                .and_then(|f| f.strip_suffix(".objbin"))
            {
                obj_paths.insert(stem.to_string(), path.to_string());
            }
        } else if path.contains("/script/lua/")
            && path.ends_with(".lua.bin")
            && let Some(stem) = path
                .rsplit('/')
                .next()
                .and_then(|f| f.strip_suffix(".lua.bin"))
        {
            // Les scripts portent parfois un suffixe de version (`_1.02.92.00`) : on teste le
            // nom complet ET sa racine, sinon `main_menu_1.02.92.00` échapperait au préfixe.
            let base = stem
                .split_once(char::is_numeric)
                .map_or(stem, |(a, _)| a.trim_end_matches('_'));
            if matches(def, stem) || matches(def, base) {
                facts.lua.insert(path.to_string());
            }
        }
    }

    for path in cfg_paths {
        let Some(stem) = path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix("_setting.cfg.bin"))
        else {
            continue;
        };
        if !matches(def, stem) {
            continue;
        }
        let Ok(bytes) = vfs.read(&path) else { continue };
        let Ok(file) = cfgbin::parse_t2b(&bytes) else {
            continue;
        };
        facts.screens.insert(stem.to_string(), path.clone());

        walk(&file.entries, &mut |e: &CfgEntry| {
            if e.name.contains("LIST_BEG") || e.name.contains("LIST_END") {
                return;
            }
            if e.name.starts_with("MENU_LAYER_INFO")
                && let Some(n) = first_string(e)
            {
                facts.layers.insert(n.to_string());
            } else if e.name.starts_with("MENU_FOCUS_BASE_INFO") {
                facts.focus += 1;
            }
        });
    }

    // Calques -> objbin -> assets. Un calque nomme son objbin (même stem).
    for layer in facts.layers.clone() {
        let Some(p) = obj_paths.get(&layer) else {
            continue;
        };
        facts.objbins.insert(p.clone());
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        if let Some(g) = &obj.g4pkm_path {
            facts.g4pkm.insert(g.clone());
        }
        if let Some(t) = &obj.g4tx_path {
            facts.g4tx.insert(t.clone());
        }
        for c in &obj.components {
            facts.components.insert(component_type_name(c).to_string());
            match c {
                // Depuis le correctif de préservation typée, un composant non reconnu expose ses
                // chaînes : c'est là que vivent les chemins de texture (`m_texPath`).
                objbin::MenuComponent::Unknown(u) => {
                    for s in u.strings() {
                        if s.ends_with(".g4tx") {
                            facts.g4tx.insert(s.to_string());
                        }
                    }
                }
                // Le pont UI -> texte : chaque slot porte le CRC-32 de son libellé.
                objbin::MenuComponent::Text(t) => {
                    for e in &t.entries {
                        for h in &e.hashes {
                            if *h != 0 {
                                facts
                                    .text_slots
                                    .insert((obj.name.clone(), e.key.clone(), *h));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    facts
}

/// Table `cmdId -> VA handler` de `data/re/funclua-cmdid-handlers.json`, ou vide si absente.
///
/// Remonte depuis le répertoire courant (même convention que `fichier_re` dans l'exemple
/// `couverture_funclua`) : le fichier est gitignoré (© LEVEL-5, dump de reverse), donc absent
/// tant qu'il n'a pas été régénéré sur la machine — son absence n'est pas une erreur, juste un
/// enrichissement en moins.
fn charger_handlers_funclua() -> BTreeMap<u32, u64> {
    let mut out = BTreeMap::new();
    let Ok(cwd) = std::env::current_dir() else {
        return out;
    };
    let mut courant: &std::path::Path = &cwd;
    let chemin = loop {
        let candidat = courant.join("data/re/funclua-cmdid-handlers.json");
        if candidat.is_file() {
            break candidat;
        }
        match courant.parent() {
            Some(p) => courant = p,
            None => return out,
        }
    };
    let Ok(texte) = std::fs::read_to_string(&chemin) else {
        return out;
    };
    let Ok(brut) = serde_json::from_str::<BTreeMap<String, String>>(&texte) else {
        return out;
    };
    for (k, v) in brut {
        let (Some(id), Some(va)) = (
            k.strip_prefix("0x")
                .and_then(|h| u32::from_str_radix(h, 16).ok()),
            v.strip_prefix("0x")
                .and_then(|h| u64::from_str_radix(h, 16).ok()),
        ) else {
            continue;
        };
        out.insert(id, va);
    }
    out
}

/// Analyse **byte-exacte** d'un script `.lua.bin` : désassemble le conteneur bytecode Lua 5.2
/// réel ([`nie_lua::bytecode::parse`], PAS un décompilateur externe — cf. tête de ce module dans
/// `nie-lua`) et en extrait ce qui intéresse une fiche de mode : nombre d'instructions/fonctions,
/// modules `INCLUDE`d, et les commandes `funcLuaMenuCommand` que le script est **structurellement
/// capable d'émettre** (un entier constant du pool qui correspond à un `cmdId` connu du dump de
/// reverse — un faux positif exigerait une collision de hash 32 bits, négligeable sur ~3 700
/// entrées).
///
/// Renvoie `{"erreur": ...}` si le fichier n'est pas un bytecode Lua 5.2 reconnu, plutôt qu'un
/// objet vide qui se ferait passer pour « rien à signaler ».
fn analyse_lua(bytes: &[u8], handlers: &BTreeMap<u32, u64>) -> Json {
    let chunk = match bytecode::parse(bytes) {
        Ok(c) => c,
        Err(e) => return serde_json::json!({ "erreur": e.to_string() }),
    };

    // Parcourt le prototype principal ET tous les prototypes imbriqués : chacun a son propre
    // pool de constantes en Lua 5.2 (pas un pool global partagé par chunk).
    let mut includes = BTreeSet::new();
    let mut cmd_ids = BTreeSet::new();
    fn walk_proto(
        p: &bytecode::Prototype,
        includes: &mut BTreeSet<String>,
        cmd_ids: &mut BTreeSet<u32>,
        handlers: &BTreeMap<u32, u64>,
    ) {
        for c in &p.constants {
            match c {
                bytecode::Constant::String(s) => {
                    // Les modules partagés du moteur portent tous ce préfixe (`LUA_MENU_DEF`,
                    // `LUA_LISTVIEW_INC`…) : c'est la même convention que lit `INCLUDE()` côté VM
                    // (`nie_lua::lib::install_include`), pas une supposition locale.
                    if let Ok(txt) = core::str::from_utf8(s)
                        && txt.starts_with("LUA_")
                    {
                        includes.insert(txt.to_string());
                    }
                }
                // Les cmdId arrivent en f64 côté Lua (cf. doc `menu_host.rs`) ; on ne retient que
                // les entiers exacts dans l'espace u32 ET présents dans le dump de handlers —
                // sinon un flottant de jeu ordinaire (score, ratio…) pourrait coïncider.
                bytecode::Constant::Number(n)
                    if *n >= 0.0 && n.fract() == 0.0 && *n <= f64::from(u32::MAX) =>
                {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let id = *n as u32;
                    if handlers.contains_key(&id) {
                        cmd_ids.insert(id);
                    }
                }
                _ => {}
            }
        }
        for sub in &p.protos {
            walk_proto(sub, includes, cmd_ids, handlers);
        }
    }
    walk_proto(&chunk.main, &mut includes, &mut cmd_ids, handlers);

    let commandes: Vec<Json> = cmd_ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "cmdId": format!("0x{id:08X}"),
                "nom": nie_lua::menu_host::command_name(*id),
                "handler": handlers.get(id).map(|va| format!("0x{va:X}")),
            })
        })
        .collect();

    serde_json::json!({
        "instructions": chunk.main.total_instructions(),
        "fonctions": chunk.main.total_protos() + 1,
        "includes": includes,
        "commandes": commandes,
    })
}

/// Le **contenu** des fichiers d'un mode, pas seulement leur inventaire.
///
/// `collect` compte et nomme ; ici on ouvre. Chaque écran rend ses calques et ses focusables,
/// chaque `objbin` son objet parsé, chaque `g4tx` ses textures et leurs régions, et les clés de
/// texte du mode — celles que porte `nie.exe`, pas celles des objets — sont résolues dans les
/// tables localisées.
///
/// # Erreurs
///
/// Rend une erreur si le VFS ne s'ouvre pas. Un fichier illisible **individuellement** n'en est
/// pas une : il est simplement absent du résultat, comme dans `collect`.
pub fn contenu_json(vfs: &Vfs, def: &ModeDef, exe: Option<&std::path::Path>) -> Result<Json> {
    let facts = collect(vfs, def);

    // Écrans : on relit le cfg pour rendre ses calques dans l'ordre du fichier, ce que
    // l'ensemble trié de `collect` perd.
    let mut screens = Vec::new();
    for (stem, path) in &facts.screens {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(file) = cfgbin::parse_t2b(&bytes) else {
            continue;
        };
        let (mut layers, mut focus) = (Vec::new(), 0usize);
        walk(&file.entries, &mut |e: &CfgEntry| {
            if e.name.contains("LIST_BEG") || e.name.contains("LIST_END") {
                return;
            }
            if e.name.starts_with("MENU_LAYER_INFO")
                && let Some(n) = first_string(e)
            {
                layers.push(n.to_string());
            } else if e.name.starts_with("MENU_FOCUS_BASE_INFO") {
                focus += 1;
            }
        });
        screens.push(serde_json::json!({
            "screen": stem, "cfg": path, "octets": bytes.len(),
            "layers": layers, "focus": focus,
        }));
    }

    // Objets de menu : l'objet parsé en entier (composants compris).
    let mut objbins = Vec::new();
    for path in &facts.objbins {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        objbins.push(serde_json::json!({
            "path": path, "octets": bytes.len(), "objet": obj,
        }));
    }

    // Textures : dimensions et régions, telles que le `.g4tx` les déclare.
    let mut textures = Vec::new();
    for path in &facts.g4tx {
        // Le chemin catalogué porte `<LG>` pour la locale ; le VFS, lui, veut un chemin réel.
        for candidat in chemins_locale(path) {
            let Ok(bytes) = vfs.read(&candidat) else {
                continue;
            };
            let Ok(atlas) = nie_formats::g4tx::parse(&bytes) else {
                continue;
            };
            let tex: Vec<Json> = atlas
                .textures
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id, "nom": t.name, "largeur": t.width, "hauteur": t.height,
                        "dds": t.is_dds,
                        "regions": t.sub_textures.iter().map(|s| serde_json::json!({
                            "nom": s.name, "x": s.x, "y": s.y, "w": s.width, "h": s.height,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            textures.push(serde_json::json!({
                "path": candidat, "catalogue": path, "octets": bytes.len(), "textures": tex,
            }));
            break;
        }
    }

    let handlers_funclua = charger_handlers_funclua();
    let lua: Vec<Json> = facts
        .lua
        .iter()
        .map(|p| {
            let Ok(bytes) = vfs.read(p) else {
                return serde_json::json!({ "path": p, "octets": 0 });
            };
            let mut entry = serde_json::json!({ "path": p, "octets": bytes.len() });
            let analyse = analyse_lua(&bytes, &handlers_funclua);
            if let (Json::Object(e), Json::Object(a)) = (&mut entry, analyse) {
                e.extend(a);
            }
            entry
        })
        .collect();

    let messages = def.key_pattern.map_or_else(
        || serde_json::json!({}),
        |motif| messages_du_mode(vfs, motif, exe),
    );

    Ok(serde_json::json!({
        "slug": def.slug,
        "label": def.label,
        "screens": screens,
        "objbins": objbins,
        "textures": textures,
        "lua": lua,
        "messages": messages,
    }))
}

/// Chemins réels possibles pour un chemin catalogué portant `<LG>`.
///
/// L'ordre porte une décision : `fr` d'abord — le wiki est francophone —, puis la variante sans
/// dossier de locale, qui est celle des atlas non localisés.
fn chemins_locale(path: &str) -> Vec<String> {
    let prefixe = |p: &str| {
        if p.starts_with("data/") {
            p.to_string()
        } else {
            format!("data/{p}")
        }
    };
    if !path.contains("<LG>") {
        return vec![prefixe(path)];
    }
    let mut v: Vec<String> = ["fr", "en", "ja"]
        .iter()
        .map(|l| prefixe(&path.replace("<LG>", l)))
        .collect();
    v.push(prefixe(&path.replace("<LG>/", "")));
    v
}

/// Résout les clés de texte du mode que porte le binaire, dans chaque locale.
///
/// Les libellés d'écran passent par les slots des `objbin` ; les **messages** du mode, eux
/// (erreurs réseau, confirmations), ne sont nommés que dans `nie.exe` — les tables ne portent
/// que leur CRC-32. On lit donc les chaînes du binaire, on garde celles qui contiennent `motif`,
/// et on les cherche dans toutes les tables de la locale.
fn messages_du_mode(vfs: &Vfs, motif: &str, exe: Option<&std::path::Path>) -> Json {
    let Some(exe) = exe else {
        return serde_json::json!({});
    };
    let Ok(bin) = std::fs::read(exe) else {
        return serde_json::json!({});
    };

    // Chaînes ASCII et UTF-16LE imprimables du binaire, filtrées sur le motif du mode.
    // Les noms des classes/menus Kizuna sont stockés en UTF-16 dans nie.exe ; se limiter à
    // l'ASCII produisait un catalogue de messages vide, alors que les CRC étaient bien présents.
    let mut cles: BTreeSet<String> = BTreeSet::new();
    let mut collect = |s: &str| {
        if s.len() >= 4
            && s.contains(motif)
            && !s.contains('/')
            && !s.contains('.')
            && s.bytes().all(|b| b.is_ascii_graphic() || b == b'_')
        {
            cles.insert(s.to_string());
        }
    };

    let mut ascii = Vec::new();
    for &b in &bin {
        if b.is_ascii_graphic() || b == b'_' {
            ascii.push(b);
        } else {
            if ascii.len() >= 4
                && let Ok(s) = core::str::from_utf8(&ascii)
            {
                collect(s);
            }
            ascii.clear();
        }
    }
    if ascii.len() >= 4
        && let Ok(s) = core::str::from_utf8(&ascii)
    {
        collect(s);
    }

    let mut wide = Vec::new();
    for pair in bin.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if (0x20..=0x7e).contains(&unit) {
            wide.push(unit);
        } else {
            if wide.len() >= 4
                && let Ok(s) = String::from_utf16(&wide)
            {
                collect(&s);
            }
            wide.clear();
        }
    }
    if wide.len() >= 4
        && let Ok(s) = String::from_utf16(&wide)
    {
        collect(&s);
    }

    // Le dictionnaire de RE est la source de vérité lorsque l'exécutable ne conserve que le
    // CRC (cas fréquent pour les tables de texte). Il permet notamment de relier les noms
    // `TEXT_ID_*`, `*_message_*` et les identifiants Kizuna aux entrées localisées.
    if let Ok(cwd) = std::env::current_dir() {
        let mut courant = cwd.as_path();
        'cherche_dico: loop {
            let chemin = courant.join("data/re/menu-crc32-dictionary.json");
            if chemin.is_file() {
                if let Ok(texte) = std::fs::read_to_string(chemin)
                    && let Ok(dico) = serde_json::from_str::<BTreeMap<String, String>>(&texte)
                {
                    for nom in dico.values().filter(|nom| nom.contains(motif)) {
                        collect(nom);
                    }
                }
                break 'cherche_dico;
            }
            match courant.parent() {
                Some(parent) => courant = parent,
                None => break 'cherche_dico,
            }
        }
    }

    let mut out = serde_json::Map::new();
    for locale in LOCALES {
        let prefixe = format!("data/common/text/{locale}/");
        let tables: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                p.starts_with(&prefixe)
                    && p.ends_with(".cfg.bin")
                    && p[prefixe.len()..].find('/').is_none()
            })
            .collect();
        let mut par_cle = serde_json::Map::new();
        for table in tables {
            let Ok(bytes) = vfs.read(&table) else {
                continue;
            };
            let Ok(file) = cfgbin::parse_t2b(&bytes) else {
                continue;
            };
            let mut index: BTreeMap<u32, String> = BTreeMap::new();
            walk(&file.entries, &mut |e: &CfgEntry| {
                if !e.name.starts_with("TEXT_INFO")
                    || e.name.contains("BEGIN")
                    || e.name.contains("END")
                {
                    return;
                }
                let hash = e.variables.iter().find_map(|v| match v {
                    Value::Int(i) => Some(u32::from_ne_bytes(i.to_ne_bytes())),
                    _ => None,
                });
                let texte = e.variables.iter().rev().find_map(|v| match v {
                    Value::String(s) if !s.is_empty() => Some(s.clone()),
                    _ => None,
                });
                if let (Some(h), Some(t)) = (hash, texte) {
                    index.insert(h, t);
                }
            });
            if index.is_empty() {
                continue;
            }
            let court = table.rsplit('/').next().unwrap_or(&table).to_string();
            for cle in &cles {
                if let Some(t) = index.get(&cfgbin::crc32(cle.as_bytes())) {
                    par_cle.insert(
                        cle.clone(),
                        serde_json::json!({ "texte": t, "table": court }),
                    );
                }
            }
        }
        out.insert(locale.to_string(), Json::Object(par_cle));
    }
    Json::Object(out)
}

fn component_type_name(c: &objbin::MenuComponent) -> &str {
    use objbin::MenuComponent as M;
    match c {
        M::Render(x) => &x.type_name,
        M::Animation(x) => &x.type_name,
        M::Text(x) => &x.type_name,
        M::Primitive(x) => &x.type_name,
        M::AttachLocator(x) => &x.type_name,
        M::Collision(x) => &x.type_name,
        M::SoundCmd(x) => &x.type_name,
        M::MeshVisible(x) => &x.type_name,
        M::Unknown(x) => &x.type_name,
    }
}

/// Crée les tables du catalogue si elles manquent.
///
/// Volontairement hors de `schema.sql` (nie-index) : ce catalogue est un produit de l'outillage
/// UI, pas du socle RE, et le poser ici évite de toucher un fichier partagé par d'autres
/// chantiers.
pub fn ensure_schema(conn: &nie_index::rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mode (
            id          INTEGER PRIMARY KEY,
            slug        TEXT NOT NULL UNIQUE,
            label       TEXT NOT NULL,
            label_en    TEXT,
            label_ja    TEXT,
            text_hash   INTEGER,
            official    INTEGER NOT NULL DEFAULT 0,
            icon_atlas  TEXT,
            icon_region TEXT,
            screens     INTEGER NOT NULL DEFAULT 0,
            layers      INTEGER NOT NULL DEFAULT 0,
            focus       INTEGER NOT NULL DEFAULT 0,
            note        TEXT
        );
        CREATE TABLE IF NOT EXISTS mode_screen (
            id       INTEGER PRIMARY KEY,
            mode_id  INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            screen   TEXT NOT NULL,
            cfg_path TEXT NOT NULL,
            UNIQUE(mode_id, screen)
        );
        CREATE TABLE IF NOT EXISTS mode_asset (
            id      INTEGER PRIMARY KEY,
            mode_id INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            kind    TEXT NOT NULL,
            path    TEXT NOT NULL,
            UNIQUE(mode_id, kind, path)
        );
        CREATE INDEX IF NOT EXISTS idx_mode_asset ON mode_asset(mode_id, kind);
        CREATE TABLE IF NOT EXISTS mode_text (
            id      INTEGER PRIMARY KEY,
            mode_id INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            obj     TEXT NOT NULL,
            slot    TEXT NOT NULL,
            hash    INTEGER NOT NULL,
            locale  TEXT NOT NULL,
            text    TEXT NOT NULL,
            UNIQUE(mode_id, obj, slot, hash, locale)
        );
        CREATE INDEX IF NOT EXISTS idx_mode_text ON mode_text(mode_id, locale);",
    )
    .context("création des tables du catalogue de modes")?;
    Ok(())
}

/// Indexe tous les modes et écrit le catalogue. Renvoie (modes, écrans, assets).
pub fn index(db: &nie_index::Db, vfs: &Vfs) -> Result<(usize, usize, usize, usize)> {
    let conn = db.conn();
    ensure_schema(conn)?;
    conn.execute_batch("BEGIN")?;

    // Libellés officiels : le nom que le JEU affiche, dans les trois locales.
    let textes: Vec<(&str, BTreeMap<u32, String>)> = LOCALES
        .iter()
        .map(|lg| (*lg, charger_menu_text(vfs, lg)))
        .collect();
    let libelle = |lg: &str, h: Option<u32>| -> Option<String> {
        let h = h?;
        textes.iter().find(|(l, _)| *l == lg)?.1.get(&h).cloned()
    };

    let (mut n_modes, mut n_screens, mut n_assets, mut n_texts) = (0usize, 0usize, 0usize, 0usize);
    for def in MODES {
        let f = collect(vfs, def);
        // Le libellé du jeu prime sur le nom de repli ; s'il manque, on garde le nôtre.
        let label_fr = libelle("fr", def.text_hash).unwrap_or_else(|| def.label.to_string());
        conn.execute(
            "INSERT INTO mode(slug, label, label_en, label_ja, text_hash, official,
                              icon_atlas, icon_region, screens, layers, focus, note)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(slug) DO UPDATE SET
                label=excluded.label, label_en=excluded.label_en, label_ja=excluded.label_ja,
                text_hash=excluded.text_hash, official=excluded.official,
                icon_atlas=excluded.icon_atlas,
                icon_region=excluded.icon_region, screens=excluded.screens,
                layers=excluded.layers, focus=excluded.focus, note=excluded.note",
            nie_index::rusqlite::params![
                def.slug,
                label_fr,
                libelle("en", def.text_hash),
                libelle("ja", def.text_hash),
                def.text_hash.map(i64::from),
                i64::from(def.official),
                def.icon_region.map(
                    |_| "data/dx11/menu/100_mainmenu/mainmenu90/mainmenu90_01/mainmenu90_01.g4tx"
                ),
                def.icon_region,
                f.screens.len() as i64,
                f.layers.len() as i64,
                f.focus as i64,
                def.note,
            ],
        )?;
        let mode_id: i64 =
            conn.query_row("SELECT id FROM mode WHERE slug=?1", [def.slug], |r| {
                r.get(0)
            })?;

        // Réindexation exacte : les ensembles d'un écran peuvent diminuer après une mise à
        // jour du VFS. `INSERT OR IGNORE` seul conservait alors des assets fantômes dans SQLite,
        // ce qui rendait l'API différente de l'export JSON courant.
        for table in ["mode_screen", "mode_asset", "mode_text"] {
            conn.execute(&format!("DELETE FROM {table} WHERE mode_id=?1"), [mode_id])?;
        }

        for (stem, path) in &f.screens {
            conn.execute(
                "INSERT OR IGNORE INTO mode_screen(mode_id, screen, cfg_path) VALUES(?1,?2,?3)",
                nie_index::rusqlite::params![mode_id, stem, path],
            )?;
            n_screens += 1;
        }
        for (kind, set) in [
            ("layer", &f.layers),
            ("objbin", &f.objbins),
            ("g4pkm", &f.g4pkm),
            ("g4tx", &f.g4tx),
            ("component", &f.components),
            ("lua", &f.lua),
        ] {
            for p in set {
                conn.execute(
                    "INSERT OR IGNORE INTO mode_asset(mode_id, kind, path) VALUES(?1,?2,?3)",
                    nie_index::rusqlite::params![mode_id, kind, p],
                )?;
                n_assets += 1;
            }
        }
        // Textes d'interface de l'écran, résolus dans chaque locale disponible. Un slot dont le
        // hash n'est pas dans `menu_text` n'est PAS inséré : mieux vaut un trou visible qu'une
        // ligne vide qui se ferait passer pour un libellé.
        for (obj, slot, hash) in &f.text_slots {
            for (lg, table) in &textes {
                if let Some(t) = table.get(hash) {
                    conn.execute(
                        "INSERT OR IGNORE INTO mode_text(mode_id, obj, slot, hash, locale, text)
                         VALUES(?1,?2,?3,?4,?5,?6)",
                        nie_index::rusqlite::params![mode_id, obj, slot, i64::from(*hash), lg, t],
                    )?;
                    n_texts += 1;
                }
            }
        }

        n_modes += 1;
        println!(
            "  {} {:<15} ecrans={:<3} calques={:<4} objbin={:<4} g4pkm={:<4} g4tx={:<4} lua={:<3} focus={}",
            if def.official { "*" } else { " " },
            def.slug,
            f.screens.len(),
            f.layers.len(),
            f.objbins.len(),
            f.g4pkm.len(),
            f.g4tx.len(),
            f.lua.len(),
            f.focus
        );
    }

    conn.execute_batch("COMMIT")?;
    Ok((n_modes, n_screens, n_assets, n_texts))
}

/// Exporte le catalogue en JSON (pour azalée).
pub fn export_json(db: &nie_index::Db) -> Result<serde_json::Value> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, slug, label, icon_atlas, icon_region, screens, layers, focus, note,
                label_en, label_ja, official
         FROM mode ORDER BY official DESC, slug",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<String>>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, i64>(11)?,
        ))
    })?;

    let mut modes = Vec::new();
    for row in rows {
        let (id, slug, label, atlas, region, screens, layers, focus, note, en, ja, official) = row?;
        let mut screens_v = Vec::new();
        let mut s = conn
            .prepare("SELECT screen, cfg_path FROM mode_screen WHERE mode_id=?1 ORDER BY screen")?;
        for r in s.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (screen, cfg) = r?;
            screens_v.push(serde_json::json!({ "screen": screen, "cfg": cfg }));
        }
        let mut assets = serde_json::Map::new();
        let mut a =
            conn.prepare("SELECT kind, path FROM mode_asset WHERE mode_id=?1 ORDER BY kind, path")?;
        for r in a.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (kind, path) = r?;
            assets
                .entry(kind)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .expect("tableau")
                .push(serde_json::Value::String(path));
        }
        // Textes d'interface, regroupés par locale.
        let mut textes = serde_json::Map::new();
        let mut t = conn.prepare(
            "SELECT locale, obj, slot, text FROM mode_text WHERE mode_id=?1
             ORDER BY locale, obj, slot",
        )?;
        for r in t.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (locale, obj, slot, texte) = r?;
            textes
                .entry(locale)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .expect("tableau")
                .push(serde_json::json!({ "obj": obj, "slot": slot, "text": texte }));
        }

        modes.push(serde_json::json!({
            "texts": textes,
            "slug": slug,
            "label": label,
            "labelEn": en,
            "labelJa": ja,
            "official": official != 0,
            "icon": { "atlas": atlas, "region": region },
            "counts": { "screens": screens, "layers": layers, "focus": focus },
            "note": note,
            "screens": screens_v,
            "assets": assets,
        }));
    }
    Ok(serde_json::json!({ "modes": modes }))
}

#[cfg(test)]
mod tests {
    use super::{MODES, classify_screen};

    #[test]
    fn coverage_keeps_empty_prefix_modes_unassigned() {
        assert!(
            classify_screen("competition").is_empty(),
            "un mode sans préfixe ne doit pas capturer un stem"
        );
    }

    #[test]
    fn coverage_uses_the_same_prefixes_as_mode_index() {
        assert_eq!(classify_screen("chara_edit_top"), vec!["chara-edit"]);
        assert_eq!(
            classify_screen("victory_road_top_menu"),
            vec!["victory-road"]
        );
        assert_eq!(MODES.iter().filter(|def| def.official).count(), 5);
    }
}
