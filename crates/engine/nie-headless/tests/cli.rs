//! Tests d'intégration CLI `nie-headless` via `assert_cmd`.
//!
//! Chaque test construit un fichier synthétique en mémoire dans un fichier
//! temporaire, invoque le binaire et vérifie la sortie JSON ou terse.

#![allow(clippy::pedantic)]

use std::io::Write;

use assert_cmd::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Crée un fichier temporaire avec le contenu donné et retourne son chemin.
fn fichier_temp(contenu: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("tempfile");
    f.write_all(contenu).expect("écriture temp");
    f
}

/// Retourne une commande nie-headless prête à l'usage.
fn cmd() -> Command {
    Command::cargo_bin("nie-headless").expect("binaire nie-headless")
}

// ---------------------------------------------------------------------------
// Helper : construit un fichier @UTF minimal (2 colonnes, 2 lignes)
// ---------------------------------------------------------------------------

fn build_utf_fixture() -> Vec<u8> {
    let string_pool: &[u8] = b"TestTable\0ColA\0ColB\0hello\0world\0";
    let schema: &[u8] = &[0x24, 0x00, 0x00, 0x00, 0x0A, 0x2A, 0x00, 0x00, 0x00, 0x0F];
    let row_data: &[u8] = &[
        0x00, 0x00, 0x00, 42, 0x00, 0x00, 0x00, 20, 0x00, 0x00, 0x00, 99, 0x00, 0x00, 0x00, 26,
    ];

    let rows_offset_rel: u32 = 0x22;
    let string_offset_rel: u32 = 0x32;
    let data_offset_rel: u32 = 0x52;

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&rows_offset_rel.to_be_bytes());
    body.extend_from_slice(&string_offset_rel.to_be_bytes());
    body.extend_from_slice(&data_offset_rel.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&2u16.to_be_bytes());
    body.extend_from_slice(&8u16.to_be_bytes());
    body.extend_from_slice(&2u32.to_be_bytes());
    body.extend_from_slice(schema);
    body.extend_from_slice(row_data);
    body.extend_from_slice(string_pool);

    let table_size = body.len() as u32;
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&[0x40, 0x55, 0x54, 0x46]); // @UTF
    out.extend_from_slice(&table_size.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

// ---------------------------------------------------------------------------
// Helper : construit un fichier CRILAYLA avec un payload littéral
// ---------------------------------------------------------------------------

fn build_crilayla_payload(payload: &[u8]) -> Vec<u8> {
    struct Bw {
        bytes: Vec<u8>,
        cur: u8,
        used: u32,
    }
    impl Bw {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                cur: 0,
                used: 0,
            }
        }
        fn push_bit(&mut self, bit: u8) {
            self.cur = (self.cur << 1) | (bit & 1);
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.cur);
                self.cur = 0;
                self.used = 0;
            }
        }
        fn push_bits(&mut self, val: u32, n: u32) {
            for i in (0..n).rev() {
                self.push_bit(((val >> i) & 1) as u8);
            }
        }
        fn finish(mut self) -> Vec<u8> {
            if self.used > 0 {
                self.cur <<= 8 - self.used;
                self.bytes.push(self.cur);
            }
            self.bytes
        }
    }

    let mut bw = Bw::new();
    for &b in payload.iter().rev() {
        bw.push_bit(0);
        bw.push_bits(b as u32, 8);
    }
    let mut bs = bw.finish();
    bs.reverse();

    let header_offset = bs.len() as u32;
    let uncompressed_size = payload.len() as u32;

    let mut out = Vec::new();
    out.extend_from_slice(b"CRILAYLA");
    out.extend_from_slice(&uncompressed_size.to_le_bytes());
    out.extend_from_slice(&header_offset.to_le_bytes());
    out.extend_from_slice(&bs);
    out.extend(vec![0xABu8; 0x100]);
    out
}

// ---------------------------------------------------------------------------
// Helper : construit un fichier RDBN vide
// ---------------------------------------------------------------------------

fn build_rdbn_vide() -> Vec<u8> {
    let mut buf = vec![0u8; 0x50];
    buf[0..4].copy_from_slice(b"RDBN");
    buf[4..6].copy_from_slice(&0x50i16.to_le_bytes());
    buf[6..10].copy_from_slice(&100i32.to_le_bytes());
    buf[10..12].copy_from_slice(&0x14i16.to_le_bytes());
    buf
}

// ---------------------------------------------------------------------------
// Helper : construit un CPK minimal (magic + 12 zéros + @UTF)
// ---------------------------------------------------------------------------

fn build_cpk_minimal() -> Vec<u8> {
    let utf = build_utf_fixture();
    let mut out = Vec::new();
    out.extend_from_slice(b"CPK ");
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(&utf);
    out
}

// ---------------------------------------------------------------------------
// Tests sous-commande `detect`
// ---------------------------------------------------------------------------

#[test]
fn format_utf_detecte() {
    let f = fichier_temp(&build_utf_fixture());
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["format"], "@UTF");
    assert_eq!(json["detail"]["nom_table"], "TestTable");
    assert_eq!(json["detail"]["colonnes"], 2);
    assert_eq!(json["detail"]["lignes"], 2);
}

#[test]
fn format_cpk_detecte() {
    let f = fichier_temp(&build_cpk_minimal());
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["format"], "CPK");
    assert_eq!(json["detail"]["nom_table"], "TestTable");
}

#[test]
fn format_crilayla_detecte() {
    let payload = b"hello headless";
    let f = fichier_temp(&build_crilayla_payload(payload));
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["format"], "CRILAYLA");
    assert_eq!(json["detail"]["taille_decompresse"], 0x100 + payload.len());
}

#[test]
fn format_cfgbin_detecte() {
    let f = fichier_temp(&build_rdbn_vide());
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["format"], "cfg.bin");
    assert_eq!(json["detail"]["version"], 100);
    assert_eq!(json["detail"]["types"], 0);
    assert_eq!(json["detail"]["champs"], 0);
    assert_eq!(json["detail"]["racines"], 0);
}

#[test]
fn format_inconnu_renvoie_vide() {
    let f = fichier_temp(b"NOFORMAT_RANDOM_DATA_XYZ\x00\x01\x02\x03");
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["format"], "?");
    assert_eq!(json["detail"], serde_json::json!({}));
}

#[test]
fn taille_octets_correcte() {
    let donnees = build_utf_fixture();
    let taille = donnees.len() as u64;
    let f = fichier_temp(&donnees);
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide");
    assert_eq!(json["taille_octets"], taille);
}

#[test]
fn fichier_inexistant_renvoie_erreur() {
    cmd()
        .arg("detect")
        .arg("/tmp/__nie_headless_inexistant_abc123.bin")
        .assert()
        .failure();
}

#[test]
fn sortie_json_compacte_valide() {
    let f = fichier_temp(&build_utf_fixture());
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(sortie).expect("UTF-8");
    let lignes: Vec<&str> = s.lines().collect();
    assert_eq!(lignes.len(), 1, "JSON compact = 1 seule ligne");
}

#[test]
fn sortie_json_pretty_valide() {
    let f = fichier_temp(&build_utf_fixture());
    let sortie = cmd()
        .arg("detect")
        .arg(f.path())
        .arg("--indent=2")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&sortie).expect("JSON valide (pretty)");
    assert!(json["format"].is_string());
}

// ---------------------------------------------------------------------------
// Tests sous-commande `match`
// ---------------------------------------------------------------------------

/// La sous-commande `match` sans arguments produit une sortie terse valide.
#[test]
fn match_sortie_terse_valide() {
    let sortie = cmd()
        .arg("match")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(sortie).expect("UTF-8");
    let s = s.trim();
    // Format attendu : "home=... away=... résultat=H-A horloge=CLOCK"
    assert!(
        s.starts_with("home="),
        "sortie doit commencer par 'home=': {s}"
    );
    assert!(
        s.contains("résultat="),
        "sortie doit contenir 'résultat=': {s}"
    );
    assert!(
        s.contains("horloge="),
        "sortie doit contenir 'horloge=': {s}"
    );
}

/// L'horloge finale est toujours 900_000 pour 90 minutes (CONFIRMÉ).
///
/// Formule C : `minutes * 10_000 + seconds` (`FUN_1412aa4a0` case 7 L282-288).
#[test]
fn match_horloge_900000() {
    let sortie = cmd()
        .arg("match")
        .arg("--seed=42")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(sortie).expect("UTF-8");
    assert!(
        s.contains("horloge=900000"),
        "horloge finale doit être 900000 : {s}"
    );
}

/// Deux exécutions avec la même graine produisent la même sortie (déterminisme).
#[test]
fn match_determinisme_meme_seed() {
    let run = |seed: u64| {
        String::from_utf8(
            cmd()
                .arg("match")
                .arg(format!("--seed={seed}"))
                .assert()
                .success()
                .get_output()
                .stdout
                .clone(),
        )
        .expect("UTF-8")
    };
    let r1 = run(999);
    let r2 = run(999);
    assert_eq!(r1, r2, "même graine → même sortie");
}

/// La simulation est **sensible à la graine** : sur une plage de graines, plusieurs résultats
/// distincts apparaissent.
///
/// NB : tester deux graines *précises* (ex. 1 vs 2) est fragile — le modèle de but étant
/// probabiliste et grossier, certaines paires collisionnent sur un même score (1 et 2 donnent
/// tous deux « 1-1 »). On vérifie donc la sensibilité sur un balayage : ≥ 2 sorties distinctes.
#[test]
fn match_graines_differentes() {
    let run = |seed: u64| {
        String::from_utf8(
            cmd()
                .arg("match")
                .arg(format!("--seed={seed}"))
                .assert()
                .success()
                .get_output()
                .stdout
                .clone(),
        )
        .expect("UTF-8")
    };
    let distinctes: std::collections::HashSet<String> = (1..=10u64).map(run).collect();
    assert!(
        distinctes.len() >= 2,
        "graines différentes → sorties variées (sim sensible à la graine) ; obtenu {} distincte(s) sur 10 graines",
        distinctes.len()
    );
}

/// Les noms d'équipes passés en argument apparaissent dans la sortie.
#[test]
fn match_noms_equipes_dans_sortie() {
    let sortie = cmd()
        .arg("match")
        .arg("--home=Raimon")
        .arg("--away=Teikoku")
        .arg("--seed=1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(sortie).expect("UTF-8");
    assert!(s.contains("home=Raimon"), "nom domicile manquant : {s}");
    assert!(s.contains("away=Teikoku"), "nom visiteur manquant : {s}");
}

/// Les stats personnalisées sont acceptées et produisent une sortie valide.
#[test]
fn match_stats_personnalisees_valides() {
    cmd()
        .arg("match")
        .arg("--home-stats=200:190:185:175:160:170:155")
        .arg("--away-stats=180:175:170:165:155:160:150")
        .arg("--seed=7")
        .assert()
        .success();
}

/// Des stats malformées produisent une erreur.
#[test]
fn match_stats_malformees_erreur() {
    cmd()
        .arg("match")
        .arg("--home-stats=200:190") // seulement 2 valeurs au lieu de 7
        .assert()
        .failure();
}
