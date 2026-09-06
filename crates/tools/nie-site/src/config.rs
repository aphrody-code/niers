//! Configuration du serveur — variables d'environnement et options de ligne de commande.
//!
//! Aucun chemin de machine n'est compilé ici : la racine du jeu se résout à l'exécution par
//! [`nie_formats::vfs::resolve_game_dir`], et les autres chemins ont une valeur par défaut
//! **relative** au répertoire de travail (celui de l'unité systemd, `/home/ubuntu/niers`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

/// Adresse d'écoute par défaut : boucle locale uniquement, nginx est devant.
pub const ADRESSE_DEFAUT: &str = "127.0.0.1:8085";
/// Miroir SQLite par défaut (lien symbolique daté, rebasculé chaque nuit par `nie-miroir`).
pub const DB_DEFAUT: &str = "var/mirror.sqlite";
/// Amont de décodage par défaut — `nie-model-serve`, lui aussi sur la boucle locale.
pub const AMONT_DEFAUT: &str = "http://127.0.0.1:8790";
/// Bundle statique par défaut, produit par `bun run build` dans `apps/nie-web`.
pub const STATIQUE_DEFAUT: &str = "apps/nie-web/dist";

/// Catalogue des épisodes par défaut — le gisement `anime` du dépôt.
pub const EPISODES_DEFAUT: &str = "data/anime/episodes.db";

/// Nombre maximal d'éléments par page d'API. Le catalogue complet (250 800 fichiers, 53 126
/// textures) n'est **jamais** servi d'un coup : c'est une borne, pas une suggestion.
pub const PER_PAGE_MAX: u32 = 200;
/// Nombre d'éléments par page quand le client n'en demande pas.
pub const PER_PAGE_DEFAUT: u32 = 50;

/// Configuration effective du serveur.
#[derive(Debug, Clone)]
pub struct Config {
    /// Adresse d'écoute (`NIE_SITE_ADDR`).
    pub adresse: SocketAddr,
    /// Miroir SQLite lu en `SQLITE_OPEN_READ_ONLY` (`NIE_SITE_DB`).
    pub db: PathBuf,
    /// Base de `nie-model-serve`, sans slash final (`NIE_MODEL_SERVE_URL`).
    pub amont: String,
    /// Racine du bundle statique (`NIE_SITE_STATIC_DIR`).
    pub statique: PathBuf,
    /// Catalogue des épisodes de la série (`NIE_SITE_EPISODES`), lu en lecture seule.
    ///
    /// C'est la base que le cron du VPS rafraîchit chaque nuit, et la source de
    /// `/api/v1/episodes` — la porte par laquelle les Inacord déjà installés se mettent à jour.
    pub episodes: PathBuf,
    /// Délai maximal d'un appel vers l'amont.
    pub delai_amont: Duration,
    /// Nombre d'appels simultanés autorisés vers l'amont.
    pub concurrence_amont: usize,
    /// Taille maximale d'une réponse d'amont mise en cache et servie, en octets.
    pub taille_max_amont: usize,
    /// Poids total du cache d'assets, en octets.
    pub cache_octets: u64,
    /// Durée de vie d'une entrée du cache d'assets.
    pub cache_ttl: Duration,
    /// Origine publique, utilisée par `sitemap.xml`, `robots.txt` et les balises `og:`.
    pub origine: String,
    /// Borne de débit par IP réelle du client (cf. [`crate::debit`]).
    ///
    /// Elle ferme un trou mesuré : le vhost pose un `limit_req` sur `nie.` et sur `api.`, et
    /// **aucun** sur `aphrody.com`. `par_seconde = 0` la désactive entièrement.
    pub debit: crate::debit::Reglage,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adresse: ADRESSE_DEFAUT.parse().expect("adresse par defaut valide"),
            db: PathBuf::from(DB_DEFAUT),
            amont: AMONT_DEFAUT.to_owned(),
            statique: PathBuf::from(STATIQUE_DEFAUT),
            episodes: PathBuf::from(EPISODES_DEFAUT),
            delai_amont: Duration::from_secs(10),
            concurrence_amont: 16,
            taille_max_amont: 32 * 1024 * 1024,
            cache_octets: 256 * 1024 * 1024,
            cache_ttl: Duration::from_secs(300),
            origine: "https://aphrody.com".to_owned(),
            debit: crate::debit::Reglage::defaut(),
        }
    }
}

/// Options de ligne de commande. Chaque option a son équivalent en variable d'environnement ;
/// la ligne de commande gagne, l'environnement vient ensuite, la valeur par défaut en dernier.
#[derive(Debug, Parser)]
#[command(
    name = "nie-site",
    about = "Aphrody — serveur HTTP du bundle nie-web, de /api/v1, des espaces VFS /f et /b et du proxy nie-model-serve"
)]
pub struct Options {
    /// Adresse d'écoute (défaut `127.0.0.1:8085`).
    #[arg(long, env = "NIE_SITE_ADDR")]
    pub listen: Option<String>,
    /// Miroir SQLite lu en lecture seule (défaut `var/mirror.sqlite`).
    #[arg(long, env = "NIE_SITE_DB")]
    pub db: Option<PathBuf>,
    /// Base de `nie-model-serve` (défaut `http://127.0.0.1:8790`).
    #[arg(long, env = "NIE_MODEL_SERVE_URL")]
    pub upstream: Option<String>,
    /// Racine du bundle statique (défaut `apps/nie-web/dist`).
    #[arg(long, env = "NIE_SITE_STATIC_DIR")]
    pub bundle_dir: Option<PathBuf>,
    /// Catalogue des épisodes de la série (défaut `data/anime/episodes.db`).
    #[arg(long, env = "NIE_SITE_EPISODES")]
    pub episodes: Option<PathBuf>,
    /// Origine publique annoncée dans `sitemap.xml` et les balises `og:`.
    #[arg(long, env = "NIE_SITE_ORIGIN")]
    pub origin: Option<String>,
    /// Poids du cache d'assets, en mébioctets (défaut 256).
    #[arg(long, env = "NIE_SITE_CACHE_MIB")]
    pub cache_mib: Option<u64>,
    /// Durée de vie d'une entrée du cache d'assets, en secondes (défaut 300).
    #[arg(long, env = "NIE_SITE_CACHE_TTL")]
    pub cache_ttl: Option<u64>,
    /// Requêtes par seconde et par IP en régime établi (défaut 30, `0` désactive la borne).
    #[arg(long, env = "NIE_SITE_DEBIT")]
    pub debit: Option<f64>,
    /// Requêtes qu'une rafale peut consommer d'un coup, par IP (défaut 120).
    #[arg(long, env = "NIE_SITE_RAFALE")]
    pub rafale: Option<f64>,
}

impl Options {
    /// Applique les options sur la configuration par défaut.
    ///
    /// # Errors
    ///
    /// Rend une erreur quand `--listen` n'est pas une adresse `hôte:port` valide : mieux vaut
    /// refuser de démarrer que d'écouter ailleurs que là où nginx pointe.
    pub fn en_config(self) -> Result<Config, std::net::AddrParseError> {
        let mut cfg = Config::default();
        if let Some(a) = self.listen.filter(|s| !s.trim().is_empty()) {
            cfg.adresse = a.trim().parse()?;
        }
        if let Some(d) = self.db.filter(|p| !p.as_os_str().is_empty()) {
            cfg.db = d;
        }
        if let Some(u) = self.upstream.filter(|s| !s.trim().is_empty()) {
            cfg.amont = u.trim().trim_end_matches('/').to_owned();
        }
        if let Some(b) = self.bundle_dir.filter(|p| !p.as_os_str().is_empty()) {
            cfg.statique = b;
        }
        if let Some(e) = self.episodes.filter(|p| !p.as_os_str().is_empty()) {
            cfg.episodes = e;
        }
        if let Some(o) = self.origin.filter(|s| !s.trim().is_empty()) {
            cfg.origine = o.trim().trim_end_matches('/').to_owned();
        }
        if let Some(m) = self.cache_mib.filter(|m| *m > 0) {
            cfg.cache_octets = m.saturating_mul(1024 * 1024);
        }
        if let Some(t) = self.cache_ttl.filter(|t| *t > 0) {
            cfg.cache_ttl = Duration::from_secs(t);
        }
        // `0` est une valeur SIGNIFIANTE ici — elle éteint le limiteur — et c'est pourquoi ce
        // champ n'a pas le `filter(> 0)` des autres : le refuser rendrait la désactivation
        // impossible autrement qu'en recompilant.
        if let Some(d) = self.debit.filter(|d| d.is_finite() && *d >= 0.0) {
            cfg.debit.par_seconde = d;
        }
        if let Some(r) = self.rafale.filter(|r| r.is_finite() && *r >= 0.0) {
            cfg.debit.rafale = r;
        }
        Ok(cfg)
    }
}

/// Pagination demandée par le client, déjà bornée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    /// Numéro de page, à partir de 1.
    pub page: u32,
    /// Nombre d'éléments par page, dans `1..=`[`PER_PAGE_MAX`].
    pub per_page: u32,
}

impl Pagination {
    /// Borne une demande brute : `page` au minimum 1, `per_page` dans `1..=`[`PER_PAGE_MAX`].
    ///
    /// Une demande absurde n'est pas une erreur — elle est ramenée dans les bornes et la
    /// réponse annonce ce qui a réellement été appliqué.
    #[must_use]
    pub fn borner(page: Option<u32>, per_page: Option<u32>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1),
            per_page: per_page.unwrap_or(PER_PAGE_DEFAUT).clamp(1, PER_PAGE_MAX),
        }
    }

    /// Décalage correspondant, en éléments.
    #[must_use]
    pub fn offset(self) -> usize {
        (self.page as usize - 1).saturating_mul(self.per_page as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_bornee() {
        assert_eq!(
            Pagination::borner(None, None),
            Pagination {
                page: 1,
                per_page: 50
            }
        );
        assert_eq!(Pagination::borner(Some(0), Some(0)).page, 1);
        assert_eq!(Pagination::borner(Some(0), Some(0)).per_page, 1);
        assert_eq!(
            Pagination::borner(Some(3), Some(10_000)).per_page,
            PER_PAGE_MAX
        );
        assert_eq!(Pagination::borner(Some(3), Some(10)).offset(), 20);
    }

    #[test]
    fn defauts_surs() {
        let cfg = Config::default();
        assert_eq!(cfg.adresse.to_string(), ADRESSE_DEFAUT);
        assert!(
            cfg.adresse.ip().is_loopback(),
            "n'ecouter que sur la boucle locale"
        );
        assert_eq!(cfg.amont, AMONT_DEFAUT);
        assert_eq!(cfg.delai_amont.as_secs(), 10);
    }
}
