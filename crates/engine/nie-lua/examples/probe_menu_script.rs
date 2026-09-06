//! Probe jetable (D1.c) : trouve le(s) script(s) `.lua.bin` d'un écran de menu, l'exécute dans
//! la vraie VM Lua 5.2 (install_menu_host + install_include) et dump le MenuState résultant
//! (layers, objets visibles/sprite/texte/nombre, cmdIds inconnus). Sert à découvrir le layerId
//! et la sémantique avant de brancher le renderer.
//!
//! Usage : `cargo run -p nie-lua --example probe_menu_script -- title02`
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use nie_lua::{install_include, install_menu_host, new_vm, run_menu};

fn main() {
    let needle = std::env::args().nth(1).unwrap_or_else(|| "title02".into());
    // Aucun chemin de machine en dur : la racine se résout à l'exécution (`NIE_GAME_DIR`, cwd,
    // ancêtre portant `data/cpk_list.cfg.bin`). Un chemin WSL compilé ici échouait sous Windows
    // natif sur « impossible d'ouvrir cpk_list.cfg.bin », qui accuse le VFS au lieu du chemin.
    let dir = nie_formats::vfs::resolve_game_dir();
    let mut vfs = nie_formats::vfs::Vfs::new();
    vfs.init(Path::new(&dir).join("data").as_path())
        .expect("vfs init");

    // Index basename → chemin (pour INCLUDE).
    // Index par **base logique versionless** (`script_logical_base`), la clé qu'attend la
    // résolution d'INCLUDE : le fichier porte un suffixe de version
    // (`main_menu_inc_3.00.01.00.lua.bin`) que le nom logique n'a pas. Indexer par basename brut
    // fait échouer l'inclusion *en silence* — la table `MENU_DEF` reste incomplète et `OnInit`
    // plante bien plus loin sur « attempt to index field 'MAIN_MENU' (a nil value) ».
    let mut by_base: HashMap<String, String> = HashMap::new();
    for (p, _) in vfs.iter() {
        if let Some(b) = p.rsplit('/').next().filter(|b| b.ends_with(".lua.bin")) {
            by_base
                .entry(nie_lua::script_logical_base(b))
                .or_insert_with(|| p.to_string());
        }
    }

    // Scripts de menu correspondant au besoin.
    let mut scripts: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.starts_with("data/common/script/lua/menu/")
                && p.ends_with(".lua.bin")
                && p.to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
        })
        .collect();
    scripts.sort();
    println!("scripts contenant '{needle}': {}", scripts.len());
    for s in &scripts {
        println!("  {}", s.rsplit('/').next().unwrap());
    }

    let vfs = Rc::new(vfs);
    let by_base = Rc::new(by_base);

    // ── Pass DRIVER : tente d'émuler la boucle de construction du moteur ──────────
    // (OnInit → GetItemButtonNum → SetupItemButton[i] → Step), tolérant aux erreurs,
    // pour découvrir quels funcLuaMenuCommand sont émis. Cf. DESIGN.md §13.
    if let Some(path) = scripts.first() {
        let bytes = vfs.read(path).unwrap();
        let name = path.rsplit('/').next().unwrap();
        let lua = new_vm();
        {
            let (vfs, by_base) = (Rc::clone(&vfs), Rc::clone(&by_base));
            install_include(&lua, move |n| {
                // `INCLUDE` reçoit un nom logique (`LUA_MAIN_MENU_INC`), pas un nom de fichier.
                let hit = by_base.get(&nie_lua::include_logical_base(n));
                // Tracer chaque INCLUDE : une inclusion non résolue échoue *en silence* et
                // ne se manifeste que bien plus tard, par un champ nil dans une table.
                match hit {
                    Some(p) => eprintln!("  INCLUDE {n:<28} -> {p}"),
                    None => eprintln!("  INCLUDE {n:<28} -> NON RESOLU"),
                }
                hit.and_then(|p| vfs.read(p).ok())
            })
            .unwrap();
        }
        let state = install_menu_host(&lua).unwrap();
        let func = nie_lua::load_bytecode(&lua, &bytes, name).unwrap();
        let _ = func.call::<()>(());

        // Trace des globals ABSENTS, posée après le top-level pour ne relever que ce qui manque
        // pendant la construction. Non intrusive : `__index` renvoie nil, exactement ce que Lua
        // ferait sans métatable — la sémantique du script est inchangée, on ne fait qu'observer.
        lua.load(
            r"
            _MISSING = {}
            setmetatable(_G, { __index = function(_, k)
                _MISSING[k] = (_MISSING[k] or 0) + 1
                return nil
            end })
            ",
        )
        .set_name("<missing-recorder>")
        .exec()
        .unwrap();

        let call0 = |fname: &str| -> String {
            match lua.globals().get::<mlua::Function>(fname) {
                Ok(f) => match f.call::<mlua::Variadic<mlua::Value>>(()) {
                    Ok(v) => format!(
                        "ok({})",
                        v.iter()
                            .map(|x| format!("{x:?}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    Err(e) => format!("ERR({})", e.to_string().lines().next().unwrap_or("")),
                },
                Err(_) => "absent".into(),
            }
        };
        let call1 = |fname: &str, a: i64| -> String {
            match lua.globals().get::<mlua::Function>(fname) {
                Ok(f) => match f.call::<mlua::Variadic<mlua::Value>>(a as f64) {
                    Ok(v) => format!(
                        "ok({})",
                        v.iter()
                            .map(|x| format!("{x:?}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    Err(e) => format!("ERR({})", e.to_string().lines().next().unwrap_or("")),
                },
                Err(_) => "absent".into(),
            }
        };
        println!("\n########## DRIVER PROBE [{name}] ##########");
        // Séquence CONFIRMÉE par RE (manager 0x14109D190) : set __menuObjPtr, OnInit() (0 arg),
        // puis OnSetupLayer(layerId) par layer (c'est là que les objets sont créés).
        let _ = lua.globals().set("__menuObjPtr", 1.0_f64);
        println!("  OnInit() -> {}", call0("OnInit"));
        // Test de l'hypothèse "n'importe quel id stable suffit" : OnSetupLayer(id) pour des
        // candidats (0, puis l'id de couche réel s'il existe).
        for id in [0_i64, 1, 0x1176_F7AB, 0xE6EC_6AA3] {
            let before = state
                .borrow()
                .layers
                .values()
                .map(|l| l.objects.len())
                .sum::<usize>();
            let r = call1("OnSetupLayer", id);
            let after = state
                .borrow()
                .layers
                .values()
                .map(|l| l.objects.len())
                .sum::<usize>();
            println!("  OnSetupLayer(0x{id:08X}) -> {r}  (objets {before} -> {after})");
        }
        // Balayage : `OnSetupLayer` est un dispatcher qui compare le layerId reçu à ses propres
        // constantes. Le CRC32 du nom de l'ÉCRAN n'est pas celui d'un LAYER — d'où 0 objet sur
        // les candidats devinés. On essaie donc tous les layers connus de la KB
        // (`var/live/layers.txt`, lignes « <hash décimal> <nom> ») et on ne retient que ceux qui
        // font effectivement croître le nombre d'objets.
        if let Ok(list) = std::fs::read_to_string("var/live/layers.txt") {
            let mut hits = 0_usize;
            let mut tried = 0_usize;
            for line in list.lines() {
                let Some((h, lname)) = line.split_once(' ') else {
                    continue;
                };
                let Ok(id) = h.trim().parse::<u32>() else {
                    continue;
                };
                tried += 1;
                let before = state
                    .borrow()
                    .layers
                    .values()
                    .map(|l| l.objects.len())
                    .sum::<usize>();
                let _ = call1("OnSetupLayer", i64::from(id));
                let _ = call1("OnOpenLayer", i64::from(id));
                let after = state
                    .borrow()
                    .layers
                    .values()
                    .map(|l| l.objects.len())
                    .sum::<usize>();
                if after > before {
                    hits += 1;
                    println!(
                        "  ++ layer 0x{id:08X} {lname} -> +{} objets",
                        after - before
                    );
                }
            }
            println!("  => balayage : {tried} layers essayés, {hits} produisent des objets");
        }
        // Provenance des hashes de sprite : le handler `0x140CE74D0` les reçoit en valeurs
        // NUMÉRIQUES (RE : lecture d'args → `cvttsd2si`), donc le nom n'existe pas côté hôte.
        // S'ils viennent d'une table Lua, la CLÉ d'accès porte le nom symbolique — on parcourt
        // `_G` en profondeur pour retrouver le chemin d'accès de chaque valeur observée.
        {
            let wanted: Vec<u32> = state
                .borrow()
                .layers
                .values()
                .flat_map(|l| l.objects.values())
                .filter_map(|o| o.sprite_texture_hash)
                .collect();
            if !wanted.is_empty() {
                let list = wanted
                    .iter()
                    .map(|h| format!("[{h}]=true"))
                    .collect::<Vec<_>>()
                    .join(",");
                let src = format!(
                    r"
                    local want = {{{list}}}
                    local seen, out = {{}}, {{}}
                    local function walk(t, path, depth)
                        if depth > 4 or seen[t] then return end
                        seen[t] = true
                        for k, v in pairs(t) do
                            local kp = path .. '.' .. tostring(k)
                            if type(v) == 'number' and want[v] then
                                out[#out+1] = string.format('%s = %d', kp, v)
                            elseif type(v) == 'table' then
                                walk(v, kp, depth + 1)
                            end
                        end
                    end
                    walk(_G, '_G', 0)
                    return table.concat(out, '\n')
                    "
                );
                match lua.load(&src).set_name("<hash-origin>").eval::<String>() {
                    Ok(s) if !s.is_empty() => {
                        println!("  => provenance des hashes de sprite :");
                        for l in s.lines().take(20) {
                            println!("       {l}");
                        }
                    }
                    Ok(_) => println!(
                        "  => provenance : aucun hash de sprite n'est une constante de table Lua \
                         (calculés à la volée, ou portés par le bytecode)"
                    ),
                    Err(e) => println!("  => provenance : ERR {e}"),
                }
            }
        }
        let st = state.borrow();
        let nobj: usize = st.layers.values().map(|l| l.objects.len()).sum();
        println!(
            "  => OnInit MenuState: layers={} objects={} known={} unknown={}",
            st.layers.len(),
            nobj,
            st.known_cmd_log.len(),
            st.unknown_cmd_log.len()
        );
        // Les commandes RÉELLEMENT émises : un menu qui ne construit rien mais émet quand même
        // prouve que le canal `funcLuaMenuCommand` marche, et dit où le script s'arrête.
        for (cmd, layer) in &st.known_cmd_log {
            println!("    cmd connue   {cmd:<28} layer=0x{layer:08X}");
        }
        for (cmd, layer, arg) in &st.unknown_cmd_log {
            println!("    cmd INCONNUE 0x{cmd:08X} layer=0x{layer:08X} {arg}");
        }
        // Ce que le script a cherché sans le trouver : la surface d'API hôte qui reste à fournir.
        // Un menu qui ne construit rien échoue d'abord ici, pas dans ses callbacks.
        if let Ok(missing) = lua.globals().get::<mlua::Table>("_MISSING") {
            let mut names: Vec<(String, i64)> = missing
                .pairs::<String, i64>()
                .filter_map(Result::ok)
                .filter(|(k, _)| k != "_MISSING")
                .collect();
            names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            println!(
                "  => globals ABSENTS pendant la construction : {}",
                names.len()
            );
            for (k, n) in names.iter().take(25) {
                println!("       {k:<44} x{n}");
            }
        }
        for (lid, layer) in &st.layers {
            println!(
                "    layer 0x{lid:08X} vis={} obj={}",
                layer.visible,
                layer.objects.len()
            );
            for (oid, o) in &layer.objects {
                println!(
                    "      obj 0x{oid:08X} vis={} sprite={:?} text={:?}",
                    o.visible, o.sprite_texture_hash, o.text
                );
            }
        }

        // JOIN CHECK : crc32(objbin object name) vs hashes (layer+object) du MenuState.
        let mut hashes = std::collections::HashSet::new();
        for (lid, layer) in &st.layers {
            hashes.insert(*lid);
            for oid in layer.objects.keys() {
                hashes.insert(*oid);
            }
        }
        let mut names: Vec<(String, u32)> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                p.contains("/menu/obj/")
                    && p.ends_with(".objbin")
                    && p.rsplit('/')
                        .next()
                        .is_some_and(|b| b.starts_with("title02"))
            })
            .filter_map(|p| {
                vfs.read(&p)
                    .ok()
                    .and_then(|b| nie_formats::objbin::parse(&b).ok())
            })
            .map(|o| {
                let h = nie_formats::cfgbin::crc32(o.name.as_bytes());
                (o.name, h)
            })
            .collect();
        names.sort();
        let matched: Vec<&(String, u32)> =
            names.iter().filter(|(_, h)| hashes.contains(h)).collect();
        println!(
            "  JOIN: {} hashes MenuState, {} objbin title02 ; crc32(name) match = {}",
            hashes.len(),
            names.len(),
            matched.len()
        );
        for (n, h) in matched.iter().take(10) {
            println!("    MATCH 0x{h:08X} = {n}");
        }

        // ARG LAYOUT brut : 1 ex. par cmdId émis (vérité terrain pour porter le dispatch).
        let mut seen = std::collections::HashSet::new();
        println!("  -- arg layout funcLuaMenuCommand(cmdId, layerId, rest...) --");
        for (c, l, repr) in &st.unknown_cmd_log {
            if seen.insert(*c) {
                println!("    0x{c:08X}(layer=0x{l:08X}, rest=[{repr}])");
            }
        }
    }

    for path in scripts.iter().take(6) {
        let Ok(bytes) = vfs.read(path) else { continue };
        let name = path.rsplit('/').next().unwrap();
        // Essaie plusieurs layerId candidats : 0 (= tous), puis le crc du nom de l'écran.
        for &layer_id in &[0u32, nie_formats::cfgbin::crc32(needle.as_bytes())] {
            let lua = new_vm();
            {
                let (vfs, by_base) = (Rc::clone(&vfs), Rc::clone(&by_base));
                install_include(&lua, move |n| {
                    // Même résolution logique que la passe DRIVER ci-dessus.
                    by_base
                        .get(&nie_lua::include_logical_base(n))
                        .and_then(|p| vfs.read(p).ok())
                })
                .unwrap();
            }
            let state = install_menu_host(&lua).unwrap();
            let r = run_menu(&lua, &bytes, path, layer_id);
            // Dump des callbacks définis par le script (pour comprendre le driver de menu).
            if layer_id == 0 {
                let mut fns: Vec<String> = Vec::new();
                for pair in lua.globals().pairs::<mlua::Value, mlua::Value>().flatten() {
                    if let (mlua::Value::String(k), mlua::Value::Function(_)) = pair {
                        fns.push(k.to_string_lossy().to_string());
                    }
                }
                fns.sort();
                println!("[{name}] globals fn définies par le script (hors host): {fns:?}");
            }
            let st = state.borrow();
            let nobj: usize = st.layers.values().map(|l| l.objects.len()).sum();
            println!(
                "\n[{name}] layer_id=0x{layer_id:08X} OnOpenLayer={r:?} layers={} objects={nobj} known={} unknown={}",
                st.layers.len(),
                st.known_cmd_log.len(),
                st.unknown_cmd_log.len()
            );
            for (lid, layer) in &st.layers {
                println!(
                    "  layer 0x{lid:08X}: vis={} obj={}",
                    layer.visible,
                    layer.objects.len()
                );
                for (oid, o) in layer.objects.iter().take(12) {
                    println!(
                        "    obj 0x{oid:08X} vis={} sprite={:?} text={:?} num={:?}",
                        o.visible, o.sprite_texture_hash, o.text, o.number
                    );
                }
            }
            let mut uniq = std::collections::BTreeMap::new();
            for (c, _, _) in &st.unknown_cmd_log {
                *uniq.entry(*c).or_insert(0usize) += 1;
            }
            if !uniq.is_empty() {
                println!(
                    "  cmdIds inconnus: {:?}",
                    uniq.iter()
                        .map(|(c, n)| format!("0x{c:08X}×{n}"))
                        .collect::<Vec<_>>()
                );
            }
            if nobj > 0 {
                break;
            } // ce layerId peuple → suffisant
        }
    }
}
