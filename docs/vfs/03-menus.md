# Domaine VFS 3 — Menus et interface (44 612 fichiers)

Inventaire source : `var/vfs/lot3-menu.txt` (`chemin taille [cpk]`), 44 612 lignes,
`data/dx11/menu/` + `data/common/menu/`. Toutes les mesures ci-dessous sont rejouables sur ce
fichier ou par requête HTTP live (commande citée à chaque fois).

## 1. Chiffres

```
wc -l var/vfs/lot3-menu.txt                                          → 44 612 fichiers
awk '{s+=$2} END{print s}' var/vfs/lot3-menu.txt                     → 22 236 573 774 o (20,71 GiB)
```

Ventilation par extension (`awk -F. '{print $NF}' … | awk '{print $1}' | sort | uniq -c`) :

| Extension | Fichiers | Rôle |
|---|---|---|
| `.g4tx` | 41 191 | atlas de texture (format Level-5, décodé par `nie-formats::g4tx`) |
| `.g4pkm` | 1 705 | layout/packing d'un atlas (position des sprites dans le g4tx jumeau) |
| `.g4mg` | 1 705 | mesh/geometry menu (compagnon du `.g4pkm`, un couple par écran de `common/menu`) |
| `.g4pk` | 4 | variante g4pk (à confirmer au cas par cas, non prioritaire vu le volume) |
| `.g4tg` | 1 | singleton, à vérifier avant de le généraliser |
| `.bin` | 6 | fichiers hors format nommé, à identifier par `xxd`/`hexyl` avant de les router |

Ventilation par montage (`awk '{n=split($1,a,"/"); print a[2]"/"a[3]} … | sort | uniq -c`) :

| Montage | Fichiers |
|---|---|
| `dx11/menu` | 41 192 — toutes les textures et layouts dépendant de la résolution/API |
| `common/menu` | 3 420 — les `.g4mg`/`.g4pkm` indépendants du renderer (00_soccer, etc.) |

Ventilation par famille `NN_xxx` sous `dx11/menu` (`awk '{n=split($1,a,"/"); if(a[2]=="dx11") print a[4]…}'`),
les 10 plus grosses :

| Famille | Fichiers | Contenu |
|---|---|---|
| `200_icon` | 19 534 | icônes (emblèmes, items, rareté, compétences…), plates, sans sous-écran |
| `220_img` | 17 085 | images plates (photos d'activité, illustrations) |
| `00_soccer` | 1 133 | HUD de match (`common/menu`, `.g4mg`+`.g4pkm`) |
| `102_team` | 412 | éditeur d'équipe / cartes personnage |
| `02_btl` | 398 | combat/tactiques |
| `31_universe` | 367 | univers/monde |
| `10_win` | 281 | fenêtres génériques (popups, listes) |
| `75_vroad` | 239 | mode Victory Road |
| `107_vs` | 215 | écrans de mise en versus |
| `100_mainmenu` | 186 | **menu principal — DA de référence d'Aphrody** |

Le reste (161_avatar, 160_town, 91_quest, 150_shop, 108_option, 105_datafile, 210_minimap,
103_item, 115_calendar, 90_map, 106_info, 122_opponent, 150_list, 121_inacord, 55_information,
113_activity, 125_multi, 100_topmenu, 109_system, 60_story, 51_save, 13_map, 112_gallery,
901_temp, 600_event, 110_playguide, 163_help, 11_loading) totalise le solde à 41 192.

## 2. Grammaire des chemins

Forme dominante, vérifiée sur `loading01` :

```
data/dx11/menu/<NN_famille>/<ecran>/<ecran_NN>/<ecran_NN>.g4tx
```

Exemple réel : `data/dx11/menu/11_loading/loading01/loading01_01/loading01_01.g4tx`.

Profondeur mesurée (`rg '^data/dx11/menu/' … | awk -F/ '{print NF}' | sort | uniq -c`) :

| Profondeur (nb de `/`) | Fichiers | Forme |
|---|---|---|
| 6 | 5 771 | `<NN_famille>/<fichier>.ext` — familles plates (200_icon, 220_img : pas de sous-dossier écran) |
| 7 | 32 338 | `<NN_famille>/<ecran>/<ecran_NN>/<ecran_NN>.ext` — forme dominante |
| 8 | 3 083 | `<NN_famille>/<ecran>/<ecran_NN>/<locale>/<ecran_NN>.ext` — variante localisée |

Sous `common/menu` (`rg '^data/common/menu/' … | awk -F/ …`) : profondeur 7 dominante (3 376),
5 et 8 marginales (6 + 38). Forme : `data/common/menu/<NN_famille>/<ecran>/<ecran_NN>/<ecran_NN>.g4mg|.g4pkm`
(exemple : `data/common/menu/00_soccer/soccer00/soccer00_01/soccer00_01.g4mg`).

**Exceptions observées :**
- `200_icon` et `220_img` (36 619 fichiers à eux deux, 82 % du domaine) **n'ont pas** de
  sous-dossier `<ecran>` : `data/dx11/menu/200_icon/01_icon_emblem/em0001.g4tx`,
  `data/dx11/menu/220_img/activity_photo/activity_note_001.g4tx`. Ce sont des **atlas plats**,
  pas des écrans — à traiter comme une galerie, jamais comme des layouts.
- Localisation à la **profondeur 8**, sous deux formes distinctes :
  - un dossier `<locale>` intercalé entre l'écran et le fichier :
    `dx11/menu/100_mainmenu/mainmenu04/mainmenu04_00_2/de/mainmenu04_00_2.g4tx` (8 locales
    trouvées : `de en es fr it pt zh_hans zh_hant` — pas de `ja`/`ko` dans ce domaine) ;
  - le fichier neutre **coexistant** avec ses variantes localisées dans le même dossier
    d'écran (`160_town/town01/town01_01/{de,en,…}/town01_01.g4tx` **et**
    `160_town/town01/town01_01/town01_01.g4tx` au même niveau) — la locale par défaut n'est
    pas dans un sous-dossier `fr` mais posée nue à côté des autres locales.
- `11_loading` ne contient **qu'un seul fichier** (`loading01_01.g4tx`, 107 104 o) — la famille
  citée en exemple dans la mission est en réalité la plus petite du domaine.

## 3. Inventaire des écrans

Comptage des couples `(famille, écran)` à profondeur 2 sous chaque racine :
```
awk '{n=split($1,a,"/"); if(a[2]=="dx11") print a[4]"|"a[5]; else print a[2]"|"a[4]}' \
  var/vfs/lot3-menu.txt | sort -u | wc -l
```
→ **305 couples distincts** dans l'inventaire de fichiers (dont les 36 619 fichiers plats de
`200_icon`/`220_img`, comptés comme un seul « écran-répertoire » par sous-dossier de thème,
gonflent artificiellement le compte — un « écran » y désigne un thème d'icônes, pas un layout).

Ceci est le compte de **répertoires**, pas d'écrans logiques. La source de vérité des écrans
logiques est le runtime lui-même :

```
curl -s http://127.0.0.1:8790/menu-tree.json | jq '.count'   → 475
```

Le commentaire source (`crates/tools/nie-model-serve/src/main.rs:5064-5067`) documente
« 440 `*_setting`, dont 304 `*_menu_setting` + fenêtres/sélecteurs » ; la mesure live du
2026-09-06 rend **475** (`screens[]`), à préférer au chiffre commenté (le corpus de
`*_setting.cfg.bin.json` a pu croître depuis l'écriture du commentaire — ne pas trancher sans
revérifier `sel.is_empty()` à la prochaine session).

**Piège vérifié en direct** — un écran = un calque (layout statique) + un comportement
(script Lua), rarement les deux sous le même nom :

| Écran | Objets de calque | Scripts Lua | Détail mesuré |
|---|---|---|---|
| `mainmenu01` | **34** (26 sprites statiques) | **0** (`aucun script .lua.bin pour l'écran 'mainmenu01'`) | export réel : `nie-game --runtime --menu mainmenu01 --export-layout` → JSON complet, transform+texte+sprite pour chacun des 34 objets |
| `kizuna_town_mainmenu` | **0** objet visible | **1** (`kizuna_town_mainmenu_5.00.11.00.lua.bin`, `on_open=true layers=2 objects=5 known=66 unknown=20`) | le driver Lua tourne (`OnInit`/`OnOpenLayer` exécutés, 66 appels de commande connus : `SetObjectVisible`, `SetIconSprite`, `GetText`…) mais produit **0 objet dans le JSON exporté** |

Cause du deuxième cas, vérifiée dans le message d'avertissement du binaire lui-même
(`crates/engine/nie-game/src/main.rs:3078`, fonction `cmd_export_layout_runtime`) :
> « 0 objet muté par le runtime. Le chemin driver → MenuState → layout est CÂBLÉ et exécute
> les vrais scripts, mais `GetItemButtonNum` (fonction DU script) lit l'état scène/save C++
> que niers ne fournit pas encore ⇒ `OnSetupLayer` crée 0 objet. »

`GetItemButtonNum` est implémentée dans `crates/engine/nie-lua/src/menu_host.rs:1799-1976`
(commentaire `:397-399` : « le nombre d'items vient du SCRIPT, pas (seulement) du save-state »).

**Objets muets et non positionnés — mesurés sur `mainmenu01.json` :**
```
jq '[.objects[] | select(.sprite==null)] | length'    → 8   (objets sans texture assignée)
jq '[.objects[] | select(.transform==null)] | length'  → 0   (tous ont une position)
jq '[.objects[] | select(.text|length==0)] | length'   → 26  (26/34 n'ont aucun libellé texte)
```
Ce sont des manques réels de l'export (piste ouverte pour compléter `menu_host.rs`), pas des
absences dans le jeu — à ne jamais présenter comme « l'écran n'a que 26 objets textuels ».

Une deuxième source d'écrans, indépendante des calques : `/menu-tree.json` (port 8790, 475
écrans) donne pour **chaque** écran son `crc32` de nom, ses `layers[]` (hash CRC32, nom,
`objbin` source — `common/gamedata/menu/obj/*.objbin`) et ses `commands[]`
(`CMD_BACK`/`CMD_ENTER`/`CMD_FUNCTION` avec `commandHash`/`layerHash`). Exemple mesuré :
l'écran `ability_learning_board_menu` a 63 layers et 49 commandes, `consistent: true` (chaque
`hash == CRC32(name)`, preuve byte-exacte de la table de navigation). C'est la carte de
**navigation** (quel écran mène à quel autre) ; `--export-layout` est la carte de **rendu**
(quoi dessiner) ; les deux sont complémentaires et aucune des deux seule ne suffit à rejouer
un écran.

## 4. Ce que le dépôt sait déjà faire

| Capacité | Fichier:ligne |
|---|---|
| Détection/parse d'un atlas `.g4tx` | `crates/engine/nie-formats/src/g4tx.rs:153` (`is_g4tx`), `:379` (`parse`) |
| Décodage RGBA/PNG d'une texture d'atlas | `crates/engine/nie-formats/src/g4tx_decode.rs:136` (`decode_texture_rgba`), `:197`/`:205` (`decode_best_to_rgba`/`_png`), `:235`/`:255` (`decode_named_to_*`) |
| Placement d'un sprite sur le canevas (transform, ancre, échelle) | `crates/engine/nie-formats/src/menu.rs:37` (`ScreenTransform`), `:60` (`place_on_canvas`), `:158` (`PositionedMenuObject`), `:267`/`:277` (`compose`/`compose_over`) |
| Points d'attache (`CMenuAttachLocator`, os du squelette) | `crates/engine/nie-formats/src/menu.rs:388` (`AttachSlot`), `:454` (`attach_slots`) — cf. mémoire `attach-locator-position-widgets` |
| Hôte Lua du menu (état, commandes, drive des écrans) | `crates/engine/nie-lua/src/menu_host.rs:211` (`MenuState`), `:960` (`install_menu_host`), `:1976` (`run_menu`), `:2159` (`drive_menu_for_frames`) |
| Export de la disposition réelle d'un écran (CLI) | `crates/engine/nie-game/src/main.rs:2537` (`cmd_export_layout`, statique), `:3078` (`cmd_export_layout_runtime`, piloté Lua) |
| Composition PNG à partir d'un JSON de layout | option `--compose-layout` du même binaire (`nie-game`) |
| Rendu React d'une disposition exportée | `packages/inacord-ui/src/shell/layout-render.tsx:260` (`LayoutRender`), `:77` (`GameCanvas`), `:50` (`useEchelleCanvas`) |
| Géométrie spécifique du menu principal | `packages/inacord-ui/src/shell/geometrie-mainmenu.ts` (143 l.) |
| Layout générique jeu (hors mainmenu) | `packages/inacord-ui/src/shell/layout-jeu.ts` (356 l.) |
| Arbre de navigation (crc32, layers, commandes) par écran | `crates/tools/nie-model-serve/src/main.rs:5064` et suivantes, port `objbin`/`nie_data::menu_setting` |
| Décodage `.objbin` (les layers cités par `/menu-tree.json`) | `crates/engine/nie-formats/src/objbin.rs` |

## 5. Ce que le site sert déjà (mesuré en direct, 2026-09-06)

Deux services distincts répondent sur cette machine :
- `nie-site` (crate Rust servant Aphrody), `127.0.0.1:8085`
- `nie-model-serve`, `127.0.0.1:8790` — routes `/tex`, `/menu-tree`, `/vfs`

```
curl -s http://127.0.0.1:8085/api/v1/formats
```
→ `{"service":"nie-site","version":"0.5.9","vfs_pret":true,"vfs_entrees":255308,…}`.
Entrée `.g4tx` : `{"decodage":"delegue","route":"/assets/tex/{chemin}.png","sortie":"image/png",
"fichiers":54203,"octets":82004879296}` — chiffre couvrant tout le VFS (menus + autres domaines
texture), pas seulement ce lot de 44 612.

```
curl -s -o /dev/null -w "%{http_code} %{size_download}\n" \
  "http://127.0.0.1:8085/assets/tex/dx11/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.png"
```
→ `200 131` — code 200, PNG valide (signature `\x89PNG`, `IHDR`/`IDAT`/`IEND` vérifiés en tête
d'octets), 131 o (texture minuscule ici : la source `.g4tx` fait 2 960 o pour un sprite
4×92 px de texte de guide, cohérent). **Convention d'URL vérifiée** : le `logicalPath` du JSON
exporté (`dx11/menu/100_mainmenu/mainmenu01/mainmenu01_06/mainmenu01_06.g4tx`) perd son suffixe
`.g4tx` et gagne le préfixe `/assets/tex/` pour devenir l'URL — jamais l'inverse (pas de
`data/` dans l'URL, l'extension `.g4tx` disparaît toujours).

```
curl -s -o /dev/null -w "%{http_code} %{size_download}\n" http://127.0.0.1:8790/menu-tree.json
```
→ `200 1 255 162` (1,2 Mio, 475 écrans complets).

```
curl -s -o /dev/null -w "%{http_code} %{size_download}\n" \
  http://127.0.0.1:8790/menu-tree/mainmenu01.json
```
→ `404 18` — `mainmenu01` n'est **pas** une clé de `/menu-tree/<nom>` : le sélecteur attend un
`stem` de fichier `*_setting.cfg.bin.json` (ex. `ability_learning_board`), pas le nom du calque
`--export-layout`. **Piège confirmé une deuxième fois** : le nommage « écran » diverge selon la
source (calque vs script vs `*_setting`) — toute route qui accepte un nom d'écran doit
documenter LEQUEL des trois espaces de noms elle attend.

`GET /menu-tree.json` sous `127.0.0.1:8085` (nie-site) répond 200 mais rend la **page HTML**
d'exploration du VFS (`<title>menu-tree.json — Aphrody</title>`), pas le JSON — cette route
n'existe que côté `nie-model-serve` (8790), Aphrody ne l'a pas encore relayée.

## 6. Tableau de couverture (matrice `docs/PLAN-SITE-ULTIME.md` §4)

| Famille | État | Détail |
|---|---|---|
| Textures `.g4tx` isolées (icônes, images plates) | **servi** | `/assets/tex/<chemin>.png` — `200`, mesuré ci-dessus |
| Layout statique d'un écran à calque (ex. `mainmenu01`) | **interne** | `nie-game --export-layout` produit le JSON complet (34/34 objets positionnés), mais aucune route HTTP ne l'expose encore côté Aphrody — seul `/menu-tree.json` (nav) est servi |
| Comportement piloté-script d'un écran (ex. `kizuna_town_mainmenu`) | **manquant** | le driver Lua tourne et compte les commandes, mais produit 0 objet visible (`GetItemButtonNum` sans état scène/save C++) — rien à servir tant que la couche donnée n'existe pas |
| Table de navigation (crc32, layers, commandes) | **servi** (model-serve seulement) | `/menu-tree.json` → 200, 475 écrans ; **manquant** côté `nie-site`/Aphrody |
| Composition PNG d'un écran depuis son layout | **interne** | `--compose-layout` existe en CLI, jamais exposé en route |
| `.g4mg`/`.g4pkm` (`common/menu`, HUD de match) | **manquant** | aucun décodeur/route identifiés dans ce passage — à vérifier dans `nie-formats` avant d'écrire une route (pas confirmé ici, ne pas affirmer l'absence de code) |
| `.g4pk`/`.g4tg` (10 fichiers au total) | **manquant** | volume négligeable, à router seulement si un besoin concret apparaît |
| Localisation (8 locales, profondeur 8) | **interne** | le JSON exporté porte déjà `"locale":"fr"` (piloté par le runtime), mais aucune route ne permet de choisir la locale servie |

## 7. Routes à créer (une par capacité réelle, aucune sans décodeur)

| Route | Contenu | Décodeur/parseur existant |
|---|---|---|
| `GET /api/v1/menu/screens` | relais du `/menu-tree.json` de model-serve (475 écrans, nav+layers+commandes) | `nie-model-serve` port 8790, déjà fonctionnel — relayer, pas réécrire |
| `GET /api/v1/menu/screens/{stem}` | un écran de nav (`/menu-tree/{stem}.json`) — **documenter que `{stem}` est le nom du `*_setting.cfg.bin.json`, pas celui du calque** | idem, sélecteur déjà câblé (`crates/tools/nie-model-serve/src/main.rs:5064` et suite) |
| `GET /api/v1/menu/layout/{ecran}` | layout statique d'un écran à calque (objets, transform, sprite, texte) — `{ecran}` est ici le nom du calque (`mainmenu01`), **troisième espace de noms**, à ne jamais confondre avec `{stem}` ci-dessus | `nie-game --runtime --menu {ecran} --export-layout` (`crates/engine/nie-game/src/main.rs:3078`) — à faire tourner en bibliothèque plutôt qu'en sous-process pour une route HTTP |
| `GET /api/v1/menu/layout/{ecran}.png` | composition PNG de l'écran | `--compose-layout` (même binaire) |
| `GET /assets/tex/{chemin}.png` | **déjà servi**, vérifié 200 sur `mainmenu01_06` — rien à créer, juste à documenter dans le cahier des charges comme référence de convention | `nie-site`, `crates/engine/nie-formats/src/g4tx_decode.rs:197` |
| `GET /api/v1/menu/icons/{theme}` | galerie plate d'un thème `200_icon/<theme>/*.g4tx` (19 534 fichiers, pas de layout — juste une liste de textures) | même décodeur g4tx, pas de layout à charger |
| `GET /api/v1/menu/images/{theme}` | idem pour `220_img` (17 085 fichiers) | idem |

## 8. Combien d'écrans le site pourrait servir

- Table de **navigation** : 475/475 accessibles dès qu'`/api/v1/menu/screens` relaie
  `nie-model-serve` — c'est un relais pur, 0 décodage à écrire.
- Table de **rendu** (layout réellement dessinable) : seuls les écrans à calque statique
  (type `mainmenu01`) produisent un JSON complet aujourd'hui. Le compte exact d'écrans « à
  calque, 0 script » contre « à script, 0 calque statique » **n'a pas été mesuré ici** — il
  faudrait faire tourner `--export-layout` sur les 475 stems et compter `objets>0` vs
  `objets==0`, ce qui dépasse le périmètre d'écriture de ce document (aucun script). À faire
  avant d'écrire les routes du §7 pour chiffrer précisément `servis / 475` (mission demandait
  `/440`, la mesure live des écrans donne 475 — préférer 475, plus récent).
- Chiffre honnête à ce stade : **1/475 vérifié en profondeur** (`mainmenu01`, 34/34 objets
  positionnés, 26/34 avec texte) + **1/475 vérifié en échec documenté**
  (`kizuna_town_mainmenu`, 0/5 objets rendus, cause connue et non un bug de mesure). Le reste
  est à passer au même protocole (`--export-layout` + comptage `sprite==null`/`text==0`) avant
  toute annonce de couverture globale.
