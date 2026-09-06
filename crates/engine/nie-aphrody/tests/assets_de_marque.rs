//! Les assets de marque sont-ils réellement utilisables ?
//!
//! Produire un fichier n'est pas le rendre valide : un PNG aux mauvaises dimensions, un `.ico`
//! dont les offsets sont faux ou un manifeste sans les tailles exigées passent tous
//! l'écriture sur disque sans un mot. Ces tests vérifient la forme des octets, pas leur
//! présence.

use nie_aphrody::{
    Pet,
    assets::{TAILLES_FAVICON, encoder_png, reduire_rgba, svg_depuis_png},
};

fn pet() -> Pet {
    Pet::bundled().expect("paquet embarqué")
}

#[test]
fn la_reduction_moyenne_les_pixels_au_lieu_d_en_jeter() {
    // Deux pixels opaques, l'un noir l'autre blanc, réduits à un seul : la moyenne est le
    // gris. Le plus proche voisin rendrait l'un des deux, ce qui est le défaut qu'on évite.
    let src = [0, 0, 0, 255, 255, 255, 255, 255];
    let out = reduire_rgba(&src, 2, 1, 1, 1).expect("réduction");
    assert_eq!(out[3], 255, "l'alpha doit rester opaque");
    assert!(
        (126..=129).contains(&out[0]),
        "attendu un gris median, obtenu {}",
        out[0]
    );
}

#[test]
fn un_pixel_transparent_ne_teinte_pas_son_voisin() {
    // Rouge opaque + magenta ENTIÈREMENT transparent. Sans prémultiplication, la moyenne
    // naïve rendrait un rose : la couleur d'un pixel invisible n'a aucune raison de compter.
    let src = [255, 0, 0, 255, 255, 0, 255, 0];
    let out = reduire_rgba(&src, 2, 1, 1, 1).expect("réduction");
    assert_eq!(
        (out[0], out[1], out[2]),
        (255, 0, 0),
        "la teinte doit rester celle du seul pixel visible"
    );
    assert_eq!(out[3], 128, "l'alpha, lui, est bien la moyenne des deux");
}

#[test]
fn les_dimensions_incoherentes_sont_refusees() {
    assert!(
        reduire_rgba(&[0; 8], 3, 1, 1, 1).is_none(),
        "tampon trop court"
    );
    assert!(reduire_rgba(&[0; 8], 2, 1, 0, 1).is_none(), "cible nulle");
    assert!(reduire_rgba(&[], 0, 0, 1, 1).is_none(), "source vide");
}

#[test]
fn le_jeu_complet_est_produit_et_chaque_png_a_sa_taille() {
    let p = pet();
    let frame = &p.animation("idle").expect("idle").frames[0];
    let fichiers = p
        .assets_de_marque(frame, TAILLES_FAVICON)
        .expect("assets produits");

    for t in TAILLES_FAVICON {
        let f = fichiers
            .iter()
            .find(|f| f.nom == format!("icone-{t}.png"))
            .unwrap_or_else(|| panic!("icone-{t}.png manquante"));
        // Signature PNG, puis largeur et hauteur lues dans l'en-tête IHDR.
        assert_eq!(&f.octets[..8], b"\x89PNG\r\n\x1a\n", "signature PNG");
        let w = u32::from_be_bytes(f.octets[16..20].try_into().expect("largeur"));
        let h = u32::from_be_bytes(f.octets[20..24].try_into().expect("hauteur"));
        assert_eq!((w, h), (*t, *t), "icone-{t}.png annonce {w}×{h}");
    }

    for nom in ["favicon.ico", "icone.svg", "site.webmanifest"] {
        assert!(fichiers.iter().any(|f| f.nom == nom), "{nom} manquant");
    }
}

#[test]
fn l_ico_declare_ses_entrees_avec_des_offsets_qui_tombent_juste() {
    let p = pet();
    let frame = &p.animation("idle").expect("idle").frames[0];
    let fichiers = p.assets_de_marque(frame, TAILLES_FAVICON).expect("assets");
    let ico = &fichiers
        .iter()
        .find(|f| f.nom == "favicon.ico")
        .expect("favicon.ico")
        .octets;

    assert_eq!(&ico[0..2], &[0, 0], "champ réservé");
    assert_eq!(&ico[2..4], &[1, 0], "type icône");
    let n = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(n >= 3, "au moins trois tailles dans le .ico, obtenu {n}");

    // Chaque entrée doit pointer sur une image réellement présente : un offset faux produit
    // un .ico que Windows accepte d'ouvrir et affiche vide.
    for i in 0..n {
        let e = 6 + 16 * i;
        let taille = u32::from_le_bytes(ico[e + 8..e + 12].try_into().expect("taille")) as usize;
        let offset = u32::from_le_bytes(ico[e + 12..e + 16].try_into().expect("offset")) as usize;
        assert!(
            offset + taille <= ico.len(),
            "entrée {i} déborde : {offset}+{taille} > {}",
            ico.len()
        );
        assert_eq!(
            &ico[offset..offset + 8],
            b"\x89PNG\r\n\x1a\n",
            "entrée {i} ne pointe pas sur un PNG"
        );
    }
}

#[test]
fn le_manifeste_porte_les_deux_tailles_exigees_par_une_app_installable() {
    let p = pet();
    let m = p.manifeste_web(TAILLES_FAVICON);
    let v: serde_json::Value = serde_json::from_str(&m).expect("manifeste JSON valide");
    let icones = v["icons"].as_array().expect("icons");
    for taille in ["192x192", "512x512"] {
        assert!(
            icones.iter().any(|i| i["sizes"] == taille),
            "un manifeste installable exige {taille}"
        );
    }
    assert!(
        icones.iter().any(|i| i["purpose"] == "any maskable"),
        "au moins une icône maskable"
    );
}

#[test]
fn le_svg_reste_leger_et_bien_forme() {
    let png = encoder_png(&[255, 0, 0, 255], 1, 1).expect("png");
    let svg = svg_depuis_png(&png, 1, 1, "Test");
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("data:image/png;base64,"));
    assert!(
        svg.contains("<title>Test</title>"),
        "un titre pour l'accessibilité"
    );

    // Le SVG du jeu complet ne doit pas embarquer la plus grande icône : en base64 (+33 %),
    // la 512 pesait 138 Ko, soit plus que toutes les autres réunies.
    let p = pet();
    let frame = &p.animation("idle").expect("idle").frames[0];
    let fichiers = p.assets_de_marque(frame, TAILLES_FAVICON).expect("assets");
    let svg = fichiers.iter().find(|f| f.nom == "icone.svg").expect("svg");
    assert!(
        svg.octets.len() < 40_000,
        "SVG de {} o : trop lourd pour un favicon",
        svg.octets.len()
    );
}
