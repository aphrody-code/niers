//! Limitation de débit par **IP réelle du client**.
//!
//! ## Le trou, mesuré
//!
//! Relevé le 2026-09-05 dans `/etc/nginx/conf.d/aphrody.com.conf`, tel qu'il est installé :
//!
//! - ligne 131, hôte `nie.` : `limit_req zone=nie_assets burst=20 nodelay` ;
//! - ligne 211, hôte `api.` : `limit_req zone=nie_assets burst=40 nodelay` ;
//! - lignes 50 à 77, hôte **`aphrody.com` lui-même** : `proxy_pass 127.0.0.1:8085`, et
//!   **aucun `limit_req`**.
//!
//! Le site public est donc le seul des trois à n'avoir aucune borne de débit, et c'est celui
//! qui expose les catalogues, la recherche `?q=` (un parcours de l'index à chaque appel) et les
//! 255 000 chemins de `/f`. Cette couche ferme exactement ce trou-là, et rien d'autre.
//!
//! ## Pourquoi pas `tower_governor`
//!
//! `tower_governor` 0.8.0 (MIT/Apache-2.0, compatible axum 0.8) a été évalué, source en main.
//! Deux raisons mesurées de ne pas le prendre **ici** :
//!
//! 1. **Ses extracteurs de clé ne conviennent pas à cette topologie.** `PeerIpKeyExtractor`
//!    verrait toujours `127.0.0.1` — nginx est le seul pair — et ferait donc un seau unique
//!    pour toute la planète : le limiteur se transformerait en déni de service à lui tout seul.
//!    `SmartIpKeyExtractor` lit `x-forwarded-for` **avant** `x-real-ip`
//!    (`key_extractor.rs:129-135`) et y prend la **première** entrée analysable. Or le vhost
//!    pose `X-Forwarded-For $proxy_add_x_forwarded_for` (ligne 77), qui *préfixe* la valeur
//!    envoyée par le client : la première entrée est donc contrôlée par l'attaquant, et la clé
//!    de limitation se contourne en changeant un en-tête. `X-Real-IP` (ligne 76) vient de
//!    `$remote_addr` et est **écrasé** par nginx : c'est le seul des deux qu'un client ne peut
//!    pas fabriquer.
//! 2. **Le coût.** Mesuré par `cargo tree` sur une caisse témoin : 12 caisses absentes du
//!    `Cargo.lock` du dépôt (`governor`, `dashmap`, `quanta`, `nonzero_ext`, `spinning_top`,
//!    `futures-timer`, `nonempty`, `forwarded-header-value`, `tower_governor`, plus `tonic`,
//!    `hyper-timeout` et `tokio-stream` par ses features par défaut).
//!
//! Le seau à jetons ci-dessous tient en 60 lignes, s'appuie sur `moka` et `parking_lot` **déjà
//! présents**, et prend sa clé là où elle est digne de foi. Ce n'est pas réinventer une roue :
//! c'est la seule roue qui roule dans cette topologie.
//!
//! ## Ce qui n'est pas limité, et pourquoi
//!
//! - `/healthz` : une sonde de santé qu'on étrangle est une sonde qui ment. Le vhost l'appelle
//!   (ligne 88) et systemd s'en sert.
//! - Une requête **sans `X-Real-IP`** passe sans être comptée. Elle vient d'un chemin nginx qui
//!   ne nomme pas son client — les `location` d'`api.` (lignes 226 à 228) sont dans ce cas — et
//!   ces chemins-là portent **déjà** un `limit_req` nginx (ligne 211). Les rassembler dans un
//!   seau commun reviendrait à faire s'entre-limiter des clients étrangers les uns aux autres.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use moka::future::Cache;
use parking_lot::Mutex;

use crate::state::EtatSite;

/// L'en-tête qui porte l'IP du client, posé par nginx depuis `$remote_addr`.
///
/// Volontairement **pas** `x-forwarded-for` : le vhost l'assemble avec
/// `$proxy_add_x_forwarded_for`, qui conserve ce que le client a envoyé.
pub const ENTETE_IP: &str = "x-real-ip";

/// Chemins jamais limités, quel que soit le réglage.
pub const EXEMPTS: [&str; 1] = ["/healthz"];

/// Nombre maximal d'IP suivies simultanément.
///
/// Borne la mémoire du limiteur face à une rotation d'adresses : chaque seau pèse une
/// vingtaine d'octets utiles, `moka` évince les moins récemment vues au-delà. 20 000 IP
/// distinctes en vol simultané est déjà deux ordres de grandeur au-dessus du trafic observé.
pub const IP_SUIVIES_MAX: u64 = 20_000;

/// Réglage du seau à jetons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglage {
    /// Débit de remplissage, en requêtes par seconde. `0` désactive la limitation.
    pub par_seconde: f64,
    /// Capacité du seau : le nombre de requêtes qu'une rafale peut consommer d'un coup.
    pub rafale: f64,
}

impl Reglage {
    /// Réglage par défaut : 30 requêtes par seconde en régime établi, 180 en rafale.
    ///
    /// La rafale est dimensionnée sur la page la plus lourde du site, pas au jugé : une page de
    /// catalogue rend [`crate::routes::pages::PAR_PAGE`] = 60 vignettes, chacune une requête
    /// vers `/assets`, plus le document, la feuille de style, le script, le manifeste et
    /// l'icône — soit 65. 180 laisse donc passer **deux** pages complètes enchaînées sans
    /// jamais toucher la borne, avec 50 requêtes de marge.
    ///
    /// 30 r/s en régime établi est trois fois ce que nginx accorde à `nie.` (`rate=10r/s`,
    /// ligne 103 du vhost) : le site sert des pages, pas des assets bruts, et une page coûte
    /// plusieurs requêtes là où `nie.` en coûte une.
    #[must_use]
    pub const fn defaut() -> Self {
        Self { par_seconde: 30.0, rafale: 180.0 }
    }

    /// Dit si le réglage limite réellement quelque chose.
    #[must_use]
    pub fn actif(self) -> bool {
        self.par_seconde > 0.0 && self.rafale >= 1.0
    }
}

impl Default for Reglage {
    fn default() -> Self {
        Self::defaut()
    }
}

/// Verdict rendu pour une requête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// La requête passe : un jeton a été consommé.
    Passe,
    /// La requête est refusée. `retenter` est la valeur de `Retry-After`, en secondes.
    Refuse {
        /// Secondes à attendre avant qu'un jeton ne soit de nouveau disponible.
        retenter: u64,
    },
}

/// Un seau à jetons, rempli en continu.
#[derive(Debug)]
struct Seau {
    jetons: f64,
    dernier: Instant,
}

/// Le limiteur : un seau par IP, borné en nombre et en durée de vie.
#[derive(Clone)]
pub struct Limiteur {
    reglage: Reglage,
    seaux: Cache<IpAddr, Arc<Mutex<Seau>>>,
}

impl std::fmt::Debug for Limiteur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Limiteur")
            .field("reglage", &self.reglage)
            .field("ip_suivies", &self.seaux.entry_count())
            .finish()
    }
}

impl Limiteur {
    /// Construit un limiteur, ou `None` quand le réglage ne limite rien.
    #[must_use]
    pub fn nouveau(reglage: Reglage) -> Option<Self> {
        if !reglage.actif() {
            return None;
        }
        // Un seau plein ne porte aucune information : passé le temps qu'il faut pour le
        // remplir entièrement, l'oublier et le recréer donnent exactement le même verdict.
        // C'est ce qui rend l'éviction par inactivité sûre plutôt qu'approximative.
        let remplissage =
            Duration::from_secs_f64((reglage.rafale / reglage.par_seconde).max(1.0)).min(
                Duration::from_secs(3600),
            );
        Some(Self {
            reglage,
            seaux: Cache::builder()
                .max_capacity(IP_SUIVIES_MAX)
                .time_to_idle(remplissage)
                .build(),
        })
    }

    /// Réglage effectif.
    #[must_use]
    pub fn reglage(&self) -> Reglage {
        self.reglage
    }

    /// Nombre d'IP actuellement suivies (approché : `moka` évince en tâche de fond).
    #[must_use]
    pub fn ip_suivies(&self) -> u64 {
        self.seaux.entry_count()
    }

    /// Consomme un jeton pour cette IP et rend le verdict.
    pub async fn consommer(&self, ip: IpAddr) -> Verdict {
        self.consommer_a(ip, Instant::now()).await
    }

    /// Même chose, à un instant imposé — c'est ce qui rend le remplissage testable sans
    /// attendre réellement une seconde dans une suite de tests.
    pub async fn consommer_a(&self, ip: IpAddr, maintenant: Instant) -> Verdict {
        let rafale = self.reglage.rafale;
        let seau = self
            .seaux
            .get_with(ip, async move {
                Arc::new(Mutex::new(Seau { jetons: rafale, dernier: maintenant }))
            })
            .await;
        // Verrou pris et relâché sans point d'attente au milieu : aucun `await` ne traverse la
        // section critique, donc aucun risque de retenir le seau d'une IP sur un fil bloqué.
        let mut s = seau.lock();
        let ecoule = maintenant.saturating_duration_since(s.dernier).as_secs_f64();
        s.dernier = maintenant;
        s.jetons = (s.jetons + ecoule * self.reglage.par_seconde).min(rafale);
        if s.jetons >= 1.0 {
            s.jetons -= 1.0;
            return Verdict::Passe;
        }
        // `Retry-After` est un entier de secondes et ne vaut jamais 0 : annoncer « réessayez
        // dans 0 s » invite le client à repartir aussitôt et à se faire refuser de nouveau.
        let manque = 1.0 - s.jetons;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "manque est dans ]0,1] et par_seconde > 0 : le quotient arrondi tient sur u64"
        )]
        let retenter = (manque / self.reglage.par_seconde).ceil().max(1.0) as u64;
        Verdict::Refuse { retenter }
    }
}

/// Extrait l'IP du client de l'en-tête posé par nginx.
///
/// Rend `None` quand l'en-tête est absent ou illisible : la requête n'est alors pas comptée
/// (cf. le préambule du module). Un en-tête présent mais impossible à analyser est traité comme
/// absent — jamais comme une clé littérale, ce qui rassemblerait tous les clients fautifs dans
/// un seau commun.
#[must_use]
pub fn ip_du_client(entetes: &axum::http::HeaderMap) -> Option<IpAddr> {
    entetes
        .get(ENTETE_IP)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
}

/// Couche : refuse en `429` ce qui dépasse la borne, laisse tout le reste intact.
pub async fn limiter(State(etat): State<EtatSite>, requete: Request, suite: Next) -> Response {
    let Some(limiteur) = etat.limiteur.as_ref() else {
        return suite.run(requete).await;
    };
    if EXEMPTS.contains(&requete.uri().path()) {
        return suite.run(requete).await;
    }
    let Some(ip) = ip_du_client(requete.headers()) else {
        return suite.run(requete).await;
    };
    match limiteur.consommer(ip).await {
        Verdict::Passe => suite.run(requete).await,
        Verdict::Refuse { retenter } => {
            tracing::debug!(%ip, retenter, chemin = requete.uri().path(), "debit depasse");
            let mut reponse = crate::ErreurSite::TropDeRequetes(format!(
                "debit depasse: {} requetes par seconde",
                limiteur.reglage.par_seconde
            ))
            .into_response();
            if let Ok(v) = HeaderValue::from_str(&retenter.to_string()) {
                reponse.headers_mut().insert(header::RETRY_AFTER, v);
            }
            reponse
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u8) -> IpAddr {
        IpAddr::from([203, 0, 113, n])
    }

    #[test]
    fn un_reglage_nul_ne_construit_aucun_limiteur() {
        assert!(Limiteur::nouveau(Reglage { par_seconde: 0.0, rafale: 100.0 }).is_none());
        assert!(Limiteur::nouveau(Reglage { par_seconde: 10.0, rafale: 0.0 }).is_none());
        assert!(Limiteur::nouveau(Reglage::defaut()).is_some());
    }

    #[tokio::test]
    async fn la_rafale_est_exactement_consommee() {
        let l = Limiteur::nouveau(Reglage { par_seconde: 1.0, rafale: 5.0 }).expect("actif");
        let t = Instant::now();
        // Cinq jetons, cinq passages — au même instant, donc sans un iota de remplissage.
        for i in 0..5 {
            assert_eq!(l.consommer_a(ip(1), t).await, Verdict::Passe, "requete {i}");
        }
        assert_eq!(l.consommer_a(ip(1), t).await, Verdict::Refuse { retenter: 1 }, "la 6e");
        assert_eq!(l.ip_suivies(), 0, "moka compte en differe, pas en synchrone");
    }

    #[tokio::test]
    async fn les_ip_ne_se_partagent_pas_leur_seau() {
        let l = Limiteur::nouveau(Reglage { par_seconde: 1.0, rafale: 2.0 }).expect("actif");
        let t = Instant::now();
        for _ in 0..2 {
            assert_eq!(l.consommer_a(ip(1), t).await, Verdict::Passe);
        }
        assert!(matches!(l.consommer_a(ip(1), t).await, Verdict::Refuse { .. }));
        // Le voisin n'a rien consommé : il doit repartir d'un seau plein.
        for _ in 0..2 {
            assert_eq!(l.consommer_a(ip(2), t).await, Verdict::Passe, "IP distincte");
        }
    }

    #[tokio::test]
    async fn le_seau_se_remplit_avec_le_temps() {
        let l = Limiteur::nouveau(Reglage { par_seconde: 10.0, rafale: 2.0 }).expect("actif");
        let t = Instant::now();
        for _ in 0..2 {
            assert_eq!(l.consommer_a(ip(3), t).await, Verdict::Passe);
        }
        assert!(matches!(l.consommer_a(ip(3), t).await, Verdict::Refuse { .. }));
        // 100 ms a 10 r/s = exactement un jeton : un passage, puis de nouveau un refus.
        let plus_tard = t + Duration::from_millis(100);
        assert_eq!(l.consommer_a(ip(3), plus_tard).await, Verdict::Passe, "un jeton rendu");
        assert!(matches!(l.consommer_a(ip(3), plus_tard).await, Verdict::Refuse { .. }));
        // Et jamais au-dela de la capacite, meme apres une heure d'inactivite.
        let bien_plus_tard = t + Duration::from_secs(3600);
        for _ in 0..2 {
            assert_eq!(l.consommer_a(ip(3), bien_plus_tard).await, Verdict::Passe);
        }
        assert!(matches!(l.consommer_a(ip(3), bien_plus_tard).await, Verdict::Refuse { .. }));
    }

    #[tokio::test]
    async fn retry_after_annonce_une_attente_utile() {
        // A 2 requetes par seconde, un jeton met 500 ms a revenir : arrondi a 1 s.
        let l = Limiteur::nouveau(Reglage { par_seconde: 2.0, rafale: 1.0 }).expect("actif");
        let t = Instant::now();
        assert_eq!(l.consommer_a(ip(4), t).await, Verdict::Passe);
        assert_eq!(l.consommer_a(ip(4), t).await, Verdict::Refuse { retenter: 1 });
        // A 0,1 requete par seconde, il faut dix secondes.
        let lent = Limiteur::nouveau(Reglage { par_seconde: 0.1, rafale: 1.0 }).expect("actif");
        assert_eq!(lent.consommer_a(ip(5), t).await, Verdict::Passe);
        assert_eq!(lent.consommer_a(ip(5), t).await, Verdict::Refuse { retenter: 10 });
    }

    #[test]
    fn la_cle_ne_vient_que_de_x_real_ip() {
        let mut h = axum::http::HeaderMap::new();
        assert_eq!(ip_du_client(&h), None, "aucun en-tete: pas de cle");
        // Un `X-Forwarded-For` seul ne donne AUCUNE cle : sa premiere entree est celle que le
        // client a envoyee, nginx ne fait que la prefixer (`$proxy_add_x_forwarded_for`).
        h.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4, 10.0.0.1"));
        assert_eq!(ip_du_client(&h), None, "x-forwarded-for est falsifiable, donc ignore");
        h.insert(ENTETE_IP, HeaderValue::from_static("203.0.113.9"));
        assert_eq!(ip_du_client(&h), Some(ip(9)));
        h.insert(ENTETE_IP, HeaderValue::from_static("pas-une-ip"));
        assert_eq!(ip_du_client(&h), None, "illisible vaut absent, jamais cle litterale");
        h.insert(ENTETE_IP, HeaderValue::from_static("  2001:db8::1  "));
        assert_eq!(ip_du_client(&h), "2001:db8::1".parse().ok(), "IPv6 acceptee");
    }

    #[test]
    fn la_rafale_absorbe_une_page_de_catalogue_entiere() {
        let r = Reglage::defaut();
        // 60 vignettes + le document + la feuille + le script + le manifeste + le favicon.
        let page = crate::routes::pages::PAR_PAGE as f64 + 5.0;
        assert!(r.rafale >= page * 2.0, "deux pages completes doivent passer sans borne");
        assert!(r.par_seconde >= 10.0, "au moins ce que nginx accorde a nie.");
    }
}
