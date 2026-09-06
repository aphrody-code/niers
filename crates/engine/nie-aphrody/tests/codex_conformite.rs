//! Aphrody est-il installable comme un Codex pet ?
//!
//! Le paquet suit la spécification publique des Codex pets (galerie
//! `legeling/awesome-codex-pet`, code MIT). Ces tests le vérifient plutôt que de l'affirmer :
//! une spécification tierce peut évoluer, et le jour où notre atlas ne correspondra plus, il
//! vaut mieux l'apprendre ici qu'au moment où quelqu'un tente d'installer le pet.

use nie_aphrody::{
    BUNDLED_ATLAS_WEBP, BUNDLED_PET_JSON, Pet,
    codex::{Version, conformite, entree_installation},
};

#[test]
fn le_paquet_embarque_est_un_codex_pet_v2_conforme() {
    let pet = Pet::bundled().expect("paquet");
    let c = conformite(&pet.pet, &pet.manifest);
    assert!(c.ok(), "écarts : {:?}", c.ecarts);
    assert_eq!(c.version, Some(Version::V2));
}

#[test]
fn l_entree_d_installation_decrit_les_octets_reellement_embarques() {
    let pet = Pet::bundled().expect("paquet");
    let e = entree_installation(
        &pet.pet,
        &pet.manifest,
        BUNDLED_PET_JSON.as_bytes(),
        BUNDLED_ATLAS_WEBP,
    )
    .expect("paquet conforme");

    assert_eq!(e.sprite_version_number, 2);
    // Les dimensions de la v2, telles que la spécification les fixe.
    assert_eq!((e.spritesheet_width, e.spritesheet_height), (1536, 2288));
    assert_eq!(e.pet_json_bytes, BUNDLED_PET_JSON.len());
    assert_eq!(e.spritesheet_bytes, BUNDLED_ATLAS_WEBP.len());
    assert_eq!(e.pet_json_sha256.len(), 64);

    // L'entrée doit se sérialiser sous les noms que l'installeur lit — un champ renommé de
    // travers produirait un manifeste syntaxiquement valide et fonctionnellement mort.
    let j = serde_json::to_value(&e).expect("sérialisable");
    for champ in [
        "name",
        "spriteVersionNumber",
        "petJsonSha256",
        "petJsonBytes",
        "spritesheetSha256",
        "spritesheetBytes",
        "spritesheetWidth",
        "spritesheetHeight",
    ] {
        assert!(
            j.get(champ).is_some(),
            "champ absent du manifeste : {champ}"
        );
    }
}

#[test]
fn un_atlas_hors_specification_est_refuse_avec_sa_raison() {
    let mut pet = Pet::bundled().expect("paquet");
    pet.manifest.atlas.rows = 10;
    pet.manifest.atlas.height = 2080;

    let c = conformite(&pet.pet, &pet.manifest);
    assert!(!c.ok());
    assert_eq!(c.version, None);
    assert!(
        c.ecarts
            .iter()
            .any(|e| e.contains("ni v1") && e.contains("ni v2")),
        "l'écart doit nommer les deux versions attendues : {:?}",
        c.ecarts
    );

    // Et l'entrée d'installation refuse de se produire : publier une entrée pour un paquet
    // qu'un installeur rejettera ne rend service à personne.
    assert!(
        entree_installation(
            &pet.pet,
            &pet.manifest,
            BUNDLED_PET_JSON.as_bytes(),
            BUNDLED_ATLAS_WEBP
        )
        .is_err()
    );
}
