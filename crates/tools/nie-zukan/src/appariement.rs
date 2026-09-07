//! Appariement flou zukan ↔ miroir inagle, et audit des appariements posés.
//!
//! Port de `packages/inagle/src/zukan/matcher.ts` (434 l.) et
//! `packages/inagle/src/zukan/audit.ts` (302 l.).
//!
//! # Pourquoi ici
//!
//! [`crate::cross`] apparie déjà zukan et le miroir inagle — mais par **égalité
//! exacte** de `game_id`/`internal_code`. Ce module est la voie floue du même
//! problème : quand aucun identifiant commun n'existe, on note poste, élément,
//! genre, ère, description et profil de statistiques, puis on assigne en 1:1.
//! Les deux voies appartiennent au même crate parce qu'elles répondent à la
//! même question sur les mêmes données.
//!
//! # La fusion des deux Spearman
//!
//! `matcher.ts:169` (`spearmanCorrelation`) et `audit.ts:122`
//! (`auditSpearmanCorrelation`) sont **le même algorithme, dupliqué** —
//! `audit.ts` le dit lui-même : « Doublon assumé du précédent ». Les deux corps
//! sont identiques instruction par instruction. Ils sont fusionnés ici en une
//! seule [`correlation_spearman`], sous le nom de l'antériorité (celui du
//! matcher).
//!
//! Ce qui **n'est pas** fusionné, parce que ce sont de vraies divergences que le
//! TS documente comme volontaires (`audit.ts:14-19`) :
//!
//! | Table | Matcher | Audit |
//! |---|---|---|
//! | positions | + `Entraîneur`/`Coach` | + `Defenseur` (sans cédille) |
//! | éléments | + caractères japonais, + `Aucun` | sous-ensemble strict |
//! | jeu → série | branche `Orion` | **pas** de branche `Orion` |
//! | ères | identiques → fusionnées en [`ERES`] | idem |
//!
//! # Ce qui n'est pas porté
//!
//! Les scripts qui *alimentent* ces fonctions (`map-zukan-images.ts`,
//! `remap-zukan-mirror.ts`, `audit-zukan-matches.ts`, `audit-zukan-mirror.ts`)
//! lisent Postgres ou le miroir `SQLite` : c'est de l'I/O, hors périmètre.

// `float_cmp` : la détection des ex æquo de Spearman reproduit le `===` de
// JavaScript. Une tolérance à epsilon changerait les rangs, donc le score.
// `cast_precision_loss` : les comptes portent sur 7 statistiques et quelques
// dizaines de mots — très loin des 2^53 où un `usize` cesse d'être exact.
// `cast_possible_truncation` : les scores partiels sont bornés (30 et 15).
// `implicit_hasher` : les signatures reproduisent des `Map` du TS ; imposer un
// paramètre de hacheur aux appelants n'apporterait rien.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::implicit_hasher,
    clippy::items_after_statements
)]

use std::collections::{HashMap, HashSet};

// ═══════════════════════════════════════════════════════════════════════════
// Entrées
// ═══════════════════════════════════════════════════════════════════════════

/// Profil de statistiques utilisé par l'appariement, dans l'ordre exact des
/// deux tableaux comparés (`matcher.ts:328-345`).
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StatsAppariement {
    /// Frappe.
    pub kick: f64,
    /// Contrôle.
    pub control: f64,
    /// Technique.
    pub technique: f64,
    /// Pression.
    pub pressure: f64,
    /// Physique.
    pub physical: f64,
    /// Agilité.
    pub agility: f64,
    /// Intelligence.
    pub intelligence: f64,
}

impl StatsAppariement {
    /// Rend les 7 valeurs dans l'ordre comparé : `kick, control, technique,
    /// pressure, physical, agility, intelligence`.
    #[must_use]
    pub fn en_tableau(self) -> [f64; 7] {
        [
            self.kick,
            self.control,
            self.technique,
            self.pressure,
            self.physical,
            self.agility,
            self.intelligence,
        ]
    }
}

/// Sous-ensemble d'une entrée zukan nécessaire à l'appariement et à l'audit.
///
/// Fusion de `ZukanMatchEntry` (`matcher.ts:29-43`) et `AuditZukanEntry`
/// (`audit.ts:24-34`). Le champ `nickname`, déclaré par les deux interfaces,
/// n'est **lu par aucune des deux** : il n'est pas porté.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntreeZukan {
    /// Nom tel que publié par zukan.
    pub nom: String,
    /// Empreinte de l'entrée (`zukanHash`), clé du mapping 1:1.
    pub zukan_hash: Option<String>,
    /// Poste, tel que publié (EN, FR ou japonais).
    pub position: Option<String>,
    /// Élément, tel que publié.
    pub element: Option<String>,
    /// Statistiques publiées (niveau 50 côté zukan).
    pub stats: Option<StatsAppariement>,
    /// Titre du jeu d'apparition.
    pub jeu: Option<String>,
    /// Genre (`Male`, `Female`, `男`, `女`).
    pub genre: Option<String>,
    /// Description anglaise.
    pub description: Option<String>,
}

/// Sous-ensemble d'une ligne `inagle_characters` nécessaire aux deux moteurs.
///
/// Fusion de `ZukanMatchDbChar` (`matcher.ts:46-64`) et `AuditDbRow`
/// (`audit.ts:37-54`). `matchScore` ne lit **aucun** des trois noms ; seul
/// l'audit s'en sert.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LigneInagle {
    /// Identifiant de la ligne.
    pub id: String,
    /// Nom anglais.
    pub name_en: String,
    /// Nom français.
    pub name_fr: Option<String>,
    /// Nom japonais.
    pub name_ja: Option<String>,
    /// Poste.
    pub position: String,
    /// Élément.
    pub element: String,
    /// Genre (`M`/`F`).
    pub gender: Option<String>,
    /// Libellé de rareté (`Héros` déclenche un traitement particulier).
    pub rarity_label: String,
    /// Série d'appartenance.
    pub series: Option<String>,
    /// Empreinte zukan déjà assignée.
    pub zukan_hash: Option<String>,
    /// Statistiques niveau 99, dans l'ordre `frappe, contrôle, technique,
    /// pression, physique, agilité, intelligence`. `None` sur la première
    /// valeur désactive toute comparaison de profil.
    pub stats: Option<StatsAppariement>,
    /// Description anglaise.
    pub description_en: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Normalisation — matcher
// ═══════════════════════════════════════════════════════════════════════════

/// Table de normalisation des postes du matcher (`matcher.ts:71-82`).
pub const POSITIONS_MATCHER: [(&str, &str); 10] = [
    ("FW", "FW"),
    ("MF", "MF"),
    ("DF", "DF"),
    ("GK", "GK"),
    ("Attaquant", "FW"),
    ("Milieu", "MF"),
    ("Défenseur", "DF"),
    ("Gardien", "GK"),
    ("Entraîneur", "Coach"),
    ("Coach", "Coach"),
];

/// Table de normalisation des éléments du matcher (`matcher.ts:85-102`).
pub const ELEMENTS_MATCHER: [(&str, &str); 18] = [
    ("Fire", "Fire"),
    ("Wind", "Wind"),
    ("Forest", "Forest"),
    ("Mountain", "Mountain"),
    ("Void", "Void"),
    ("Feu", "Fire"),
    ("Vent", "Wind"),
    ("Forêt", "Forest"),
    ("Montagne", "Mountain"),
    ("Néant", "Void"),
    ("Foret", "Forest"),
    ("Neant", "Void"),
    ("Aucun", "Void"),
    ("火", "Fire"),
    ("風", "Wind"),
    ("林", "Forest"),
    ("山", "Mountain"),
    ("無", "Void"),
];

fn resoudre<'a>(table: &[(&str, &'a str)], cle: &'a str) -> &'a str {
    table
        .iter()
        .find(|(k, _)| *k == cle)
        .map_or(cle, |(_, v)| *v)
}

/// Normalise un poste vers sa forme anglaise canonique (`matcher.ts:104`).
///
/// Une valeur absente de la table est rendue **telle quelle**, pas rejetée.
#[must_use]
pub fn norm_pos(p: Option<&str>) -> Option<String> {
    let p = p.filter(|s| !s.is_empty())?;
    Some(resoudre(&POSITIONS_MATCHER, p).to_string())
}

/// Normalise un élément vers sa forme anglaise canonique (`matcher.ts:108`).
#[must_use]
pub fn norm_elem(e: Option<&str>) -> Option<String> {
    let e = e.filter(|s| !s.is_empty())?;
    Some(resoudre(&ELEMENTS_MATCHER, e).to_string())
}

/// Traduit un titre de jeu zukan en nom de série du miroir (`matcher.ts:113-127`).
///
/// La variante d'audit ([`audit_zukan_jeu_vers_serie`]) diffère : elle n'a pas
/// la branche `Orion`.
#[must_use]
pub fn zukan_jeu_vers_serie(jeu: Option<&str>) -> Option<String> {
    let jeu = jeu.filter(|s| !s.is_empty())?;
    let serie = if jeu == "Inazuma Eleven: Victory Road" {
        "Victory Road"
    } else if jeu == "Inazuma Eleven Ares" {
        "Ares"
    } else if jeu == "Inazuma Eleven Orion" {
        "Orion"
    } else if jeu.starts_with("Inazuma Eleven GO Galaxy") {
        "Galaxy"
    } else if jeu.starts_with("Inazuma Eleven GO Chrono Stones")
        || jeu.starts_with("Inazuma Eleven GO2")
    {
        "Chrono Stone"
    } else if jeu.starts_with("Inazuma Eleven GO") {
        "Inazuma Eleven GO"
    } else if jeu.starts_with("Inazuma Eleven 3") {
        "Inazuma Eleven 3"
    } else if jeu.starts_with("Inazuma Eleven 2") {
        "Inazuma Eleven 2"
    } else if jeu == "Inazuma Eleven" {
        "Inazuma Eleven"
    } else {
        return None;
    };
    Some(serie.to_string())
}

/// Série → ère. `ERAS` (`matcher.ts:130-140`) et `AUDIT_ERAS` (`audit.ts:96-106`)
/// sont **identiques** : elles sont fusionnées ici.
pub const ERES: [(&str, &str); 9] = [
    ("Inazuma Eleven", "OG"),
    ("Inazuma Eleven 2", "OG"),
    ("Inazuma Eleven 3", "OG"),
    ("Inazuma Eleven GO", "GO"),
    ("Chrono Stone", "GO"),
    ("Galaxy", "GO"),
    ("Ares", "Modern"),
    ("Orion", "Modern"),
    ("Victory Road", "Modern"),
];

/// Ordre canonique des ères — plus petit = plus ancien (`matcher.ts:143`).
pub const ORDRE_ERES: [(&str, u32); 3] = [("OG", 1), ("GO", 2), ("Modern", 3)];

/// Ère d'une série, ou `None` si la série est inconnue.
#[must_use]
pub fn ere(serie: &str) -> Option<&'static str> {
    ERES.iter().find(|(s, _)| *s == serie).map(|(_, e)| *e)
}

/// Deux séries distinctes appartiennent-elles à la même ère ? (`matcher.ts:146`)
#[must_use]
pub fn meme_ere(a: &str, b: &str) -> bool {
    match (ere(a), ere(b)) {
        (Some(ea), Some(eb)) => ea == eb && a != b,
        _ => false,
    }
}

/// Normalise le genre zukan vers la forme du miroir (`matcher.ts:153-158`).
#[must_use]
pub fn norm_genre(g: Option<&str>) -> Option<String> {
    match g.filter(|s| !s.is_empty())? {
        "Male" | "男" => Some("M".to_string()),
        "Female" | "女" => Some("F".to_string()),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Similarité
// ═══════════════════════════════════════════════════════════════════════════

/// Corrélation de rang de Spearman entre deux séries de même longueur.
///
/// **Fusion** de `spearmanCorrelation` (`matcher.ts:167-192`) et
/// `auditSpearmanCorrelation` (`audit.ts:122-142`), corps identiques. Le nom
/// retenu est celui de l'antériorité — le matcher, dont `audit.ts` se déclare
/// lui-même le doublon.
///
/// Les rangs ex æquo reçoivent leur rang **moyen**. Rend `0.0` si les séries
/// comptent moins de 2 éléments ou n'ont pas la même longueur (le TS n'inspecte
/// que `a.length`, et n'est appelé que sur des tableaux de 7).
#[must_use]
pub fn correlation_spearman(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    if n < 2 || b.len() != n {
        return 0.0;
    }

    fn rangs(valeurs: &[f64]) -> Vec<f64> {
        let n = valeurs.len();
        let mut ordre: Vec<usize> = (0..n).collect();
        // Tri STABLE croissant, comme `Array.prototype.sort` en JS moderne.
        ordre.sort_by(|&x, &y| {
            valeurs[x]
                .partial_cmp(&valeurs[y])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut r = vec![0.0f64; n];
        let mut i = 0usize;
        while i < n {
            let mut j = i;
            while j < n - 1 && valeurs[ordre[j + 1]] == valeurs[ordre[j]] {
                j += 1;
            }
            let rang_moyen = (i + j) as f64 / 2.0 + 1.0;
            for k in i..=j {
                r[ordre[k]] = rang_moyen;
            }
            i = j + 1;
        }
        r
    }

    let ra = rangs(a);
    let rb = rangs(b);

    let somme_d2: f64 = ra
        .iter()
        .zip(rb.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum();

    let n = n as f64;
    1.0 - (6.0 * somme_d2) / (n * (n * n - 1.0))
}

/// Mots vides écartés du calcul de similarité de description (`matcher.ts:194-239`).
pub const MOTS_VIDES: [&str; 38] = [
    "a", "an", "the", "and", "or", "but", "in", "on", "of", "to", "is", "are", "was", "were", "he",
    "she", "they", "his", "her", "their", "it", "its", "with", "for", "at", "from", "that", "this",
    "as", "be", "by", "has", "have", "had", "not", "who", "which", "when",
];

/// Découpe une description comme le TS : minuscules, tout caractère hors
/// `[a-z0-9\s]` **supprimé** (pas remplacé), mots de plus de 2 lettres, hors
/// mots vides.
fn tokeniser(s: &str) -> HashSet<String> {
    let filtre: String = s
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c.is_whitespace())
        .collect();
    filtre
        .split_whitespace()
        .filter(|m| m.len() > 2 && !MOTS_VIDES.contains(m))
        .map(str::to_string)
        .collect()
}

/// Similarité de Jaccard entre deux descriptions anglaises (`matcher.ts:241-259`).
///
/// Rend `0.0` si l'une des deux ne produit aucun mot retenu.
#[must_use]
pub fn similarite_description(a: &str, b: &str) -> f64 {
    let sa = tokeniser(a);
    let sb = tokeniser(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let intersection = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    intersection as f64 / union as f64
}

// ═══════════════════════════════════════════════════════════════════════════
// Score d'appariement
// ═══════════════════════════════════════════════════════════════════════════

/// Score minimal exigé par défaut pour retenir un appariement
/// (`matcher.ts:402`, `:422`) — il impose au moins poste + élément.
pub const SCORE_MINIMAL: i32 = 20;

/// Score de compatibilité entre une entrée zukan et une ligne du miroir.
///
/// Port de `matchScore` (`matcher.ts:264-359`). Plus haut = meilleur ; **`-1`
/// est un rejet dur**, pas un mauvais score.
///
/// Les trois rejets durs :
/// - poste connu des deux côtés et différent ;
/// - élément connu des deux côtés et différent ;
/// - ères connues des deux côtés et différentes — **sauf** pour les `Héros`,
///   qui reprennent leur design d'origine et privilégient l'ère la plus
///   ancienne.
#[must_use]
pub fn score_appariement(z: &EntreeZukan, db: &LigneInagle) -> i32 {
    let z_pos = norm_pos(z.position.as_deref());
    let z_elem = norm_elem(z.element.as_deref());
    let d_pos = norm_pos(Some(db.position.as_str()));
    let d_elem = norm_elem(Some(db.element.as_str()));

    if let (Some(zp), Some(dp)) = (&z_pos, &d_pos)
        && zp != dp
    {
        return -1;
    }
    if let (Some(ze), Some(de)) = (&z_elem, &d_elem)
        && ze != de
    {
        return -1;
    }

    let mut score = 0i32;
    if z_pos.is_some() && z_pos == d_pos {
        score += 10;
    }
    if z_elem.is_some() && z_elem == d_elem {
        score += 10;
    }

    // Genre : +5 si concordance, -10 si divergence explicite.
    if let (Some(zg), Some(dg)) = (norm_genre(z.genre.as_deref()), db.gender.as_deref())
        && !dg.is_empty()
    {
        if zg == dg {
            score += 5;
        } else {
            score -= 10;
        }
    }

    // Série — barrière d'ère dure.
    let est_heros = db.rarity_label == "Héros";
    let z_serie = zukan_jeu_vers_serie(z.jeu.as_deref());
    if let (Some(zs), Some(dbs)) = (&z_serie, db.series.as_deref().filter(|s| !s.is_empty())) {
        let ere_db = ere(dbs);
        let ere_z = ere(zs);
        if est_heros {
            let ordre_z = ere_z
                .and_then(|e| ORDRE_ERES.iter().find(|(k, _)| *k == e).map(|(_, v)| *v))
                .unwrap_or(0);
            if ordre_z == 1 {
                score += 60; // ère d'origine = le meilleur portrait pour un Héros
            } else if zs == dbs {
                score += 40;
            } else if ere_z == ere_db {
                score += 30;
            } else {
                score += 5;
            }
        } else {
            if ere_db.is_some() && ere_z.is_some() && ere_db != ere_z {
                return -1; // barrière dure entre ères
            }
            if zs == dbs {
                score += 50;
            } else if ere_db.is_some() && ere_z.is_some() && ere_db == ere_z {
                score += 20;
            }
        }
    }

    // Similarité de description.
    if let (Some(zd), Some(dd)) = (
        z.description.as_deref().filter(|s| !s.is_empty()),
        db.description_en.as_deref().filter(|s| !s.is_empty()),
    ) {
        let sim = similarite_description(zd, dd);
        if sim >= 0.15 {
            score += (sim * 30.0).round() as i32;
        } else if sim == 0.0 {
            score -= 10; // aucun mot commun = mauvais signe
        }
    }

    // Profil de statistiques — Spearman, ramené sur 0..15.
    if let (Some(zs), Some(ds)) = (z.stats, db.stats) {
        let z_arr = zs.en_tableau();
        let d_arr = ds.en_tableau();
        let z_total: f64 = z_arr.iter().sum();
        let d_total: f64 = d_arr.iter().sum();
        if z_total > 0.0 && d_total > 0.0 {
            let corr = correlation_spearman(&z_arr, &d_arr);
            score += (corr.max(0.0) * 15.0).round() as i32;
        }
    }

    score
}

// ═══════════════════════════════════════════════════════════════════════════
// Assignation 1:1
// ═══════════════════════════════════════════════════════════════════════════

/// Contexte d'assignation : les empreintes déjà consommées sur ce run.
///
/// Le matcher d'origine gardait ce jeu au niveau du module ; `matcher.ts:377`
/// l'a paramétré pour qu'aucun état ne fuite d'un run à l'autre. **Le
/// comportement par run est inchangé** — c'est ce qui garantit le 1:1.
#[derive(Debug, Clone, Default)]
pub struct ContexteAppariement {
    /// Empreintes zukan déjà assignées.
    pub hashes_utilises: HashSet<String>,
}

impl ContexteAppariement {
    /// Crée un contexte vide (`createMatchContext`, `matcher.ts:385`).
    #[must_use]
    pub fn nouveau() -> Self {
        Self::default()
    }
}

/// Apparie un groupe zukan et un groupe du miroir en respectant le 1:1.
///
/// Port de `matchGroupsStrict` (`matcher.ts:387-415`) : toutes les paires de
/// score ≥ [`SCORE_MINIMAL`] sont construites, triées par score décroissant
/// (tri **stable**, comme en JS), puis consommées gloutonnement.
pub fn apparier_groupes_strict(
    ctx: &mut ContexteAppariement,
    groupe_zukan: &[EntreeZukan],
    groupe_db: &[LigneInagle],
    assignes: &mut HashMap<String, String>,
) {
    let db_libres: Vec<&LigneInagle> = groupe_db
        .iter()
        .filter(|c| !assignes.contains_key(&c.id))
        .collect();
    if db_libres.is_empty() {
        return;
    }

    let disponibles: Vec<&EntreeZukan> = groupe_zukan
        .iter()
        .filter(|z| {
            z.zukan_hash
                .as_ref()
                .is_some_and(|h| !ctx.hashes_utilises.contains(h))
        })
        .collect();

    let mut paires: Vec<(&LigneInagle, &EntreeZukan, i32)> = Vec::new();
    for db in &db_libres {
        for z in &disponibles {
            let s = score_appariement(z, db);
            if s >= SCORE_MINIMAL {
                paires.push((db, z, s));
            }
        }
    }

    paires.sort_by_key(|(_, _, s)| std::cmp::Reverse(*s));
    for (db, z, _) in paires {
        let Some(hash) = z.zukan_hash.as_ref() else {
            continue;
        };
        if assignes.contains_key(&db.id) || ctx.hashes_utilises.contains(hash) {
            continue;
        }
        assignes.insert(db.id.clone(), hash.clone());
        ctx.hashes_utilises.insert(hash.clone());
    }
}

/// Assigne à une ligne du miroir la meilleure entrée zukan encore libre.
///
/// Port de `assignBest` (`matcher.ts:417-434`). `score_min` vaut
/// [`SCORE_MINIMAL`] dans le TS ; il reste explicite ici, la valeur par défaut
/// d'un paramètre n'étant pas reproductible en Rust.
pub fn assigner_meilleur(
    ctx: &mut ContexteAppariement,
    c: &LigneInagle,
    candidats: &[EntreeZukan],
    assignes: &mut HashMap<String, String>,
    score_min: i32,
) {
    if assignes.contains_key(&c.id) {
        return;
    }
    let mut classes: Vec<(&EntreeZukan, i32)> = candidats
        .iter()
        .filter(|z| {
            z.zukan_hash
                .as_ref()
                .is_some_and(|h| !ctx.hashes_utilises.contains(h))
        })
        .map(|z| (z, score_appariement(z, c)))
        .filter(|(_, s)| *s >= score_min)
        .collect();
    classes.sort_by_key(|(_, s)| std::cmp::Reverse(*s));

    if let Some((z, _)) = classes.first()
        && let Some(hash) = z.zukan_hash.as_ref()
    {
        assignes.insert(c.id.clone(), hash.clone());
        ctx.hashes_utilises.insert(hash.clone());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Audit — normalisation divergente, préservée telle quelle
// ═══════════════════════════════════════════════════════════════════════════

/// Table des postes de l'audit (`audit.ts:59-69`) — ajoute `Defenseur` sans
/// cédille, omet `Entraîneur`/`Coach`. **Ne pas remplacer** par
/// [`POSITIONS_MATCHER`] : la divergence est documentée comme volontaire.
pub const POSITIONS_AUDIT: [(&str, &str); 9] = [
    ("FW", "FW"),
    ("MF", "MF"),
    ("DF", "DF"),
    ("GK", "GK"),
    ("Attaquant", "FW"),
    ("Milieu", "MF"),
    ("Défenseur", "DF"),
    ("Defenseur", "DF"),
    ("Gardien", "GK"),
];

/// Table des éléments de l'audit (`audit.ts:71-84`) — sous-ensemble strict de
/// [`ELEMENTS_MATCHER`] : ni caractères japonais, ni `Aucun`.
pub const ELEMENTS_AUDIT: [(&str, &str); 12] = [
    ("Fire", "Fire"),
    ("Wind", "Wind"),
    ("Forest", "Forest"),
    ("Mountain", "Mountain"),
    ("Void", "Void"),
    ("Feu", "Fire"),
    ("Vent", "Wind"),
    ("Forêt", "Forest"),
    ("Foret", "Forest"),
    ("Montagne", "Mountain"),
    ("Néant", "Void"),
    ("Neant", "Void"),
];

/// Normalise un poste avec la table de l'audit (`audit.ts:86`).
#[must_use]
pub fn audit_norm_pos(p: Option<&str>) -> Option<String> {
    let p = p.filter(|s| !s.is_empty())?;
    Some(resoudre(&POSITIONS_AUDIT, p).to_string())
}

/// Normalise un élément avec la table de l'audit (`audit.ts:91`).
#[must_use]
pub fn audit_norm_elem(e: Option<&str>) -> Option<String> {
    let e = e.filter(|s| !s.is_empty())?;
    Some(resoudre(&ELEMENTS_AUDIT, e).to_string())
}

/// Traduit un titre de jeu en série, variante d'audit (`audit.ts:109-121`).
///
/// **Sans** la branche `Orion` : « Inazuma Eleven Orion » y rend `None`, là où
/// [`zukan_jeu_vers_serie`] rend `Some("Orion")`. Verbatim.
#[must_use]
pub fn audit_zukan_jeu_vers_serie(jeu: Option<&str>) -> Option<String> {
    let jeu = jeu.filter(|s| !s.is_empty())?;
    let serie = if jeu == "Inazuma Eleven: Victory Road" {
        "Victory Road"
    } else if jeu == "Inazuma Eleven Ares" {
        "Ares"
    } else if jeu.starts_with("Inazuma Eleven GO Galaxy") {
        "Galaxy"
    } else if jeu.starts_with("Inazuma Eleven GO Chrono Stones")
        || jeu.starts_with("Inazuma Eleven GO2")
    {
        "Chrono Stone"
    } else if jeu.starts_with("Inazuma Eleven GO") {
        "Inazuma Eleven GO"
    } else if jeu.starts_with("Inazuma Eleven 3") {
        "Inazuma Eleven 3"
    } else if jeu.starts_with("Inazuma Eleven 2") {
        "Inazuma Eleven 2"
    } else if jeu == "Inazuma Eleven" {
        "Inazuma Eleven"
    } else {
        return None;
    };
    Some(serie.to_string())
}

/// Nature d'une anomalie d'audit (`audit.ts:148-152`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeAnomalie {
    /// L'empreinte assignée n'existe plus dans les données zukan courantes.
    HashPerime,
    /// Les noms ne se recoupent pas.
    NomDivergent,
    /// Les genres divergent.
    GenreDivergent,
    /// Poste, élément, ère ou profil de statistiques divergent.
    AttributDivergent,
}

impl TypeAnomalie {
    /// Libellé exact utilisé par inagle.
    #[must_use]
    pub fn libelle_inagle(self) -> &'static str {
        match self {
            Self::HashPerime => "STALE_HASH",
            Self::NomDivergent => "NAME_MISMATCH",
            Self::GenreDivergent => "GENDER_MISMATCH",
            Self::AttributDivergent => "ATTR_MISMATCH",
        }
    }
}

/// Anomalie relevée sur une ligne du miroir (`audit.ts:155-162`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalieAudit {
    /// Nature de l'anomalie.
    pub type_anomalie: TypeAnomalie,
    /// Ligne du miroir concernée.
    pub db: LigneInagle,
    /// Entrée zukan résolue par empreinte — absente si l'empreinte est périmée.
    pub zukan: Option<EntreeZukan>,
    /// Raisons, dans l'ordre où le TS les empile.
    pub raisons: Vec<String>,
    /// Corrélation de profil, si les deux côtés portent des statistiques.
    pub correlation_stats: Option<f64>,
}

/// Évalue une ligne du miroir contre les entrées zukan indexées par empreinte.
///
/// Port de `evaluateRow` (`audit.ts:179-292`). Rend `None` si aucun critère
/// n'échoue.
///
/// Critères, dans l'ordre : nom (exact EN/FR, premier mot, inclusion, base
/// `MixiMax`), poste, élément (**ignoré** pour un `MixiMax`), genre, ère (ignorée
/// pour un Héros d'Ares), corrélation de profil `< 0.3`.
///
/// # Piège porté tel quel
///
/// Le test de nom se termine par `zukanName.includes(dbFirst)`. Si `name_en`
/// est vide, `dbFirst` vaut la chaîne vide et l'inclusion est **toujours
/// vraie** : une ligne sans nom anglais ne peut pas être signalée en
/// `NAME_MISMATCH`. C'est le comportement du TS, reproduit et testé.
#[must_use]
pub fn evaluer_ligne(
    db: &LigneInagle,
    zukan_par_hash: &HashMap<String, Vec<EntreeZukan>>,
) -> Option<AnomalieAudit> {
    let entrees = db
        .zukan_hash
        .as_ref()
        .filter(|h| !h.is_empty())
        .and_then(|h| zukan_par_hash.get(h));

    let Some(z) = entrees.and_then(|v| v.first()) else {
        return Some(AnomalieAudit {
            type_anomalie: TypeAnomalie::HashPerime,
            db: db.clone(),
            zukan: None,
            raisons: vec!["STALE: hash not in current zukan data".to_string()],
            correlation_stats: None,
        });
    };

    let db_nom_en = db.name_en.to_lowercase().trim().to_string();
    let nom_zukan = z.nom.to_lowercase().trim().to_string();
    let db_nom_fr = db
        .name_fr
        .as_deref()
        .unwrap_or_default()
        .to_lowercase()
        .trim()
        .to_string();
    let premier = |s: &str| s.split_whitespace().next().unwrap_or_default().to_string();
    let zukan_premier = premier(&nom_zukan);
    let db_premier = premier(&db_nom_en);
    let db_premier_fr = premier(&db_nom_fr);

    let est_miximax = db_nom_en.contains('×') || db_nom_en.contains('+');
    let db_nom_base = est_miximax.then(|| {
        let avant = db_nom_en
            .split(['×', '+'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        premier(&avant)
    });

    let nom_concorde = nom_zukan == db_nom_en
        || nom_zukan == db_nom_fr
        || zukan_premier == db_premier
        || zukan_premier == db_premier_fr
        || db_nom_base.as_ref().is_some_and(|b| zukan_premier == *b)
        || nom_zukan.contains(&db_premier)
        || db_nom_en.contains(&zukan_premier);

    let poste_diverge = match (
        audit_norm_pos(z.position.as_deref()),
        audit_norm_pos(Some(db.position.as_str())),
    ) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    let element_diverge = !est_miximax
        && match (
            audit_norm_elem(z.element.as_deref()),
            audit_norm_elem(Some(db.element.as_str())),
        ) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        };
    // L'audit n'accepte que `Male`/`Female` — pas les formes japonaises du matcher.
    let z_genre = match z.genre.as_deref() {
        Some("Male") => Some("M"),
        Some("Female") => Some("F"),
        _ => None,
    };
    let genre_diverge = match (z_genre, db.gender.as_deref().filter(|s| !s.is_empty())) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };

    let est_heros_ares = db.rarity_label == "Héros" && db.series.as_deref() == Some("Ares");
    let z_serie = audit_zukan_jeu_vers_serie(z.jeu.as_deref());
    let ere_db = db.series.as_deref().filter(|s| !s.is_empty()).and_then(ere);
    let ere_z = z_serie.as_deref().and_then(ere);
    let ere_diverge = !est_heros_ares
        && match (ere_db, ere_z) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        };

    let mut correlation_stats = None;
    if let (Some(zs), Some(ds)) = (z.stats, db.stats) {
        let z_arr = zs.en_tableau();
        let d_arr = ds.en_tableau();
        let z_total: f64 = z_arr.iter().sum();
        let d_total: f64 = d_arr.iter().sum();
        if z_total > 0.0 && d_total > 0.0 {
            correlation_stats = Some(correlation_spearman(&z_arr, &d_arr));
        }
    }

    let mut raisons: Vec<String> = Vec::new();
    if !nom_concorde {
        raisons.push(format!("NAME: zukan=\"{}\" ≠ db=\"{}\"", z.nom, db.name_en));
    }
    if poste_diverge {
        raisons.push(format!(
            "POS: zukan={} ≠ db={}",
            z.position.as_deref().unwrap_or_default(),
            db.position
        ));
    }
    if element_diverge {
        raisons.push(format!(
            "ELEM: zukan={} ≠ db={}",
            z.element.as_deref().unwrap_or_default(),
            db.element
        ));
    }
    if genre_diverge {
        raisons.push(format!(
            "GENDER: zukan={} ≠ db={}",
            z.genre.as_deref().unwrap_or_default(),
            db.gender.as_deref().unwrap_or_default()
        ));
    }
    if ere_diverge {
        raisons.push(format!(
            "ERA: zukan={}({}) ≠ db={}({})",
            z_serie.as_deref().unwrap_or_default(),
            ere_z.unwrap_or_default(),
            db.series.as_deref().unwrap_or_default(),
            ere_db.unwrap_or_default()
        ));
    }
    if let Some(c) = correlation_stats
        && c < 0.3
    {
        raisons.push(format!("STATS: correlation={c:.2} (very low)"));
    }

    if raisons.is_empty() {
        return None;
    }

    let type_anomalie = if raisons.iter().any(|r| r.starts_with("NAME")) {
        TypeAnomalie::NomDivergent
    } else if raisons.iter().any(|r| r.starts_with("GENDER")) {
        TypeAnomalie::GenreDivergent
    } else {
        TypeAnomalie::AttributDivergent
    };

    Some(AnomalieAudit {
        type_anomalie,
        db: db.clone(),
        zukan: Some(z.clone()),
        raisons,
        correlation_stats,
    })
}

/// Indexe toutes les entrées zukan par empreinte (`audit.ts:295-303`).
#[must_use]
pub fn indexer_zukan_par_hash(entrees: &[EntreeZukan]) -> HashMap<String, Vec<EntreeZukan>> {
    let mut map: HashMap<String, Vec<EntreeZukan>> = HashMap::new();
    for z in entrees {
        if let Some(h) = z.zukan_hash.as_ref().filter(|h| !h.is_empty()) {
            map.entry(h.clone()).or_default().push(z.clone());
        }
    }
    map
}

/// Indexe la **première** entrée rencontrée par empreinte (`audit.ts:306-312`).
#[must_use]
pub fn indexer_zukan_premier_par_hash(entrees: &[EntreeZukan]) -> HashMap<String, EntreeZukan> {
    let mut map: HashMap<String, EntreeZukan> = HashMap::new();
    for z in entrees {
        if let Some(h) = z.zukan_hash.as_ref().filter(|h| !h.is_empty()) {
            map.entry(h.clone()).or_insert_with(|| z.clone());
        }
    }
    map
}

/// Recense les noms partageant une même empreinte (`audit.ts:315-324`).
///
/// Une empreinte associée à plus d'un nom trahit un appariement 1:1 rompu. Le
/// nom retenu est `name_en`, sinon `name_ja`, sinon la chaîne vide — chaîne
/// vide comprise, comme le `||` de JavaScript.
#[must_use]
pub fn detecter_hashes_dupliques(lignes: &[LigneInagle]) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    for r in lignes {
        let Some(h) = r.zukan_hash.as_ref().filter(|h| !h.is_empty()) else {
            continue;
        };
        let nom = if r.name_en.is_empty() {
            r.name_ja.clone().unwrap_or_default()
        } else {
            r.name_en.clone()
        };
        map.entry(h.clone()).or_default().insert(nom);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(v: [f64; 7]) -> StatsAppariement {
        StatsAppariement {
            kick: v[0],
            control: v[1],
            technique: v[2],
            pressure: v[3],
            physical: v[4],
            agility: v[5],
            intelligence: v[6],
        }
    }

    fn zukan(nom: &str) -> EntreeZukan {
        EntreeZukan {
            nom: nom.to_string(),
            zukan_hash: Some(format!("h_{nom}")),
            ..EntreeZukan::default()
        }
    }

    fn ligne(id: &str) -> LigneInagle {
        LigneInagle {
            id: id.to_string(),
            name_en: "Mark".to_string(),
            position: "GK".to_string(),
            element: "Wind".to_string(),
            rarity_label: "Normal".to_string(),
            ..LigneInagle::default()
        }
    }

    // ── Spearman ────────────────────────────────────────────────────────────

    #[test]
    fn spearman_series_identiques_vaut_un() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        assert!((correlation_spearman(&a, &a) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn spearman_series_inversees_vaut_moins_un() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let b = [7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        assert!((correlation_spearman(&a, &b) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn spearman_ex_aequo_recoivent_le_rang_moyen() {
        // 3 valeurs égales → rangs 1, 2, 3 → rang moyen 2 partout, donc rho = 1.
        let a = [5.0, 5.0, 5.0];
        let b = [1.0, 2.0, 3.0];
        // ra = [2,2,2] ; rb = [1,2,3] ; sumD2 = 1 + 0 + 1 = 2
        // rho = 1 - 6*2 / (3 * 8) = 1 - 0.5 = 0.5
        assert!((correlation_spearman(&a, &b) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn spearman_moins_de_deux_valeurs_vaut_zero() {
        assert_eq!(correlation_spearman(&[1.0], &[1.0]), 0.0);
        assert_eq!(correlation_spearman(&[], &[]), 0.0);
    }

    #[test]
    fn spearman_longueurs_differentes_vaut_zero() {
        assert_eq!(correlation_spearman(&[1.0, 2.0], &[1.0, 2.0, 3.0]), 0.0);
    }

    // ── Normalisation ───────────────────────────────────────────────────────

    #[test]
    fn normalisation_du_matcher_couvre_le_japonais_et_aucun() {
        assert_eq!(norm_elem(Some("火")).as_deref(), Some("Fire"));
        assert_eq!(norm_elem(Some("山")).as_deref(), Some("Mountain"));
        assert_eq!(norm_elem(Some("無")).as_deref(), Some("Void"));
        assert_eq!(norm_elem(Some("Aucun")).as_deref(), Some("Void"));
        assert_eq!(norm_pos(Some("Entraîneur")).as_deref(), Some("Coach"));
    }

    #[test]
    fn valeur_inconnue_est_rendue_telle_quelle() {
        assert_eq!(norm_pos(Some("Libéro")).as_deref(), Some("Libéro"));
        assert_eq!(norm_elem(Some("Lumière")).as_deref(), Some("Lumière"));
        assert_eq!(norm_pos(None), None);
        assert_eq!(norm_pos(Some("")), None);
    }

    /// Fige les quatre divergences volontaires entre matcher et audit.
    #[test]
    fn tables_du_matcher_et_de_l_audit_divergent_comme_documente() {
        // 1. `Defenseur` sans cédille : connu de l'audit seul.
        assert_eq!(audit_norm_pos(Some("Defenseur")).as_deref(), Some("DF"));
        assert_eq!(norm_pos(Some("Defenseur")).as_deref(), Some("Defenseur"));
        // 2. `Entraîneur` : connu du matcher seul.
        assert_eq!(
            audit_norm_pos(Some("Entraîneur")).as_deref(),
            Some("Entraîneur")
        );
        // 3. Japonais et `Aucun` : matcher seul.
        assert_eq!(audit_norm_elem(Some("火")).as_deref(), Some("火"));
        assert_eq!(audit_norm_elem(Some("Aucun")).as_deref(), Some("Aucun"));
        // 4. `Orion` : le matcher le traduit, l'audit non.
        assert_eq!(
            zukan_jeu_vers_serie(Some("Inazuma Eleven Orion")).as_deref(),
            Some("Orion")
        );
        assert_eq!(
            audit_zukan_jeu_vers_serie(Some("Inazuma Eleven Orion")),
            None
        );
    }

    #[test]
    fn les_prefixes_de_jeu_sont_testes_du_plus_precis_au_plus_general() {
        assert_eq!(
            zukan_jeu_vers_serie(Some("Inazuma Eleven GO Galaxy: Big Bang")).as_deref(),
            Some("Galaxy")
        );
        assert_eq!(
            zukan_jeu_vers_serie(Some("Inazuma Eleven GO Chrono Stones: Wildfire")).as_deref(),
            Some("Chrono Stone")
        );
        assert_eq!(
            zukan_jeu_vers_serie(Some("Inazuma Eleven GO: Shine")).as_deref(),
            Some("Inazuma Eleven GO")
        );
        assert_eq!(
            zukan_jeu_vers_serie(Some("Inazuma Eleven")).as_deref(),
            Some("Inazuma Eleven")
        );
        assert_eq!(zukan_jeu_vers_serie(Some("Autre chose")), None);
    }

    #[test]
    fn meme_ere_exige_deux_series_distinctes() {
        assert!(meme_ere("Inazuma Eleven", "Inazuma Eleven 2"));
        assert!(!meme_ere("Inazuma Eleven", "Inazuma Eleven"));
        assert!(!meme_ere("Inazuma Eleven", "Victory Road"));
        assert!(!meme_ere("Inconnue", "Inazuma Eleven"));
    }

    #[test]
    fn genres_japonais_reconnus_par_le_matcher() {
        assert_eq!(norm_genre(Some("男")).as_deref(), Some("M"));
        assert_eq!(norm_genre(Some("女")).as_deref(), Some("F"));
        assert_eq!(norm_genre(Some("Autre")), None);
    }

    // ── Similarité de description ───────────────────────────────────────────

    #[test]
    fn jaccard_ignore_les_mots_vides_et_les_mots_courts() {
        // « the » et « of » sont vides, « is » aussi ; « ai » fait 2 lettres.
        let s = similarite_description("The captain of the team", "captain team");
        assert!((s - 1.0).abs() < 1e-12, "obtenu {s}");
    }

    #[test]
    fn jaccard_supprime_les_caracteres_non_ascii_sans_les_remplacer() {
        // « café » perd son accent : il devient « caf », pas « caf e ».
        let s = similarite_description("café goalkeeper", "caf goalkeeper");
        assert!((s - 1.0).abs() < 1e-12, "obtenu {s}");
    }

    #[test]
    fn jaccard_vaut_zero_si_un_cote_ne_produit_aucun_mot() {
        assert_eq!(similarite_description("the of a", "captain"), 0.0);
        assert_eq!(similarite_description("", "captain"), 0.0);
    }

    // ── Score ───────────────────────────────────────────────────────────────

    #[test]
    fn poste_divergent_est_un_rejet_dur() {
        let mut z = zukan("Mark");
        z.position = Some("FW".to_string());
        let db = ligne("c1"); // position GK
        assert_eq!(score_appariement(&z, &db), -1);
    }

    #[test]
    fn element_divergent_est_un_rejet_dur() {
        let mut z = zukan("Mark");
        z.element = Some("Fire".to_string());
        let db = ligne("c1"); // element Wind
        assert_eq!(score_appariement(&z, &db), -1);
    }

    #[test]
    fn poste_et_element_concordants_valent_vingt() {
        let mut z = zukan("Mark");
        z.position = Some("Gardien".to_string());
        z.element = Some("Vent".to_string());
        assert_eq!(score_appariement(&z, &ligne("c1")), 20);
    }

    #[test]
    fn genre_concordant_ajoute_cinq_divergent_retire_dix() {
        let mut db = ligne("c1");
        db.gender = Some("M".to_string());
        let mut z = zukan("Mark");
        z.genre = Some("Male".to_string());
        assert_eq!(score_appariement(&z, &db), 5);
        z.genre = Some("Female".to_string());
        assert_eq!(score_appariement(&z, &db), -10);
    }

    #[test]
    fn barriere_d_ere_dure_pour_un_personnage_ordinaire() {
        let mut db = ligne("c1");
        db.series = Some("Victory Road".to_string()); // Modern
        let mut z = zukan("Mark");
        z.jeu = Some("Inazuma Eleven".to_string()); // OG
        assert_eq!(score_appariement(&z, &db), -1);
    }

    #[test]
    fn heros_franchit_la_barriere_et_privilegie_l_ere_d_origine() {
        let mut db = ligne("c1");
        db.rarity_label = "Héros".to_string();
        db.series = Some("Ares".to_string()); // Modern
        let mut z = zukan("Mark");
        z.jeu = Some("Inazuma Eleven".to_string()); // OG → ordre 1
        // Pas de rejet, et +60 pour l'ère d'origine.
        assert_eq!(score_appariement(&z, &db), 60);
    }

    #[test]
    fn meme_serie_vaut_cinquante_meme_ere_vingt() {
        let mut db = ligne("c1");
        db.series = Some("Inazuma Eleven".to_string());
        let mut z = zukan("Mark");
        z.jeu = Some("Inazuma Eleven".to_string());
        assert_eq!(score_appariement(&z, &db), 50);
        // Même ère, série différente.
        z.jeu = Some("Inazuma Eleven 2".to_string());
        assert_eq!(score_appariement(&z, &db), 20);
    }

    #[test]
    fn profil_de_stats_ajoute_jusqu_a_quinze() {
        let mut db = ligne("c1");
        let profil = [7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        db.stats = Some(stats(profil));
        let mut z = zukan("Mark");
        z.stats = Some(stats(profil));
        // rho = 1 → +15.
        assert_eq!(score_appariement(&z, &db), 15);
        // Profil inversé → rho = -1, ramené à 0 : aucun bonus, aucun malus.
        z.stats = Some(stats([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]));
        assert_eq!(score_appariement(&z, &db), 0);
    }

    #[test]
    fn description_sans_mot_commun_retire_dix() {
        let mut db = ligne("c1");
        db.description_en = Some("captain goalkeeper legend".to_string());
        let mut z = zukan("Mark");
        z.description = Some("striker forward speed".to_string());
        assert_eq!(score_appariement(&z, &db), -10);
    }

    // ── Assignation ─────────────────────────────────────────────────────────

    #[test]
    fn appariement_est_un_a_un_sur_tout_le_run() {
        let mut ctx = ContexteAppariement::nouveau();
        let mut assignes = HashMap::new();

        let mut z1 = zukan("Mark");
        z1.position = Some("GK".to_string());
        z1.element = Some("Wind".to_string());
        z1.jeu = Some("Inazuma Eleven".to_string());

        let mut db1 = ligne("c1");
        db1.series = Some("Inazuma Eleven".to_string());
        let mut db2 = ligne("c2");
        db2.series = Some("Inazuma Eleven".to_string());

        apparier_groupes_strict(&mut ctx, &[z1], &[db1, db2], &mut assignes);
        assert_eq!(assignes.len(), 1, "une seule empreinte pour deux lignes");
        assert_eq!(ctx.hashes_utilises.len(), 1);
    }

    #[test]
    fn appariement_sous_le_seuil_n_assigne_rien() {
        let mut ctx = ContexteAppariement::nouveau();
        let mut assignes = HashMap::new();
        // Aucun attribut renseigné → score 0 < 20.
        apparier_groupes_strict(&mut ctx, &[zukan("X")], &[ligne("c1")], &mut assignes);
        assert!(assignes.is_empty());
    }

    #[test]
    fn assigner_meilleur_prend_le_score_le_plus_haut() {
        let mut ctx = ContexteAppariement::nouveau();
        let mut assignes = HashMap::new();
        let mut db = ligne("c1");
        db.series = Some("Inazuma Eleven".to_string());

        let mut faible = zukan("faible");
        faible.position = Some("GK".to_string());
        faible.element = Some("Wind".to_string());
        let mut fort = zukan("fort");
        fort.position = Some("GK".to_string());
        fort.element = Some("Wind".to_string());
        fort.jeu = Some("Inazuma Eleven".to_string()); // +50

        assigner_meilleur(&mut ctx, &db, &[faible, fort], &mut assignes, SCORE_MINIMAL);
        assert_eq!(assignes.get("c1").map(String::as_str), Some("h_fort"));
    }

    #[test]
    fn assigner_meilleur_ne_retouche_pas_une_ligne_deja_assignee() {
        let mut ctx = ContexteAppariement::nouveau();
        let mut assignes = HashMap::new();
        assignes.insert("c1".to_string(), "deja".to_string());
        let mut z = zukan("Mark");
        z.position = Some("GK".to_string());
        z.element = Some("Wind".to_string());
        assigner_meilleur(&mut ctx, &ligne("c1"), &[z], &mut assignes, SCORE_MINIMAL);
        assert_eq!(assignes.get("c1").map(String::as_str), Some("deja"));
        assert!(ctx.hashes_utilises.is_empty());
    }

    // ── Audit ───────────────────────────────────────────────────────────────

    #[test]
    fn hash_absent_du_zukan_courant_est_perime() {
        let mut db = ligne("c1");
        db.zukan_hash = Some("inconnu".to_string());
        let a = evaluer_ligne(&db, &HashMap::new()).expect("anomalie attendue");
        assert_eq!(a.type_anomalie, TypeAnomalie::HashPerime);
        assert_eq!(a.type_anomalie.libelle_inagle(), "STALE_HASH");
        assert!(a.zukan.is_none());
    }

    #[test]
    fn ligne_coherente_ne_produit_aucune_anomalie() {
        let mut z = zukan("Mark");
        z.position = Some("GK".to_string());
        z.element = Some("Wind".to_string());
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1");
        db.zukan_hash = Some("h_Mark".to_string());
        assert!(evaluer_ligne(&db, &index).is_none());
    }

    #[test]
    fn nom_divergent_prime_sur_les_autres_raisons() {
        let mut z = zukan("Axel");
        z.position = Some("FW".to_string());
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1"); // name_en = Mark, position GK
        db.zukan_hash = Some("h_Axel".to_string());
        let a = evaluer_ligne(&db, &index).expect("anomalie attendue");
        assert_eq!(a.type_anomalie, TypeAnomalie::NomDivergent);
        assert_eq!(a.raisons.len(), 2, "nom + poste");
        assert!(a.raisons[0].starts_with("NAME"));
        assert!(a.raisons[1].starts_with("POS"));
    }

    #[test]
    fn element_ignore_pour_un_miximax() {
        let mut z = zukan("Mark");
        z.element = Some("Fire".to_string()); // db = Wind
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1");
        db.name_en = "Mark × Axel".to_string();
        db.zukan_hash = Some("h_Mark".to_string());
        // Le nom concorde par la base MixiMax, et l'élément est ignoré.
        assert!(evaluer_ligne(&db, &index).is_none());
    }

    #[test]
    fn heros_ares_echappe_a_la_divergence_d_ere() {
        let mut z = zukan("Mark");
        z.jeu = Some("Inazuma Eleven".to_string()); // OG
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1");
        db.zukan_hash = Some("h_Mark".to_string());
        db.series = Some("Ares".to_string()); // Modern
        db.rarity_label = "Héros".to_string();
        assert!(evaluer_ligne(&db, &index).is_none());
        // Sans le statut de Héros, l'ère devient une anomalie.
        db.rarity_label = "Normal".to_string();
        let a = evaluer_ligne(&db, &index).expect("anomalie d'ère attendue");
        assert_eq!(a.type_anomalie, TypeAnomalie::AttributDivergent);
        assert!(a.raisons[0].starts_with("ERA"));
    }

    #[test]
    fn correlation_basse_est_signalee_avec_deux_decimales() {
        let mut z = zukan("Mark");
        z.stats = Some(stats([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]));
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1");
        db.zukan_hash = Some("h_Mark".to_string());
        db.stats = Some(stats([7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]));
        let a = evaluer_ligne(&db, &index).expect("corrélation -1 attendue");
        assert_eq!(a.correlation_stats, Some(-1.0));
        assert!(
            a.raisons
                .iter()
                .any(|r| r == "STATS: correlation=-1.00 (very low)"),
            "{:?}",
            a.raisons
        );
    }

    /// Piège porté tel quel : `name_en` vide rend le test d'inclusion toujours
    /// vrai, donc aucune ligne sans nom anglais n'est signalée en `NAME`.
    #[test]
    fn nom_anglais_vide_ne_peut_pas_etre_signale() {
        let z = zukan("Personne");
        let index = indexer_zukan_par_hash(&[z]);
        let mut db = ligne("c1");
        db.name_en = String::new();
        db.zukan_hash = Some("h_Personne".to_string());
        db.position = String::new();
        db.element = String::new();
        assert!(evaluer_ligne(&db, &index).is_none());
    }

    #[test]
    fn index_premier_par_hash_garde_la_premiere_entree() {
        let mut a = zukan("A");
        a.zukan_hash = Some("h".to_string());
        let mut b = zukan("B");
        b.zukan_hash = Some("h".to_string());
        let tous = indexer_zukan_par_hash(&[a.clone(), b.clone()]);
        assert_eq!(tous["h"].len(), 2);
        let premier = indexer_zukan_premier_par_hash(&[a, b]);
        assert_eq!(premier["h"].nom, "A");
    }

    #[test]
    fn hash_partage_par_deux_noms_est_detecte() {
        let mut a = ligne("c1");
        a.zukan_hash = Some("h".to_string());
        a.name_en = "Mark".to_string();
        let mut b = ligne("c2");
        b.zukan_hash = Some("h".to_string());
        b.name_en = "Axel".to_string();
        let mut c = ligne("c3");
        c.zukan_hash = Some("autre".to_string());

        let doublons = detecter_hashes_dupliques(&[a, b, c]);
        assert_eq!(doublons["h"].len(), 2);
        assert_eq!(doublons["autre"].len(), 1);
    }

    #[test]
    fn nom_vide_retombe_sur_le_japonais() {
        let mut a = ligne("c1");
        a.zukan_hash = Some("h".to_string());
        a.name_en = String::new();
        a.name_ja = Some("円堂".to_string());
        let doublons = detecter_hashes_dupliques(&[a]);
        assert!(doublons["h"].contains("円堂"));
    }
}
