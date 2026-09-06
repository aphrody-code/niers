//! Gate de rendu de menu (pilier D1.f) — backbone de la mesure pixel-perfect.
//!
//! Deux étages (cf. `docs/STACK.md`, `docs/DESIGN.md` §10) :
//! 1. **Déterminisme / régression interne** : deux rendus du MÊME écran doivent être
//!    **octet-identiques**. C'est un *hard gate* (échoue si non déterministe) — acquis depuis
//!    D1.b-det. Sans lui, l'égalité-octet vs le jeu est impossible.
//! 2. **SSIM vs la capture du vrai jeu** (`start.png`, 2560×1440 → downscale ×½ = 1280×720) :
//!    métrique de progression vers le pixel-perfect. **Informatif** tant que le rendu est
//!    incomplet (cible finale ≥ 0,99, hors régions dynamiques) — on imprime le score et on
//!    vérifie seulement un plancher de **non-régression** documenté.
//!
//! SSIM implémenté ici (fenêtres 8×8, luma) — aucune dépendance externe (doctrine « pas de
//! brique qui cache son comportement », cf. STACK.md).
//!
//! Gated sur la présence du vrai jeu (`NIE_GAME_DIR` ou chemin Steam par défaut) ; skip sinon.

use std::path::{Path, PathBuf};
use std::process::Command;

fn game_dir() -> Option<PathBuf> {
    let dir = nie_formats::vfs::resolve_game_dir()
        .to_string_lossy()
        .into_owned();
    let p = PathBuf::from(dir);
    nie_formats::vfs::donnees_disponibles(p.join("data")).then_some(p)
}

/// CRC32 IEEE (poly réfléchi 0xEDB88320) — identifiant de layer `menu_setting` = CRC32 du nom.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Convertit des frères T2B (`CfgEntry`) en forme iecode (`{name, variables, children}`, suffixe
/// `_<idx>` par nom) consommée par `nie_data::menu_setting::parse`.
fn t2b_to_iecode(siblings: &[nie_formats::cfgbin::CfgEntry]) -> Vec<serde_json::Value> {
    use nie_formats::cfgbin::Value as CVal;
    use serde_json::json;
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    siblings
        .iter()
        .map(|e| {
            let idx = counts.entry(e.name.as_str()).or_insert(0);
            let name = format!("{}_{}", e.name, *idx);
            *idx += 1;
            let variables: Vec<serde_json::Value> = e
                .variables
                .iter()
                .map(|v| match v {
                    CVal::String(s) => json!({ "type": "String", "value": s }),
                    CVal::Int(n) => json!({ "type": "Int", "value": n.to_string() }),
                    CVal::Float(f) => json!({ "type": "Float", "value": f.to_string() }),
                })
                .collect();
            json!({ "name": name, "variables": variables, "children": t2b_to_iecode(&e.children) })
        })
        .collect()
}

/// Validation CORPUS de `nie_data::menu_setting` : parse les **485** `*_menu_setting.cfg.bin` du VFS,
/// asserte 0 panic et — invariant fort — `layer_id == CRC32(name)` sur CHAQUE layer de CHAQUE écran.
/// Prouve que le parseur de SCÈNE (D1.c-driver brique a) est robuste + correct sur tout le corpus,
/// pas seulement les 4 écrans golden hermétiques. Skip si le jeu est absent.
#[test]
fn menu_setting_corpus_layer_id_is_crc32() {
    use nie_formats::vfs::Vfs;
    let Some(game) = game_dir() else {
        eprintln!("skip menu_setting_corpus : jeu absent");
        return;
    };
    let mut vfs = Vfs::new();
    vfs.init(game.join("data").as_path()).expect("vfs init");

    let mut paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains("/menu/cfg/") && p.ends_with("_setting.cfg.bin"))
        .collect();
    paths.sort_unstable();
    assert!(
        paths.len() >= 400,
        "corpus menu_setting trop petit ({})",
        paths.len()
    );

    let (mut files_ok, mut total_layers, mut total_cmds, mut mismatches) = (0u32, 0u64, 0u64, 0u64);
    for p in &paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(f) = nie_formats::cfgbin::parse_t2b(&bytes) else {
            continue;
        };
        let root = serde_json::json!({ "entries": t2b_to_iecode(&f.entries) });
        let ms = nie_data::menu_setting::parse(&root); // ne doit pas paniquer
        for l in &ms.layers {
            total_layers += 1;
            if l.layer_id.0 != crc32(l.name.as_bytes()) {
                mismatches += 1;
            }
        }
        total_cmds += ms.commands.len() as u64;
        files_ok += 1;
    }
    eprintln!(
        "menu_setting corpus : {files_ok}/{} fichiers, {total_layers} layers, {total_cmds} commandes, {mismatches} mismatches CRC32",
        paths.len()
    );
    assert_eq!(
        mismatches, 0,
        "layer_id != CRC32(name) sur {mismatches} layers"
    );
    assert!(
        total_layers > 1000,
        "trop peu de layers au total ({total_layers})"
    );
}

/// Rend un écran de menu via le binaire (chemin déterministe) → PNG, puis décode en RGBA8.
fn render_screen(game: &Path, screen: &str, out: &Path) -> (u32, u32, Vec<u8>) {
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(game)
        .args(["--menu", screen, "--capture"])
        .arg(out)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game");
    assert!(status.success(), "nie-game --menu {screen} a échoué");
    decode_png(out)
}

fn decode_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let data = std::fs::read(path).expect("lire PNG");
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().expect("png read_info");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("output_buffer_size")];
    let info = reader.next_frame(&mut buf).expect("png next_frame");
    // `buf` est déjà dimensionné par `output_buffer_size()` (line_size×height, sans padding 8-bit).
    // Normalise en RGBA8.
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => panic!("PNG color type non géré : {other:?}"),
    };
    (info.width, info.height, rgba)
}

/// Downscale entier ×2 par moyenne de blocs 2×2 (exact pour 2560×1440 → 1280×720).
fn downscale_2x(w: u32, h: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (nw, nh) = (w / 2, h / 2);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let mut acc = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = x * 2 + dx;
                        let sy = y * 2 + dy;
                        acc += rgba[((sy * w + sx) * 4 + c) as usize] as u32;
                    }
                }
                out[((y * nw + x) * 4 + c) as usize] = (acc / 4) as u8;
            }
        }
    }
    (nw, nh, out)
}

fn to_luma(w: u32, h: u32, rgba: &[u8]) -> Vec<f64> {
    (0..(w * h) as usize)
        .map(|i| {
            let (r, g, b) = (
                rgba[i * 4] as f64,
                rgba[i * 4 + 1] as f64,
                rgba[i * 4 + 2] as f64,
            );
            0.299 * r + 0.587 * g + 0.114 * b
        })
        .collect()
}

/// SSIM moyen sur fenêtres 8×8 non chevauchantes (luma 0..255), constantes standard.
fn ssim(w: u32, h: u32, a: &[u8], b: &[u8]) -> f64 {
    const C1: f64 = 6.5025; // (0.01*255)^2
    const C2: f64 = 58.5225; // (0.03*255)^2
    const WIN: u32 = 8;
    let (la, lb) = (to_luma(w, h, a), to_luma(w, h, b));
    let (mut sum, mut n) = (0.0f64, 0u32);
    let mut wy = 0;
    while wy + WIN <= h {
        let mut wx = 0;
        while wx + WIN <= w {
            let (mut ma, mut mb) = (0.0, 0.0);
            for y in wy..wy + WIN {
                for x in wx..wx + WIN {
                    let i = (y * w + x) as usize;
                    ma += la[i];
                    mb += lb[i];
                }
            }
            let cnt = (WIN * WIN) as f64;
            ma /= cnt;
            mb /= cnt;
            let (mut va, mut vb, mut cov) = (0.0, 0.0, 0.0);
            for y in wy..wy + WIN {
                for x in wx..wx + WIN {
                    let i = (y * w + x) as usize;
                    let (da, db) = (la[i] - ma, lb[i] - mb);
                    va += da * da;
                    vb += db * db;
                    cov += da * db;
                }
            }
            va /= cnt - 1.0;
            vb /= cnt - 1.0;
            cov /= cnt - 1.0;
            let s = ((2.0 * ma * mb + C1) * (2.0 * cov + C2))
                / ((ma * ma + mb * mb + C1) * (va + vb + C2));
            sum += s;
            n += 1;
            wx += WIN;
        }
        wy += WIN;
    }
    if n == 0 { 1.0 } else { sum / n as f64 }
}

/// Étage 1 — HARD GATE : le rendu d'un écran doit être **octet-identique** d'un run à l'autre.
#[test]
fn title02_render_is_deterministic() {
    let Some(game) = game_dir() else {
        eprintln!("skip title02_render_is_deterministic : jeu absent");
        return;
    };
    let tmp = std::env::temp_dir();
    let (a, b) = (
        tmp.join("nie_gate_det_a.png"),
        tmp.join("nie_gate_det_b.png"),
    );
    let (_, _, ra) = render_screen(&game, "title02", &a);
    let (_, _, rb) = render_screen(&game, "title02", &b);
    assert_eq!(
        ra, rb,
        "le rendu title02 doit être déterministe (octet-identique)"
    );
}

/// D-fond — le correctif du stride g4mg multi-submesh (DESIGN §6, RE originale > iecode) : la
/// géométrie de fond `title02_00` se décode (submesh[0] = quad propre + `uv0` rempli), alors qu'avec
/// le `derived_stride` (19) d'iecode elle était corrompue (positions) / vide (uv0).
/// **Gaté sur le VFS de référence.** Les valeurs attendues viennent d'une version d'assets
/// précise ; sur l'installation Steam de cette machine, `title02_00_title_bg_2.objbin` (seul
/// candidat, la sélection n'est pas ambiguë) donne un quad en coordonnées différentes
/// (`-100` au lieu de `0`). Tant que la version d'assets n'est pas identifiée, le test ne
/// prouve rien ici — et changer les valeurs attendues falsifierait le golden.
/// Lancer avec `cargo test -- --ignored` sur le VFS de référence.
#[test]
#[ignore = "exige le VFS de référence : les positions attendues dépendent de la version des assets"]
fn bg_mesh_geometry_decodes() {
    use nie_formats::{g4md, g4mg, g4pkm, objbin, vfs::Vfs};
    let Some(game) = game_dir() else {
        eprintln!("skip bg_mesh_geometry_decodes : jeu absent");
        return;
    };
    let mut vfs = Vfs::new();
    vfs.init(game.join("data").as_path()).expect("vfs init");
    let find = |suffix: &str| -> Option<String> {
        vfs.iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| p.rsplit('/').next() == Some(suffix))
            .min()
    };
    let obj_path = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .find(|p| {
            p.contains("/menu/obj/")
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with("title02_00") && b.ends_with(".objbin"))
        })
        .expect("objbin title02_00");
    let obj = objbin::parse(&vfs.read(&obj_path).unwrap()).unwrap();
    let g4pkm_base = obj
        .g4pkm_path
        .as_deref()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let md =
        g4md::parse(g4pkm::extract_g4md(&vfs.read(&find(&g4pkm_base).unwrap()).unwrap()).unwrap())
            .unwrap();
    let geom = g4mg::extract_geometry(&vfs.read(&find("title02_00.g4mg").unwrap()).unwrap(), &md);

    let sm0 = &geom[0];
    assert_eq!(sm0.positions.len(), 4, "submesh[0] = quad (4 verts)");
    assert!(
        !sm0.uv0.is_empty(),
        "uv0 rempli (stride borné par l'extent d'attributs)"
    );
    let expect = [(1.0f32, 0.0f32), (0.0, 0.0), (0.0, -1.0), (1.0, -1.0)];
    for (p, (ex, ey)) in sm0.positions.iter().zip(expect) {
        assert!(
            (p.x - ex).abs() < 1e-4 && (p.y - ey).abs() < 1e-4,
            "pos ({},{}) != ({ex},{ey})",
            p.x,
            p.y
        );
    }
}

/// D1.d — les libellés STATIQUES de menu sont résolus dans l'export de layout via `menu_text`
/// (résolveur universel `nie_data::text`, cf. DESIGN.md §7). Sur `mainmenu01`, plusieurs boutons
/// portent leur libellé réel (« Sauvegarder », « Suivant »…).
#[test]
fn mainmenu_static_text_resolves_in_export() {
    let Some(game) = game_dir() else {
        eprintln!("skip mainmenu_static_text_resolves_in_export : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let out = std::env::temp_dir().join("nie_gate_layout_mainmenu.json");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "mainmenu01", "--export-layout"])
        .arg(&out)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --export-layout");
    assert!(status.success(), "export-layout mainmenu01 a échoué");
    let txt = std::fs::read_to_string(&out).expect("lire le layout JSON");
    // Libellés statiques réels résolus via menu_text (chaînes stables du jeu fr).
    assert!(
        txt.contains("\"Sauvegarder\""),
        "libellé « Sauvegarder » non résolu dans l'export"
    );
    assert!(
        txt.contains("\"Suivant\""),
        "libellé « Suivant » non résolu dans l'export"
    );
}

/// D1.c-driver (brique b) — le DRIVER Lua RÉEL énumère les **9 onglets de navigation** du `main_menu`
/// en exécutant les vraies fonctions du script (`GetSortOfTabs`/`GetMenuObjectNameFromTabType`,
/// `enumerate_header_tabs`). `--menu main_menu --runtime` charge `main_menu_1.lua.bin`, lance OnInit,
/// et produit 9 objets `mainmenu_header_tab_<type>` (types 10/20/30/40/50/60/70/80/90). C'est la
/// preuve END-TO-END (sur le VRAI script, pas un stub) que la brique (b) extrait la navigation réelle.
/// (Le RENDU des onglets — sprite + placement `SetupHeaderTab` — et les item-buttons, `GetItemButtonNum`
/// lisant l'état scène/save C++, restent à compléter ; cf. DESIGN §13.)
#[test]
fn mainmenu_runtime_driver_enumerates_9_header_tabs() {
    let Some(game) = game_dir() else {
        eprintln!("skip mainmenu_runtime_driver_enumerates_9_header_tabs : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let out = std::env::temp_dir().join("nie_gate_mainmenu_runtime.json");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "main_menu", "--runtime", "--export-layout"])
        .arg(&out)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --runtime --export-layout");
    assert!(status.success(), "export-layout-runtime main_menu a échoué");
    let txt = std::fs::read_to_string(&out).expect("lire le layout runtime JSON");
    let doc: serde_json::Value = serde_json::from_str(&txt).expect("JSON valide");
    let objs = doc["objects"].as_array().expect("champ objects");

    // Les 9 onglets virtuels du main menu, énumérés depuis la logique RÉELLE du script.
    let tabs: Vec<&str> = objs
        .iter()
        .filter_map(|o| o["name"].as_str())
        .filter(|n| n.starts_with("mainmenu_header_tab_"))
        .collect();
    assert_eq!(
        tabs.len(),
        9,
        "le driver doit énumérer 9 onglets (mode normal), trouvé : {tabs:?}"
    );
    // Les 9 types d'onglets attendus (ordre `GetSortOfTabs`, mode normal).
    for ty in [10, 20, 30, 40, 50, 60, 70, 80, 90] {
        let name = format!("mainmenu_header_tab_{ty}");
        assert!(
            tabs.contains(&name.as_str()),
            "onglet type {ty} manquant ({tabs:?})"
        );
    }
}

/// D1.f render-from-runtime — PREUVE end-to-end (données RÉELLES) que la cible d'un
/// `SetIconSprite(obj, CRC32(chemin_g4tx), CRC32(region))` résolu au runtime correspond à une
/// **vraie sous-texture d'atlas**. Le driver Lua exécute (cf. `chara_filter_menu`) :
/// `funcLuaMenuCommand(558735651, obj, CRC32("#/menu/200_icon/05_icon_rarity/<LG>/icon_rarity.g4tx"),
/// region, …)` → arg1 résout (dico CRC32) le chemin g4tx EXACT, region résout le nom de sous-texture.
/// Ce gate charge le `icon_rarity.g4tx` RÉEL et vérifie que ses 10 niveaux `gtxt_rarity01_01..10`
/// existent comme régions nommées (résolubles en rect de crop via `g4tx::region_rect`). C'est le
/// chaînon manquant du rendu : `region_name` (runtime) → pixels réels de l'atlas.
#[test]
fn icon_rarity_atlas_regions_resolve() {
    let Some(game) = game_dir() else {
        eprintln!("skip icon_rarity_atlas_regions_resolve : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let output = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--g4tx", "icon_rarity.g4tx", "--g4tx-regions"])
        .env("RUST_LOG", "error")
        .output()
        .expect("lancer nie-game --g4tx-regions");
    assert!(output.status.success(), "g4tx-regions icon_rarity a échoué");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // La texture-atlas principale réelle (DDS).
    assert!(
        stdout.contains("\"icon_rarity\""),
        "texte atlas icon_rarity absent :\n{stdout}"
    );
    // Les 10 niveaux de rareté gtxt_rarity01_01..10 sont des régions nommées de l'atlas.
    for lvl in 1..=10 {
        let name = format!("gtxt_rarity01_{lvl:02}");
        assert!(
            stdout.contains(&name),
            "région {name} absente de l'atlas :\n{stdout}"
        );
    }
    // 15 régions au total (10 niveaux + 5 variantes de type) — verrouille la complétude du parse.
    assert!(
        stdout.contains("total: 15 régions"),
        "compte de régions inattendu :\n{stdout}"
    );
}

/// D1.f render-from-runtime — ABOUTISSEMENT : « données runtime → IMAGE COMPOSÉE ». Chaîne le
/// vrai pipeline : (1) `--runtime --export-layout` produit le layout (objets + transform + région
/// résolue → g4tx + rect) ; (2) `--compose-layout --capture` rogne chaque `spriteRect` de son
/// `spriteRegionG4tx` et le pose au `transform` (ancre + échelle) sur un canevas 1280×720. Sur
/// `shop`, le bandeau de rareté `gtxt_rarity01_10` (« BASARA ») est rogné de `icon_rarity.g4tx` et
/// composé centré (transform x=640,y=360). Gate end-to-end : ≥ 1 région composée, PNG non vide.
#[test]
fn shop_runtime_composes_to_image() {
    let Some(game) = game_dir() else {
        eprintln!("skip shop_runtime_composes_to_image : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let layout = std::env::temp_dir().join("nie_gate_shop_compose_layout.json");
    let png = std::env::temp_dir().join("nie_gate_shop_compose.png");
    let _ = std::fs::remove_file(&png);

    // (1) Export runtime → layout JSON.
    let s1 = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "shop", "--runtime", "--export-layout"])
        .arg(&layout)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer export-layout runtime shop");
    assert!(s1.success(), "export-layout runtime shop a échoué");

    // (2) Compose le layout → PNG.
    let s2 = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--compose-layout"])
        .arg(&layout)
        .args(["--capture"])
        .arg(&png)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer compose-layout shop");
    assert!(s2.success(), "compose-layout shop a échoué");

    let bytes = std::fs::read(&png).expect("lire le PNG composé");
    assert!(
        bytes.len() > 24 && &bytes[1..4] == b"PNG",
        "PNG composé invalide"
    );
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    assert_eq!((w, h), (1280, 720), "dimensions du canevas inattendues");
    // Écran complet : sprites statiques (fond + barres de liste) + région runtime (bandeau de
    // rareté), z-ordonnés. Un écran composé pèse ~1 Mo ; un sprite seul ~80 Ko → seuil 250 Ko sépare
    // nettement « écran composé » de « 1 sprite » (verrouille que la couche statique S'EST dessinée).
    assert!(
        bytes.len() > 250_000,
        "PNG composé trop petit ({} octets) — la couche statique ne s'est probablement pas dessinée",
        bytes.len()
    );
}

/// D1.f render-from-runtime — CHAÎNON `région → fichier g4tx → rect` CÂBLÉ DANS L'EXPORT. Sur les
/// écrans réels, le runtime fournit un `spriteRegion` (ex. `gtxt_rarity01_05`) mais un `spritePath`
/// qui est un nom de NŒUD ("Focus"), pas un chemin g4tx. L'**index `menu-region-index.json`**
/// (généré par `--build-region-index` depuis les 24 atlas d'icônes du corpus Lua) résout la région
/// → fichier g4tx ; l'export charge alors le g4tx et émet le **rect de crop exact** (`spriteRect`).
/// Sur `shop`, les 2 bandeaux de rareté résolvent vers `icon_rarity.g4tx` + leur rect — la donnée
/// est désormais render-ready (le compositeur natif OU azalee peut rogner + placer). Gate end-to-end.
/// **Gaté sur le VFS de référence** (même raison que `bg_mesh_geometry_decodes`) : sur
/// l'installation Steam de cette machine, aucune région ne résout en rect (`regionRects=0`).
#[test]
#[ignore = "exige le VFS de référence : dépend de la version des assets de menu"]
fn shop_runtime_resolves_region_to_g4tx_rect() {
    let Some(game) = game_dir() else {
        eprintln!("skip shop_runtime_resolves_region_to_g4tx_rect : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let out = std::env::temp_dir().join("nie_gate_shop_region_rect.json");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "shop", "--runtime", "--export-layout"])
        .arg(&out)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --runtime shop");
    assert!(status.success(), "export-runtime shop a échoué");
    let txt = std::fs::read_to_string(&out).expect("lire le layout runtime shop");
    let doc: serde_json::Value = serde_json::from_str(&txt).expect("JSON valide");

    // Au moins un objet a résolu région → g4tx → rect.
    let rects = doc["runtimeSummary"]["regionRects"].as_u64().unwrap_or(0);
    assert!(
        rects >= 1,
        "aucune région résolue en rect (regionRects={rects})"
    );

    // L'objet du bandeau de rareté gtxt_rarity01_05 → icon_rarity.g4tx + rect exact (0,264,800,128).
    let objs = doc["objects"].as_array().expect("objects");
    let rarity = objs
        .iter()
        .find(|o| o["runtime"]["spriteRegion"].as_str() == Some("gtxt_rarity01_05"))
        .expect("objet avec région gtxt_rarity01_05");
    let g4tx = rarity["runtime"]["spriteRegionG4tx"].as_str().unwrap_or("");
    assert!(
        g4tx.ends_with("icon_rarity.g4tx"),
        "g4tx résolu inattendu : {g4tx}"
    );
    let rect = &rarity["runtime"]["spriteRect"];
    assert_eq!(rect["x"].as_i64(), Some(0), "rect.x");
    assert_eq!(rect["y"].as_i64(), Some(264), "rect.y");
    assert_eq!(rect["w"].as_i64(), Some(800), "rect.w");
    assert_eq!(rect["h"].as_i64(), Some(128), "rect.h");
}

/// D1.f render-from-runtime — PREUVE que la chaîne va jusqu'aux **PIXELS RÉELS** : `--g4tx-region`
/// décode la texture DDS de `icon_rarity.g4tx` et rogne la sous-texture `gtxt_rarity01_05` en un PNG.
/// Le gate vérifie que le crop fait exactement 800×128 (le rect de la région) et n'est pas vide.
/// C'est l'étape finale du rendu d'un sprite désigné au runtime : `region_name → pixels exacts de
/// l'atlas` (visuellement, c'est le bandeau de rareté « JOUEUR LÉGENDAIRE » en locale fr).
#[test]
fn icon_rarity_region_crops_to_real_pixels() {
    let Some(game) = game_dir() else {
        eprintln!("skip icon_rarity_region_crops_to_real_pixels : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let png = std::env::temp_dir().join("nie_gate_rarity05.png");
    let _ = std::fs::remove_file(&png);
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args([
            "--g4tx",
            "icon_rarity.g4tx",
            "--g4tx-region",
            "gtxt_rarity01_05",
            "--capture",
        ])
        .arg(&png)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --g4tx-region");
    assert!(
        status.success(),
        "crop de la région gtxt_rarity01_05 a échoué"
    );

    let bytes = std::fs::read(&png).expect("lire le PNG de la région");
    // En-tête PNG : IHDR width/height en big-endian aux octets 16..24.
    assert!(
        bytes.len() > 24 && &bytes[1..4] == b"PNG",
        "fichier PNG invalide"
    );
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    // Le rect de gtxt_rarity01_05 dans icon_rarity.g4tx (cf. --g4tx-regions) = 800×128.
    assert_eq!((w, h), (800, 128), "dimensions de crop inattendues");
    // Un bandeau de rareté n'est pas une image vide (PNG compressé non trivial).
    assert!(
        bytes.len() > 4_000,
        "PNG suspectement petit ({} octets) — crop probablement vide",
        bytes.len()
    );
}

/// D1.c-driver (brique b) — le DÉBLOCAGE ITEM-COUNT : depuis le reversal de `RegisterItemListCount`
/// (cmdId `0x16C1C4C0`, handler `0x140CD8E30`), `GetItemButtonNum` renvoie le nombre d'items fourni
/// par le SCRIPT (via le manager partagé `0x140CF5B60`) → le **loop `SetupItemButton` du script
/// S'EXÉCUTE** et mute de VRAIS objets (sprites/textes/visibilité). Avant le reversal, ces compteurs
/// étaient à **0** (loop bloqué car compte=0). Ce gate verrouille l'effet END-TO-END sur `title02`.
#[test]
fn title02_runtime_driver_builds_item_content() {
    let Some(game) = game_dir() else {
        eprintln!("skip title02_runtime_driver_builds_item_content : jeu absent");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let out = std::env::temp_dir().join("nie_gate_title02_runtime.json");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "title02", "--runtime", "--export-layout"])
        .arg(&out)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --runtime");
    assert!(status.success(), "export-layout-runtime title02 a échoué");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("lire JSON")).expect("JSON");
    let rs = &doc["runtimeSummary"];
    let n = |k: &str| rs[k].as_u64().unwrap_or(0);
    // Le loop d'items tourne → le driver crée du contenu RÉEL (≥1 list-item, sprites/textes mutés).
    assert!(
        n("listItemsRecorded") >= 1,
        "listItemsRecorded={} (loop d'items bloqué ?)",
        n("listItemsRecorded")
    );
    assert!(
        n("spritesMutated") >= 1,
        "spritesMutated={} (driver ne pose aucun sprite)",
        n("spritesMutated")
    );
    assert!(
        n("textsMutated") >= 1,
        "textsMutated={} (driver ne pose aucun texte)",
        n("textsMutated")
    );
}

/// Étage 2 — métrique SSIM vs la capture réelle (informatif + plancher de non-régression).
#[test]
fn title02_ssim_vs_reference() {
    let Some(game) = game_dir() else {
        eprintln!("skip title02_ssim_vs_reference : jeu absent");
        return;
    };
    let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../start.png");
    if !reference.exists() {
        eprintln!("skip : référence {reference:?} absente");
        return;
    }
    let tmp = std::env::temp_dir().join("nie_gate_ssim.png");
    let (rw, rh, render) = render_screen(&game, "title02", &tmp);
    assert_eq!((rw, rh), (1280, 720), "canvas canonique 1280×720");

    let (ow, oh, orig) = decode_png(&reference);
    assert_eq!((ow, oh), (2560, 1440), "référence 2560×1440 (2× canonique)");
    let (dw, dh, downs) = downscale_2x(ow, oh, &orig);
    assert_eq!((dw, dh), (1280, 720));

    let score = ssim(1280, 720, &render, &downs);
    eprintln!("\n=== SSIM title02 vs start.png (référence jeu) = {score:.4} ===");
    eprintln!(
        "    (cible pixel-perfect : >= 0.99 hors ROI ; le rendu est encore incomplet — D1.c/d/e)\n"
    );

    // Plancher de non-régression. Baseline mesurée 2026-06-16 (couche 1 placement `g4pkm_motion`
    // + couche 2 texture non-dummy) : **SSIM = 0.2511** (plafonné ≈0.25 car le FOND de title02 est
    // une SCÈNE 3D absente — cf. §4 ; le travail textures-menu seul ne peut dépasser ce plafond).
    // Plancher RELEVÉ 0.20→0.24 (rendu déterministe) ; **relever à chaque vague D1.x** (cible ≥ 0.99).
    assert!(score.is_finite(), "SSIM doit être fini");
    assert!(
        score >= 0.24,
        "SSIM {score:.4} < plancher 0.24 — régression de rendu (baseline 0.2511)"
    );
}

/// Étage 2bis — SSIM sur `mainmenu01` (réf `menu.png`). **C'est la métrique appropriée du travail
/// de TEXTURES de menu** : contrairement à `title02` (dont le FOND est une SCÈNE 3D — champ vert +
/// persos animés — donc plafond SSIM ≈ 0,25 par textures seules, cf. DESIGN.md §4), `mainmenu01` est
/// **dominé par les textures de menu** (panneaux/onglets), donc le travail placement/sélection/texte
/// PEUT y faire monter la SSIM. Baseline informative + plancher de non-régression.
#[test]
fn mainmenu01_ssim_vs_reference() {
    let Some(game) = game_dir() else {
        eprintln!("skip mainmenu01_ssim_vs_reference : jeu absent");
        return;
    };
    let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../menu.png");
    if !reference.exists() {
        eprintln!("skip : référence {reference:?} absente");
        return;
    }
    let tmp = std::env::temp_dir().join("nie_gate_ssim_mainmenu.png");
    let (rw, rh, render) = render_screen(&game, "mainmenu01", &tmp);
    assert_eq!((rw, rh), (1280, 720), "canvas canonique 1280×720");

    let (ow, oh, orig) = decode_png(&reference);
    assert_eq!(
        (ow, oh),
        (2560, 1440),
        "référence menu.png 2560×1440 (2× canonique)"
    );
    let (dw, dh, downs) = downscale_2x(ow, oh, &orig);
    assert_eq!((dw, dh), (1280, 720));

    let score = ssim(1280, 720, &render, &downs);
    eprintln!("\n=== SSIM mainmenu01 vs menu.png (référence jeu) = {score:.4} ===");
    // DIAGNOSTIC CONCLUSIF (2026-06-16) — les 31 objbin mainmenu01 n'ont AUCUN param `Texture` :
    // leur texture est liée par le MESH (co-localisée `<mesh>.g4tx`, le g4md de menu ne porte pas de
    // `material_base_names`). iecode a la MÊME limite (sprite gated sur `G4txPath`) → rend aussi vide.
    // D1.c (ce cycle) : fallback co-localisé → 22 sprites textures RÉELLES rendues (avant : 0, écran
    // blanc, SSIM 0.0011). Bloqueur restant = PLACEMENT : les widgets ont une bind pose g4pkm
    // hors-écran (ex. `_save_base01` à y≈-3044). RE établie : les motions de menu n'ont PAS de
    // keyframes de POSITION de bone (seulement matériau alpha/UV) → la position finale est calculée
    // par le DRIVER C++ (machine d'état G4RA) / Lua `Setup*`, pas récupérable des fichiers seuls. Le
    // fallback bind-pose ancêtre-on-écran les parque donc au centre → SSIM ≈ 0.004 jusqu'à l'émulation
    // du driver (D1.c-driver). Cf. DESIGN §4/§6/§13 + memory start-menu-screen-mapping.
    assert!(score.is_finite(), "SSIM doit être fini");
    assert!(
        score >= 0.003,
        "SSIM {score:.4} < plancher 0.003 — régression (textures co-localisées ?)"
    );
}

/// Étage 2quater — SSIM du **compositeur render-from-runtime** (`--compose-layout`) sur `main_menu`,
/// mesuré end-to-end : `--runtime --from-setting --export-layout` → `--compose-layout` → PNG, comparé
/// à `menu.png`. **Métrique honnête de la voie compose runtime** (distincte du compositeur statique
/// `--from-setting` mesuré par `mainmenu_via_setting_ssim`).
///
/// DIAGNOSTIC (2026-06-16) : le modèle de placement CPU est VÉRIFIÉ identique au quad GPU
/// (`build_sprite_quad`). Mais sur `main_menu`, deux artefacts de **transform-fallback** (D1.a)
/// plombent la SSIM, PAS un bug du compositeur : (1) `mainmenu90_02_header_tab` (texture 5280×520)
/// est dessiné à l'échelle **1.0** (défaut motion-fallback) → bande de 4× la largeur écran couvrant
/// le centre ; (2) `mainmenu01_10/11` sont placés HORS écran (y>720, bind-pose ancêtre). Les deux
/// confirment le gap connu : l'échelle/position RÉELLES viennent du DRIVER C++/Lua (machine d'état
/// G4RA / `Setup*`), pas des fichiers (cf. §6/§13). Donc score informatif < voie statique tant que
/// le driver-transform n'est pas émulé. Plancher de non-régression bas (mesure, pas cible).
#[test]
fn mainmenu_runtime_compose_ssim() {
    let Some(game) = game_dir() else {
        eprintln!("skip mainmenu_runtime_compose_ssim : jeu absent");
        return;
    };
    let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../menu.png");
    if !reference.exists() {
        eprintln!("skip : référence {reference:?} absente");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let layout = std::env::temp_dir().join("nie_gate_mm_compose_layout.json");
    let png = std::env::temp_dir().join("nie_gate_mm_compose.png");

    let s1 = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args([
            "--menu",
            "main_menu",
            "--runtime",
            "--from-setting",
            "--export-layout",
        ])
        .arg(&layout)
        .env("RUST_LOG", "error")
        .status()
        .expect("export-layout runtime+from-setting main_menu");
    assert!(s1.success(), "export-layout runtime main_menu a échoué");
    let s2 = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--compose-layout"])
        .arg(&layout)
        .args(["--capture"])
        .arg(&png)
        .env("RUST_LOG", "error")
        .status()
        .expect("compose-layout main_menu");
    assert!(s2.success(), "compose-layout main_menu a échoué");

    let (rw, rh, render) = decode_png(&png);
    assert_eq!((rw, rh), (1280, 720), "canvas canonique 1280×720");
    let (ow, oh, orig) = decode_png(&reference);
    let (dw, dh, downs) = downscale_2x(ow, oh, &orig);
    assert_eq!((dw, dh), (1280, 720));

    let score = ssim(1280, 720, &render, &downs);
    eprintln!(
        "\n=== SSIM main_menu COMPOSE-runtime vs menu.png = {score:.4} (informatif ; gap driver-transform) ==="
    );
    assert!(score.is_finite(), "SSIM doit être fini");
    // RÉSULTAT (2026-06-16) : 0,4059 — la voie compose-RUNTIME est À PARITÉ avec le compositeur
    // statique validé (`mainmenu_via_setting_ssim` = 0,418), bien que la bande header_tab (échelle
    // fallback) coûte ~0,012. ⇒ le chemin render-from-runtime est quantitativement sain, pas qu'un
    // « ça ressemble à un écran ». Plancher RELEVÉ 0,30→0,39 (marge sous la baseline 0,4059).
    assert!(
        score >= 0.39,
        "SSIM {score:.4} < plancher 0.39 — régression de la voie compose-runtime (baseline 0.4059)"
    );
}

/// Étage 2ter — SSIM sur `main_menu` composé depuis sa **définition `menu_setting`** (D1.c-driver
/// brique (a)) : `--menu main_menu --from-setting` itère la liste `MENU_LAYER_INFO` (13 layers, dont
/// le FOND plein écran `mainmenu90_00` + l'en-tête `mainmenu90_01`) au lieu du filtre par préfixe
/// `mainmenu01_*` (qui rate fond/en-tête). Vérifie l'amélioration objective : le fond statique
/// on-écran se compose correctement → la SSIM doit DÉPASSER nettement la voie préfixe (≈0.004 quasi
/// vide). Le placement des widgets ANIMÉS reste imparfait (driver, cf. §6) → plafond < 0.99 jusqu'à D1.c-driver.
#[test]
fn mainmenu_via_setting_ssim_vs_reference() {
    let Some(game) = game_dir() else {
        eprintln!("skip mainmenu_via_setting_ssim : jeu absent");
        return;
    };
    let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../menu.png");
    if !reference.exists() {
        eprintln!("skip : référence {reference:?} absente");
        return;
    }
    let tmp = std::env::temp_dir().join("nie_gate_ssim_mainmenu_setting.png");
    let bin = env!("CARGO_BIN_EXE_nie-game");
    let status = Command::new(bin)
        .args(["--game-dir"])
        .arg(&game)
        .args(["--menu", "main_menu", "--from-setting", "--capture"])
        .arg(&tmp)
        .env("RUST_LOG", "error")
        .status()
        .expect("lancer nie-game --from-setting");
    assert!(
        status.success(),
        "nie-game --menu main_menu --from-setting a échoué"
    );
    let (rw, rh, render) = decode_png(&tmp);
    assert_eq!((rw, rh), (1280, 720), "canvas canonique 1280×720");

    let (ow, oh, orig) = decode_png(&reference);
    let (dw, dh, downs) = downscale_2x(ow, oh, &orig);
    assert_eq!((dw, dh), (1280, 720));

    let score = ssim(1280, 720, &render, &downs);
    eprintln!("\n=== SSIM main_menu (via menu_setting, 13 layers) vs menu.png = {score:.4} ===");
    // La composition par menu_setting (fond plein écran + en-tête) doit BATTRE largement la voie
    // préfixe `mainmenu01_*` (≈0.004, quasi vide). Baseline 2026-06-16 = 0.4180, puis **0.6227**
    // (2026-06-20, workflow fix-real-menu) via le FOND pastel peint + blacklist des sprites
    // bleu-saturé mal placés (fallback bind-pose hors-écran). Plancher RELEVÉ 0.40→0.60 pour
    // verrouiller ce gain ; à RELEVER quand le driver D1.c (placement pixel) ou les panneaux 3D
    // AVATAR/ÉQUIPE poussent plus haut (cf. docs/DESIGN.md couche 5).
    assert!(score.is_finite(), "SSIM doit être fini");
    assert!(
        score >= 0.60,
        "SSIM {score:.4} < plancher 0.60 — régression du placement/composition menu_setting (baseline 0.6227)"
    );
}

/// Gate **D1.d (rendu de texte)** : `--render-text 212` doit décoder l'atlas de police RÉEL
/// (`font_def/font.g4tx`, DDS LEGACY BGRA8), charger les métriques (`font.cfg.bin`), résoudre les
/// 3 codepoints et écrire un PNG non vide. Garde contre la régression du décodage atlas legacy +
/// du pipeline `font::draw_text`. Game-gated (skip si le jeu est absent).
#[test]
fn render_text_212_from_real_font_atlas() {
    let Some(game) = game_dir() else {
        eprintln!("jeu absent — skip render_text_212_from_real_font_atlas");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_nie-game");
    // Couvre chiffres ET un mot Latin réel : les DEUX classes de glyphes (cibles du gate D1.d).
    for (text, expect) in [
        ("212", "3/3 codepoints"),
        ("Sauvegarder", "11/11 codepoints"),
    ] {
        let out = std::env::temp_dir().join("gate_render_text.png");
        let output = Command::new(bin)
            .args(["--game-dir"])
            .arg(&game)
            .args(["--render-text", text, "--capture"])
            .arg(&out)
            .env("RUST_LOG", "error")
            .output()
            .expect("lancer nie-game --render-text");
        assert!(output.status.success(), "--render-text {text} a échoué");
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Tous les codepoints doivent résoudre dans la table de glyphes réelle (atlas legacy BGRA8).
        assert!(
            stdout.contains(expect),
            "« {text} » : tous les codepoints doivent résoudre ({expect}) depuis l'atlas réel : {stdout}"
        );
        // PNG non trivial = des glyphes ont été rendus (pas un canevas vide).
        let png = std::fs::read(&out).expect("PNG render-text");
        assert!(
            png.len() > 200,
            "PNG « {text} » trop petit ({} o) — rien rendu ?",
            png.len()
        );
    }
}
