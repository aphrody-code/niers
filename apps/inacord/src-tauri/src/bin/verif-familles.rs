//! Vérification RÉELLE des familles de l'encyclopédie sur le jeu monté — un binaire, pas un test.
//!
//! `cargo test` ne démarre pas dans ce crate sur le poste Windows (`STATUS_ENTRYPOINT_NOT_FOUND`,
//! avant le premier test — piège documenté dans `CLAUDE.md`) : le test
//! `game_data::tests::nouvelles_familles_sur_le_vrai_jeu` ne peut donc pas y être exécuté. Ce
//! binaire appelle exactement les mêmes fonctions et imprime le compte de chaque famille, pour
//! qu'aucun onglet de l'encyclopédie ne soit livré sans avoir décodé une seule ligne.
//!
//! `cargo run --bin verif-familles --features dev-bindings`
use nie_explorer_lib::game_data;
use nie_formats::vfs::Vfs;

fn main() {
    let dir = nie_formats::vfs::resolve_game_dir();
    let data_dir = dir.join("data");
    if !nie_formats::vfs::donnees_disponibles(&data_dir) {
        eprintln!("jeu absent sous {} — rien à vérifier", data_dir.display());
        std::process::exit(2);
    }
    let mut vfs = Vfs::new();
    vfs.init(&data_dir).expect("vfs init");
    println!("VFS monté : {}", data_dir.display());

    let mut echecs = 0;
    macro_rules! verifie {
        ($libelle:literal, $f:path) => {
            match $f(&vfs) {
                Ok(v) if v.is_empty() => {
                    echecs += 1;
                    println!("  ✗ {:<16} liste VIDE", $libelle);
                }
                Ok(v) => println!("  ✓ {:<16} {} ligne(s)", $libelle, v.len()),
                Err(e) => {
                    echecs += 1;
                    println!("  ✗ {:<16} {e}", $libelle);
                }
            }
        };
    }

    verifie!("personnages", game_data::list_charas);
    verifie!("dictionnaire", game_data::list_dictionary);
    verifie!("techniques", game_data::list_skills);
    verifie!("tactiques", game_data::list_special_tactics);
    verifie!("passifs", game_data::list_passives);
    verifie!("feintes", game_data::list_tricks);
    verifie!("auras", game_data::list_auras);
    verifie!("objets", game_data::list_items);
    verifie!("boutiques", game_data::list_shops);
    verifie!("butin", game_data::list_drops);
    verifie!("capsules", game_data::list_capsule_rates);
    verifie!("équipes", game_data::list_belong_teams);
    verifie!("adversaires", game_data::list_opponent_teams);
    verifie!("formations", game_data::list_formations);
    verifie!("uniformes", game_data::list_uniforms);
    verifie!("écussons", game_data::list_emblems);
    verifie!("stades", game_data::list_stadiums);
    verifie!("quêtes", game_data::list_quests);
    verifie!("activités", game_data::list_activities);
    verifie!("succès", game_data::list_trophies);
    verifie!("galerie", game_data::list_gallery);
    verifie!("vidéos", game_data::list_movies);
    verifie!("musiques", game_data::list_musics);
    verifie!("expérience", game_data::list_exp_table);

    if echecs > 0 {
        eprintln!("{echecs} famille(s) en échec");
        std::process::exit(1);
    }
    println!("toutes les familles décodent.");
}
