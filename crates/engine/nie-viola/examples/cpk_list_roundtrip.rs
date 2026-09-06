//! Vérifie les deux allers-retours du `cpk_list.cfg.bin`, séparément.
//!
//! 1. **Enveloppe seule** : `decrypt` → `encrypt` sans rien toucher. Si c'est fidèle, on peut
//!    patcher les octets en clair et rechiffrer sans risque.
//! 2. **Réencodage T2B** : `decode` → `encode_t2b`. C'est l'étape que `pack_mod` emploie
//!    aujourd'hui, et que `CLAUDE.md` donne pour infidèle — on le mesure au lieu de le croire.
//!
//! Usage : `cargo run -p nie-viola --example cpk_list_roundtrip -- <cpk_list.cfg.bin>`

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: cpk_list_roundtrip <cpk_list.cfg.bin>");
    let brut = std::fs::read(&path).expect("lecture");
    println!("fichier   {path}");
    println!("octets    {}", brut.len());

    // ── 1. enveloppe seule ──────────────────────────────────────────────────────
    let clair = nie_formats::cpk::decrypt_cpk_list(&brut).expect("déchiffrement AES");
    println!("clair     {} octets", clair.len());
    let rechiffre = nie_formats::cpk::encrypt_cpk_list(&clair);
    let env_ok = rechiffre == brut;
    println!(
        "enveloppe decrypt→encrypt : {} ({} octets)",
        if env_ok { "FIDÈLE" } else { "INFIDÈLE" },
        rechiffre.len()
    );
    if !env_ok {
        let n = rechiffre
            .iter()
            .zip(brut.iter())
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "          {n} octets diffèrent, Δtaille = {}",
            rechiffre.len() as i64 - brut.len() as i64
        );
    }

    // ── 2. réencodage T2B ───────────────────────────────────────────────────────
    let cfg = nie_formats::cfgbin::cfgbin_parse(&clair).expect("parse T2B");
    let reencode = nie_formats::cfgbin::encode_t2b(&cfg.entries);
    let t2b_ok = reencode == clair;
    println!(
        "T2B       decode→encode  : {} ({} octets)",
        if t2b_ok { "FIDÈLE" } else { "INFIDÈLE" },
        reencode.len()
    );
    if !t2b_ok {
        let n = reencode
            .iter()
            .zip(clair.iter())
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "          {n} octets diffèrent, Δtaille = {}",
            reencode.len() as i64 - clair.len() as i64
        );
    }

    println!(
        "\nverdict   patcher les octets en clair est {}",
        if env_ok {
            "SÛR (l'enveloppe est fidèle)"
        } else {
            "à proscrire (l'enveloppe elle-même dérive)"
        }
    );
}
