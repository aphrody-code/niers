//! Sonde : les `null_layer_hashes` d'un `CMenuAttachLocator` désignent-ils des **os nommés**
//! de squelettes `.g4pkm` ?
//!
//! ## La question
//!
//! Un objet de menu porteur d'un `CMenuAttachLocator` n'a aucune position propre dans les
//! fichiers : `MENU_LAYER_INFO` n'en déclare pas, et le composant ne porte qu'une liste plate
//! d'entiers (`null_layer_hashes`). Le compositeur retombe donc sur le centre du canvas, et
//! **27 des 42 objets** de `victory_road_top_menu` s'empilent en (640, 360).
//!
//! L'hypothèse testée ici : ces entiers sont des **CRC-32 de noms d'os** (les « null layers »
//! du vocabulaire Level-5 — des os non-déformants qui servent de points d'ancrage). Si elle
//! tient, la position réelle de ces objets est déjà dans les fichiers, à une indirection près,
//! et le repli au centre est une lacune de résolution, pas une donnée manquante.
//!
//! ## Ce que la sonde mesure
//!
//! Elle construit le dictionnaire `CRC-32(nom d'os) → (fichier, nom, pose monde)` depuis TOUS les
//! `.g4pkm` du VFS, puis confronte chaque entier de chaque `CMenuAttachLocator` du corpus.
//! Le verdict est un **taux de résolution par position dans le quadruplet** : les entrées sont
//! groupées par 4 (`A, B, C, index` — le 4ᵉ est un compteur 0,1,2… observé), et seule une
//! position qui résout massivement désigne un champ « nom d'os ». Un taux faible et diffus
//! réfuterait l'hypothèse — c'est le résultat qu'il faut pouvoir lire, pas seulement le succès.
//!
//! ```text
//! cargo run -p nie-formats --example attach_locator_probe --release [-- <préfixe objbin>]
//! ```

use std::collections::{BTreeMap, BTreeSet};

use nie_formats::cfgbin::crc32;
use nie_formats::objbin::{self, MenuComponent};
use nie_formats::vfs::Vfs;

fn main() {
    let filtre = std::env::args().nth(1);

    let racine = nie_formats::vfs::resolve_game_dir();
    let mut vfs = Vfs::default();
    if let Err(e) = vfs.init(racine.join("data")) {
        eprintln!("skip : VFS indisponible ({e}) — cette sonde exige les assets du jeu.");
        return;
    }

    // ── 1. Dictionnaire CRC-32(nom d'os) → os, construit depuis les squelettes ───────────────
    let g4pkm_paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.ends_with(".g4pkm"))
        .collect();

    // Un même nom d'os (`_pos_base01`…) revient dans des centaines de squelettes : on garde le
    // compte des fichiers porteurs plutôt qu'un seul, sinon le rapport laisserait croire à une
    // correspondance unique là où il y a ambiguïté — ce qui change tout pour un futur port.
    let mut par_hash: BTreeMap<u32, (String, BTreeSet<String>)> = BTreeMap::new();
    let mut n_os = 0usize;
    for p in &g4pkm_paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(layout) = nie_formats::g4pkm::parse(&bytes) else {
            continue;
        };
        for b in &layout.bones {
            n_os += 1;
            let h = crc32(b.name.as_bytes());
            par_hash
                .entry(h)
                .or_insert_with(|| (b.name.clone(), BTreeSet::new()))
                .1
                .insert(p.clone());
        }
    }
    println!(
        "dictionnaire : {} squelettes .g4pkm, {n_os} os, {} hash distincts",
        g4pkm_paths.len(),
        par_hash.len()
    );

    // ── 2. Confrontation avec les CMenuAttachLocator du corpus ──────────────────────────────
    let obj_paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.starts_with("data/common/gamedata/menu/obj/")
                && p.ends_with(".objbin")
                && filtre.as_ref().is_none_or(|f| p.contains(f.as_str()))
        })
        .collect();

    // Second dictionnaire : les NOMS D'OBJET de menu (stem des `.objbin`, qui est aussi le nom
    // de calque dans `MENU_LAYER_INFO`). Les slots que les os ne résolvent pas désignent
    // vraisemblablement des objets — sans ce contre-dictionnaire, on conclurait « inconnu »
    // là où la réponse est simplement dans une autre table.
    let mut par_objet: BTreeMap<u32, String> = BTreeMap::new();
    for p in vfs.iter().map(|(p, _)| p.to_string()).collect::<Vec<_>>() {
        if !p.starts_with("data/common/gamedata/menu/obj/") || !p.ends_with(".objbin") {
            continue;
        }
        if let Some(stem) = p.rsplit('/').next().and_then(|f| f.strip_suffix(".objbin")) {
            par_objet.insert(crc32(stem.as_bytes()), stem.to_string());
        }
    }
    println!("contre-dico : {} noms d'objet de menu", par_objet.len());

    // Résolutions par position dans le quadruplet : c'est la mesure qui tranche.
    let mut vus = [0usize; 4];
    let mut resolus = [0usize; 4];
    let mut resolus_objet = [0usize; 4];
    let mut exemples: Vec<(String, usize, u32, String)> = Vec::new();
    let mut n_locators = 0usize;
    let mut non_multiple_de_4 = 0usize;

    for p in &obj_paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        for c in &obj.components {
            let MenuComponent::AttachLocator(a) = c else {
                continue;
            };
            n_locators += 1;
            if !a.null_layer_hashes.len().is_multiple_of(4) {
                non_multiple_de_4 += 1;
            }
            for (i, h) in a.null_layer_hashes.iter().enumerate() {
                let slot = i % 4;
                vus[slot] += 1;
                if let Some((nom, fichiers)) = par_hash.get(h) {
                    resolus[slot] += 1;
                    if exemples.len() < 12 && slot != 3 {
                        exemples.push((
                            obj.name.clone(),
                            slot,
                            *h,
                            format!("{nom}  ({} squelette(s))", fichiers.len()),
                        ));
                    }
                }
                if par_objet.contains_key(h) {
                    resolus_objet[slot] += 1;
                }
            }
        }
    }

    println!(
        "corpus      : {} objbin, {n_locators} CMenuAttachLocator ({non_multiple_de_4} de taille \
         non multiple de 4)",
        obj_paths.len()
    );
    println!("\ntaux de résolution par position du quadruplet :");
    println!("  slot      os nommé (g4pkm)        objet de menu (.objbin)");
    for slot in 0..4 {
        let v = vus[slot];
        #[allow(clippy::cast_precision_loss)]
        let pct = |n: usize| {
            if v == 0 {
                0.0
            } else {
                n as f64 * 100.0 / v as f64
            }
        };
        println!(
            "  {slot}    {:>6}/{:<6} = {:>6.2} %    {:>6}/{:<6} = {:>6.2} %",
            resolus[slot],
            v,
            pct(resolus[slot]),
            resolus_objet[slot],
            v,
            pct(resolus_objet[slot])
        );
    }

    if exemples.is_empty() {
        println!(
            "\naucune résolution : l'hypothèse « CRC-32 de nom d'os » est réfutée telle quelle."
        );
    } else {
        println!("\nexemples résolus :");
        for (obj, slot, h, nom) in &exemples {
            println!("  {obj:<45} slot {slot}  0x{h:08X} -> {nom}");
        }
    }

    // ── 3. Validation de l'ALGORITHME, pas seulement du champ ───────────────────────────────
    //
    // Un taux global élevé ne dit pas encore COMMENT placer : il faut vérifier que le couple
    // (slot 1, slot 2) se lit bien « l'os `slot1` DANS le squelette de l'objet `slot2` ». On le
    // teste en n'acceptant une résolution que si l'os existe dans CE squelette-là — beaucoup plus
    // exigeant que l'appartenance au dictionnaire global, et c'est ce test qui autorise un port.
    let mut squelette_de_objet: BTreeMap<String, String> = BTreeMap::new();
    for p in &obj_paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        if let Some(g) = &obj.g4pkm_path {
            let chemin = if g.starts_with("data/") {
                g.clone()
            } else {
                format!("data/{g}")
            };
            squelette_de_objet.insert(obj.name.clone(), chemin);
        }
    }

    let mut couples_vus = 0usize;
    let mut dans_hote = 0usize; // variante A : os dans le squelette de l'objet slot 2
    let mut dans_porteur = 0usize; // variante B : os dans le squelette DU LOCATOR lui-même
    let mut echantillon: Vec<String> = Vec::new();
    let mut cache: BTreeMap<String, Option<nie_formats::g4pkm::G4pkmLayout>> = BTreeMap::new();

    let charger = |chemin: &str,
                   cache: &mut BTreeMap<String, Option<nie_formats::g4pkm::G4pkmLayout>>|
     -> bool {
        cache.contains_key(chemin) || {
            let v = vfs
                .read(chemin)
                .ok()
                .and_then(|b| nie_formats::g4pkm::parse(&b).ok());
            cache.insert(chemin.to_string(), v);
            true
        }
    };

    for p in &obj_paths {
        let Ok(bytes) = vfs.read(p) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        let sk_porteur = obj.g4pkm_path.as_ref().map(|g| {
            if g.starts_with("data/") {
                g.clone()
            } else {
                format!("data/{g}")
            }
        });
        for c in &obj.components {
            let MenuComponent::AttachLocator(a) = c else {
                continue;
            };
            for quad in a.null_layer_hashes.chunks_exact(4) {
                couples_vus += 1;
                let (h_os, h_autre) = (quad[1], quad[2]);
                let Some((nom_os, _)) = par_hash.get(&h_os) else {
                    continue;
                };

                // Variante A : dans le squelette de l'objet nommé au slot 2.
                if let Some(nom_autre) = par_objet.get(&h_autre)
                    && let Some(sk) = squelette_de_objet.get(nom_autre)
                {
                    charger(sk, &mut cache);
                    if cache
                        .get(sk)
                        .and_then(|o| o.as_ref())
                        .is_some_and(|l| l.world_pose(nom_os).is_some())
                    {
                        dans_hote += 1;
                    }
                }

                // Variante B : dans le squelette du LOCATOR porteur.
                if let Some(sk) = &sk_porteur {
                    charger(sk, &mut cache);
                    if let Some(t) = cache
                        .get(sk)
                        .and_then(|o| o.as_ref())
                        .and_then(|l| l.world_pose(nom_os))
                    {
                        dans_porteur += 1;
                        if echantillon.len() < 10 && (t.x != 0.0 || t.y != 0.0) {
                            let cible = par_objet
                                .get(&h_autre)
                                .map_or_else(|| format!("0x{h_autre:08X}"), Clone::clone);
                            echantillon.push(format!(
                                "  {:<40} os {:<16} @ ({:7.1}, {:7.1})  <- pour {}",
                                obj.name, nom_os, t.x, t.y, cible
                            ));
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let pct = |n: usize, d: usize| {
        if d == 0 {
            0.0
        } else {
            n as f64 * 100.0 / d as f64
        }
    };
    println!("\nvalidation de l'algorithme — quel squelette porte l'os du slot 1 ?");
    println!("  quadruplets                                {couples_vus}");
    println!(
        "  A. squelette de l'objet nommé au slot 2    {dans_hote} ({:.2} %)",
        pct(dans_hote, couples_vus)
    );
    println!(
        "  B. squelette du LOCATOR porteur            {dans_porteur} ({:.2} %)",
        pct(dans_porteur, couples_vus)
    );
    if !echantillon.is_empty() {
        println!("\npositions réelles résolues par la variante gagnante (échantillon) :");
        for l in &echantillon {
            println!("{l}");
        }
    }
}
