//! Validation du parseur g4sk contre un vrai fichier : FK(poses locales C) doit reproduire la
//! bind-pose MONDE (section A), et `world · inverse_bind ≈ I`.
//! Usage : `cargo run -p nie-formats --example validate_g4sk -- <fichier.g4sk>`

use nie_formats::g4sk;

#[allow(clippy::needless_range_loop)] // transposition row-major → col-major : indices explicites.
fn read_mat_a(data: &[u8], i: usize) -> [[f32; 4]; 4] {
    // Section A (bind-pose monde) : commence à l'octet 0x40, matrice 3×4 row-major par os.
    let o = 0x40 + i * 12 * 4;
    let f = |k: usize| f32::from_le_bytes(data[o + k * 4..o + k * 4 + 4].try_into().unwrap());
    let mut m = [[0.0f32; 4]; 4];
    for r in 0..3 {
        for c in 0..4 {
            m[c][r] = f(r * 4 + c);
        }
    }
    m[3][3] = 1.0;
    m
}

fn max_diff(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> f32 {
    let mut d = 0.0f32;
    for c in 0..4 {
        for r in 0..4 {
            d = d.max((a[c][r] - b[c][r]).abs());
        }
    }
    d
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: validate_g4sk <fichier.g4sk>");
    let data = std::fs::read(&path).expect("lire g4sk");
    let header = g4sk::parse_header(&data).expect("header");
    let bones = g4sk::parse_hierarchy(&data, &header);
    let poses = g4sk::parse_poses(&data, &header).expect("poses");
    let parents: Vec<i16> = bones.bones.iter().map(|b| b.parent_index).collect();
    println!(
        "os={} heuristic_parents={}",
        header.bone_count, bones.heuristic
    );

    let world = g4sk::rest_world_matrices(&poses, &parents);

    // 1) FK(C) ≈ A (bind-pose monde).
    let mut max_fk = 0.0f32;
    for (i, w) in world.iter().enumerate() {
        max_fk = max_fk.max(max_diff(w, &read_mat_a(&data, i)));
    }
    println!("max |FK(C) − A| = {max_fk:.2e}  (attendu < 1e-4)");

    // 2) world(A) · inverse_bind ≈ identité.
    let ident = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut max_inv = 0.0f32;
    for (i, p) in poses.iter().enumerate() {
        let a = read_mat_a(&data, i);
        let prod = g4sk::mat_mul(&a, &p.inverse_bind);
        max_inv = max_inv.max(max_diff(&prod, &ident));
    }
    println!("max |A · inverse_bind − I| = {max_inv:.2e}  (attendu < 1e-4)");

    let ok = max_fk < 1e-4 && max_inv < 1e-4;
    println!("{}", if ok { "VALIDÉ ✓" } else { "ÉCHEC ✗" });
    if !ok {
        std::process::exit(1);
    }
}
