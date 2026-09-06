//! Les `.lua.bin` du jeu passent-ils par le dispatch partagé ?
//!
//! Ce sont des chunks **bytecode Lua 5.2 PUC-Rio** (`\x1bLua`, version `0x52`). Le décodeur est
//! celui de `nie-lua` ; ce test vérifie qu'il est bien branché sur `nie_formats::decode` — donc
//! atteignable par `nie_decode_json`, `niers decode`, l'explorateur et le MCP — et que le JSON
//! produit porte réellement le contenu du chunk, pas seulement un en-tête.
//!
//! Il **annonce son saut** quand ni l'installation ni le dump ne sont disponibles.

#![cfg(all(feature = "std", feature = "serde", feature = "lua"))]

use nie_formats::vfs::{self, Vfs};

fn corpus() -> Option<Vfs> {
    match vfs::open_game() {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("skip : ni installation ni dump ({e:?})");
            None
        }
    }
}

#[test]
fn les_lua_bin_se_decodent_par_le_dispatch() {
    let Some(vfs) = corpus() else { return };
    let mut chemins: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".lua.bin"))
        .collect();
    chemins.sort_unstable();
    if chemins.is_empty() {
        eprintln!("skip : aucun .lua.bin dans le corpus monté");
        return;
    }

    let (mut ok, mut instructions, mut constantes, mut protos) = (0usize, 0usize, 0usize, 0usize);
    let mut phase = 0usize;
    let mut echecs: Vec<String> = Vec::new();
    for p in &chemins {
        let Ok(octets) = vfs.read(p) else {
            echecs.push(format!("{p} : lecture impossible"));
            continue;
        };
        // le magic doit être celui du bytecode Lua 5.2, sinon l'échantillon n'est pas ce qu'on croit
        if !octets.starts_with(&nie_lua::LUA52_BYTECODE_SIGNATURE) {
            echecs.push(format!("{p} : signature Lua 5.2 absente"));
            continue;
        }
        let Some(d) = nie_formats::decode::decode(&octets) else {
            echecs.push(format!("{p} : le dispatch ne décode pas"));
            continue;
        };
        if d.format != "lua-bytecode" {
            echecs.push(format!("{p} : routé vers « {} »", d.format));
            continue;
        }
        let chunk = nie_lua::bytecode::parse(&octets).expect("déjà décodé par le dispatch");
        instructions += chunk.main.total_instructions();
        protos += 1 + chunk.main.total_protos();
        constantes += chunk.main.constants.len();
        if p.contains("/phase/") {
            phase += 1;
        }
        // le JSON doit porter le corps, pas seulement l'en-tête
        let json = String::from_utf8(d.json).expect("json utf-8");
        for cle in ["\"header\"", "\"main\"", "\"code\"", "\"constants\""] {
            if !json.contains(cle) {
                echecs.push(format!("{p} : JSON sans {cle}"));
                break;
            }
        }
        ok += 1;
    }

    eprintln!(
        "lua.bin : {ok}/{} décodés (dont {phase} sous gamedata/phase) — {protos} prototypes, \
         {instructions} instructions, {constantes} constantes de tête",
        chemins.len()
    );
    for e in echecs.iter().take(10) {
        eprintln!("  ÉCHEC {e}");
    }
    assert!(echecs.is_empty(), "{} fichier(s) en échec", echecs.len());
    assert!(
        instructions > 0,
        "aucune instruction décodée : rien n'est prouvé"
    );
}

/// `format_name` doit nommer le bytecode Lua **avant** de retomber sur « inconnu ».
#[test]
fn format_name_nomme_le_bytecode_lua() {
    assert_eq!(
        nie_formats::decode::format_name(b"\x1bLua\x52\0suite"),
        "lua-bytecode"
    );
    assert_eq!(nie_formats::decode::format_name(b"lip\0suite"), "lip");
}
