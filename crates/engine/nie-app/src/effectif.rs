//! L'effectif réel du jeu, chargé depuis le VFS pour l'écran « Composition d'équipe ».
//!
//! Jusqu'ici cet onglet affichait « Mode en cours d'intégration » alors que les données étaient
//! là et déjà parsées par `nie-data` : personnages (`chara_base`), leurs paramètres de match
//! (`chara_param`) et leurs noms localisés (`chara_text`). Ce module fait la jointure et rend une
//! liste affichable — c'est le premier onglet du menu principal à montrer du vrai contenu.
//!
//! Tout part du VFS, pas de fichiers pré-générés : les `cfg.bin` sont décodés en mémoire
//! (`cfgbin::to_iecode_json`), donc l'écran fonctionne sur une installation comme sur un dump.

use anyhow::{Context, Result};
use nie_data::chara_base::{self, CharaBase};
use nie_data::chara_param::{self, CharaParam};
use nie_data::chara_text;
use nie_formats::vfs::Vfs;

/// Un joueur tel que l'écran d'effectif l'affiche.
#[derive(Debug, Clone)]
pub struct Joueur {
    /// Nom affichable — « Prénom Nom » quand les deux sont résolus, sinon le code interne.
    pub nom: String,
    /// Code interne (`c01000010`), qui reste la seule identité stable.
    pub code: String,
    /// Poste principal, en toutes lettres.
    pub poste: &'static str,
    /// Élément, en toutes lettres.
    pub element: &'static str,
}

impl Joueur {
    /// Ligne d'affichage : `Nom — Poste · Élément`.
    #[must_use]
    pub fn ligne(&self) -> String {
        format!("{} — {} · {}", self.nom, self.poste, self.element)
    }
}

/// Poste principal en clair. Valeurs relevées dans `chara_param` (`mainPosition`).
fn poste(v: i64) -> &'static str {
    match v {
        1 => "Gardien",
        2 => "Attaquant",
        3 => "Milieu",
        4 => "Défenseur",
        _ => "Poste inconnu",
    }
}

/// Élément en clair. Valeurs relevées dans `chara_param` (`element`).
fn element(v: i64) -> &'static str {
    match v {
        1 => "Vent",
        2 => "Forêt",
        3 => "Feu",
        4 => "Montagne",
        _ => "Sans élément",
    }
}

/// Lit un `cfg.bin` du VFS et le rend sous la forme JSON qu'attend `nie-data`.
fn charger_json(vfs: &Vfs, chemin: &str) -> Result<serde_json::Value> {
    let octets = vfs
        .read(chemin)
        .map_err(|e| anyhow::anyhow!("{chemin} : {e:?}"))?;
    nie_formats::cfgbin::to_iecode_json(&octets).with_context(|| format!("décodage de {chemin}"))
}

/// Trouve le premier fichier du VFS sous `prefixe` dont le nom commence par `radical`.
///
/// Les tables du jeu portent leur version dans leur nom (`chara_param_1.03.66.00.cfg.bin`) :
/// la coder en dur rendrait l'écran vide à la première mise à jour du jeu.
fn resoudre(vfs: &Vfs, prefixe: &str, radical: &str) -> Option<String> {
    let mut candidats: Vec<&str> = vfs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| {
            p.starts_with(prefixe)
                && p.ends_with(".cfg.bin")
                && p.rsplit('/').next().is_some_and(|n| n.starts_with(radical))
        })
        .collect();
    // Ordre stable : deux exécutions doivent choisir le même fichier.
    candidats.sort_unstable();
    candidats.first().map(|s| (*s).to_string())
}

/// Charge les `max` premiers personnages jouables, noms résolus.
///
/// # Errors
///
/// Rend une erreur si les tables sont absentes du VFS ou illisibles. L'appelant décide quoi
/// afficher : un écran d'effectif vide vaut mieux qu'un plantage, mais il doit dire pourquoi.
pub fn charger(vfs: &Vfs, max: usize, langue: &str) -> Result<Vec<Joueur>> {
    const DOSSIER: &str = "data/common/gamedata/character/";
    let p_param = resoudre(vfs, DOSSIER, "chara_param").context("aucun chara_param dans le VFS")?;
    let p_base = resoudre(vfs, DOSSIER, "chara_base").context("aucun chara_base dans le VFS")?;
    let p_text = format!("data/common/text/{langue}/chara_text.cfg.bin");

    let params: Vec<CharaParam> =
        chara_param::parse_all_chara_params(&charger_json(vfs, &p_param)?);
    let bases: Vec<CharaBase> = chara_base::parse_all_chara_base(&charger_json(vfs, &p_base)?);
    // Les noms sont facultatifs : sans eux on affiche les codes internes plutôt que rien.
    let noms = charger_json(vfs, &p_text)
        .map(|v| chara_text::parse_all_nouns(&v))
        .unwrap_or_default();

    let mut out = Vec::with_capacity(max);
    for p in &params {
        let Some(base) = chara_base::find_by_chara_id(&bases, p.chara_base_id) else {
            continue; // paramètre orphelin : rien à nommer, on l'écarte plutôt que d'inventer.
        };
        let prenom = chara_base::resolve_first_name(base, &noms);
        let nom_famille = chara_base::resolve_last_name(base, &noms);
        let nom = match (prenom, nom_famille) {
            // Certains personnages n'ont pas de nom de famille : le champ répète alors le prénom,
            // et une concaténation naïve affiche « Destin Destin » ou « Raika Raika ». Constaté
            // sur six des vingt premiers personnages du jeu.
            (Some(p), Some(n)) if p.eq_ignore_ascii_case(n) => p.to_string(),
            (Some(p), Some(n)) => format!("{p} {n}"),
            (Some(p), None) => p.to_string(),
            (None, Some(n)) => n.to_string(),
            (None, None) => base.internal_code.clone(),
        };
        out.push(Joueur {
            nom,
            code: base.internal_code.clone(),
            poste: poste(p.main_position),
            element: element(p.element),
        });
        if out.len() >= max {
            break;
        }
    }
    anyhow::ensure!(
        !out.is_empty(),
        "aucun personnage jointable entre chara_param et chara_base"
    );
    Ok(out)
}

/// Vrai si `t` est une phrase réellement localisée en alphabet latin.
///
/// Le jeu livre ses tables de texte partiellement traduites : une même table mêle des entrées
/// françaises et des originaux japonais. Les afficher toutes remplirait un menu français de
/// caractères que la police latine ne sait pas dessiner.
///
/// Deux critères, chacun pour une raison : compter les **caractères** et non les octets (le
/// japonais en pèse trois par caractère, la longueur en octets le surestime), et exiger que les
/// lettres soient très majoritairement latines — ce qu'un texte japonais ne fait jamais.
#[must_use]
pub fn est_localise(t: &str) -> bool {
    let lettres: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    let latines = lettres.iter().filter(|c| c.is_ascii_alphabetic()).count();
    t.chars().count() >= 8 && lettres.len() >= 5 && latines * 10 >= lettres.len() * 9
}

/// Charge les entrées **localisées** d'une table de texte du jeu, pour les onglets dont le
/// contenu EST une liste de textes (aide, fichier de données…).
///
/// Les entrées non traduites sont écartées plutôt qu'affichées en japonais : une interface
/// française qui montre des glyphes manquants ne renseigne personne.
///
/// # Errors
///
/// Rend une erreur si la table est absente, illisible, ou n'a aucune entrée localisée — ce
/// dernier cas signifiant que l'onglet n'est pas traduit dans cette langue, et qu'il vaut mieux
/// garder son écran d'information.
pub fn charger_textes(vfs: &Vfs, fichier: &str, max: usize, langue: &str) -> Result<Vec<String>> {
    let chemin = format!("data/common/text/{langue}/{fichier}.cfg.bin");
    let lignes: Vec<String> = nie_data::text::parse_text_file(&charger_json(vfs, &chemin)?)
        .into_iter()
        .map(|(_, t)| t)
        // Une entrée multiligne tiendrait sur plusieurs lignes de menu : on garde la première
        // phrase, qui est le titre ou l'amorce, et la liste reste lisible.
        .map(|t| t.lines().next().unwrap_or_default().trim().to_string())
        .filter(|t| est_localise(t))
        .take(max)
        .collect();
    anyhow::ensure!(!lignes.is_empty(), "aucune entrée localisée dans {chemin}");
    Ok(lignes)
}

/// Charge les répliques d'une scène du mode Histoire.
///
/// `event_id` est l'identifiant d'événement du jeu (`ev02_01400`). Les répliques reviennent dans
/// l'ordre du fichier, déjà nettoyées de leur balisage par `nie-data`.
///
/// # Errors
///
/// Rend une erreur si l'événement est absent du VFS, illisible, ou vide.
pub fn charger_dialogue(vfs: &Vfs, event_id: &str, langue: &str) -> Result<Vec<String>> {
    let chemin = format!("data/common/text/{langue}/event/{event_id}.cfg.bin");
    let entrees = nie_data::text::parse_text_file(&charger_json(vfs, &chemin)?);
    let lignes: Vec<String> = entrees
        .into_iter()
        .map(|(_, t)| t)
        .filter(|t| !t.trim().is_empty())
        .collect();
    anyhow::ensure!(!lignes.is_empty(), "aucune réplique dans {chemin}");
    Ok(lignes)
}

/// Trouve un événement réellement traduit dans `langue`, pour ouvrir le mode Histoire sur du
/// contenu lisible plutôt que sur des marqueurs de test.
///
/// Le jeu compte près de 4 000 fichiers d'événement, dont beaucoup ne portent que des placeholders
/// japonais (« テスト１ ») : en choisir un au hasard donnerait un mode Histoire incompréhensible.
/// On scanne donc jusqu'à trouver une scène d'au moins `min_repliques` répliques localisées.
#[must_use]
pub fn premier_dialogue_traduit(vfs: &Vfs, langue: &str, min_repliques: usize) -> Option<String> {
    let prefixe = format!("data/common/text/{langue}/event/");
    let mut chemins: Vec<&str> = vfs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| p.starts_with(&prefixe) && p.ends_with(".cfg.bin"))
        .collect();
    // Ordre stable : la même scène d'une exécution à l'autre.
    chemins.sort_unstable();
    for chemin in chemins {
        let id = chemin
            .rsplit('/')
            .next()
            .and_then(|n| n.strip_suffix(".cfg.bin"))
            .unwrap_or_default();
        let Ok(lignes) = charger_dialogue(vfs, id, langue) else {
            continue;
        };
        // Une scène convient si l'ESSENTIEL de ses répliques est une vraie phrase localisée.
        //
        // Un critère trop lâche choisit mal : « au moins cinq répliques de plus de 25 octets
        // contenant une lettre latine » a retenu `ev00_00700`, dont la deuxième réplique est
        // « T ». Une réplique de dialogue doit en outre être plus longue qu'un libellé de menu,
        // d'où le seuil supplémentaire.
        let bonnes = lignes
            .iter()
            .filter(|t| est_localise(t) && t.chars().count() >= 20)
            .count();
        // La majorité de la scène doit tenir, pas seulement quelques lignes : une scène à moitié
        // japonaise s'afficherait en caractères manquants une réplique sur deux.
        if bonnes >= min_repliques && bonnes * 2 >= lignes.len() {
            return Some(id.to_string());
        }
    }
    None
}

/// Catégorie d'objet en français, pour l'affichage.
///
/// `ItemCategory::as_str` rend l'identifiant interne (`special_tactics`) : correct pour du code,
/// illisible dans un menu.
fn categorie(c: nie_data::item::ItemCategory) -> &'static str {
    use nie_data::item::ItemCategory as C;
    match c {
        C::Consume => "Consommable",
        C::Shoes => "Chaussures",
        C::Misanga => "Bracelet",
        C::Accessory => "Accessoire",
        C::Special => "Spécial",
        C::Formation => "Formation",
        C::SpecialTactics => "Tactique spéciale",
        C::SuperTactics => "Super-tactique",
        C::SpecialSkill => "Technique spéciale",
        C::Title => "Titre",
        C::Fashion => "Tenue",
        C::Costume => "Costume",
        C::Emblem => "Écusson",
        C::Unique => "Unique",
        C::CraftObj => "Objet d'artisanat",
        C::Animal => "Animal",
        C::KizunaLink => "Lien",
        C::NamePlate => "Plaque",
        C::Performance => "Performance",
        C::Important => "Objet important",
    }
}

/// Un objet du jeu, tel que l'écran « Objets » l'affiche.
#[derive(Debug, Clone)]
pub struct Objet {
    /// Nom localisé, ou le code interne si le texte manque.
    pub nom: String,
    /// Catégorie en français.
    pub categorie: &'static str,
    /// Prix d'achat, quand l'objet en a un.
    pub prix: Option<i64>,
}

impl Objet {
    /// Ligne d'affichage : `Nom — Catégorie` et le prix s'il existe.
    #[must_use]
    pub fn ligne(&self) -> String {
        match self.prix {
            Some(p) => format!("{} — {} · {p}", self.nom, self.categorie),
            None => format!("{} — {}", self.nom, self.categorie),
        }
    }
}

/// Charge les `max` premiers objets du jeu, noms résolus.
///
/// # Errors
///
/// Rend une erreur si la table d'objets est absente du VFS ou illisible.
pub fn charger_objets(vfs: &Vfs, max: usize, langue: &str) -> Result<Vec<Objet>> {
    const DOSSIER: &str = "data/common/gamedata/item/";
    let p_item = resoudre(vfs, DOSSIER, "item_config").context("aucun item_config dans le VFS")?;
    let items = nie_data::item::parse_all_items(&charger_json(vfs, &p_item)?);
    let textes = charger_json(vfs, &format!("data/common/text/{langue}/item_text.cfg.bin"))
        .map(|v| nie_data::text::parse_text_file(&v))
        .unwrap_or_default();

    let mut out = Vec::with_capacity(max);
    for it in &items {
        // Un objet sans nom résolu reste utile : son code interne l'identifie. Le taire
        // donnerait une liste trompeusement courte.
        let nom = nie_data::item::resolve_name(it, &textes)
            .map(str::to_owned)
            .or_else(|| it.internal_code.clone())
            .unwrap_or_else(|| format!("objet {:#010x}", it.item_id.0));
        // Certains libellés portent une VARIABLE que le jeu substitue à l'exécution (un nom
        // d'équipe) : le texte stocké est « Esprits « » », et sept objets s'affichent alors à
        // l'identique. Rien ne permet de les substituer ici — mais on peut au moins les
        // distinguer, en accolant leur code interne plutôt qu'en répétant sept fois la même ligne.
        let nom = if nom.contains("« »") || nom.contains("\"\"") || nom.contains("<>") {
            match &it.internal_code {
                Some(code) => format!("{nom} [{code}]"),
                None => format!("{nom} [{:#010x}]", it.item_id.0),
            }
        } else {
            nom
        };
        out.push(Objet {
            nom,
            categorie: categorie(it.category),
            prix: it.price,
        });
        if out.len() >= max {
            break;
        }
    }
    anyhow::ensure!(!out.is_empty(), "aucun objet dans {p_item}");
    Ok(out)
}
