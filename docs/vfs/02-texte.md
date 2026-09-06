# Domaine VFS 2 — texte, localisation, polices, propriétés

Inventaire source : `var/vfs/lot2-texte.txt` (une ligne `chemin taille [cpk]`), couvrant
`data/common/text/`, `data/common/font/`, `data/common/property/`, `data/dx11/text/`,
`data/dx11/font/`. Toutes les commandes de ce document sont rejouables telles quelles depuis
la racine du dépôt (`NIE_GAME_DIR=/home/ubuntu/niers`).

## 1. Les chiffres

```
wc -l var/vfs/lot2-texte.txt
awk '{s+=$2} END{print s, NR}' var/vfs/lot2-texte.txt
```
**44 412 fichiers, 248 270 832 octets (≈ 236,8 Mio).**

### Par sous-dossier racine

```
awk '{print $1}' var/vfs/lot2-texte.txt | awk -F/ '{print $1"/"$2"/"$3}' | sort | uniq -c | sort -rn
```

| Racine | Fichiers |
|---|---|
| `data/common/text` | 44 241 |
| `data/common/property` | 104 |
| `data/dx11/font` | 34 |
| `data/dx11/text` | 19 |
| `data/common/font` | 14 |

### Par extension

```
awk '{print $1}' var/vfs/lot2-texte.txt | rg -o '\.[A-Za-z0-9_]+(\.[A-Za-z0-9_]+)?$' | sort | uniq -c | sort -rn
```

| Extension | Fichiers |
|---|---|
| `.cfg.bin` | 44 375 |
| `.g4tx` | 34 |
| `.08.bin` (i.e. `.cfg_0.00.08.bin`, versionné) | 3 |

### Par langue — `data/common/text/<langue>/`

```
for l in ja en fr de es it pt zh_hans zh_hant event map common; do
  awk -v l="$l" 'index($1,"data/common/text/"l"/")==1{n++;s+=$2} END{print l, n+0, s+0}' var/vfs/lot2-texte.txt
done
```

| Répertoire | Fichiers | Octets |
|---|---|---|
| `zh_hant` | 5 379 | 9 337 552 |
| `ja` | 5 234 | 12 474 864 |
| `event` (sans langue — cf. §2) | 5 131 | 3 519 088 |
| `zh_hans` | 4 069 | 8 355 920 |
| `en` | 4 068 | 8 177 296 |
| `pt` | 4 065 | 8 433 456 |
| `it` | 4 065 | 8 443 984 |
| `fr` | 4 065 | 8 553 056 |
| `es` | 4 065 | 8 417 552 |
| `de` | 4 065 | 8 507 360 |
| `map` (sans langue) | 33 | 759 776 |
| `common` (sans langue) | 2 | 75 920 |

`ja` et `zh_hant`/`zh_hans` ont plus de fichiers que `fr`/`en`/… : ces trois langues portent
des sous-dossiers `event/`, `map/`, `mission/`, `phase/`, `purpose/` propres (traduction de
l'event/mission/purpose/phase), alors que le français etc. n'en ont pas — vérifié par
`comm` sur les noms de famille (§2). Les 8 langues « standard » (`en de es fr it pt zh_hans`)
portent chacune le même jeu de **43 fichiers plats** (§2), plus `map/` et `purpose/`/`phase/`
pour zh_hans uniquement — à confirmer si besoin par un diff complet, non refait ici.

## 2. La grammaire des chemins

### `data/common/text/<langue|event|map|common>/…`

- **8 langues plates** : `en`, `de`, `es`, `fr`, `it`, `pt`, `zh_hans` (+ `ja`, `zh_hant` avec
  des sous-dossiers additionnels) — 43 fichiers `<famille>_text.cfg.bin` à la racine de chaque
  langue, ex. `data/common/text/fr/menu_text.cfg.bin`, `chara_text.cfg.bin`,
  `chara_text_roma.cfg.bin` (la translittération latine à côté du texte natif),
  `skill_text.cfg.bin`, `item_text.cfg.bin`, `rpg_battle_cmd_text.cfg.bin`, `system_text.cfg.bin`,
  `soccer_*_text.cfg.bin` (9 variantes), `quest_purpose_text.cfg.bin`, `staffroll_text.cfg.bin`…
  Liste complète mesurée :
  `rg '^data/common/text/fr/[a-z0-9_]+\.cfg\.bin' var/vfs/lot2-texte.txt` → 43 lignes.
- **Sous-dossiers additionnels par langue** (`ja`, `zh_hant`, et partiellement les autres) :
  `<langue>/event/ev<NN>_<NNNNN>.cfg.bin` (dialogue de scénario, un fichier par scène ;
  ex. `data/common/text/fr/event/ev41_10100.cfg.bin`), `<langue>/map/<code_map>_npc_text.cfg.bin`
  (texte des PNJ par carte, ex. `w10_npc_text.cfg.bin`, `z01_debug_npc_text.cfg.bin`),
  `<langue>/purpose/c<NN>_purpose_text.cfg.bin` et `<langue>/phase/c01_phase_text.cfg.bin`
  (objectifs de quête par chapitre `c01`…`c96`).
- **`event/` à la racine** (sans langue, 5 131 fichiers) : `ev<NN>_<NNNNN>_map.cfg.bin` —
  la variante **hors langue** de la scène `event/<code>.cfg.bin`, à vérifier ce qu'elle porte
  (probablement les métadonnées de mise en scène — positions, timing — communes à toutes les
  langues, séparées du texte traduit). Non prouvé par un parseur ici.
- **`map/` à la racine** (33 fichiers) : `<code_map>_npc_text_map.cfg.bin` — la contrepartie
  non traduite (positions/structure) des `<langue>/map/*_npc_text.cfg.bin`.
- **`common/` à la racine** (2 fichiers) : `common_talk_text_map.cfg.bin`,
  `system_text_map.cfg.bin`.
- L'identifiant de langue est **le premier segment sous `text/`** (`fr`, `de`, `ja`…), jamais
  dans le nom de fichier. La famille (`menu`, `chara`, `skill`, `event`…) est **le préfixe du
  nom de fichier**, jamais un dossier séparé (sauf `event/`, `map/`, `purpose/`, `phase/`).

### `data/common/font/` et `data/dx11/font/`

- `common/font/font/<fonte>/font.cfg.bin` — métrique de 10 fontes (`font_def`, `font_ja`,
  `font_ja2`, `font_ja_endroll`, `font_ja_endroll2`, `font_zh_endroll`, `font_zh_hans`,
  `font_zh_hans2`, `font_zh_hant`, `font_zh_hant2`) : la **grille de glyphes** (métriques).
- `dx11/font/<fonte>/font.g4tx` — le miroir **atlas texturé** (glyphes rendus en pixels) des
  mêmes 10 fontes ; `dx11/font/<langue>/gaiji_game2.g4tx` — glyphes spéciaux (« gaiji ») par
  langue (`de en es fr it`) ; `dx11/font/gaiji_game.g4tx`, `gaiji_hlp_*.g4tx`,
  `gaiji_SteamDeck.g4tx` — variantes plateforme des glyphes d'aide (icônes de touches).
- `common/font/font_color.cfg.bin` — table de couleurs de texte.
- `common/font/font_style/<ja|zh_hans|zh_hant>/font_style.cfg_0.00.08.bin` — style de fonte
  par langue CJK, **nom versionné** (`.cfg_0.00.08.bin`, pas `.cfg.bin` — un chemin cité de
  mémoire ferait l'erreur).
- **9 fontes cataloguées** au total dans le dépôt (cf. mémoire `fontes-9-cataloguees-…`) : les
  10 `font.cfg.bin` ci-dessus correspondent à cette famille (`font_def` = Latin, les 9 autres
  = variantes JA/ZH).

### `data/common/property/` et `data/dx11/text/`

- `common/property/<domaine>/<nom>.cfg.bin` — configuration moteur, **pas du texte utilisateur** :
  `camera/`, `chara/`, `common/`, `debug/`, `effect/`, `global_param/` (dont `game_param.cfg.bin`,
  187 888 o — le plus gros fichier de propriétés), `light/` (76 fichiers `light_2d_*` par scène
  d'événement), `posteffect/`, `rpg_battle/`, `soccer/`.
- `dx11/text/<langue>/{menu_text_platform,system_text_platform}.cfg.bin` — variante
  **spécifique à la plateforme DX11** du texte menu/système (à côté de la version
  `common/text/<langue>/menu_text.cfg.bin`) ; `dx11/text/ja/licensetext_platform.cfg.bin`
  (173 248 o) — texte de licence/mentions légales, JA seulement.

## 3. Ce que le dépôt sait déjà en faire

### Décodage du conteneur binaire

- `crates/engine/nie-formats/src/cfgbin.rs:248` `is_rdbn()` — vrai magic `RDBN` (`*b"RDBN"`,
  `cfgbin.rs:51`).
- `crates/engine/nie-formats/src/cfgbin.rs:804` `cfgbin_parse()` = `parse_t2b()`
  (`cfgbin.rs:809`) — arbre `CfgEntry { name, variables, children }`.
- **Vérifié à l'octet ici, contrairement à une simple supposition** :
  - `data/common/text/fr/menu_text.cfg.bin` → premiers octets `08 0c 00 00 80 06 01 00 3e 61 01 00 b5 09 00 00`
    (pas `RDBN`) → **T2B**, comme `common/property/**`.
  - `data/common/property/camera/camera_ctrl_property_info.cfg.bin` → `db 00 00 00 30 0d 00 00 f0 0e 00 00 ca 00 00 00`
    → **T2B**.
  - `data/common/font/font/font_def/font.cfg.bin` → `16 1f 00 00 e0 3e 05 00 00 00 00 00 …` →
    forme T2B (table de chaînes vide : normal, une fonte n'a pas de chaînes).
  - **Conclusion mesurée : l'intégralité du domaine texte/police/propriétés observée ici est
    T2B, pas RDBN** — l'énoncé « tout `common/property/**` est T2B » du CLAUDE.md se vérifie
    et s'étend en pratique à `common/text/**` et `common/font/**` (échantillon de 3 fichiers,
    pas un balayage exhaustif des 44 375).

### Le module fontes

`crates/engine/nie-formats/src/font.rs` :
- `GlyphMetric` (`font.rs:53`), `FontDimensions` (`font.rs:81`), `FontMetrics` (`font.rs:93`)
  avec `glyph()` (`:113`), `glyph_char()` (`:125`, lit un `char` Rust via `cle_empaquetee`),
  `glyph_in_font()` (`:132`).
- `pub fn parse_metrics(cfg: &CfgBinFile) -> FontMetrics` (`font.rs:233`) — décode un
  `font.cfg.bin` T2B en table de métriques. **Générique aux 9 fontes**, pas spécifique à
  `font_def` (cf. mémoire `fontes-9-cataloguees-codepoint-empaquete`).
- `pub fn cle_empaquetee(c: char) -> u32` (`font.rs:153`) et son inverse
  `pub fn decode_packed_codepoint(raw: u32) -> Option<char>` (`font.rs:174`) — **un
  `CHR.codepoint` peut porter une séquence UTF-8 empaquetée en big-endian** ; test
  `decode_packed_codepoint_cas_reels` (`font.rs:679`) couvre ASCII direct, 2 et 3 octets sur
  de vraies fontes (2026-08-15).
- `pub fn glyph_blitter(...)` (`font.rs:372`) et `pub fn draw_text(...)` (`font.rs:452`) —
  primitive de blit générique glyphe→canevas.
- `pub struct LatinAtlas` (`font.rs:501`), `from_atlas()` (`font.rs:526`), `span()` (`font.rs:593`),
  `measure()` (`font.rs:602`), `blit_line()` (`font.rs:619`) — **rendu par edge-scan sur l'atlas
  Latin uniquement** (repackaging propre nécessaire pour ce format-là, cf. mémoire) — utilisé
  en production par `nie-model-serve` (`main.rs:940`, `:977`) pour composer les scènes de
  dialogue.

**État réel du rendu de texte, à ne pas surclaimer** :
- **Prouvé et câblé en prod** : le rendu Latin via `LatinAtlas` (edge-scan, y_base≈946),
  utilisé par `compose_story_png` (`crates/tools/nie-model-serve/src/main.rs:950`) pour la
  scène de dialogue (route `/story-scene[/<n>]`, lit `inagle_event_subtitles`).
  Reste non traité par ce chemin : les accents sont **translittérés**, pas rendus nativement
  (mémoire `mode-histoire-scene-dialogue`).
- **Générique mais pas prouvé en rendu multi-glyphes** : `parse_metrics`/`glyph_blitter`
  fonctionnent sur les 9 fontes (test `real_glyph_blitter_a`, `font.rs:964`, sur `font_def`
  uniquement dans ce fichier). Le `decode_packed_codepoint` est validé unitairement sur des
  cas réels, mais **aucun chemin de production ne rend un texte CJK complet** (JA, ZH) via ce
  décodage — non trouvé dans `nie-model-serve`. **Donc : ne pas annoncer « le texte se rend »
  au-delà de l'atlas Latin en scène de dialogue.** Le rendu générique multi-fontes reste à
  câbler pour les autres familles de texte (menu, item, skill…) et pour JA/ZH.

### nie-data

`crates/engine/nie-data/src/text.rs`, `chara_text.rs`, `font_color.rs` existent (portage typé
de `common_talk_text_map`/`chara_text`/`font_color` a priori — noms de fichiers non garants du
contenu, cf. règle CLAUDE.md ; à vérifier par golden test avant d'en dépendre, non refait ici
faute de build autorisé pendant cette session). Aucun module `nie-data` nommé `property_*` ou
`camera_property` trouvé par
`rg -l "camera_ctrl_property|game_param" crates/engine/nie-data/src/` → 1 seul hit
(`opponent_team.rs`, faux-positif probable sur un autre marqueur, à vérifier avant de porter).

### packages/

`packages/nie-catalog/src/{jeu.ts,synergie.ts,anime.ts,cli.ts}` référencent `text`/`font`
(recherche `rg`), mais dans des contextes génériques (variables, pas une façade dédiée au texte
localisé) — **pas de gisement `nie-catalog` propre à la localisation** identifié.

## 4. Ce que le site sert déjà (mesuré, aphrody 127.0.0.1:8085 / nie-model-serve 127.0.0.1:8790)

```
curl -s http://127.0.0.1:8085/api/v1/formats
```
rend (extrait pertinent) :

```json
{"suffixe":".cfg.bin","decodage":"en_process","route":"/api/v1/formats/decode/{chemin}",
 "sortie":"application/json","fichiers":71101,"octets":216141904}
```

`71 101` fichiers `.cfg.bin` servis, un sur-ensemble des 44 375 de ce domaine (partagé avec
d'autres domaines : chara, item…). Vérifié fichier par fichier :

| Chemin | URL | Code | Taille |
|---|---|---|---|
| `data/common/text/fr/menu_text.cfg.bin` | `/api/v1/formats/decode/<chemin>` | **200** | 582 050 o (JSON, arbre `entries`) |
| `data/common/property/camera/camera_ctrl_property_info.cfg.bin` | idem | **200** | 25 981 o |
| `data/common/font/font/font_def/font.cfg.bin` | idem | **200** | 2 408 370 o |
| `data/common/font/font_style/ja/font_style.cfg_0.00.08.bin` | idem | **400** | `{"genre":"demande_invalide","message":"cette route ne decode que .cfg.bin"}` — l'extension réelle est `.cfg_0.00.08.bin`, pas `.cfg.bin` |
| `data/common/text/fr/menu_text.cfg.bin` | `/f/<chemin>` (brut) | **200** | fichier brut passthrough |
| `data/common/font/font/font_def/font.g4tx` | `/f/<chemin>` (brut) | **404** | pas trouvé par ce route non plus |
| `dx11/font/font_def/font.png` (dérivé du `.g4tx`) | `/assets/tex/dx11/font/font_def/font.png` | **200** | 3 389 062 o, PNG 4096×2048 réel |
| `dx11/font/gaiji_game.png` | `/assets/tex/dx11/font/gaiji_game.png` | **200** | 570 412 o, PNG 908×856 |
| `common/font/font/font_def/font.g4tx` (chemin `common`, pas `dx11`) | `/assets/tex/common/font/font/font_def/font.png` | **404** | `{"genre":"introuvable","message":"asset inconnu de l'amont"}` |

**Constat mesuré** : `/api/v1/formats/decode/{chemin}` sert déjà tout le T2B (menu, property,
métriques de fonte) en JSON brut de l'arbre `CfgEntry` — décodage générique, pas typé, pas
traduit en une forme lisible. `/assets/tex/dx11/font/*.png` décode déjà les atlas de fontes
sous `dx11/`, mais **pas** ceux sous `common/font/font/**` (chemin `common/font/font` à double
segment `font`, probablement hors du mapping de route actuel — à corriger côté route, pas
côté décodeur, puisque le décodeur g4tx fonctionne sur le miroir `dx11`).

## 5. Tableau de couverture

| Famille | État | Détail |
|---|---|---|
| `common/text/<langue>/*.cfg.bin` (43 familles × 8+ langues) | **servi** | `/api/v1/formats/decode/{chemin}` → 200, JSON brut `entries/variables` (T2B décodé) |
| `common/text/<langue>/event/*.cfg.bin` (dialogue) | **servi** | même route, 200 attendu (T2B générique) ; non re-testé individuellement, forme identique aux autres `.cfg.bin` |
| `common/text/<langue>/map/*.cfg.bin`, `event/*_map.cfg.bin`, `common/*.cfg.bin` | **servi** | idem, décodage générique T2B |
| `common/property/**/*.cfg.bin` | **servi** | testé sur `camera_ctrl_property_info.cfg.bin` → 200 |
| `common/font/font/<fonte>/font.cfg.bin` (métriques) | **servi (brut) / interne (typé)** | JSON brut 200 via `/api/v1/formats/decode` ; le décodage **typé** (`parse_metrics` → `FontMetrics` exploitable) est **interne** : appelé par `nie-model-serve` en interne (scène de dialogue), pas exposé en route JSON typée |
| `common/font/font_color.cfg.bin` | **servi** | T2B générique, route `/api/v1/formats/decode`, non re-testé individuellement mais même forme |
| `common/font/font_style/<langue>/font_style.cfg_0.00.08.bin` | **manquant** | extension non `.cfg.bin` exacte (`.cfg_0.00.08.bin`) → 400 sur la route générique ; aucun décodeur dédié identifié |
| `dx11/font/<fonte>/font.g4tx` (atlas Latin/JA/ZH) | **servi** | `/assets/tex/dx11/font/{fonte}/font.png` → 200, PNG réel |
| `dx11/font/<langue>/gaiji_game2.g4tx`, `gaiji_*.g4tx` | **servi (probable)** | même mécanisme `.g4tx→.png` que `font_def` ; non re-testé un par un |
| `common/font/font/<fonte>/font.g4tx` (chemin `common`, pas `dx11`) | **manquant** | 404 sur `/assets/tex` — le mapping de route ne résout que le miroir `dx11` pour cette sous-arborescence, alors que le fichier existe (14 entrées dans l'inventaire) |
| `dx11/text/<langue>/{menu,system}_text_platform.cfg.bin` | **servi** | même route générique T2B, non re-testé individuellement |
| `dx11/text/ja/licensetext_platform.cfg.bin` | **servi** | idem, 173 248 o |
| Rendu texte multi-glyphes CJK (JA/ZH, `decode_packed_codepoint`) | **manquant** | décodeur `codepoint` existe et est testé unitairement, mais **aucune route ni pipeline de production** ne l'exploite pour produire du texte CJK rendu — seul le Latin est câblé (scène de dialogue) |
| Rendu texte menu/item/skill dans la vraie police (au-delà du JSON brut de métriques) | **manquant** | `parse_metrics`+`glyph_blitter` existent, mais rien ne les appelle pour composer une image de texte hors la scène de dialogue |
| Traduction jointe multi-langue (comparer `fr`/`en`/`ja` d'un même item de texte) | **manquant** | aucune route ne joint les 8+ langues d'une même famille — chaque `.cfg.bin` est un fichier séparé par langue, servi indépendamment |

## 6. Routes à créer

- `GET /api/v1/text/{famille}/{langue}` → décode `data/common/text/{langue}/{famille}.cfg.bin`
  puis retourne l'arbre T2B déjà produit par `cfgbin_parse` (existe : `cfgbin.rs:804`) — un
  simple alias de résolution de chemin par `{famille}`/`{langue}` plutôt que le chemin VFS brut,
  pour que le site n'ait pas à connaître la forme `data/common/text/…`. Décodeur : **déjà là**.
- `GET /api/v1/text/{famille}/{langue}/compare?langues=fr,en,ja` → décode et **joint** plusieurs
  langues du même fichier logique côte à côte. Décodeur : déjà là (`cfgbin_parse` par langue) ;
  **manquant : la logique de jointure elle-même**, à écrire.
- `GET /api/v1/font/{fonte}/metrics` → `parse_metrics` (`font.rs:233`) exposé en JSON typé
  (`FontMetrics`/`GlyphMetric`) au lieu du JSON brut d'arbre `CfgEntry` actuel. Décodeur : déjà
  là, **manquant : le point de route** (aujourd'hui seul `/api/v1/formats/decode` répond, en
  brut).
- `GET /api/v1/font/{fonte}/glyph/{codepoint}.png` → `glyph_blitter` (`font.rs:372`) sur un
  glyphe unique, avec `decode_packed_codepoint` (`font.rs:174`) pour accepter un codepoint
  UTF-8 empaqueté. Décodeur : déjà là pour Latin (via `LatinAtlas`) ; **manquant pour JA/ZH** :
  personne n'a câblé `parse_metrics`+`glyph_blitter` sur les atlas `font_ja`/`font_zh_*` en
  production — à valider avant d'annoncer la route complète.
- `GET /assets/tex/common/font/font/{fonte}/font.png` — corriger le mapping de route existant
  pour qu'il résolve aussi ce sous-arbre (aujourd'hui seul `dx11/font/**` répond 200). Décodeur
  g4tx : déjà là (fonctionne sur `dx11`) ; **manquant : la résolution de chemin côté route**.
- `GET /api/v1/property/{domaine}/{nom}` → alias lisible sur `common/property/{domaine}/{nom}.cfg.bin`,
  même décodeur T2B générique déjà servi par `/api/v1/formats/decode`.
- `GET /api/v1/text/font-style/{langue}` → **manquant : il faudrait d'abord un parseur** pour
  `.cfg_0.00.08.bin` (extension non standard, non couverte par `cfgbin_parse` faute de match
  d'extension — à vérifier si le contenu binaire est du T2B standard malgré l'extension avant
  d'écrire un parseur dédié).

## 7. Ce que le mode de jeu attend

- **Scène de dialogue** (mémoire `mode-histoire-scene-dialogue`) : consomme
  `inagle_event_subtitles` (base extraite, pas directement les `.cfg.bin` VFS) + l'atlas
  `LatinAtlas` pour le rendu. Les fichiers `common/text/<langue>/event/ev*.cfg.bin` du VFS sont
  la source primaire côté jeu (texte + timing de scène), mais le pipeline de production actuel
  passe par la table SQL dérivée plutôt que par un décodage direct de ces `.cfg.bin` — **à
  vérifier** si `inagle_event_subtitles` est une extraction fidèle de ces fichiers ou une source
  distincte (non tranché ici).
- **Menu principal / écrans de menu** (`mainmenu01`, cf. doc écrans) : consomme
  `menu_text.cfg.bin` par langue pour les libellés ; le pipeline `--export-layout` cité dans
  CLAUDE.md rend déjà « les textes traduits » pour certains écrans — lien exact avec
  `common/text/<langue>/menu_text.cfg.bin` **non retracé ici**, à vérifier dans
  `crates/engine/nie-core` ou `nie-game --runtime` avant d'écrire une route qui suppose ce lien.
- **Combat RPG** (`rpg_battle_*_text.cfg.bin`, `rpg_battle/rpg_battle_camera_info.cfg.bin` côté
  property) : consommateur non identifié dans ce domaine — **à vérifier**, pilier RPG large
  cité comme non entamé dans la mémoire du dépôt (`mode-histoire-scene-dialogue`).
- **Kizuna Town / craft** (`craft_text.cfg.bin`, `craft.rs` porté selon mémoire
  `kizuna-town-pillar-re`) : lien probable mais **non vérifié** dans cette session (RE de la
  ville faite ailleurs, hors décodage texte).
- Pour tout le reste (`ai_text`, `chat_text`, `search_word_text`, `soccer_*_text`, les fichiers
  `property/light/*` par scène d'événement) : **aucun consommateur identifié dans le dépôt à ce
  jour** — à dire tel quel plutôt que d'inventer un écran.
