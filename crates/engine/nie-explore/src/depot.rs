//! Accès confiné au **code du dépôt** : lecture, listing, recherche de fichiers et de contenu.
//!
//! ## Pourquoi ce module existe ici
//!
//! Trois façades doivent offrir la même chose au client — `niers find`/`grep` (CLI), le serveur
//! MCP `niers-game` (`repo_read`/`repo_list`/`repo_find`/`repo_grep`, via `nie-ffi`) et l'app
//! desktop `nie-explorer` (commandes Tauri). Chacune l'avait implémentée, ou pas du tout :
//!
//! - `nie-cli/src/search_cmd.rs` portait le moteur `ignore`/`grep-*`, mais mêlé à l'affichage
//!   texte (`find()` rendait un `usize` et imprimait) : inutilisable par un autre appelant.
//! - `apps/nie-mcp/src/repo.ts` réimplémentait la lecture confinée en TypeScript, sans
//!   listing ni recherche.
//! - `nie-explorer` n'avait rien.
//!
//! Le module rassemble la logique et **ne rend que des données** ; le formatage appartient aux
//! façades. C'est la règle déjà appliquée par [`crate::listing`] et le dispatch d'aperçu de
//! [`crate::lib`] : un seul moteur, plusieurs présentations, pour qu'elles ne dérivent pas.
//!
//! ## Confinement
//!
//! Tout chemin est résolu **puis** re-vérifié après résolution des liens symboliques : un lien
//! qui sort du dépôt est refusé. Le dépôt en contient de vrais (`var/mirror.sqlite` pointe
//! l'instantané daté), donc la vérification ne peut pas se contenter du chemin lexical.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::{WalkBuilder, WalkState};
use serde::{Deserialize, Serialize};

/// Dossiers de premier niveau exclus par défaut : ce ne sont pas du code, et ils pèsent
/// l'essentiel du dépôt (`data/` = 112 Go d'assets © LEVEL-5, `target/` et `node_modules/` sont
/// des artefacts, `var/` héberge des bases dont une de 15,5 Go, `.git/` l'historique compressé).
/// Les exposer ferait tomber n'importe quel client sur un fichier illisible ou gigantesque.
///
/// Tout le **code** reste atteignable : `crates/`, `apps/`, `packages/`, `src/`, `csharp/`,
/// `docs/`, `scripts/`, `plugins/`, `supabase/`, et les manifestes de la racine.
pub const DOSSIERS_EXCLUS: &[&str] = &["refs", "data", "var", ".git", "target", "node_modules"];

/// Fichiers refusés quel que soit leur emplacement : ils portent des secrets, pas du code.
///
/// Sans cette barrière, `lire(".env.local")` rendait les 6 727 octets de secrets du dépôt —
/// acceptable pour un outil local, inadmissible dès que la même fonction sert un client distant
/// (serveur MCP, et a fortiori une façade HTTP). Le filtre porte sur le **nom** de fichier, donc
/// il tient à n'importe quelle profondeur, y compris pour un `.env.local` d'un sous-projet.
///
/// Il complète, sans le remplacer, l'exclusion des fichiers cachés au parcours : `*.key` et
/// `*.pem` ne commencent pas par un point, et un appelant peut demander `caches: true`.
fn fichier_sensible(nom: &str) -> bool {
    let bas = nom.to_ascii_lowercase();
    if bas.starts_with(".env") {
        return true;
    }
    const SUFFIXES: &[&str] = &[".key", ".pem", ".p12", ".pfx", ".keystore", ".jks", ".asc"];
    if SUFFIXES.iter().any(|s| bas.ends_with(s)) {
        return true;
    }
    const NOMS: &[&str] = &[
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        ".npmrc",
        ".netrc",
        ".pgpass",
        "credentials",
        "secrets.json",
        ".htpasswd",
    ];
    NOMS.contains(&bas.as_str())
}

/// Taille de lecture par défaut quand l'appelant n'en impose pas (256 Kio).
pub const OCTETS_DEFAUT: u64 = 256 * 1024;

/// Plafond absolu de lecture (8 Mio) : au-delà, le contenu n'est pas renvoyé du tout.
pub const OCTETS_PLAFOND: u64 = 8 * 1024 * 1024;

/// Racine de dépôt validée, prête à servir de base à toute résolution de chemin.
///
/// Construite par [`Depot::ouvrir`], qui résout les liens symboliques de la racine une fois
/// pour toutes : les comparaisons de confinement portent ensuite sur des chemins réels.
#[derive(Debug, Clone)]
pub struct Depot {
    racine: PathBuf,
    exclus: BTreeSet<String>,
}

/// Un fichier du dépôt, lu ou décrit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FichierDepot {
    /// Chemin relatif à la racine du dépôt, séparateurs `/`.
    pub chemin: String,
    /// Chemin absolu réel (liens symboliques résolus).
    pub chemin_absolu: String,
    /// Taille totale du fichier en octets.
    pub taille: u64,
    /// Vrai si le contenu renvoyé s'arrête avant la fin du fichier.
    pub tronque: bool,
    /// Vrai si le fichier contient des octets nuls : le contenu n'est alors pas renvoyé.
    pub binaire: bool,
    /// Contenu textuel, absent pour un fichier binaire ou au-delà du plafond.
    pub contenu: Option<String>,
    /// Explication lisible quand le contenu manque.
    pub note: Option<String>,
}

/// Une entrée de dossier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntreeDepot {
    /// Chemin relatif à la racine du dépôt, séparateurs `/`.
    pub chemin: String,
    /// Nom seul de l'entrée.
    pub nom: String,
    /// Vrai pour un dossier.
    pub dossier: bool,
    /// Taille en octets (0 pour un dossier).
    pub taille: u64,
}

/// Une ligne trouvée par [`Depot::chercher`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correspondance {
    /// Chemin relatif à la racine du dépôt, séparateurs `/`.
    pub chemin: String,
    /// Numéro de ligne, à partir de 1.
    pub ligne: u64,
    /// Texte de la ligne, sans le saut de ligne final.
    pub texte: String,
}

/// Options communes au parcours de l'arbre, pour [`Depot::trouver`] et [`Depot::chercher`].
#[derive(Debug, Clone)]
pub struct OptionsParcours {
    /// Sous-dossier de départ, relatif à la racine (vide = tout le dépôt).
    pub sous_dossier: String,
    /// Motifs glob (`**/*.rs`), cumulatifs avec [`Self::extensions`].
    pub globs: Vec<String>,
    /// Extensions (`rs`, `.toml`) — sucre pour `**/*.<ext>`.
    pub extensions: Vec<String>,
    /// Inclure les fichiers cachés.
    pub caches: bool,
    /// Ignorer les règles `.gitignore`.
    pub sans_ignore: bool,
    /// Profondeur maximale de descente.
    pub profondeur: Option<usize>,
    /// Nombre maximal de résultats (0 = illimité).
    pub limite: usize,
    /// Recherche sensible à la casse.
    pub sensible_casse: bool,
}

impl Default for OptionsParcours {
    fn default() -> Self {
        Self {
            sous_dossier: String::new(),
            globs: Vec::new(),
            extensions: Vec::new(),
            caches: false,
            sans_ignore: false,
            profondeur: None,
            limite: 200,
            sensible_casse: false,
        }
    }
}

impl Depot {
    /// Ouvre un dépôt à `racine`, en résolvant ses liens symboliques.
    ///
    /// # Erreurs
    /// Si la racine n'existe pas ou n'est pas un dossier.
    pub fn ouvrir(racine: impl AsRef<Path>) -> Result<Self> {
        let racine = racine.as_ref();
        let reelle = std::fs::canonicalize(racine)
            .with_context(|| format!("racine du dépôt introuvable : {}", racine.display()))?;
        if !reelle.is_dir() {
            bail!(
                "la racine du dépôt n'est pas un dossier : {}",
                reelle.display()
            );
        }
        Ok(Self {
            racine: reelle,
            exclus: DOSSIERS_EXCLUS.iter().map(|s| (*s).to_string()).collect(),
        })
    }

    /// Racine réelle du dépôt.
    #[must_use]
    pub fn racine(&self) -> &Path {
        &self.racine
    }

    /// Remplace la liste des dossiers de premier niveau exclus.
    ///
    /// Sert aux appelants qui savent ce qu'ils font (un export complet, par exemple) ; la
    /// valeur par défaut est [`DOSSIERS_EXCLUS`].
    #[must_use]
    pub fn avec_exclusions(mut self, exclus: &[String]) -> Self {
        self.exclus = exclus.iter().cloned().collect();
        self
    }

    /// Résout un chemin relatif (ou absolu sous la racine) en chemin réel confiné.
    ///
    /// Refuse la traversée (`..`), les dossiers exclus, et tout lien symbolique qui sortirait
    /// du dépôt — la vérification est refaite **après** `canonicalize`, car un chemin lexical
    /// valide peut pointer ailleurs.
    ///
    /// # Erreurs
    /// Si le chemin est vide, sort du dépôt, vise un dossier exclu, ou n'existe pas.
    pub fn resoudre(&self, chemin: &str) -> Result<PathBuf> {
        let brut = chemin.trim();
        if brut.is_empty() {
            bail!("chemin vide");
        }
        if brut.contains('\0') {
            bail!("chemin invalide");
        }

        let demande = Path::new(brut);
        let absolu = if demande.is_absolute() {
            demande.to_path_buf()
        } else {
            self.racine.join(demande)
        };

        // Normalisation lexicale : `..` est résolu ici pour être refusé avant tout accès disque.
        let mut normalise = PathBuf::new();
        for part in absolu.components() {
            match part {
                Component::ParentDir => {
                    if !normalise.pop() {
                        bail!("chemin hors du dépôt : {brut}");
                    }
                }
                Component::CurDir => {}
                autre => normalise.push(autre.as_os_str()),
            }
        }
        if normalise != self.racine && !normalise.starts_with(&self.racine) {
            bail!("chemin hors du dépôt : {brut}");
        }

        self.verifier_exclusion(&normalise)?;

        let reel = std::fs::canonicalize(&normalise)
            .with_context(|| format!("chemin introuvable : {brut}"))?;
        // Un lien symbolique peut viser l'extérieur : on revérifie sur le chemin réel.
        if reel != self.racine && !reel.starts_with(&self.racine) {
            bail!("le lien symbolique sort du dépôt : {brut}");
        }
        self.verifier_exclusion(&reel)?;
        Ok(reel)
    }

    /// Refuse un chemin dont le premier segment est un dossier exclu, ou dont un segment
    /// quelconque est un fichier de secrets.
    ///
    /// C'est le point de contrôle unique : [`Self::resoudre`] l'appelle avant **et** après
    /// `canonicalize`, et [`Self::lister`] s'en sert pour masquer les entrées.
    fn verifier_exclusion(&self, absolu: &Path) -> Result<()> {
        let Ok(rel) = absolu.strip_prefix(&self.racine) else {
            return Ok(());
        };
        if let Some(premier) = rel.components().next() {
            let nom = premier.as_os_str().to_string_lossy().to_string();
            if self.exclus.contains(&nom) {
                bail!(
                    "dossier interdit '{nom}/' (exclus : {})",
                    self.exclus.iter().cloned().collect::<Vec<_>>().join(", ")
                );
            }
        }
        for part in rel.components() {
            let nom = part.as_os_str().to_string_lossy();
            if fichier_sensible(&nom) {
                bail!("fichier de secrets refusé : '{nom}'");
            }
        }
        Ok(())
    }

    /// Chemin relatif à la racine, séparateurs `/` — la forme rendue à tous les clients.
    fn relatif(&self, absolu: &Path) -> String {
        absolu
            .strip_prefix(&self.racine)
            .unwrap_or(absolu)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Lit un fichier texte du dépôt, tronqué à `max_octets` (défaut [`OCTETS_DEFAUT`]).
    ///
    /// Un fichier binaire est décrit mais son contenu n'est pas renvoyé ; au-delà de
    /// [`OCTETS_PLAFOND`] il n'est même pas ouvert.
    ///
    /// # Erreurs
    /// Si le chemin est refusé par [`Self::resoudre`], ou vise un dossier.
    pub fn lire(&self, chemin: &str, max_octets: Option<u64>) -> Result<FichierDepot> {
        let reel = self.resoudre(chemin)?;
        let meta =
            std::fs::metadata(&reel).with_context(|| format!("lecture impossible : {chemin}"))?;
        if meta.is_dir() {
            bail!("'{chemin}' est un dossier — utiliser lister()");
        }
        let taille = meta.len();
        let rel = self.relatif(&reel);
        let absolu = reel.to_string_lossy().to_string();

        if taille > OCTETS_PLAFOND {
            return Ok(FichierDepot {
                chemin: rel,
                chemin_absolu: absolu,
                taille,
                tronque: true,
                binaire: false,
                contenu: None,
                note: Some(format!(
                    "fichier de {taille} octets au-delà du plafond de {OCTETS_PLAFOND} — non lu"
                )),
            });
        }

        let max = max_octets
            .filter(|v| *v > 0)
            .unwrap_or(OCTETS_DEFAUT)
            .min(OCTETS_PLAFOND);
        let a_lire = taille.min(max);
        let mut tampon = vec![0_u8; usize::try_from(a_lire).unwrap_or(usize::MAX)];
        let mut f = std::fs::File::open(&reel)
            .with_context(|| format!("ouverture impossible : {chemin}"))?;
        let lus = f
            .read(&mut tampon)
            .with_context(|| format!("lecture impossible : {chemin}"))?;
        tampon.truncate(lus);

        // Détection binaire : un octet nul dans les 8 premiers Kio suffit (même règle que la
        // version TypeScript qu'on remplace, et que `grep_searcher`).
        let binaire = tampon.iter().take(8192).any(|b| *b == 0);
        if binaire {
            return Ok(FichierDepot {
                chemin: rel,
                chemin_absolu: absolu,
                taille,
                tronque: taille > a_lire,
                binaire: true,
                contenu: None,
                note: Some("fichier binaire — contenu non renvoyé".to_string()),
            });
        }

        Ok(FichierDepot {
            chemin: rel,
            chemin_absolu: absolu,
            taille,
            tronque: taille > a_lire,
            binaire: false,
            contenu: Some(String::from_utf8_lossy(&tampon).into_owned()),
            note: None,
        })
    }

    /// Liste les entrées immédiates d'un dossier, dossiers d'abord puis ordre alphabétique.
    ///
    /// Les dossiers exclus ([`DOSSIERS_EXCLUS`]) n'apparaissent pas à la racine.
    ///
    /// # Erreurs
    /// Si le chemin est refusé par [`Self::resoudre`], ou ne vise pas un dossier.
    pub fn lister(&self, chemin: &str) -> Result<Vec<EntreeDepot>> {
        let reel = if chemin.trim().is_empty() {
            self.racine.clone()
        } else {
            self.resoudre(chemin)?
        };
        if !reel.is_dir() {
            bail!("'{chemin}' n'est pas un dossier");
        }

        let mut sortie = Vec::new();
        for entree in
            std::fs::read_dir(&reel).with_context(|| format!("listing impossible : {chemin}"))?
        {
            let Ok(e) = entree else { continue };
            let p = e.path();
            if self.verifier_exclusion(&p).is_err() {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            let dossier = meta.is_dir();
            sortie.push(EntreeDepot {
                chemin: self.relatif(&p),
                nom: e.file_name().to_string_lossy().to_string(),
                dossier,
                taille: if dossier { 0 } else { meta.len() },
            });
        }
        sortie.sort_by(|a, b| b.dossier.cmp(&a.dossier).then_with(|| a.nom.cmp(&b.nom)));
        Ok(sortie)
    }

    /// Construit le parcours `ignore` correspondant aux options.
    ///
    /// # Erreurs
    /// Si le sous-dossier de départ est refusé par [`Self::resoudre`].
    fn parcours(&self, o: &OptionsParcours) -> Result<WalkBuilder> {
        let depart = if o.sous_dossier.trim().is_empty() {
            self.racine.clone()
        } else {
            self.resoudre(&o.sous_dossier)?
        };
        let mut w = parcours_disque(&depart, o.caches, o.sans_ignore, o.profondeur);

        // Les dossiers exclus sont coupés à la source : sans ce filtre, un parcours sans
        // `.gitignore` (`sans_ignore`) descendrait dans les 112 Go de `data/`.
        let racine = self.racine.clone();
        let exclus = self.exclus.clone();
        w.filter_entry(move |e| {
            // Un fichier de secrets est écarté partout, pas seulement à la racine : `trouver` et
            // `chercher` ne doivent pas en révéler le contenu ni même l'existence.
            if let Some(nom) = e.path().file_name()
                && fichier_sensible(&nom.to_string_lossy())
            {
                return false;
            }
            let Ok(rel) = e.path().strip_prefix(&racine) else {
                return true;
            };
            match rel.components().next() {
                Some(premier) => {
                    !exclus.contains(&premier.as_os_str().to_string_lossy().to_string())
                }
                None => true,
            }
        });
        Ok(w)
    }

    /// Cherche des fichiers par sous-chaîne de chemin.
    ///
    /// `motif` vide liste tout ce que les filtres laissent passer.
    ///
    /// # Erreurs
    /// Si un glob est invalide, ou le sous-dossier de départ refusé.
    pub fn trouver(&self, motif: &str, o: &OptionsParcours) -> Result<Vec<String>> {
        let set = construire_globs(&o.globs, &o.extensions)?;
        let aiguille = if o.sensible_casse {
            motif.to_string()
        } else {
            motif.to_lowercase()
        };

        let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let vus = Arc::new(AtomicUsize::new(0));
        let racine = self.racine.clone();

        self.parcours(o)?.build_parallel().run(|| {
            let hits = Arc::clone(&hits);
            let vus = Arc::clone(&vus);
            let aiguille = aiguille.clone();
            let set = set.clone();
            let racine = racine.clone();
            let limite = o.limite;
            let sensible = o.sensible_casse;
            Box::new(move |entree| {
                let Ok(e) = entree else {
                    return WalkState::Continue;
                };
                if !e.file_type().is_some_and(|t| t.is_file()) {
                    return WalkState::Continue;
                }
                let chemin = e.path();
                if let Some(s) = &set
                    && !s.is_match(chemin)
                {
                    return WalkState::Continue;
                }
                let rel = chemin
                    .strip_prefix(&racine)
                    .unwrap_or(chemin)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !aiguille.is_empty() {
                    let foin = if sensible {
                        rel.clone()
                    } else {
                        rel.to_lowercase()
                    };
                    if !foin.contains(&aiguille) {
                        return WalkState::Continue;
                    }
                }
                if limite > 0 && vus.fetch_add(1, Ordering::Relaxed) >= limite {
                    return WalkState::Quit;
                }
                if let Ok(mut g) = hits.lock() {
                    g.push(rel);
                }
                WalkState::Continue
            })
        });

        let mut sortie = Arc::try_unwrap(hits)
            .map_err(|_| anyhow::anyhow!("références pendantes sur les résultats"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("verrou empoisonné"))?;
        sortie.sort_unstable();
        if o.limite > 0 {
            sortie.truncate(o.limite);
        }
        Ok(sortie)
    }

    /// Cherche une expression régulière dans le contenu des fichiers.
    ///
    /// Les fichiers binaires sont écartés par `grep_searcher` (détection sur octet nul).
    ///
    /// # Erreurs
    /// Si l'expression régulière ou un glob est invalide, ou le sous-dossier refusé.
    pub fn chercher(&self, motif: &str, o: &OptionsParcours) -> Result<Vec<Correspondance>> {
        if motif.trim().is_empty() {
            bail!("motif de recherche vide");
        }
        let set = construire_globs(&o.globs, &o.extensions)?;
        let matcher = grep_regex::RegexMatcherBuilder::new()
            .case_insensitive(!o.sensible_casse)
            .build(motif)
            .with_context(|| format!("expression régulière invalide : {motif}"))?;

        let hits: Arc<Mutex<Vec<Correspondance>>> = Arc::new(Mutex::new(Vec::new()));
        let vus = Arc::new(AtomicUsize::new(0));
        let racine = self.racine.clone();

        self.parcours(o)?.build_parallel().run(|| {
            let hits = Arc::clone(&hits);
            let vus = Arc::clone(&vus);
            let set = set.clone();
            let racine = racine.clone();
            let matcher: RegexMatcher = matcher.clone();
            let limite = o.limite;
            let mut chercheur = SearcherBuilder::new()
                .binary_detection(BinaryDetection::quit(0))
                .line_number(true)
                .build();
            Box::new(move |entree| {
                let Ok(e) = entree else {
                    return WalkState::Continue;
                };
                if !e.file_type().is_some_and(|t| t.is_file()) {
                    return WalkState::Continue;
                }
                let chemin = e.path();
                if let Some(s) = &set
                    && !s.is_match(chemin)
                {
                    return WalkState::Continue;
                }
                if limite > 0 && vus.load(Ordering::Relaxed) >= limite {
                    return WalkState::Quit;
                }
                let rel = chemin
                    .strip_prefix(&racine)
                    .unwrap_or(chemin)
                    .to_string_lossy()
                    .replace('\\', "/");
                let hits = Arc::clone(&hits);
                let vus = Arc::clone(&vus);
                let _ = chercheur.search_path(
                    &matcher,
                    chemin,
                    UTF8(|ligne, texte| {
                        if limite > 0 && vus.fetch_add(1, Ordering::Relaxed) >= limite {
                            return Ok(false);
                        }
                        if let Ok(mut g) = hits.lock() {
                            g.push(Correspondance {
                                chemin: rel.clone(),
                                ligne,
                                texte: texte.trim_end_matches(['\r', '\n']).to_string(),
                            });
                        }
                        Ok(true)
                    }),
                );
                WalkState::Continue
            })
        });

        let mut sortie = Arc::try_unwrap(hits)
            .map_err(|_| anyhow::anyhow!("références pendantes sur les résultats"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("verrou empoisonné"))?;
        sortie.sort_by(|a, b| a.chemin.cmp(&b.chemin).then_with(|| a.ligne.cmp(&b.ligne)));
        if o.limite > 0 {
            sortie.truncate(o.limite);
        }
        Ok(sortie)
    }
}

/// Prépare un parcours `ignore` **sans confinement** : racine libre, fichiers cachés,
/// respect de `.gitignore`, profondeur.
///
/// Public parce que `niers find`/`niers grep` cherchent sur tout le disque (dumps, scratchpad,
/// arbres hors dépôt) et ne peuvent donc pas passer par [`Depot`], qui confine par construction.
/// Ils partagent malgré tout ce parcours et [`construire_globs`], au lieu d'en garder une copie.
///
/// `caches = true` **inclut** les fichiers cachés — l'inverse du drapeau `hidden` d'`ignore`,
/// dont la sémantique se lit à l'envers et s'est déjà payée.
#[must_use]
pub fn parcours_disque(
    dir: &Path,
    caches: bool,
    sans_ignore: bool,
    profondeur: Option<usize>,
) -> WalkBuilder {
    let mut w = WalkBuilder::new(dir);
    w.hidden(!caches)
        .git_ignore(!sans_ignore)
        .git_global(!sans_ignore)
        .git_exclude(!sans_ignore)
        .ignore(!sans_ignore)
        .parents(!sans_ignore)
        .max_depth(profondeur);
    w
}

/// Construit l'ensemble de globs depuis les motifs et les extensions.
///
/// # Erreurs
/// Si un motif glob ou une extension est syntaxiquement invalide.
pub fn construire_globs(globs: &[String], exts: &[String]) -> Result<Option<GlobSet>> {
    if globs.is_empty() && exts.is_empty() {
        return Ok(None);
    }
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(Glob::new(g).with_context(|| format!("glob invalide : {g}"))?);
    }
    for e in exts {
        let e = e.trim_start_matches('.');
        b.add(
            Glob::new(&format!("**/*.{e}")).with_context(|| format!("extension invalide : {e}"))?,
        );
    }
    Ok(Some(b.build().context("construction du GlobSet")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Racine du dépôt déduite du crate : `<dépôt>/crates/engine/nie-explore` -> `<dépôt>`.
    fn depot() -> Depot {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let racine = crate_dir.join("..").join("..").join("..");
        Depot::ouvrir(racine).expect("racine du dépôt")
    }

    #[test]
    fn lit_un_fichier_du_depot() {
        let d = depot();
        let f = d
            .lire("crates/engine/nie-explore/Cargo.toml", None)
            .expect("lecture");
        assert!(!f.binaire, "un Cargo.toml n'est pas binaire");
        assert!(f.contenu.expect("contenu").contains("nie-explore"));
        assert_eq!(f.chemin, "crates/engine/nie-explore/Cargo.toml");
    }

    #[test]
    fn refuse_la_traversee() {
        let d = depot();
        assert!(d.lire("../../../etc/passwd", None).is_err());
        assert!(d.resoudre("..").is_err() || !d.resoudre("..").expect("").starts_with(d.racine()));
    }

    #[test]
    fn refuse_les_dossiers_exclus() {
        let d = depot();
        for interdit in ["data/anime/episodes.db", "target/debug", ".git/HEAD"] {
            assert!(
                d.resoudre(interdit).is_err(),
                "{interdit} aurait dû être refusé"
            );
        }
    }

    #[test]
    fn liste_la_racine_sans_les_exclus() {
        let d = depot();
        let entrees = d.lister("").expect("listing");
        assert!(entrees.iter().any(|e| e.nom == "crates" && e.dossier));
        for exclu in DOSSIERS_EXCLUS {
            assert!(
                !entrees.iter().any(|e| e.nom == *exclu),
                "'{exclu}' ne doit pas apparaître dans le listing"
            );
        }
    }

    #[test]
    fn trouve_par_extension() {
        let d = depot();
        let o = OptionsParcours {
            sous_dossier: "crates/engine/nie-explore".to_string(),
            extensions: vec!["rs".to_string()],
            limite: 50,
            ..Default::default()
        };
        let hits = d.trouver("depot", &o).expect("recherche");
        assert!(
            hits.iter().any(|h| h.ends_with("nie-explore/src/depot.rs")),
            "le module courant doit se trouver lui-même : {hits:?}"
        );
    }

    #[test]
    fn cherche_dans_le_contenu() {
        let d = depot();
        let o = OptionsParcours {
            sous_dossier: "crates/engine/nie-explore/src".to_string(),
            extensions: vec!["rs".to_string()],
            limite: 20,
            ..Default::default()
        };
        let hits = d.chercher("DOSSIERS_EXCLUS", &o).expect("recherche");
        assert!(
            !hits.is_empty(),
            "la constante doit être trouvée dans ce module"
        );
        assert!(hits.iter().all(|c| c.ligne >= 1));
    }

    #[test]
    fn refuse_les_fichiers_de_secrets() {
        // Le dépôt porte un vrai `.env.local` : sans barrière, `lire` en rendait le contenu.
        let d = depot();
        for secret in [
            ".env.local",
            ".env",
            "apps/azalee/.env.local",
            "cle.pem",
            "id_rsa",
        ] {
            assert!(
                d.lire(secret, None).is_err(),
                "'{secret}' ne doit jamais être lisible"
            );
        }
        assert!(fichier_sensible(".env.production"));
        assert!(
            fichier_sensible("serveur.KEY"),
            "la casse ne doit pas contourner le filtre"
        );
        assert!(!fichier_sensible("lib.rs"));
        assert!(
            !fichier_sensible("environment.ts"),
            "un nom qui commence par 'env' sans point"
        );
    }

    #[test]
    fn les_secrets_ne_remontent_pas_dans_une_recherche() {
        let d = depot();
        let o = OptionsParcours {
            caches: true,
            limite: 500,
            ..Default::default()
        };
        let hits = d.trouver(".env", &o).expect("recherche");
        assert!(
            hits.iter().all(|h| !h.contains(".env")),
            "un fichier d'environnement est remonté : {hits:?}"
        );
    }

    #[test]
    fn refuse_un_motif_vide() {
        let d = depot();
        assert!(d.chercher("   ", &OptionsParcours::default()).is_err());
    }
}
