//! `/couverture` et `/api/v1/couverture` — la matrice du § 4 du plan, servie.
//!
//! Le service **ne mesure pas** : mesurer, c'est lancer `niers --help`, lire quatre arbres de
//! sources et parcourir 255 308 lignes d'inventaire. Une route web ne fait pas cela. La matrice
//! est produite hors ligne par `nie-site --regenerer-couverture <fichier>` et lue ici.
//!
//! Conséquence assumée : **sur un dépôt où la commande n'a jamais tourné, `/couverture` répond
//! `503`** en citant la commande qui la produit. C'est la même règle que pour le VFS et le
//! miroir — une capacité absente se signale, elle ne s'invente pas.

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};

use crate::error::ErreurSite;
use crate::state::EtatSite;

/// Ce que la route dit quand la matrice n'a jamais été produite.
const ABSENTE: &str = "matrice de couverture absente — la produire par \
     `nie-site --regenerer-couverture var/couverture-site.json` (§ 4 du plan)";

/// Lit la matrice sur disque, telle qu'elle a été écrite.
fn lire(etat: &EtatSite) -> Result<String, ErreurSite> {
    std::fs::read_to_string(&etat.config.couverture).map_err(|e| {
        tracing::warn!(
            fichier = %etat.config.couverture.display(),
            erreur = %e,
            "matrice de couverture illisible"
        );
        ErreurSite::Indisponible(ABSENTE.to_string())
    })
}

/// `GET /api/v1/couverture` — la matrice, telle quelle.
///
/// Le corps est **republié verbatim** au lieu d'être désérialisé puis resérialisé : le fichier
/// est la mesure, et une re-sérialisation en changerait la forme sans rien y ajouter.
pub async fn json(State(etat): State<EtatSite>) -> Result<Response, ErreurSite> {
    let corps = lire(&etat)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        corps,
    )
        .into_response())
}

/// `GET /couverture` — la même matrice, lisible.
pub async fn page(State(etat): State<EtatSite>) -> Result<Response, ErreurSite> {
    let corps = lire(&etat)?;
    let matrice: crate::couverture::Matrice = serde_json::from_str(&corps).map_err(|e| {
        tracing::error!(erreur = %e, "matrice de couverture illisible (JSON)");
        ErreurSite::Indisponible("matrice de couverture illisible — la régénérer".to_string())
    })?;
    Ok(Html(rendre(&matrice)).into_response())
}

/// Rend la matrice en HTML — sans script, sans dépendance, et sans rien afficher que la mesure.
fn rendre(m: &crate::couverture::Matrice) -> String {
    let mut html = String::with_capacity(64 * 1024);
    html.push_str(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"robots\" content=\"noindex\">\
         <title>Couverture</title><style>\
         :root{color-scheme:dark}\
         body{background:#12141c;color:#e7ecf5;font:15px/1.55 system-ui,sans-serif;margin:0 auto;padding:2rem 1.25rem;max-width:74rem}\
         h1{font-size:1.5rem;margin:0 0 .25rem}h2{font-size:1.1rem;margin:2.5rem 0 .75rem}\
         p.sous{color:#93a0b8;margin:0 0 2rem}\
         table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}\
         th,td{text-align:left;padding:.35rem .6rem;border-bottom:1px solid #232838}\
         th{color:#93a0b8;font-weight:600}td.n,th.n{text-align:right}\
         .servi{color:#5fd39a}.partiel{color:#e8c46a}.manquant{color:#ef8a6a}\
         .bloque{color:#a58ce0}.interne{color:#93a0b8}\
         .gate{display:inline-block;padding:.35rem .7rem;border-radius:.3rem;font-weight:600}\
         .tenue{background:#12351f;color:#5fd39a}.rompue{background:#3a1c14;color:#ef8a6a}\
         code{color:#93a0b8}\
         </style></head><body>",
    );
    html.push_str("<h1>Couverture</h1><p class=\"sous\">");
    html.push_str(&echapper(&format!(
        "Générée le {} par nie-site {} — {} routes montées. Chaque compte se rejoue par la commande de sa source.",
        m.genere_le, m.version, m.routes_montees
    )));
    html.push_str("</p>");

    let classe = if m.gate.tenue { "tenue" } else { "rompue" };
    html.push_str(&format!(
        "<p><span class=\"gate {classe}\">gate maîtresse : manquant = {} · partiel = {} — {}</span></p>",
        m.gate.manquant,
        m.gate.partiel,
        if m.gate.tenue { "tenue" } else { "rompue" }
    ));

    html.push_str("<h2>Par état</h2><table><tr><th>État</th><th class=\"n\">Capacités</th><th class=\"n\">Poids</th></tr>");
    for nom in crate::couverture::Etat::NOMS {
        let c = m.par_etat.get(nom).copied().unwrap_or_default();
        html.push_str(&format!(
            "<tr><td class=\"{nom}\">{nom}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
            c.capacites, c.poids
        ));
    }
    html.push_str(&format!(
        "<tr><th>total</th><th class=\"n\">{}</th><th class=\"n\">{}</th></tr></table>",
        m.total.capacites, m.total.poids
    ));

    html.push_str("<h2>Par source</h2><table><tr><th>Source</th><th class=\"n\">Total</th>");
    for nom in crate::couverture::Etat::NOMS {
        html.push_str(&format!("<th class=\"n {nom}\">{nom}</th>"));
    }
    html.push_str("<th>Commande</th></tr>");
    for ligne in &m.par_source {
        html.push_str(&format!(
            "<tr><td>{}</td><td class=\"n\">{}</td>",
            echapper(&ligne.libelle),
            ligne.total.capacites
        ));
        for nom in crate::couverture::Etat::NOMS {
            let c = ligne.par_etat.get(nom).copied().unwrap_or_default();
            html.push_str(&format!("<td class=\"n\">{}</td>", c.capacites));
        }
        html.push_str(&format!(
            "<td><code>{}</code></td></tr>",
            echapper(&ligne.commande)
        ));
    }
    html.push_str("</table>");

    if !m.incoherences.is_empty() {
        html.push_str("<h2>Incohérences corrigées à la génération</h2><ul>");
        for i in &m.incoherences {
            html.push_str(&format!("<li>{}</li>", echapper(i)));
        }
        html.push_str("</ul>");
    }
    if !m.regles_mortes.is_empty() {
        html.push_str("<h2>Règles sans effet aujourd'hui</h2><ul>");
        for r in &m.regles_mortes {
            html.push_str(&format!("<li><code>{}</code></li>", echapper(r)));
        }
        html.push_str("</ul>");
    }

    html.push_str("<h2>Ce qui reste à faire</h2><table><tr><th>Source</th><th>Capacité</th><th class=\"n\">Poids</th><th>Où est le décodeur</th></tr>");
    let mut restes: Vec<&crate::couverture::Capacite> = m.etat("manquant");
    restes.extend(m.etat("partiel"));
    restes.sort_by_key(|c| std::cmp::Reverse(c.poids));
    for c in restes {
        let detail = match &c.etat {
            crate::couverture::Etat::Manquant { decodeur } => decodeur.as_ref(),
            crate::couverture::Etat::Partiel { manque, .. } => manque.as_ref(),
            _ => "",
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"n\">{}</td><td>{}</td></tr>",
            echapper(c.source.libelle()),
            echapper(&c.nom),
            c.poids,
            echapper(detail)
        ));
    }
    html.push_str("</table>");
    html.push_str("<p class=\"sous\">La matrice complète, ligne à ligne : <a href=\"/api/v1/couverture\">/api/v1/couverture</a></p>");
    html.push_str("</body></html>");
    html
}

/// Échappe le texte inséré dans le document. Une raison de classement est du texte libre :
/// elle contient des backticks, des chevrons et des guillemets.
fn echapper(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    for c in texte.chars() {
        match c {
            '&' => sortie.push_str("&amp;"),
            '<' => sortie.push_str("&lt;"),
            '>' => sortie.push_str("&gt;"),
            '"' => sortie.push_str("&quot;"),
            _ => sortie.push(c),
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::couverture::{Source, construire, mesure};

    fn matrice_temoin() -> crate::couverture::Matrice {
        let entrees = vec![
            // Une capacite manquante — le cas que la page doit rendre visible, avec son
            // detail et pas seulement son compte.
            mesure::Entree {
                source: Source::Azalee,
                nom: "/tools/compare".to_string(),
                poids: 1,
            },
            mesure::Entree {
                source: Source::Vfs,
                nom: ".cfg.bin".to_string(),
                poids: 71_101,
            },
        ];
        construire(&mesure::Inventaire { entrees }, &crate::app::chemins())
    }

    #[test]
    fn la_page_publie_les_comptes_et_pas_des_statuts() {
        let html = rendre(&matrice_temoin());
        assert!(html.contains("71101"), "le poids servi est affiché");
        assert!(html.contains("gate maîtresse"));
        assert!(html.contains("rompue"), "manquant = 1 : la gate est rompue");
        assert!(html.contains("/tools/compare"), "la capacité manquante est nommée");
        assert!(
            html.contains("les dix 308"),
            "la raison est publiée, pas seulement le compte"
        );
    }

    #[test]
    fn le_texte_libre_est_echappe() {
        assert_eq!(echapper("<script>&\"x\""), "&lt;script&gt;&amp;&quot;x&quot;");
        // Les raisons de classement contiennent réellement des chevrons :
        // `crates/engine/nie-data/src/<module>.rs`.
        let html = rendre(&matrice_temoin());
        assert!(!html.contains("<module>"), "un chevron non échappé casse le document");
    }
}
