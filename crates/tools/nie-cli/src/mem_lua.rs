//! `niers mem lua-field` — relève la valeur d'un champ de table Lua dans le jeu vivant.
//!
//! Certaines valeurs d'écran ne vivent dans **aucun fichier** du jeu : le chunk Lua déclare la
//! clé (`listRowNum`, `listLineNum`, `pageNum`…) mais ne l'affecte jamais ; c'est le moteur C++
//! qui la pose au moment où l'écran s'instancie. Cinq sources de fichiers ont été épuisées sans
//! la trouver (cf. `docs/AVATAR.md`). La seule lecture possible est donc la mémoire du process.
//!
//! La chaîne, telle qu'elle a été établie à la main sur le process du jeu :
//!
//! 1. Lua interne les chaînes courtes. Une `TString` x86-64 (Lua 5.2) est
//!    `next(8) | tt(1) marked(1) extra(1) pad(1) hash(4) | len(8) | données…` : chercher
//!    `len` suivi du nom et de son NUL identifie l'objet chaîne, et `TString* = addr(len) - 16`.
//! 2. Une entrée de table est un `Node { TValue i_val; TKey i_key; }` = **40 octets**
//!    (`TValue` 16 ; `TKey` = `value_(8) tt_(4) pad(4) next(8)` 24). Chercher le pointeur vers
//!    la `TString` donne la position de `i_key.value_` ; la valeur est 16 octets AVANT.
//! 3. `i_key.tt_` doit valoir `LUA_TSTRING` (4) — le runtime observé marque en plus le bit
//!    collectable 0x40, d'où le masque.
//!
//! Le même process porte plusieurs états Lua (plusieurs `TString` internées pour un même nom) et
//! plusieurs écrans instanciés : la commande rend **toutes** les occurrences plutôt qu'une
//! réponse unique — c'est à l'appelant de savoir quel écran il a ouvert.

use anyhow::Context as _;

/// Taille d'un `Node` de table Lua 5.2 sur x86-64.
const NODE_SIZE: u64 = 40;
/// Décalage de `i_key.value_` depuis le début du `Node` (la taille d'un `TValue`).
const KEY_OFFSET: u64 = 16;
/// Décalage de `TString.len` depuis le début de l'objet `TString`.
const TSTRING_LEN_OFFSET: u64 = 16;
/// Masque qui retire le bit « collectable » du tag de type.
const TT_MASK: u32 = 0x3f;

/// Un champ relevé : où il vit, et ce qu'il vaut.
struct Releve {
    /// Adresse du `Node` (début de `i_val`).
    node: u64,
    /// Valeur décodée, rendue lisible.
    valeur: String,
    /// Tag de type Lua de la valeur, masqué du bit collectable.
    tt: u32,
}

/// Nom lisible d'un tag de type Lua.
fn nom_type(tt: u32) -> &'static str {
    match tt & TT_MASK {
        0 => "nil",
        1 => "boolean",
        2 => "lightuserdata",
        3 => "number",
        4 => "string",
        5 => "table",
        6 => "function",
        7 => "userdata",
        8 => "thread",
        _ => "?",
    }
}

/// Rend un `TValue` (8 octets de charge utile + tag) sous forme lisible.
///
/// Un `number` Lua 5.2 est un `double` : `2.0` s'affiche `2` pour rester comparable à ce qu'un
/// script Lua en ferait, et la forme fractionnaire est conservée quand elle existe.
fn decoder_valeur(charge: [u8; 8], tt: u32) -> String {
    let brut = u64::from_le_bytes(charge);
    match tt & TT_MASK {
        0 => "nil".to_string(),
        1 => if brut != 0 { "true" } else { "false" }.to_string(),
        3 => {
            let d = f64::from_le_bytes(charge);
            if d.fract() == 0.0 && d.abs() < 1e15 {
                format!("{}", d as i64)
            } else {
                format!("{d}")
            }
        }
        t => format!("{}@0x{brut:x}", nom_type(t)),
    }
}

/// Lit `n` octets, ou rend `None` si la plage n'est pas lisible.
fn lire(pid: i32, addr: u64, n: usize) -> Option<Vec<u8>> {
    nie_trace::read_exact(pid, addr, n).ok()
}

/// Lit un `u32` little-endian.
fn u32_at(buf: &[u8], off: usize) -> Option<u32> {
    buf.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Lit un `u64` little-endian.
fn u64_at(buf: &[u8], off: usize) -> Option<u64> {
    let b = buf.get(off..off + 8)?;
    Some(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// Rend le nom d'une `TString` Lua, si l'objet a bien cette forme.
fn nom_tstring(pid: i32, tstring: u64, max: usize) -> Option<String> {
    let buf = lire(pid, tstring, TSTRING_LEN_OFFSET as usize + 8 + max)?;
    let tt = *buf.get(8)?;
    if tt & TT_MASK as u8 != 4 {
        return None;
    }
    let len = u64_at(&buf, TSTRING_LEN_OFFSET as usize)? as usize;
    if len == 0 || len > max {
        return None;
    }
    let debut = TSTRING_LEN_OFFSET as usize + 8;
    let octets = buf.get(debut..debut + len)?;
    std::str::from_utf8(octets).ok().map(str::to_string)
}

/// Cherche les objets `TString` internés qui portent exactement `nom`.
fn trouver_tstrings(pid: i32, nom: &str, limite: usize) -> Vec<u64> {
    let mut motif = (nom.len() as u64).to_le_bytes().to_vec();
    motif.extend_from_slice(nom.as_bytes());
    motif.push(0);
    let regions = nie_trace::module_regions(pid, "", true);
    nie_trace::scan_regions(pid, &regions, None, &motif, limite)
        .into_iter()
        .map(|h| h.addr - TSTRING_LEN_OFFSET)
        .collect()
}

/// Cherche les `Node` dont la clé est `tstring`, et décode leur valeur.
fn relever_nodes(pid: i32, tstring: u64, limite: usize) -> Vec<Releve> {
    let motif = tstring.to_le_bytes().to_vec();
    let regions = nie_trace::module_regions(pid, "", true);
    let mut out = Vec::new();
    for hit in nie_trace::scan_regions(pid, &regions, None, &motif, limite) {
        // `hit.addr` est la position de `i_key.value_` ; le Node commence 16 octets avant.
        let node = match hit.addr.checked_sub(KEY_OFFSET) {
            Some(n) => n,
            None => continue,
        };
        let Some(buf) = lire(pid, node, NODE_SIZE as usize) else {
            continue;
        };
        // La clé doit être une chaîne : sinon le pointeur a été trouvé ailleurs qu'en position
        // de clé (table de chaînes internées, pool de constantes d'un prototype…).
        let Some(key_tt) = u32_at(&buf, KEY_OFFSET as usize + 8) else {
            continue;
        };
        if key_tt & TT_MASK != 4 {
            continue;
        }
        let Some(val_tt) = u32_at(&buf, 8) else {
            continue;
        };
        let mut charge = [0u8; 8];
        charge.copy_from_slice(&buf[..8]);
        out.push(Releve {
            node,
            valeur: decoder_valeur(charge, val_tt),
            tt: val_tt & TT_MASK,
        });
    }
    out
}

/// Balaie les `Node` voisins et rend les paires `clé → valeur` du même tableau de hachage.
fn voisins(pid: i32, node: u64, rayon: i64, max_nom: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for k in -rayon..=rayon {
        let Some(addr) = node.checked_add_signed(k * NODE_SIZE as i64) else {
            continue;
        };
        let Some(buf) = lire(pid, addr, NODE_SIZE as usize) else {
            continue;
        };
        let Some(key_tt) = u32_at(&buf, KEY_OFFSET as usize + 8) else {
            continue;
        };
        if key_tt & TT_MASK != 4 {
            continue;
        }
        let Some(key_ptr) = u64_at(&buf, KEY_OFFSET as usize) else {
            continue;
        };
        let Some(nom) = nom_tstring(pid, key_ptr, max_nom) else {
            continue;
        };
        let Some(val_tt) = u32_at(&buf, 8) else {
            continue;
        };
        let mut charge = [0u8; 8];
        charge.copy_from_slice(&buf[..8]);
        out.push((nom, decoder_valeur(charge, val_tt)));
    }
    out
}

/// Point d'entrée de `niers mem lua-field`.
///
/// `rayon` > 0 déclenche l'affichage des entrées voisines de chaque `Node` trouvé — c'est ce qui
/// donne la table d'état complète (`listNum`, `listRowNum`, `listLineNum`, `pageNum`, `lastLine`)
/// plutôt qu'un champ isolé. `nombres_seuls` écarte les entrées dont la valeur n'est pas un
/// scalaire, seul bruit notable de la méthode.
pub fn lua_field(
    pid: i32,
    nom: &str,
    limite_chaines: usize,
    limite_nodes: usize,
    rayon: i64,
    nombres_seuls: bool,
) -> anyhow::Result<()> {
    let pid = if pid > 0 {
        pid
    } else {
        nie_trace::find_pid_by_name("nie.exe")
            .context("nie.exe introuvable — lance le jeu, ou précise --pid")?
    };

    let tstrings = trouver_tstrings(pid, nom, limite_chaines);
    if tstrings.is_empty() {
        println!("  aucune TString internée « {nom} » — le champ n'existe pas dans ce process.");
        return Ok(());
    }
    println!(
        "  {} objet(s) TString « {nom} » (pid {pid})",
        tstrings.len()
    );

    let mut total = 0usize;
    for ts in &tstrings {
        let mut releves = relever_nodes(pid, *ts, limite_nodes);
        // Le pointeur d'une TString se retrouve aussi en position de clé d'entrées SANS rapport
        // (tables de globales, `_LOADED`, pools de constantes). Ne garder que les valeurs
        // numériques élimine ce bruit d'un coup quand on cherche une dimension d'écran.
        if nombres_seuls {
            releves.retain(|r| r.tt == 3 || r.tt == 1);
        }
        if releves.is_empty() {
            continue;
        }
        println!(
            "\n  TString 0x{ts:x} — {} entrée(s) de table",
            releves.len()
        );
        for r in &releves {
            total += 1;
            println!("    node 0x{:012x}  {nom} = {}", r.node, r.valeur);
            if rayon > 0 {
                for (k, v) in voisins(pid, r.node, rayon, 64) {
                    if k != nom {
                        println!("      · {k} = {v}");
                    }
                }
            }
        }
    }
    if total == 0 {
        println!("\n  aucune entrée de table : la clé est internée mais aucune table ne la porte.");
    } else {
        println!("\n  {total} entrée(s) au total.");
    }
    Ok(())
}

// ─── Palettes de l'éditeur d'avatar ───────────────────────────────────────────

/// Taille d'une entrée de la table de palettes : identifiant puis couleur, alignés sur 16.
const TAILLE_ENTREE_PALETTE: usize = 16;

/// Relève la table `colorPresetID → couleur` de l'éditeur d'avatar dans le jeu vivant.
///
/// Le catalogue (`m_CharaEditColorDataList`) ne donne que des **identifiants** : les valeurs de
/// couleur n'existent ni dans ce fichier, ni dans le binaire — le motif d'une entrée connue y est
/// absent — ni sous une forme résoluble depuis les chaînes, contrairement aux canaux `red`,
/// `green` et `blue` (0 identifiant de palette sur 165 s'y retrouve). Elles ne sont donc lisibles
/// que dans la mémoire du processus, où le jeu les charge.
///
/// Forme de la table, relevée : par entrée, l'identifiant CRC-32 en little-endian sur 4 octets,
/// puis la couleur **ARGB** sur 4 octets (l'alpha en tête, `0xFF` pour une couleur opaque), le
/// reste servant d'alignement. Les entrées se suivent, alignées sur 4.
///
/// `identifiants` borne la recherche : seuls les identifiants attendus sont retenus, ce qui écarte
/// les coïncidences qu'un balayage libre de la mémoire produirait à coup sûr.
pub fn palettes(
    pid: i32,
    identifiants: &[u32],
    debut: u64,
    longueur: usize,
) -> anyhow::Result<Vec<(u32, [u8; 4])>> {
    let pid = if pid > 0 {
        pid
    } else {
        nie_trace::find_pid_by_name("nie.exe")
            .context("nie.exe introuvable — lance le jeu, ou précise --pid")?
    };
    let attendus: std::collections::BTreeSet<u32> = identifiants.iter().copied().collect();
    let octets = nie_trace::read_exact(pid, debut, longueur)
        .map_err(|e| anyhow::anyhow!("lecture de {longueur} octets à 0x{debut:x} : {e}"))?;

    let mut vus: std::collections::BTreeMap<u32, [u8; 4]> = std::collections::BTreeMap::new();
    let mut offset = 0usize;
    while offset + 8 <= octets.len() {
        let Some(id) = u32_at(&octets, offset) else {
            break;
        };
        if attendus.contains(&id) {
            let argb = [
                octets[offset + 4],
                octets[offset + 5],
                octets[offset + 6],
                octets[offset + 7],
            ];
            // Un identifiant peut réapparaître ailleurs qu'en tête d'entrée — le balayage est
            // libre, au pas de 4 octets. Les entrées de la vraie table sont OPAQUES : privilégier
            // `alpha = 255` écarte ces coïncidences, qui rendaient des couleurs semi-transparentes.
            match vus.entry(id) {
                std::collections::btree_map::Entry::Vacant(v) => {
                    v.insert(argb);
                }
                std::collections::btree_map::Entry::Occupied(mut o) => {
                    if o.get()[0] != 0xFF && argb[0] == 0xFF {
                        o.insert(argb);
                    }
                }
            }
        }
        offset += 4;
    }
    let _ = TAILLE_ENTREE_PALETTE;
    Ok(vus.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_number_entier_se_rend_sans_partie_decimale() {
        assert_eq!(decoder_valeur(2.0f64.to_le_bytes(), 3), "2");
        assert_eq!(decoder_valeur(6.0f64.to_le_bytes(), 3), "6");
    }

    #[test]
    fn un_number_fractionnaire_garde_sa_partie_decimale() {
        assert_eq!(decoder_valeur(0.5f64.to_le_bytes(), 3), "0.5");
    }

    #[test]
    fn le_bit_collectable_ne_change_pas_le_type() {
        // Le runtime observé marque les chaînes 0x44 = LUA_TSTRING | 0x40.
        assert_eq!(nom_type(0x44), "string");
        assert_eq!(nom_type(4), "string");
        assert_eq!(nom_type(0x45), "table");
    }

    #[test]
    fn un_booleen_se_rend_en_mots() {
        assert_eq!(decoder_valeur(1u64.to_le_bytes(), 1), "true");
        assert_eq!(decoder_valeur(0u64.to_le_bytes(), 1), "false");
    }
}
