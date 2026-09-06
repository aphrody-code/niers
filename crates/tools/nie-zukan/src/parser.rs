//! Parsers HTML pour les pages du Zukan Inagle.
//!
//! Utilise `scraper` (CSS selectors sur HTML5).
//! Tous les parsers sont tolérants aux données manquantes (Option).

use crate::models::{
    CharaListEntry, Lang, StatBlock, StatCurves, ZukanChara, ZukanItem, ZukanSkill,
};
use anyhow::Result;
use scraper::{Html, Selector};
use tracing::warn;

// ---------------------------------------------------------------------------
// Helpers sélecteurs
// ---------------------------------------------------------------------------

fn sel(css: &str) -> Selector {
    Selector::parse(css).unwrap_or_else(|_| panic!("sélecteur CSS invalide: {css}"))
}

/// Extrait le texte brut d'un élément (sans balises enfant), trimmé.
fn text_of(el: scraper::ElementRef<'_>) -> String {
    el.text().collect::<Vec<_>>().join("").trim().to_owned()
}

/// Extrait le texte d'un `<ruby>` en ignorant les `<rt>` (le texte principal).
fn ruby_text(el: scraper::ElementRef<'_>) -> String {
    let rt_sel = sel("rt");
    let mut parts = Vec::new();
    for child in el.children() {
        use scraper::node::Node;
        match child.value() {
            Node::Text(t) => {
                let s = t.trim();
                if !s.is_empty() {
                    parts.push(s.to_owned());
                }
            }
            Node::Element(_) => {
                let child_ref = scraper::ElementRef::wrap(child).unwrap();
                // On ignore les <rt> (ruby pronunciation)
                if !rt_sel.matches(&child_ref) {
                    parts.push(ruby_text(child_ref));
                }
            }
            _ => {}
        }
    }
    parts.join("").trim().to_owned()
}

/// Extrait le texte complet d'un `.name span.name` (qui contient des `<ruby>`).
fn extract_name_from_ruby_span(span: scraper::ElementRef<'_>) -> String {
    let ruby_sel = sel("ruby");
    let rubies: Vec<_> = span.select(&ruby_sel).collect();
    if rubies.is_empty() {
        // Pas de ruby : texte brut
        return text_of(span);
    }
    rubies.iter().map(|r| ruby_text(*r)).collect::<String>()
}

// ---------------------------------------------------------------------------
// Parser : chara_list
// ---------------------------------------------------------------------------

/// Parse une page de liste de personnages.
///
/// Retourne la liste des entrées (`game_id`, `q_param`, `q_model`, name) et le nombre
/// total de pages (extrait de la pagination).
pub fn parse_chara_list(html: &str) -> Result<(Vec<CharaListEntry>, u32)> {
    let doc = Html::parse_document(html);

    let mut entries = Vec::new();

    // Liens model_view (contiennent le character_id)
    let model_a_sel = sel("a[href*='/chara_model_view/']");
    // Liens chara_param
    let param_a_sel = sel("a[href*='/chara_param/']");
    // Nom du personnage
    let name_sel = sel(".nameBox .name");

    // Sélecteur pour itérer sur les items si nécessaire (non utilisé directement ici)
    #[allow(unused_variables)]
    let li_sel = sel(".charaListBox li, .charaList li, ul li");

    // Stratégie : collecter tous les liens model_view et param de la page
    let model_links: Vec<_> = doc.select(&model_a_sel).collect();
    let param_links: Vec<_> = doc.select(&param_a_sel).collect();
    let names: Vec<_> = doc.select(&name_sel).collect();

    // Chaque personnage a 1 lien model_view + 1 lien param
    // Ils sont dans le même ordre
    let count = model_links.len().min(param_links.len());

    for i in 0..count {
        let model_href = model_links[i].attr("href").unwrap_or("").to_owned();
        let param_href = param_links[i].attr("href").unwrap_or("").to_owned();

        // Extraire q= de l'URL
        let q_model = extract_q_from_href(&model_href);
        let q_param = extract_q_from_href(&param_href);

        if q_model.is_empty() {
            warn!(i, "model_view href sans q, skip");
            continue;
        }

        // Décoder pour obtenir le game_id
        let game_id = match crate::forge::decode_q(&q_model) {
            Ok(json) => extract_character_id_from_json(&json),
            Err(e) => {
                warn!(i, error = %e, q = %q_model, "décodage q model_view échoué");
                continue;
            }
        };

        if game_id.is_empty() {
            warn!(i, "character_id vide après décodage");
            continue;
        }

        // Nom
        let name = if i < names.len() {
            extract_name_from_ruby_span(names[i])
        } else {
            String::new()
        };

        entries.push(CharaListEntry {
            game_id,
            q_param,
            q_model,
            name,
        });
    }

    // Pagination : trouver le numéro de la dernière page
    let last_page = extract_last_page(html);

    Ok((entries, last_page))
}

/// Extrait le paramètre `q=` d'un href.
fn extract_q_from_href(href: &str) -> String {
    if let Some(pos) = href.find("?q=") {
        href[pos + 3..].to_owned()
    } else {
        String::new()
    }
}

/// Extrait `character_id` d'un JSON `{"character_id":["c01000010"]}`.
fn extract_character_id_from_json(json: &str) -> String {
    // Parser JSON simple : chercher la valeur après "character_id":["]
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json)
        && let Some(arr) = v.get("character_id").and_then(|v| v.as_array())
        && let Some(id) = arr.first().and_then(|v| v.as_str())
    {
        return id.to_owned();
    }
    String::new()
}

/// Extrait le numéro de la dernière page depuis la pagination HTML.
fn extract_last_page(html: &str) -> u32 {
    // La pagination contient des liens `?page=N`
    let prefix = "?page=";
    let mut max = 1u32;
    let mut search = html;
    while let Some(pos) = search.find(prefix) {
        let rest = &search[pos + prefix.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if !rest[..end].is_empty()
            && let Ok(n) = rest[..end].parse::<u32>()
        {
            max = max.max(n);
        }
        // Avancer la recherche au-delà de ce qu'on vient de trouver
        search = &search[pos + prefix.len() + end.max(1)..];
    }
    max
}

// ---------------------------------------------------------------------------
// Parser : chara_param
// ---------------------------------------------------------------------------

/// Parse une page `chara_param` pour un personnage donné.
///
/// Retourne un vecteur de [`ZukanChara`] (une page peut contenir plusieurs
/// variantes d'un même personnage — ex. différentes éras).
pub fn parse_chara_param(html: &str, game_id: &str, lang: Lang) -> Result<Vec<ZukanChara>> {
    let doc = Html::parse_document(html);
    let mut results = Vec::new();

    // Chaque personnage est dans un `<li>` de la liste résultats
    // Structure : .content ul.paramList li  (ou juste ul li dans la zone résultats)
    let li_sel = sel(".columnBox .mainBox ul li, .mainBox ul li, #charaParam .content ul li");
    let items: Vec<_> = doc.select(&li_sel).collect();

    if items.is_empty() {
        // Fallback : chercher directement les détails sans itérer sur li
        if let Some(chara) = parse_single_chara_param(&doc, game_id, lang)? {
            results.push(chara);
        }
        return Ok(results);
    }

    for item in &items {
        if let Some(chara) = parse_chara_param_item(item, game_id, lang)? {
            results.push(chara);
        }
    }

    Ok(results)
}

/// Parse un `<li>` de résultat `chara_param`.
fn parse_chara_param_item(
    item: &scraper::ElementRef<'_>,
    game_id: &str,
    lang: Lang,
) -> Result<Option<ZukanChara>> {
    // Nom : dans `.nameBox .name`
    let name_sel = sel(".nameBox .name");
    let name = item
        .select(&name_sel)
        .next()
        .map(|el| extract_name_from_ruby_span(el))
        .unwrap_or_default();

    if name.is_empty() {
        return Ok(None);
    }

    // Nickname : dans `.name .nickname`
    let nick_sel = sel(".name .nickname");
    let nickname = item
        .select(&nick_sel)
        .next()
        .map(|el| extract_name_from_ruby_span(el));

    // Image URL : src de l'img dans .detailBox .lBox figure
    let img_sel = sel(".detailBox .lBox figure img");
    let image_url = item
        .select(&img_sel)
        .next()
        .and_then(|el| el.attr("src"))
        .map(str::to_owned);

    let image_hash = image_url.as_ref().and_then(|url| extract_cdn_hash(url));

    // Description : `.description`
    let desc_sel = sel(".description");
    let description = item
        .select(&desc_sel)
        .next()
        .map(|el| text_of(el).replace("<br>", "\n"))
        .filter(|s| !s.is_empty());

    // Œuvre d'origine : `.appearedWorks dd`
    let appeared_sel = sel(".appearedWorks dd");
    let game_appearance = item
        .select(&appeared_sel)
        .next()
        .map(|el| text_of(el))
        .filter(|s| !s.is_empty());

    // Acquisition : `.getTxt dd`
    let get_sel = sel(".getTxt dd");
    let acquisition = item
        .select(&get_sel)
        .next()
        .map(|el| text_of(el))
        .filter(|s| !s.is_empty());

    // Stats : tableau dans `.param li dl`
    let stats = parse_stats_from_item(item);

    // Attributs de base : `.basic li dl`
    let basic_sel = sel(".basic li dl");
    let mut position = None;
    let mut element = None;
    let mut age_group = None;
    let mut school_year = None;
    let mut gender = None;
    let mut chara_category = None;

    // Position / element depuis .param li dl
    let param_li_sel = sel(".param li dl");
    for dl in item.select(&param_li_sel) {
        let dt_sel = sel("dt");
        let dd_sel = sel("dd p, dd");
        let key = dl
            .select(&dt_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        let val = dl
            .select(&dd_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        match key.as_str() {
            "ポジション" | "Position" => position = Some(val),
            "属性" | "Attribute" | "Élément" | "Element" => element = Some(val),
            _ => {}
        }
    }

    for dl in item.select(&basic_sel) {
        let dt_sel = sel("dt");
        let dd_sel = sel("dd");
        let key = dl
            .select(&dt_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        let val = dl
            .select(&dd_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        match key.as_str() {
            "年代区分" | "Age Group" | "Groupe d'âge" => age_group = Some(val),
            "学年" | "Grade" | "Niveau" => school_year = Some(val),
            "性別" | "Gender" | "Genre" => gender = Some(val),
            "キャラカテゴリ" | "Character Category" | "Catégorie" => {
                chara_category = Some(val);
            }
            _ => {}
        }
    }

    Ok(Some(ZukanChara {
        game_id: game_id.to_owned(),
        lang,
        name,
        nickname,
        description,
        game_appearance,
        acquisition,
        position,
        element,
        age_group,
        school_year,
        gender,
        chara_category,
        stats,
        image_url,
        image_hash,
    }))
}

/// Extrait les stats depuis un item `<li>` `chara_param`.
///
/// Structure HTML : `.param li dl dt` = nom stat, `.param li dl dd table tbody tr td` = valeur
/// Le HTML ne contient que Lv50 en rendu server-side.
fn parse_stats_from_item(item: &scraper::ElementRef<'_>) -> StatCurves {
    let param_li_sel = sel(".param li");
    let mut block = StatBlock::default();
    let mut has_stats = false;

    for li in item.select(&param_li_sel) {
        let dt_sel = sel("dl dt");
        let td_sel = sel("dl dd table tbody tr td");
        let key = li
            .select(&dt_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        let val_str = li
            .select(&td_sel)
            .next()
            .map(|el| text_of(el))
            .unwrap_or_default();
        let val: u32 = val_str.parse().unwrap_or(0);
        if val == 0 && val_str.is_empty() {
            continue;
        }
        has_stats = true;
        match key.as_str() {
            "キック" | "Kick" => block.kick = val,
            "コントロール" | "Control" | "Contrôle" => block.control = val,
            "テクニック" | "Technique" => block.technique = val,
            "プレッシャー" | "Pressure" | "Pression" => block.pressure = val,
            "フィジカル" | "Physical" | "Physique" => block.physical = val,
            "アジリティ" | "Agility" | "Agilité" => block.agility = val,
            "インテリジェンス" | "Intelligence" => block.intelligence = val,
            _ => {}
        }
    }

    StatCurves {
        lv50: if has_stats { Some(block) } else { None },
        lv100: None,
        lv150: None,
        lv200: None,
    }
}

/// Fallback : parse la page entière comme un seul perso (si la liste est vide).
fn parse_single_chara_param(doc: &Html, game_id: &str, lang: Lang) -> Result<Option<ZukanChara>> {
    // Vérifier qu'il y a bien un nom avant de tenter
    let name_sel = sel(".nameBox .name");
    let name = doc
        .select(&name_sel)
        .next()
        .map(|el| extract_name_from_ruby_span(el))
        .unwrap_or_default();

    if name.is_empty() {
        return Ok(None);
    }

    // Envelopper dans un li fictif pour réutiliser parse_chara_param_item
    let wrapped = format!("<li>{}</li>", doc.html());
    let fake_doc = Html::parse_fragment(&wrapped);
    let li_sel = sel("li");
    if let Some(li) = fake_doc.select(&li_sel).next() {
        return parse_chara_param_item(&li, game_id, lang);
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Parser : skill
// ---------------------------------------------------------------------------

/// Parse une page de skills (必殺技).
pub fn parse_skill_list(html: &str, lang: Lang, page: u32) -> Result<Vec<ZukanSkill>> {
    let doc = Html::parse_document(html);
    let mut results = Vec::new();

    let li_sel = sel(".skillListBox li");
    let items: Vec<_> = doc.select(&li_sel).collect();

    for item in &items {
        // Nom
        let name_sel = sel(".nameBox .name ruby");
        let name = item
            .select(&name_sel)
            .next()
            .map(|el| ruby_text(el))
            .or_else(|| {
                let span_sel = sel(".nameBox .name");
                item.select(&span_sel).next().map(|el| text_of(el))
            })
            .unwrap_or_default();

        if name.is_empty() || name == "？？？" {
            // Skill secret : on conserve quand même la position
            let placeholder = ZukanSkill {
                name: "SECRET".to_owned(),
                lang,
                description: None,
                category: None,
                page,
                video_url: None,
                poster_url: None,
                thumbnail_url: None,
            };
            results.push(placeholder);
            continue;
        }

        // Description
        let desc_sel = sel(".description");
        let description = item
            .select(&desc_sel)
            .next()
            .map(|el| text_of(el))
            .filter(|s| !s.is_empty());

        // Catégorie : dans le bouton `.btnMovie`
        let cat_sel = sel(".btnMovie");
        let category = item
            .select(&cat_sel)
            .next()
            .map(|el| text_of(el))
            .filter(|s| !s.is_empty());

        // Vidéo + poster : `data-movie-url` et `data-poster-url` sur le lien
        let video_a_sel = sel("a[data-movie-url]");
        let video_el = item.select(&video_a_sel).next();
        let video_url = video_el
            .and_then(|el| el.attr("data-movie-url"))
            .map(str::to_owned);
        let poster_url = video_el
            .and_then(|el| el.attr("data-poster-url"))
            .map(str::to_owned);

        // Thumbnail : `<img>` dans la figure
        let thumb_sel = sel("figure img");
        let thumbnail_url = item
            .select(&thumb_sel)
            .next()
            .and_then(|el| el.attr("src"))
            .map(str::to_owned);

        results.push(ZukanSkill {
            name,
            lang,
            description,
            category,
            page,
            video_url,
            poster_url,
            thumbnail_url,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Parser : item
// ---------------------------------------------------------------------------

/// Parse une page d'items.
pub fn parse_item_list(
    html: &str,
    lang: Lang,
    category_name: &str,
    page: u32,
) -> Result<Vec<ZukanItem>> {
    let doc = Html::parse_document(html);
    let mut results = Vec::new();

    let li_sel = sel(".itemListBox li");
    let items: Vec<_> = doc.select(&li_sel).collect();

    for item in &items {
        // Nom : `.name` avec ruby
        let name_sel = sel("p.name ruby, p.name");
        let (name, name_rubi) = if let Some(el) = item.select(&name_sel).next() {
            let main = ruby_text(el);
            // Rubi : tout ce qui est dans <rt>
            let rt_sel = sel("rt");
            let rubi: String = el.select(&rt_sel).map(|r| text_of(r)).collect();
            (main, if rubi.is_empty() { None } else { Some(rubi) })
        } else {
            (String::new(), None)
        };

        if name.is_empty() {
            continue;
        }

        // Image
        let img_sel = sel("figure img");
        let image_url = item
            .select(&img_sel)
            .next()
            .and_then(|el| el.attr("src"))
            .map(str::to_owned);

        let image_hash = image_url.as_ref().and_then(|url| extract_cdn_hash(url));

        results.push(ZukanItem {
            name,
            lang,
            category: Some(category_name.to_owned()),
            image_url,
            image_hash,
            name_rubi,
            page,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extrait le hash CDN d'une URL `CloudFront`.
///
/// Pattern : `https://dxi4wb638ujep.cloudfront.net/1/k/<x>/<y>/<hash>.<ext>`
fn extract_cdn_hash(url: &str) -> Option<String> {
    let parts: Vec<&str> = url.rsplitn(2, '/').collect();
    if parts.len() == 2 {
        let filename = parts[0];
        // Retirer l'extension
        if let Some(dot) = filename.rfind('.') {
            let hash = &filename[..dot];
            if !hash.is_empty() && hash.len() > 3 {
                return Some(hash.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_cdn_hash_works() {
        let url = "https://dxi4wb638ujep.cloudfront.net/1/k/d/w/dwho-wi8ruk.png";
        assert_eq!(extract_cdn_hash(url), Some("dwho-wi8ruk".to_owned()));
    }

    #[test]
    fn extract_cdn_hash_webp() {
        let url = "https://dxi4wb638ujep.cloudfront.net/1/k/d/w/dwho-wi8ruk.webp";
        assert_eq!(extract_cdn_hash(url), Some("dwho-wi8ruk".to_owned()));
    }

    #[test]
    fn parse_chara_list_fixture() {
        // Fixture minimale : 1 entrée avec les liens requis
        let html = r#"<!DOCTYPE html><html><body>
        <ul>
          <li>
            <div class="nameBox"><span class="name"><ruby>円堂<rt>えんどう</rt></ruby><ruby>守<rt>まもる</rt></ruby></span></div>
            <a href="/chara_model_view/?q=hN2cl56NnpyLmo2glpvdxaTdnM_Oz8_Pz87P3aKC">キャラビュー</a>
            <a href="/chara_param/?q=hN2ZlpOLmo2gnJeejZ6glpugjIuN3cWk3ZzPzs_Pz8_Oz92igg%3D%3D">パラム</a>
          </li>
        </ul>
        </body></html>"#;

        let (entries, _pages) = parse_chara_list(html).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].game_id, "c01000010");
        assert!(entries[0].name.contains("円堂"));
    }

    #[test]
    fn parse_chara_param_endou_fixture() {
        // Fixture tirée du vrai HTML Endou (section d'un seul li)
        let html = r#"<!DOCTYPE html><html><body>
        <ul>
          <li>
            <div class="nameBox"><span class="name">
              <ruby></ruby><ruby>円堂<rt>えんどう</rt></ruby><ruby>守<rt>まもる</rt></ruby><ruby></ruby>
            </span></div>
            <div class="detailBox">
              <div class="lBox">
                <figure><picture>
                  <img src="https://dxi4wb638ujep.cloudfront.net/1/k/d/w/dwho-wi8ruk.png" alt="円堂 守" />
                </picture></figure>
                <div class="name">
                  <span class="nickname"><ruby>円堂<rt>えんどう</rt></ruby></span>
                </div>
              </div>
              <div class="rBox">
                <p class="description">サッカーへの情熱は誰にも負けない。</p>
                <dl class="getTxt"><dt>入手方法</dt><dd>プレイヤーズユニバース</dd></dl>
                <ul class="param">
                  <li><dl><dt>ポジション</dt><dd><p>GK</p></dd></dl>
                      <dl class="box"><dt>属性</dt><dd><p>山</p></dd></dl></li>
                  <li><dl><dt>キック</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>90</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>コントロール</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>97</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>テクニック</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>91</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>プレッシャー</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>98</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>フィジカル</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>105</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>アジリティ</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>111</td></tr></tbody></table></dd></dl></li>
                  <li><dl><dt>インテリジェンス</dt><dd><table><tbody><tr><th>Lv50</th></tr><tr><td>97</td></tr></tbody></table></dd></dl></li>
                </ul>
                <ul class="basic">
                  <li><dl><dt>年代区分</dt><dd>中学生</dd></dl></li>
                  <li><dl><dt>学年</dt><dd>二年生</dd></dl></li>
                  <li><dl><dt>性別</dt><dd>男</dd></dl></li>
                  <li><dl><dt>キャラカテゴリ</dt><dd>選手</dd></dl></li>
                </ul>
              </div>
            </div>
          </li>
        </ul>
        </body></html>"#;

        let charas = parse_chara_param(html, "c01000010", Lang::Ja).unwrap();
        assert!(!charas.is_empty(), "doit retourner au moins 1 perso");
        let c = &charas[0];
        assert_eq!(c.game_id, "c01000010");
        assert!(c.name.contains("円堂"), "nom = {}", c.name);
        assert_eq!(c.position.as_deref(), Some("GK"));
        assert_eq!(c.element.as_deref(), Some("山"));
        assert_eq!(c.gender.as_deref(), Some("男"));
        assert_eq!(c.age_group.as_deref(), Some("中学生"));

        let stats = c.stats.lv50.as_ref().expect("stats Lv50 présentes");
        assert_eq!(stats.kick, 90);
        assert_eq!(stats.control, 97);
        assert_eq!(stats.technique, 91);
        assert_eq!(stats.pressure, 98);
        assert_eq!(stats.physical, 105);
        assert_eq!(stats.agility, 111);
        assert_eq!(stats.intelligence, 97);
        assert_eq!(stats.total(), 90 + 97 + 91 + 98 + 105 + 111 + 97);

        assert_eq!(c.image_hash.as_deref(), Some("dwho-wi8ruk"));
    }

    #[test]
    fn parse_skill_list_fixture() {
        // Note : utiliser r##"..."## car le HTML contient href="#..." qui contiendrait "#
        let html = r##"<!DOCTYPE html><html><body>
        <ul class="skillListBox">
          <li>
            <div class="nameBox"><span class="name"><ruby>ザ・ウォール</ruby></span></div>
            <div class="detailBox">
              <div class="lBox skillLbox">
                <figure class="movie">
                  <a href="#movieId" data-movie-url="https://cdn/video.wmv" data-poster-url="https://cdn/poster.jpg">
                    <picture><img src="https://cdn/thumb.jpg" /></picture>
                  </a>
                </figure>
                <div class="btnBox">
                  <a href="#movieId" data-movie-url="https://cdn/video.wmv" data-poster-url="https://cdn/poster.jpg" class="btnMovie">ディフェンス</a>
                </div>
              </div>
              <div class="rBox">
                <p class="description">フィールド上に巨大な壁を出現させる技.</p>
              </div>
            </div>
          </li>
        </ul>
        </body></html>"##;

        let skills = parse_skill_list(html, Lang::Ja, 1).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "ザ・ウォール");
        assert!(skills[0].description.is_some());
        assert_eq!(skills[0].category.as_deref(), Some("ディフェンス"));
        assert!(skills[0].video_url.is_some());
    }

    #[test]
    fn parse_item_list_fixture() {
        let html = r#"<!DOCTYPE html><html><body>
        <ul class="itemListBox uniform">
          <li>
            <div class="detailBox">
              <figure class="item">
                <picture>
                  <img src="https://cdn/item.png" alt="[雷門/らいもん]シューズ" />
                </picture>
              </figure>
              <p class="name">
                <ruby></ruby><ruby>雷門<rt>らいもん</rt></ruby><ruby>シューズ</ruby>
              </p>
            </div>
          </li>
        </ul>
        </body></html>"#;

        let items = parse_item_list(html, Lang::Ja, "シューズ", 1).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].name.contains("雷門"));
        assert!(items[0].name.contains("シューズ"));
        assert_eq!(items[0].category.as_deref(), Some("シューズ"));
        assert!(items[0].image_hash.is_some());
    }
}
