//! Le parcours qu'une joueuse fait réellement : écran-titre → menu → adversaires → match, et
//! retour. C'est la définition opérationnelle de « le jeu est jouable ».
//!
//! La FSM ([`nie_app::flow::Screen`]) est partagée par le front web (`nie-wasm`) et le front natif
//! (`nie-game --play`) : ce qui casse ici casse les deux. Elle n'avait aucun test — un parcours
//! interrompu au troisième écran ne se serait vu qu'à la main, front par front.
//!
//! Les commandes sont celles du jeu (`MENU_CMD_INFO` / `input_ctrl`), pas des touches : le mapping
//! clavier vit dans chaque front, la navigation vit ici.

use nie_app::flow::Screen;
use nie_app::{MENU, MODES};

/// Nom court de l'écran courant, pour des échecs lisibles.
fn ou(e: &Screen) -> String {
    match e {
        Screen::Title => "titre".into(),
        Screen::Menu { sel } => format!("menu[{sel}]"),
        Screen::ModeSelect { sel } => format!("mode[{sel}]"),
        Screen::Match { .. } => "match".into(),
        Screen::Story { idx, .. } => format!("histoire[{idx}]"),
        Screen::Info { title } => format!("info({title})"),
        Screen::Liste { titre, sel, .. } => format!("liste({titre})[{sel}]"),
    }
}

/// Titre → menu → « Adversaires » → un mode → **match en cours**.
///
/// C'est le chemin le plus court vers du jeu réel, et le seul qui traverse toute la FSM.
#[test]
fn du_titre_au_match_en_cours() {
    let mut e = Screen::new();
    assert!(
        matches!(e, Screen::Title),
        "on démarre sur le titre, pas {}",
        ou(&e)
    );

    e.input("CMD_ENTER");
    assert!(
        matches!(e, Screen::Menu { sel: 0 }),
        "titre + Entrée → menu, pas {}",
        ou(&e)
    );

    // « Adversaires » est le 6ᵉ onglet (index 5) : le seul qui mène à du jeu.
    let cible = MENU
        .iter()
        .position(|m| *m == "Adversaires")
        .expect("onglet Adversaires");
    for _ in 0..cible {
        e.input("CMD_FCS_MTX_DOWN");
    }
    assert!(
        matches!(e, Screen::Menu { sel } if sel == cible),
        "navigation jusqu'à Adversaires, pas {}",
        ou(&e),
    );

    e.input("CMD_ENTER");
    assert!(
        matches!(e, Screen::ModeSelect { sel: 0 }),
        "→ sélection de mode, pas {}",
        ou(&e)
    );

    // Le mode 0 est l'histoire (dialogues) ; les suivants lancent un vrai match.
    e.input("CMD_FCS_MTX_DOWN");
    e.input("CMD_ENTER");
    assert!(matches!(e, Screen::Match { .. }), "→ match, pas {}", ou(&e));
    assert!(e.in_match(), "in_match() doit suivre l'écran");
    assert_eq!(e.score(), vec![0, 0], "un match commence à 0-0");
}

/// Le match AVANCE : la physique tourne, elle n'est pas figée sur l'écran d'entrée.
///
/// Sans cette vérification, un `update` qui ne ferait rien laisserait un « match » parfaitement
/// immobile — et tous les tests de navigation passeraient quand même.
#[test]
fn le_match_avance_dans_le_temps() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER");
    e.input("CMD_FCS_MTX_DOWN");
    e.input("CMD_ENTER");
    assert!(
        e.in_match(),
        "le parcours doit aboutir à un match, pas {}",
        ou(&e)
    );

    let Screen::Match { world } = &e else {
        unreachable!()
    };
    let depart = world.ball.pos;

    // Une minute de jeu à 60 Hz : assez pour que le ballon bouge, sans dépendre d'un but.
    for _ in 0..3_600 {
        e.update(1.0 / 60.0);
    }

    let Screen::Match { world } = &e else {
        panic!("le match ne doit pas se terminer tout seul : {}", ou(&e))
    };
    let arrivee = world.ball.pos;
    let bouge = (arrivee.x - depart.x).abs() > 0.01
        || (arrivee.y - depart.y).abs() > 0.01
        || (arrivee.z - depart.z).abs() > 0.01;
    assert!(
        bouge,
        "le ballon n'a pas bougé en 60 s simulées : {depart:?} → {arrivee:?}"
    );
}

/// Chaque écran sait revenir en arrière — un jeu où l'on entre sans pouvoir sortir n'est pas
/// jouable, c'est une impasse.
#[test]
fn on_peut_toujours_revenir_en_arriere() {
    let mut e = Screen::new();
    e.input("CMD_ENTER"); // titre → menu
    e.input("CMD_BACK");
    assert!(
        matches!(e, Screen::Title),
        "menu + retour → titre, pas {}",
        ou(&e)
    );

    // Un onglet non encore jouable affiche un écran d'information, dont on doit ressortir.
    e.input("CMD_ENTER");
    e.input("CMD_ENTER"); // onglet 0 → Info
    assert!(
        matches!(e, Screen::Info { .. }),
        "onglet 0 → info, pas {}",
        ou(&e)
    );
    e.input("CMD_BACK");
    assert!(
        matches!(e, Screen::Menu { .. }),
        "info + retour → menu, pas {}",
        ou(&e)
    );

    // Depuis un match, le retour ramène à la sélection de mode.
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER");
    e.input("CMD_FCS_MTX_DOWN");
    e.input("CMD_ENTER");
    assert!(e.in_match(), "match attendu, pas {}", ou(&e));
    e.input("CMD_BACK");
    assert!(
        matches!(e, Screen::ModeSelect { .. }),
        "match + retour → modes, pas {}",
        ou(&e)
    );
}

/// La navigation **boucle** dans les deux sens sur toute la liste, sans jamais sortir des bornes.
///
/// Un `sel` qui déborde indexerait hors de [`MENU`]/[`MODES`] au rendu — panique, donc jeu fermé.
#[test]
fn la_navigation_boucle_sans_deborder() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");

    // Un tour complet vers le bas revient au point de départ.
    for _ in 0..MENU.len() {
        e.input("CMD_FCS_MTX_DOWN");
    }
    assert!(
        matches!(e, Screen::Menu { sel: 0 }),
        "tour complet → retour à 0, pas {}",
        ou(&e)
    );

    // Vers le haut depuis 0 : dernier élément, pas un débordement.
    e.input("CMD_FCS_MTX_UP");
    assert!(
        matches!(e, Screen::Menu { sel } if sel == MENU.len() - 1),
        "haut depuis 0 → dernier onglet, pas {}",
        ou(&e),
    );

    // Même règle sur la liste des modes, qui n'a pas la même longueur.
    e.input("CMD_FCS_MTX_DOWN"); // retour à 0
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER");
    for _ in 0..MODES.len() {
        e.input("CMD_FCS_MTX_DOWN");
    }
    assert!(
        matches!(e, Screen::ModeSelect { sel: 0 }),
        "tour des modes, pas {}",
        ou(&e)
    );
}

/// Le mode Histoire enchaîne ses répliques puis rend la main.
#[test]
fn le_mode_histoire_se_deroule_puis_revient() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER"); // → modes, index 0 = Mode Histoire
    e.input("CMD_ENTER");
    assert!(
        matches!(e, Screen::Story { idx: 0, .. }),
        "→ histoire, pas {}",
        ou(&e)
    );

    // Valider assez de fois pour dépasser la dernière réplique, quelle que soit sa longueur.
    for _ in 0..32 {
        if !matches!(e, Screen::Story { .. }) {
            break;
        }
        e.input("CMD_ENTER");
    }
    assert!(
        matches!(e, Screen::ModeSelect { .. }),
        "la scène doit rendre la main à la sélection de mode, pas {}",
        ou(&e),
    );
}

/// Le match se JOUE : la direction demandée déplace le joueur contrôlé.
///
/// Sans cela, « jouable » se limiterait à regarder une simulation tourner — c'est la différence
/// entre un écran de démonstration et un jeu.
#[test]
fn la_direction_deplace_le_joueur_controle() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER");
    e.input("CMD_FCS_MTX_DOWN");
    e.input("CMD_ENTER");
    assert!(e.in_match(), "match attendu, pas {}", ou(&e));

    let idx = e
        .controlled_player()
        .expect("un joueur doit être contrôlable en match");
    let Screen::Match { world } = &e else {
        unreachable!()
    };
    let depart = world.players[idx].pos;

    // Une seconde vers la droite du terrain (+x), à 60 Hz.
    for _ in 0..60 {
        e.set_game_input(1.0, 0.0, false);
        e.update(1.0 / 60.0);
    }

    let Screen::Match { world } = &e else {
        unreachable!()
    };
    let arrivee = world.players[idx].pos;
    assert!(
        arrivee.x > depart.x + 1.0,
        "le joueur contrôlé doit avancer vers +x : {depart:?} → {arrivee:?}",
    );
}

/// Sans entrée, la simulation reste EXACTEMENT ce qu'elle était : l'IA joue les 22 joueurs.
///
/// C'est la garantie qui permet d'ajouter le contrôle sans invalider les rejeux déterministes.
#[test]
fn sans_entree_la_simulation_est_inchangee() {
    use nie_runtime::World;

    let (mut a, mut b) = (World::kickoff(), World::kickoff());
    for _ in 0..600 {
        a.step(1.0 / 60.0);
        // `b` reçoit une entrée NEUTRE à chaque pas : elle ne doit rien changer.
        b.input = nie_runtime::Input::default();
        b.step(1.0 / 60.0);
    }
    assert_eq!(
        a.score, b.score,
        "le score diverge alors qu'aucune entrée n'a été donnée"
    );
    assert_eq!(a.tick, b.tick);
    for (i, (pa, pb)) in a.players.iter().zip(b.players.iter()).enumerate() {
        assert!(
            (pa.pos.x - pb.pos.x).abs() < 1e-6 && (pa.pos.y - pb.pos.y).abs() < 1e-6,
            "joueur {i} diverge : {:?} vs {:?}",
            pa.pos,
            pb.pos,
        );
    }
}

/// Un onglet peut être rempli de données réelles par le front, et se parcourt alors comme une
/// liste — c'est ce qui distingue « écran en cours d'intégration » de « contenu du jeu ».
#[test]
fn un_onglet_rempli_devient_une_liste_navigable() {
    let mut e = Screen::new();
    e.input("CMD_ENTER"); // → menu, onglet 0 = Composition d'équipe
    e.input("CMD_ENTER"); // → écran d'information
    assert_eq!(
        e.info_title(),
        Some(MENU[0]),
        "l'onglet doit s'annoncer, pas {}",
        ou(&e)
    );

    let lignes: Vec<String> = (0..40).map(|i| format!("Joueur {i}")).collect();
    e.fournir_liste(lignes);
    assert!(matches!(e, Screen::Liste { .. }), "→ liste, pas {}", ou(&e));

    // La navigation boucle, comme partout ailleurs, et ne déborde jamais l'index.
    e.input("CMD_FCS_MTX_UP");
    assert!(
        matches!(&e, Screen::Liste { lignes, sel, .. } if *sel == lignes.len() - 1),
        "haut depuis 0 → dernière ligne, pas {}",
        ou(&e),
    );
    e.input("CMD_BACK");
    assert!(
        matches!(e, Screen::Menu { .. }),
        "retour → menu, pas {}",
        ou(&e)
    );
}

/// Une liste VIDE ne remplace pas l'écran d'information.
///
/// Un front qui ne sait pas charger (données absentes, mauvaise langue) doit laisser le message
/// « en cours d'intégration » : un écran vide sans explication serait pris pour un bug.
#[test]
fn une_liste_vide_laisse_l_ecran_d_information() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    e.input("CMD_ENTER");
    e.fournir_liste(Vec::new());
    assert!(
        matches!(e, Screen::Info { .. }),
        "doit rester un écran d'info, pas {}",
        ou(&e)
    );
}

/// Le mode Histoire joue les répliques RÉELLES quand le front en fournit, et se termine dessus.
#[test]
fn le_mode_histoire_joue_les_repliques_fournies() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER"); // → modes
    e.input("CMD_ENTER"); // → histoire (démonstration)
    assert!(
        e.attend_dialogue(),
        "la scène doit attendre un dialogue réel, pas {}",
        ou(&e)
    );

    let repliques: Vec<String> = (0..7).map(|i| format!("Réplique {i}")).collect();
    e.fournir_dialogue("ev02_01400".into(), repliques.clone());
    assert!(
        !e.attend_dialogue(),
        "le dialogue fourni doit être pris en compte"
    );

    // Il faut exactement autant de validations que de répliques pour sortir — ni la longueur de
    // la scène de démonstration, ni une de plus.
    for i in 0..repliques.len() {
        assert!(
            matches!(e, Screen::Story { .. }),
            "sortie prématurée à la réplique {i} : {}",
            ou(&e),
        );
        e.input("CMD_ENTER");
    }
    assert!(
        matches!(e, Screen::ModeSelect { .. }),
        "la scène doit rendre la main après sa dernière réplique, pas {}",
        ou(&e),
    );
}

/// Un dialogue VIDE laisse la scène de démonstration — comme une liste vide laisse l'écran
/// d'information.
#[test]
fn un_dialogue_vide_laisse_la_demonstration() {
    let mut e = Screen::new();
    e.input("CMD_ENTER");
    for _ in 0..5 {
        e.input("CMD_FCS_MTX_DOWN");
    }
    e.input("CMD_ENTER");
    e.input("CMD_ENTER");
    e.fournir_dialogue("vide".into(), Vec::new());
    assert!(
        e.attend_dialogue(),
        "la démonstration doit rester, pas {}",
        ou(&e)
    );
}
