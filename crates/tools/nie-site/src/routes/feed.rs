//! `/feed.atom` — le flux des épisodes moissonnés, au format Atom (RFC 4287).
//!
//! ## Pourquoi un flux, et pourquoi celui-là
//!
//! Le site n'avait aucun flux. Il n'en a qu'un seul contenu possible, et il faut le dire
//! franchement : sur les cinq espaces qu'Aphrody publie, quatre — textures, modèles, sons,
//! vidéos — sont des vues de l'index du VFS, et **le VFS ne porte aucune date**. Un fichier du
//! jeu n'a pas de « date d'ajout » : il apparaît avec une mise à jour du jeu et disparaît avec
//! la suivante. Il n'existe donc rien à syndiquer là-bas, et fabriquer une date d'ajout serait
//! exactement le mensonge que `lastmod_du_gisement` refuse déjà de faire au plan du site.
//!
//! Le catalogue des épisodes, lui, est daté deux fois et réellement : `publishDate` (la date de
//! publication annoncée par l'hébergeur) et `createdAt` (l'instant de la moisson, en epoch ms,
//! renseigné sur les 1 141 lignes — vérifié : `count(*) where createdAt is null` = 0). C'est le
//! seul contenu du site dont « quoi de neuf » a un sens, donc le seul qui mérite un flux.
//!
//! ## Pourquoi pas la caisse `atom_syndication`
//!
//! `atom_syndication` 0.12.10 et `rss` 2.1.1 sont bien vivantes et sous licence acceptable
//! (MIT/Apache-2.0), mais elles apporteraient ici un constructeur XML pour **un seul**
//! document, alors qu'`askama` en rend déjà quatre (`sitemap.xml`, `robots.txt`, les deux
//! `llms`) et qu'il applique aux gabarits `.xml` le même échappeur qu'au HTML
//! (`askama_derive-0.16.1/src/config.rs:427` : `html, htm, j2, jinja, jinja2, rinja, svg,
//! **xml**`). Le besoin réel n'est pas un constructeur, c'est un échappement correct — il est
//! déjà là, et `flux_echappe_ce_que_le_catalogue_contient` le vérifie sur les 18 titres du
//! catalogue qui portent une esperluette, un chevron ou un guillemet.
//!
//! ## Ce que le flux garantit
//!
//! - `<updated>` et `<published>` sont du RFC 3339 **valide ou absent**, jamais approximatif :
//!   `publishDate` existe sous quatre formes dans la base (58 vides, 945 en `AAAA-MM-JJ`, 107
//!   en `…Z`, 31 en `…+00:00`), et une date à dix caractères n'est pas un horodatage. Voir
//!   [`horodatage`].
//! - le flux répond `503` quand la base est absente, comme `/api/v1/episodes` : un flux vide
//!   se lit « rien de neuf », ce qui n'est pas la même chose que « ce serveur ne moissonne
//!   pas la série ».

use askama::Template;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Nombre d'épisodes publiés au flux.
///
/// Un lecteur de flux relit le document entier à chaque passage : 50 entrées pèsent ~12 Kio et
/// couvrent largement une saison de moisson, là où les 1 141 lignes de la base en pèseraient
/// 270 et ne seraient jamais lues.
pub const ENTREES_MAX: u32 = 50;

/// `Cache-Control` du flux : un lecteur relit toutes les heures au mieux, et la moisson est
/// nocturne — revalider plus souvent ne rend rien de plus.
pub const CONTROLE: &str = "public, max-age=1800, stale-while-revalidate=86400";

/// Type de contenu du flux, tel que les lecteurs l'attendent.
pub const TYPE_CONTENU: &str = "application/atom+xml; charset=utf-8";

/// Une entrée du flux, déjà mise en forme — le gabarit ne calcule rien.
pub struct EntreeFlux {
    /// Identifiant permanent de l'entrée (IRI).
    pub id: String,
    /// Titre affiché.
    pub titre: String,
    /// Date de moisson, en RFC 3339.
    pub maj: String,
    /// Date de publication déclarée, quand elle est un horodatage exploitable.
    pub publie: Option<String>,
    /// Lien vers la vidéo, quand la base en porte un.
    pub lien: Option<String>,
    /// Résumé factuel : saison, épisode, piste.
    pub resume: String,
}

#[derive(Template)]
#[template(path = "feed.xml")]
struct Flux<'a> {
    titre: &'a str,
    sous_titre: &'a str,
    origine: &'a str,
    version: &'a str,
    maj: String,
    entrees: Vec<EntreeFlux>,
}

/// Convertit une valeur de `publishDate` en RFC 3339, ou rend `None`.
///
/// Les quatre formes présentes dans la base ont été relevées par
/// `select length(publishDate), count(*) from episodes group by 1` :
///
/// | Longueur | Occurrences | Exemple | Résultat |
/// |---|---|---|---|
/// | vide/`NULL` | 58 | | `None` |
/// | 10 | 945 | `2008-10-05` | `2008-10-05T00:00:00Z` |
/// | 20 | 107 | `2026-04-23T16:00:06Z` | inchangé |
/// | 25 | 31 | `2026-04-23T16:00:06+00:00` | inchangé |
///
/// Une date nue est complétée à minuit UTC — c'est la convention des flux, et elle n'invente
/// pas d'information : le jour est celui qu'annonce la source. Tout le reste rend `None` : un
/// `<published>` invalide fait rejeter l'entrée entière par un lecteur strict, en silence.
#[must_use]
pub fn horodatage(brut: &str) -> Option<String> {
    let s = brut.trim();
    if s.is_empty() {
        return None;
    }
    let jour_valide = |j: &str| {
        j.len() == 10
            && j.as_bytes().iter().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    *c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            })
    };
    let Some((jour, heure)) = s.split_once('T') else {
        return jour_valide(s).then(|| format!("{s}T00:00:00Z"));
    };
    if !jour_valide(jour) {
        return None;
    }
    // La partie horaire est `hh:mm:ss` suivi de `Z` ou d'un décalage `+hh:mm` / `-hh:mm`. Les
    // deux formes sont séparées ici, plutôt que reconnues par une expression rationnelle : une
    // date fausse doit se voir refuser sur un critère qu'on peut nommer.
    let (horloge, decalage) = match heure.strip_suffix('Z') {
        Some(h) => (h, None),
        None if heure.len() == 14 => (&heure[..8], Some(&heure[8..])),
        None => return None,
    };
    let horloge_ok = horloge.len() == 8
        && horloge.as_bytes().iter().enumerate().all(|(i, c)| {
            if i == 2 || i == 5 {
                *c == b':'
            } else {
                c.is_ascii_digit()
            }
        });
    let decalage_ok = decalage.is_none_or(|d| {
        d.as_bytes().iter().enumerate().all(|(i, c)| match i {
            0 => *c == b'+' || *c == b'-',
            3 => *c == b':',
            _ => c.is_ascii_digit(),
        })
    });
    (horloge_ok && decalage_ok).then(|| s.to_owned())
}

/// `GET /feed.atom`.
pub async fn atom(State(etat): State<EtatSite>) -> Response {
    let chemin = etat.config.episodes.clone();
    if !chemin.is_file() {
        return ErreurSite::Indisponible(
            "catalogue des épisodes absent : ce serveur ne moissonne pas la série".to_owned(),
        )
        .into_response();
    }
    let origine = etat.config.origine.clone();
    let lecture = tokio::task::spawn_blocking(move || lire(&chemin, &origine, ENTREES_MAX)).await;
    let entrees = match lecture {
        Ok(Ok(e)) => e,
        Ok(Err(e)) => return e.into_response(),
        Err(e) => return ErreurSite::from(e).into_response(),
    };

    // La date du flux est celle de son entrée la plus récente, pas `maintenant()` : un
    // `<updated>` qui bouge à chaque requête fait retélécharger le document à chaque passage,
    // ce qui est l'exact contraire de ce qu'un flux sert à faire. Sans entrée, on retombe sur
    // la date du fichier — encore un fait mesuré, jamais une invention.
    let maj = entrees
        .first()
        .map(|e| e.maj.clone())
        .or_else(|| date_du_fichier(&etat.config.episodes))
        .unwrap_or_else(|| crate::routes::well_known::iso8601_utc(0));

    let flux = Flux {
        titre: "Aphrody — épisodes",
        sous_titre: "Les épisodes de la série moissonnés par ce serveur, du plus récent au plus ancien.",
        origine: &etat.config.origine,
        version: crate::VERSION,
        maj,
        entrees,
    };
    match flux.render() {
        Ok(xml) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, TYPE_CONTENU),
                (header::CACHE_CONTROL, CONTROLE),
            ],
            xml,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(erreur = %e, "rendu du flux impossible");
            ErreurSite::Interne("flux indisponible".to_owned()).into_response()
        }
    }
}

/// Date de dernière écriture du catalogue, en RFC 3339.
fn date_du_fichier(chemin: &std::path::Path) -> Option<String> {
    let modifie = std::fs::metadata(chemin).ok()?.modified().ok()?;
    let secondes = modifie
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(crate::routes::well_known::iso8601_utc(secondes))
}

/// Lit les `limite` épisodes moissonnés le plus récemment.
///
/// L'ordre est `createdAt DESC, id DESC` : la moisson pose le même horodatage sur tout un lot
/// (mesuré : 1 141 lignes pour une poignée de valeurs distinctes de `createdAt`), et sans le
/// second critère l'ordre du flux changerait d'un appel à l'autre sur des données identiques.
///
/// # Errors
///
/// `Interne` quand la base est illisible ou qu'une ligne ne se lit pas.
pub fn lire(
    chemin: &std::path::Path,
    origine: &str,
    limite: u32,
) -> Result<Vec<EntreeFlux>, ErreurSite> {
    let cx = super::episodes::ouvrir(chemin)?;
    let mut requete = cx
        .prepare(
            "SELECT id, season, episode, title, url, titleJp, publishDate, language, createdAt \
             FROM episodes ORDER BY createdAt DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| ErreurSite::Interne(format!("requête du flux: {e}")))?;
    let lignes = requete
        .query_map(rusqlite::params![limite], |l| {
            let id: i64 = l.get(0)?;
            let saison: Option<i64> = l.get(1)?;
            let episode: Option<i64> = l.get(2)?;
            let titre: Option<String> = l.get(3)?;
            let url: Option<String> = l.get(4)?;
            let titre_jp: Option<String> = l.get(5)?;
            let publie: Option<String> = l.get(6)?;
            let langue: Option<String> = l.get(7)?;
            let moissonne: Option<i64> = l.get(8)?;
            Ok(entree(
                origine, id, saison, episode, titre, url, titre_jp, publie, langue, moissonne,
            ))
        })
        .map_err(|e| ErreurSite::Interne(format!("lecture du flux: {e}")))?;
    lignes
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ErreurSite::Interne(format!("ligne de flux illisible: {e}")))
}

/// Met une ligne en forme d'entrée Atom.
///
/// L'identifiant est dérivé de l'origine configurée, comme toutes les URL de la crate : rien
/// n'est codé en dur, au prix assumé qu'un changement d'origine renumérote le flux — un
/// déploiement de préversion doit se décrire lui-même plutôt qu'usurper `aphrody.com`.
#[expect(
    clippy::too_many_arguments,
    reason = "neuf colonnes de la table, nommees une par une"
)]
#[must_use]
pub fn entree(
    origine: &str,
    id: i64,
    saison: Option<i64>,
    episode: Option<i64>,
    titre: Option<String>,
    url: Option<String>,
    titre_jp: Option<String>,
    publie: Option<String>,
    langue: Option<String>,
    moissonne: Option<i64>,
) -> EntreeFlux {
    let numero = match (saison, episode) {
        (Some(s), Some(e)) => Some(format!("S{s:02}E{e:02}")),
        _ => None,
    };
    let brut = titre.filter(|t| !t.trim().is_empty());
    let titre = match (&numero, brut) {
        (Some(n), Some(t)) => format!("{n} — {t}"),
        (None, Some(t)) => t,
        (Some(n), None) => n.clone(),
        (None, None) => format!("Épisode {id}"),
    };
    let mut morceaux = Vec::new();
    if let Some(n) = numero {
        morceaux.push(n);
    }
    if let Some(l) = langue.filter(|l| !l.trim().is_empty()) {
        morceaux.push(format!("piste {l}"));
    }
    if let Some(jp) = titre_jp.filter(|t| !t.trim().is_empty()) {
        morceaux.push(jp);
    }
    let resume = if morceaux.is_empty() {
        titre.clone()
    } else {
        morceaux.join(" — ")
    };
    EntreeFlux {
        id: format!("{origine}/api/v1/episodes#{id}"),
        titre,
        // `createdAt` est en millisecondes ; `count(*) where createdAt is null` = 0 sur les
        // 1 141 lignes, mais la colonne est nullable et le repli vaut mieux qu'un `unwrap`.
        maj: crate::routes::well_known::iso8601_utc(
            moissonne.map_or(0, |ms| u64::try_from(ms / 1000).unwrap_or(0)),
        ),
        publie: publie.as_deref().and_then(horodatage),
        lien: url.filter(|u| u.starts_with("http://") || u.starts_with("https://")),
        resume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_quatre_formes_de_date_du_catalogue() {
        // Les quatre formes REELLEMENT presentes, relevees par `group by length(publishDate)`.
        assert_eq!(horodatage(""), None, "58 lignes vides");
        assert_eq!(horodatage("   "), None);
        assert_eq!(
            horodatage("2008-10-05").as_deref(),
            Some("2008-10-05T00:00:00Z"),
            "945 lignes en date nue : completees a minuit UTC"
        );
        assert_eq!(
            horodatage("2026-04-23T16:00:06Z").as_deref(),
            Some("2026-04-23T16:00:06Z"),
            "107 lignes deja en RFC 3339"
        );
        assert_eq!(
            horodatage("2026-04-23T16:00:06+00:00").as_deref(),
            Some("2026-04-23T16:00:06+00:00"),
            "31 lignes avec decalage explicite"
        );
        // Et tout ce qui n'est pas un horodatage rend None plutot qu'un `<published>` invalide.
        for mauvais in [
            "hier",
            "2008-10",
            "05/10/2008",
            "2008-10-05T16:00",
            "2008-10-05T16:00:06",
            "2008-10-05T16:00:06+0000",
            "2008-1x-05",
            "2008-10-05 16:00:06Z",
        ] {
            assert_eq!(horodatage(mauvais), None, "{mauvais} n'est pas du RFC 3339");
        }
    }

    fn exemple(origine: &str) -> EntreeFlux {
        entree(
            origine,
            4751,
            Some(1),
            Some(1),
            Some("Spielt Fussball".to_owned()),
            Some("https://exemple.test/v/abc".to_owned()),
            None,
            Some("2008-10-05".to_owned()),
            Some("de".to_owned()),
            Some(1_788_434_933_000),
        )
    }

    #[test]
    fn une_entree_porte_ce_que_la_ligne_dit() {
        let e = exemple("https://exemple.test");
        assert_eq!(e.id, "https://exemple.test/api/v1/episodes#4751");
        assert_eq!(e.titre, "S01E01 — Spielt Fussball");
        assert_eq!(e.resume, "S01E01 — piste de");
        assert_eq!(e.publie.as_deref(), Some("2008-10-05T00:00:00Z"));
        assert_eq!(e.lien.as_deref(), Some("https://exemple.test/v/abc"));
        // 1 788 434 933 000 ms = 1 788 434 933 s.
        assert_eq!(e.maj, crate::routes::well_known::iso8601_utc(1_788_434_933));
    }

    #[test]
    fn une_ligne_creuse_ne_produit_ni_titre_vide_ni_lien_faux() {
        let e = entree(
            "https://exemple.test",
            9,
            None,
            None,
            Some("   ".to_owned()),
            // Un `url` qui n'est pas une URL absolue ne devient PAS un `<link href>` : un
            // lecteur le resoudrait contre l'origine du flux et pointerait n'importe ou.
            Some("watch?v=abc".to_owned()),
            Some("イナズマイレブン".to_owned()),
            Some("hier".to_owned()),
            None,
            None,
        );
        assert_eq!(e.titre, "Épisode 9", "un titre blanc n'est pas un titre");
        assert_eq!(e.lien, None, "url relative refusee");
        assert_eq!(e.publie, None, "date illisible : pas de <published>");
        assert_eq!(e.resume, "イナズマイレブン");
        assert_eq!(e.maj, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn flux_echappe_ce_que_le_catalogue_contient() {
        // 18 titres du catalogue portent `&`, `<` ou `"` (mesure du 2026-09-05). Sans
        // echappement, chacun casse le document entier — et un flux invalide n'est pas
        // « degrade », il est rejete en bloc par le lecteur.
        let mut e = exemple("https://exemple.test");
        e.titre = "Fussball & <Freunde> \"Kapitel 1\"".to_owned();
        let rendu = Flux {
            titre: "T",
            sous_titre: "S",
            origine: "https://exemple.test",
            version: "0.0.0",
            maj: "2026-09-05T00:00:00Z".to_owned(),
            entrees: vec![e],
        }
        .render()
        .expect("rendu");
        // Askama echappe en references NUMERIQUES (`&#38;`, `&#60;`, `&#62;`, `&#34;`,
        // `&#39;` — `filters/escape.rs:127-133`), pas en entites nommees. Les deux sont
        // valides en XML, mais seules les numeriques le sont SANS DTD : un `&amp;` est defini
        // par XML lui-meme, un `&nbsp;` ne l'est pas. Le flux est donc du XML autonome.
        assert!(
            rendu.contains("Fussball &#38; &#60;Freunde&#62;"),
            "esperluette et chevrons"
        );
        assert!(rendu.contains("&#34;Kapitel 1&#34;"), "guillemets");
        assert!(!rendu.contains("& <"), "aucun caractere brut ne subsiste");
        assert_eq!(rendu.matches("<entry>").count(), 1);
        assert_eq!(rendu.matches("<published>").count(), 1);
        assert!(rendu.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(rendu.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
        // Les trois elements qu'un `<feed>` doit porter (RFC 4287 §4.1.1).
        for exige in ["<id>", "<title>", "<updated>"] {
            assert!(rendu.contains(exige), "{exige} absent du flux");
        }
        assert!(
            rendu.contains("rel=\"self\""),
            "un flux doit savoir se nommer"
        );
        assert!(
            !rendu.contains("aphrody.com"),
            "aucune origine codee en dur"
        );
    }

    #[test]
    fn un_flux_vide_reste_un_document_valide() {
        let rendu = Flux {
            titre: "T",
            sous_titre: "S",
            origine: "https://exemple.test",
            version: "0.0.0",
            maj: "2026-09-05T00:00:00Z".to_owned(),
            entrees: Vec::new(),
        }
        .render()
        .expect("rendu");
        assert_eq!(rendu.matches("<entry>").count(), 0);
        assert!(rendu.contains("<updated>2026-09-05T00:00:00Z</updated>"));
        assert!(rendu.trim_end().ends_with("</feed>"));
    }
}
