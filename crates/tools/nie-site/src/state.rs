//! État partagé du serveur : VFS, gisement, cache, client d'amont, capacités.
//!
//! Principe : **le service démarre toujours**. Le montage du VFS coûte des minutes sur un dump
//! de 255 000 fichiers ; il se fait donc en tâche de fond, et les routes qui en dépendent
//! répondent `503` avec un message explicite tant qu'il n'est pas prêt. Rien n'est jamais
//! supposé présent : chaque capacité se **mesure**.

use std::sync::Arc;
use std::sync::RwLock;

use bytes::Bytes;
use moka::future::Cache;
use serde::Serialize;

use crate::config::Config;
use crate::dataset::Gisement;
use crate::vfs_index::IndexVfs;

/// Une réponse d'amont mise en cache, avec son ETag déjà calculé.
#[derive(Debug, Clone)]
pub struct ReponseCachee {
    /// Corps complet.
    pub corps: Bytes,
    /// Type de contenu tel que l'amont l'a annoncé.
    pub type_contenu: String,
    /// ETag fort : `blake3` du corps, en hexadécimal, entre guillemets.
    pub etag: String,
}

/// État du montage VFS.
pub enum StatutVfs {
    /// Montage en cours (tâche de fond).
    EnCours,
    /// Monté : l'index est utilisable, et `vfs` sert les octets quand il est présent.
    ///
    /// `vfs` est `None` dans les tests, qui injectent un index synthétique sans installation
    /// du jeu : `/b` et les vues répondent, `/f` dit honnêtement que le contenu est absent.
    Pret {
        /// Le VFS monté, s'il y en a un.
        vfs: Option<Arc<nie_formats::vfs::Vfs>>,
        /// Index trié des chemins, avec les vues pré-calculées.
        index: Arc<IndexVfs>,
        /// `true` quand le montage est un dump extrait, `false` pour une installation à packs.
        dump: bool,
    },
    /// Aucun VFS : ni installation ni dump. La raison est rendue telle quelle au client.
    Absent(String),
}

impl std::fmt::Debug for StatutVfs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Vfs` n'implémente pas `Debug` (il porte un cache de CPK) : on décrit l'état, pas le
        // contenu — un `Debug` qui déverserait 255 000 chemins ne servirait personne.
        match self {
            Self::EnCours => write!(f, "StatutVfs::EnCours"),
            Self::Pret { vfs, index, dump } => write!(
                f,
                "StatutVfs::Pret {{ contenu: {}, entrees: {}, dump: {dump} }}",
                vfs.is_some(),
                index.len()
            ),
            Self::Absent(r) => write!(f, "StatutVfs::Absent({r:?})"),
        }
    }
}

/// Capacités mesurées du service, telles que `/healthz` les rapporte.
#[derive(Debug, Clone, Serialize)]
pub struct Capacites {
    /// `en_cours`, `pret` ou `absent`.
    pub vfs: &'static str,
    /// Nombre de chemins indexés (0 tant que le VFS n'est pas prêt).
    pub vfs_entrees: usize,
    /// `true` si le montage est un dump extrait.
    pub vfs_dump: bool,
    /// `true` si le VFS peut réellement rendre des octets (`/f`).
    pub vfs_contenu: bool,
    /// `true` si le miroir SQLite est présent à l'instant de la mesure.
    pub gisement: bool,
    /// Racine du bundle statique servie, si elle existe.
    pub bundle: bool,
    /// Débit maximal par IP, en requêtes par seconde, ou `None` quand la borne est éteinte.
    ///
    /// Rapportée parce qu'une borne invisible est une borne qu'on accuse à tort : devant un
    /// `429`, la première question est « laquelle », et il faut pouvoir y répondre sans lire
    /// l'unité systemd.
    pub debit: Option<f64>,
}

/// État partagé, cloné par requête (tout est derrière un `Arc`).
#[derive(Clone)]
pub struct EtatSite {
    /// Configuration effective.
    pub config: Arc<Config>,
    /// Statut du montage VFS.
    pub vfs: Arc<RwLock<StatutVfs>>,
    /// Miroir SQLite en lecture seule.
    pub gisement: Arc<Gisement>,
    /// Cache des réponses d'amont, borné en poids et en durée.
    pub cache: Cache<String, ReponseCachee>,
    /// Client HTTP vers `nie-model-serve`.
    pub client: reqwest::Client,
    /// Jetons de concurrence vers l'amont : au-delà, on attend plutôt que d'écrouler l'amont.
    pub jetons_amont: Arc<tokio::sync::Semaphore>,
    /// Borne de débit par IP, ou `None` quand le réglage l'éteint. Le cache de seaux est
    /// partagé par tous les clones de l'état — c'est ce qui fait qu'un `Limiteur` cloné à
    /// chaque requête compte quand même les requêtes ensemble.
    pub limiteur: Option<crate::debit::Limiteur>,
}

impl std::fmt::Debug for EtatSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtatSite")
            .field("config", &self.config)
            .field("gisement", &self.gisement.chemin())
            .field("cache", &self.cache.entry_count())
            .field("limiteur", &self.limiteur)
            .finish_non_exhaustive()
    }
}

impl EtatSite {
    /// Construit l'état **sans** monter le VFS : le montage est lancé séparément par
    /// [`EtatSite::monter_vfs_en_fond`], ce qui garde le démarrage instantané.
    ///
    /// # Panics
    ///
    /// Panique si le client HTTP ne peut pas être construit — ce qui n'arrive qu'avec une
    /// configuration TLS impossible, et il n'y en a aucune ici (localhost en clair).
    #[must_use]
    pub fn nouveau(config: Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.delai_amont)
            .connect_timeout(std::time::Duration::from_secs(2))
            .user_agent(concat!("nie-site/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("client HTTP local constructible");
        let cache = Cache::builder()
            .max_capacity(config.cache_octets)
            .weigher(|_k: &String, v: &ReponseCachee| {
                u32::try_from(v.corps.len()).unwrap_or(u32::MAX)
            })
            .time_to_live(config.cache_ttl)
            .build();
        let jetons_amont = Arc::new(tokio::sync::Semaphore::new(config.concurrence_amont));
        let gisement = Arc::new(Gisement::nouveau(config.db.clone()));
        let limiteur = crate::debit::Limiteur::nouveau(config.debit);
        Self {
            config: Arc::new(config),
            vfs: Arc::new(RwLock::new(StatutVfs::EnCours)),
            gisement,
            cache,
            client,
            jetons_amont,
            limiteur,
        }
    }

    /// État de test : index VFS injecté, aucun contenu, aucun amont joignable.
    #[must_use]
    pub fn pour_tests(config: Config, index: IndexVfs) -> Self {
        let etat = Self::nouveau(config);
        etat.poser_vfs(None, Arc::new(index), false);
        etat
    }

    /// Publie le résultat d'un montage.
    pub fn poser_vfs(
        &self,
        vfs: Option<Arc<nie_formats::vfs::Vfs>>,
        index: Arc<IndexVfs>,
        dump: bool,
    ) {
        if let Ok(mut w) = self.vfs.write() {
            *w = StatutVfs::Pret { vfs, index, dump };
        }
    }

    /// Publie l'absence de VFS, avec sa raison.
    pub fn poser_vfs_absent(&self, raison: impl Into<String>) {
        if let Ok(mut w) = self.vfs.write() {
            *w = StatutVfs::Absent(raison.into());
        }
    }

    /// Index VFS, ou l'erreur `503` qui explique pourquoi il n'y en a pas.
    ///
    /// # Errors
    ///
    /// `Indisponible` tant que le montage n'est pas terminé, ou quand il a échoué.
    pub fn index(&self) -> Result<Arc<IndexVfs>, crate::ErreurSite> {
        let garde = self
            .vfs
            .read()
            .map_err(|_| crate::ErreurSite::Interne("etat VFS empoisonne".to_owned()))?;
        match &*garde {
            StatutVfs::Pret { index, .. } => Ok(Arc::clone(index)),
            StatutVfs::EnCours => Err(crate::ErreurSite::Indisponible(
                "index du VFS en cours de construction, reessayez dans quelques instants"
                    .to_owned(),
            )),
            StatutVfs::Absent(r) => Err(crate::ErreurSite::Indisponible(format!(
                "VFS indisponible: {r}"
            ))),
        }
    }

    /// VFS capable de rendre des octets.
    ///
    /// # Errors
    ///
    /// `Indisponible` quand le montage n'est pas prêt, ou quand l'index est là sans contenu
    /// (cas des tests, et d'un index injecté sans installation).
    pub fn vfs(&self) -> Result<Arc<nie_formats::vfs::Vfs>, crate::ErreurSite> {
        let garde = self
            .vfs
            .read()
            .map_err(|_| crate::ErreurSite::Interne("etat VFS empoisonne".to_owned()))?;
        match &*garde {
            StatutVfs::Pret { vfs: Some(v), .. } => Ok(Arc::clone(v)),
            StatutVfs::Pret { vfs: None, .. } => Err(crate::ErreurSite::Indisponible(
                "index present mais contenu du jeu non monte".to_owned(),
            )),
            StatutVfs::EnCours => Err(crate::ErreurSite::Indisponible(
                "VFS en cours de montage, reessayez dans quelques instants".to_owned(),
            )),
            StatutVfs::Absent(r) => Err(crate::ErreurSite::Indisponible(format!(
                "VFS indisponible: {r}"
            ))),
        }
    }

    /// Mesure les capacités à l'instant présent.
    #[must_use]
    pub fn capacites(&self) -> Capacites {
        let (vfs, vfs_entrees, vfs_dump, vfs_contenu) = match self.vfs.read() {
            Ok(g) => match &*g {
                StatutVfs::Pret { vfs, index, dump } => ("pret", index.len(), *dump, vfs.is_some()),
                StatutVfs::EnCours => ("en_cours", 0, false, false),
                StatutVfs::Absent(_) => ("absent", 0, false, false),
            },
            Err(_) => ("absent", 0, false, false),
        };
        Capacites {
            vfs,
            vfs_entrees,
            vfs_dump,
            vfs_contenu,
            gisement: self.gisement.present(),
            bundle: self.config.statique.is_dir(),
            debit: self.limiteur.as_ref().map(|l| l.reglage().par_seconde),
        }
    }

    /// Monte le VFS en tâche de fond et publie le résultat quand il est prêt.
    ///
    /// Le montage d'un dump construit un index de ~255 000 entrées : c'est du travail
    /// bloquant de plusieurs secondes à plusieurs minutes, jamais sur un worker Tokio.
    pub fn monter_vfs_en_fond(&self) {
        let etat = self.clone();
        tokio::task::spawn_blocking(move || {
            let debut = std::time::Instant::now();
            match nie_formats::vfs::open_game() {
                Ok(vfs) => {
                    let dump = vfs.is_dump();
                    let entrees: Vec<(String, u32)> = vfs
                        .iter()
                        .map(|(chemin, e)| (chemin.to_owned(), e.file_size))
                        .collect();
                    let n = entrees.len();
                    let index = Arc::new(IndexVfs::depuis(entrees));
                    etat.poser_vfs(Some(Arc::new(vfs)), index, dump);
                    tracing::info!(
                        entrees = n,
                        dump,
                        secondes = debut.elapsed().as_secs_f32(),
                        "VFS monte"
                    );
                }
                Err(e) => {
                    tracing::warn!(erreur = %e, "VFS non monte — /f et /b repondront 503");
                    etat.poser_vfs_absent(e.to_string());
                }
            }
        });
    }
}
