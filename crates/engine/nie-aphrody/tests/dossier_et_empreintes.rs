//! Le dossier embarqué se lit, et le README ne ment pas sur les octets qu'il documente.
//!
//! Ce second point n'est pas décoratif : deux des quatre empreintes du README étaient
//! périmées, et le test d'intégrité échouait sur `pet.json` sans que rien n'explique pourquoi.
//! Une empreinte écrite à la main dérive dès qu'un fichier est reformaté ; celle-ci est
//! recalculée à chaque exécution et comparée au tableau, donc la dérive devient impossible à
//! introduire en silence.

use nie_aphrody::{
    BUNDLED_ANIMATIONS_JSON, BUNDLED_ATLAS_PNG, BUNDLED_ATLAS_WEBP, BUNDLED_DOSSIER_JSON,
    BUNDLED_DOSSIER_MD, BUNDLED_PET_JSON, Dossier, Pet, sha256_hex,
};

#[test]
fn le_dossier_embarque_se_lit_et_porte_ses_blocs() {
    let d = Dossier::bundled().expect("dossier embarqué valide");
    assert_eq!(d.slug, "byron-love-aphrody");
    assert!(!d.genere_le.is_empty());

    // Les trois ères du personnage : IE1, GO, Ares.
    assert_eq!(d.codes_internes.len(), 3);
    assert!(d.codes_internes.iter().all(|c| c.starts_with('c')));

    for bloc in [
        "identite",
        "statistiques",
        "techniques",
        "auras",
        "medias",
        "sources",
        "jeu",
        "pet",
    ] {
        assert!(d.bloc(bloc).is_some(), "bloc manquant : {bloc}");
    }

    // L'identité porte ce que seules les sources externes savent.
    assert_eq!(d.identite_str("romaji"), Some("Afuro Terumi"));
    assert!(d.identite_str("furigana").is_some_and(|f| !f.is_empty()));
    assert!(d.blocs().len() >= 8);
}

#[test]
fn le_bloc_pet_du_dossier_decrit_le_paquet_embarque() {
    let d = Dossier::bundled().expect("dossier");
    let pet_paquet = Pet::bundled().expect("paquet");
    let pet_bloc = d.bloc("pet").expect("bloc pet");

    // Le dossier est généré depuis les mêmes fichiers que la crate embarque : si les deux
    // divergent, c'est que le dossier a été régénéré contre un autre paquet.
    let frames_paquet = u64::try_from(pet_paquet.manifest.exported_frame_count)
        .expect("nombre de frames representable");
    assert_eq!(
        pet_bloc
            .get("total_frames")
            .and_then(serde_json::Value::as_u64),
        Some(frames_paquet),
    );
    assert_eq!(
        pet_bloc
            .get("animations")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(pet_paquet.manifest.animations.len()),
    );
}

#[test]
fn le_markdown_embarque_est_le_meme_dossier() {
    let d = Dossier::bundled().expect("dossier");
    assert!(BUNDLED_DOSSIER_MD.len() > 1_000);
    // Le Markdown doit parler du même personnage que le JSON.
    let nom = d.identite_str("nom_fr").expect("nom_fr");
    assert!(
        BUNDLED_DOSSIER_MD.contains(nom),
        "le Markdown ne mentionne pas {nom}"
    );
}

#[test]
fn le_readme_ne_ment_pas_sur_les_empreintes() {
    let readme = include_str!("../assets/aphrody/README.md");
    for (fichier, octets) in [
        ("pet.json", BUNDLED_PET_JSON.as_bytes()),
        ("animations.json", BUNDLED_ANIMATIONS_JSON.as_bytes()),
        ("sprites/spritesheet.png", BUNDLED_ATLAS_PNG),
        ("sprites/spritesheet.webp", BUNDLED_ATLAS_WEBP),
    ] {
        let attendu = sha256_hex(octets);
        assert!(
            readme.contains(&attendu),
            "README : empreinte périmée ou absente pour {fichier} (réelle : {attendu})",
        );
    }
}

#[test]
fn le_dossier_json_est_stable_et_non_vide() {
    assert!(BUNDLED_DOSSIER_JSON.len() > 10_000);
    let v: serde_json::Value = serde_json::from_str(BUNDLED_DOSSIER_JSON).expect("JSON valide");
    assert!(v.is_object());
}
