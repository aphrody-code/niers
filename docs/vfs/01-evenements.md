# Domaine 1 — Événements, scénario, scripts

Cahier des charges des routes Aphrody pour `data/common/event/`, `data/common/event_cfg/`,
`data/common/script/`. Toute mesure ci-dessous est reproductible par la commande citée.
Inventaire source : `var/vfs/lot1-event.txt` (une ligne `chemin taille [cpk]`).

## 1. Les chiffres

```
wc -l var/vfs/lot1-event.txt
```
→ **67 084 fichiers**.

```
awk '{s+=$2} END{print s}' var/vfs/lot1-event.txt
```
→ **2 036 064 115 octets** (≈ 1,90 Gio).

Ventilation par extension (`awk '{n=split($1,a,"."); print a[n]}' var/vfs/lot1-event.txt | sort | uniq -c | sort -rn`) :

| Extension | Fichiers |
|---|---:|
| `.g4pk` | 45 021 |
| `.bin` (générique, `.cfg.bin`/`.mevbin` non isolés par ce split) | 20 599 |
| `.g4cm` | 1 215 |
| `.mevbin` | 95 |
| `.g4mt` | 53 |
| `.g4ma` | 35 |
| `.objbin` | 14 |
| `.g4pkm` | 14 |
| `.g4mg` | 14 |
| autres (`r41152`, `r41000`, `g4vs`, `g4la`, `r47929`, `r47819`, `g4sk`, `r51528`, `r51265`) | ≤ 4 chacun |

Le split sur `.` fragmente `.cfg.bin` (double extension) — voir plus bas par nom de fichier :

```
grep -c '_snd\.cfg\.bin$'   var/vfs/lot1-event.txt   # 3 899 (event_cfg/snd)
grep -c '_eff\.cfg\.bin$'   var/vfs/lot1-event.txt   # 3 911 (event_cfg/eff)
grep -c 'EventMap_fix.*\.cfg\.bin$' var/vfs/lot1-event.txt
grep -c 'light_list\.cfg\.bin$'     var/vfs/lot1-event.txt
grep -c '\.lua\.bin$' var/vfs/lot1-event.txt   # 651 (script/lua)
```

Ventilation par sous-dossier (`awk '{print $1}' … | sed -E 's#(data/[^/]+/[^/]+/[^/]+/?).*#\1#' | sort | uniq -c | sort -rn`) :

| Dossier | Fichiers |
|---|---:|
| `data/common/event/ev60/` | 16 529 |
| `data/common/event/ev62/` | 9 711 |
| `data/common/event/ev63/` | 9 469 |
| `data/common/event/ev61/` | 8 382 |
| `data/common/event_cfg/eff/` | 3 911 |
| `data/common/event_cfg/snd/` | 3 899 |
| `data/common/event/ev72/` | 2 197 |
| `data/common/event_cfg/evt/` | 2 088 |
| `data/common/event/ev74/` | 1 615 |
| `data/common/event/ev71/` | 1 005 |
| `data/common/event/ev99/` | 738 |
| `data/common/script/lua/` | 651 |
| `data/common/event/ev05/`, `ev09/`, `ev04/`, `ev01/`, `ev06/`, `ev07/`, `ev03/`, `ev08/`, `ev_mot/`, `ev02/`, `ev29/`, `ev80/`, `ev75/`, `ev81/`, `ev65/`, `ev20/`, `ev50/` | 93 à 583 chacun |
| `data/common/event_cfg/other/` | 84 |

`ev60`–`ev63`, `ev71`/`ev72`/`ev74` concentrent 47 303 fichiers (70 % du domaine) : ce sont les
cinématiques longues (probablement les scènes de match/histoire les plus denses — **à vérifier**,
aucune table ne nomme ces préfixes à ce jour dans le dépôt).

## 2. La grammaire des chemins

Exemples pris tels quels dans l'inventaire (aucun chemin inventé) :

```
data/common/event/ev01/ev01_00250/ev01_00250_c11010010_s00_p00_c0010.g4pk
data/common/event/ev01/ev01_00250/ev01_00250_camera.g4cm
data/common/event/ev01/ev01_00250/ev01_00250_point_eff_c0010.g4pk
data/common/event/ev01/ev01_00250/ev01_00250_point_s00_c0010.g4pk
data/common/event/ev47/ev47_30600/ev47_30600_light/EventMap_fix_c0010.cfg.bin
data/common/event/ev60/ev60_10200/ev60_10200_light/light_list.cfg.bin
data/common/event_cfg/snd/ev00_43370_snd.cfg.bin
data/common/event_cfg/eff/ev29_04220_eff.cfg.bin
data/common/script/lua/action/ball_chest_chara_1.00.07.lua.bin
data/common/event/ev72/ev72_50010/ev72_50010_c11010019_s00_p00_c0140.mevbin
```

Structure lisible :

- **`evNN`** : famille d'événement (`ev01`…`ev99`), un dossier par famille sous `data/common/event/`.
  `ev00` n'apparaît qu'en préfixe de fichier dans `event_cfg/` (pas de dossier `ev00/` propre).
- **`evNN_XXXXX`** : une scène numérotée (5 chiffres, ex. `ev01_00250`, `ev60_10200`) — un dossier
  par scène, qui regroupe tous ses fichiers.
- À l'intérieur d'une scène, le nom de fichier reprend le préfixe `evNN_XXXXX_` puis un rôle :
  - `_c<idpersonnage><s..>_s<NN>_p<NN>_c<NNNN>.g4pk` — un paquet de scène par acteur/segment/prise
    (`c11010010` ressemble à un identifiant de personnage/costume au format du VFS des persos,
    `s00`/`p00` à un segment/prise, `c0010` à une caméra ou un compteur — **la sémantique précise
    de chaque champ n'est pas prouvée ici**, seule la position syntaxique l'est).
  - `_camera.g4cm` — animation de caméra de la scène (un seul fichier par scène en général, 1 215
    au total pour 67 084 fichiers : loin d'une scène par caméra).
  - `_point_s<NN>_c<NNNN>.g4pk` / `_point_eff_c<NNNN>.g4pk` — points d'ancrage (locators) ou
    d'effets, mêmes conventions de segment.
  - `_light/EventMap_fix_c<NNNN>.cfg.bin`, `_light/light_list.cfg.bin` — sous-dossier `_light/`
    avec la configuration d'éclairage de la scène.
- **`event_cfg/<rôle>/evNN_XXXXX_<rôle>.cfg.bin`** : configuration transverse, groupée par rôle
  et non par scène — `snd` (son), `eff` (effets), `evt` (déroulé/logique), `other`.
- **`script/lua/<catégorie>/<nom>_<version>.lua.bin`** : scripts Lua compilés, catégorisés par
  sous-dossier (`action/` vu ici), avec un numéro de version à 2 points comme partout ailleurs
  dans le VFS (`1.00.07`) — jamais un nom de fichier sans version pour ces scripts.
- **`.mevbin`** : coexiste avec `.g4pk` au même niveau de scène (`ev72_50010_c11010019_s00_p00_c0140.mevbin`
  a le même schéma de nommage qu'un `.g4pk`) — probablement une variante d'encodage du même rôle
  (motion/événement), **à vérifier par un parseur avant affirmation**.

## 3. Ce que le dépôt sait déjà décoder

Recherche par marqueur (les modules `nie-data` sont nommés par concept, pas par extension) :

| Extension/rôle | Parseur | Preuve |
|---|---|---|
| `.g4pk` | `G4pk::parse` | `crates/engine/nie-formats/src/g4pk.rs:137` (structs `G4pkHeader:65`, `G4pkFile:87`, `G4pk:101`) |
| `.g4cm` (caméra) | `CameraAnim::parse` | `crates/engine/nie-formats/src/g4cm.rs:336` (structs `Channel:211`, `Clip:243`, `AnimObject:263`, `CameraAnim:275`) |
| `.mevbin` | `parse` → `MevbinDocument` | `crates/engine/nie-formats/src/mevbin.rs:136` (structs `MevbinEvent:46`, `MevbinMotion:73`, `MevbinDocument:87`) |
| `.cfg.bin` (T2B — `EventMap_fix`, `light_list`, `event_cfg/*`) | `cfgbin::cfgbin_parse` + `cfgbin::to_iecode_json` | `crates/engine/nie-formats/src/decode.rs` (route générique `.cfg.bin`) ; forme iecode requise pour la lecture typée, cf. mémoire `cfgbin-json-forme-iecode-vs-brute` |
| `.lua.bin` (bytecode Lua) | `bytecode::Chunk`/`Prototype`/`decode_instruction` | `crates/engine/nie-lua/src/bytecode.rs:56` (`Header`), `:133` (`Prototype`), `:183` (`Chunk`), `:282` (`decode_instruction`) — 34 `pub fn` exposées (cf. `docs/PLAN-SITE-ULTIME.md` amendement 2026-09-06 (4)) |
| `.objbin` (rencontré marginalement, 14 fichiers dans ce domaine — probablement des objets de scène) | `objbin::MenuObject` et composants (`RenderComponent`, `AnimationComponent`, `TextComponent`, `AttachLocatorComponent`, `CollisionComponent`…) | `crates/engine/nie-formats/src/objbin.rs:66` et suivants |
| `.g4mt`, `.g4ma`, `.g4sk`, `.g4mg` (marginal ici, animation/squelette/skin — utilisés surtout par le domaine modèles/persos) | parseurs dédiés | `crates/engine/nie-formats/src/g4mt.rs`, `g4ma.rs`, `g4sk.rs`, `g4mg.rs` (existence confirmée par `rg -l g4pk`/`g4cm`, contenu non détaillé ici — hors périmètre du domaine événements) |
| `event_map_tag` (rapprochement RE ↔ menu) | module dédié | `crates/engine/nie-data/src/event_map_tag.rs` |
| Rôle de dossier (heuristique `event/`, `event_cfg/`, `script/`) | `folder_roles.rs` | `crates/engine/nie-explore/src/folder_roles.rs` (marqueurs `mevbin`/`objbin`/`event_cfg`) |
| Export de scène pour l'explorateur | `export.rs` | `crates/engine/nie-explore/src/export.rs` (marqueurs `mevbin`, `objbin`) |

**Absent** : aucun parseur `.r4XXXX` (les extensions numériques rencontrées 1 à 4 fois,
`r41152`/`r41000`/`r47929`/`r47819`/`r51528`/`r51265`) — nature inconnue, **à vérifier**
(probablement des fichiers résiduels/versionnés par CRC, non un format documenté).

## 4. Ce que le site sert déjà

```
curl -s http://127.0.0.1:8085/api/v1/formats
```
rend (extrait pertinent au domaine) :

| Suffixe | Route | Sortie | Fichiers (tout le VFS) |
|---|---|---|---:|
| `.cfg.bin` | `/api/v1/formats/decode/{chemin}` (en_process) | `application/json` | 71 101 |
| `.lua.bin` | `/api/v1/lua/scripts/{chemin}` (en_process) | `application/json` | 1 197 |

Vérifié par appel réel, sur des fichiers réels de ce domaine :

```
curl -s -o /tmp/o1 -w "%{http_code} %{size_download}\n" \
  "http://127.0.0.1:8085/api/v1/formats/decode/data/common/event_cfg/snd/ev00_00100_snd.cfg.bin"
# → 200 211  {"chemin":"...","donnees":{"entries":[...]},"format":"t2b","octets":112,"racines":1}

curl -s -o /tmp/o2 -w "%{http_code} %{size_download}\n" \
  "http://127.0.0.1:8085/api/v1/lua/scripts/data/common/script/lua/action/ball_chest_chara_1.00.07.lua.bin"
# → 200 5191  {"arbre":[{"chemin":"main",...}]}

curl -s -o /tmp/o3 -w "%{http_code} %{size_download}\n" \
  "http://127.0.0.1:8085/api/v1/formats/decode/data/common/event/ev01/ev01_00250/ev01_00250_c11010010_s00_p00_c0010.g4pk"
# → 400 157  {"genre":"demande_invalide","message":"cette route ne decode que .cfg.bin — ..."}
```

`.g4pk`, `.g4cm`, `.mevbin`, `.objbin` **ne sont dans aucune route** listée par `/api/v1/formats` :
non servis actuellement, malgré des parseurs Rust existants (§ 3). La route `/api/v1/3d` existe
pour `.g4md`/`.g4mg` (modèles) mais ne prend pas `.g4cm` en entrée (testé : répond avec la fiche
du service, pas un rendu — ce n'est pas une route de décodage de caméra).

## 5. Tableau de couverture

| Famille | Fichiers | État | Détail |
|---|---:|---|---|
| `event_cfg/*.cfg.bin` (snd, eff, evt, other) + `*_light/*.cfg.bin` | ~10 000+ (3 899+3 911+2 088+84+lights) | **servi** | `/api/v1/formats/decode/{chemin}`, HTTP 200 mesuré |
| `script/lua/**/*.lua.bin` | 651 | **servi** | `/api/v1/lua/scripts/{chemin}`, HTTP 200 mesuré |
| `event/**/*.g4pk` | 45 021 | **manquant** | parseur `g4pk.rs` existe, aucune route ne l'expose (HTTP 400 mesuré) |
| `event/**/*_camera.g4cm` | 1 215 | **manquant** | parseur `g4cm.rs` existe, aucune route de décodage caméra |
| `event/**/*.mevbin` | 95 | **manquant** | parseur `mevbin.rs` existe, aucune route |
| `event/**/*.objbin` (marginal, 14 dans ce lot) | 14 | **manquant** | parseur `objbin.rs` existe, aucune route dans ce domaine (une route objbin existe côté menu, § voir `nie-data::menu_setting`, non branchée pour les scènes d'événement) |
| `.g4mt`, `.g4ma`, `.g4sk`, `.g4mg` du domaine (anim/skin marginaux, 53+35+4+14) | 106 | **interne** | ces formats appartiennent au pipeline personnages/animation (domaine « modèles »), leur route naturelle est `/api/v1/3d`, pas une route « événement » dédiée — à rattacher au domaine modèles plutôt qu'à celui-ci |
| `.r41152`, `.r41000`, `.g4vs`, `.g4la`, `.r47929`, `.r47819`, `.r51528`, `.r51265` | 22 | **manquant** | nature non identifiée (§ 3), aucun parseur connu — ne pas router avant identification |

## 6. Routes à créer

Uniquement des routes appuyées sur un parseur déjà existant :

1. **`GET /api/v1/event/scene/{ev}/{scene}`** — liste tous les fichiers d'une scène
   (`data/common/event/{ev}/{ev}_{scene}/*`), catégorisés par rôle (acteur, point, caméra,
   lumière) via un simple filtre de nom, sans décodage — juste l'index VFS. Aucun parseur requis.

2. **`GET /api/v1/formats/decode3d/{chemin}.g4pk`** — décode un paquet de scène via
   `nie_formats::g4pk::parse` (`crates/engine/nie-formats/src/g4pk.rs:137`) et rend la liste de
   fichiers embarqués (`G4pkFile`) en JSON, sur le modèle de la route `.cfg.bin` existante.
   S'appuie sur un parseur prouvé (`g4pk.rs`).

3. **`GET /api/v1/event/camera/{chemin}.g4cm`** — décode une animation de caméra via
   `CameraAnim::parse` (`g4cm.rs:336`) et rend les canaux/clips en JSON (`Channel`, `Clip`,
   `AnimObject`). Permet ensuite un player de trajectoire de caméra dans le front. Parseur prouvé.

4. **`GET /api/v1/event/mevbin/{chemin}.mevbin`** — décode via `mevbin::parse`
   (`mevbin.rs:136`), rend `MevbinDocument` (événements + motions). Parseur prouvé.

5. **`GET /api/v1/event/light/{ev}/{scene}`** — sert `EventMap_fix_c*.cfg.bin` et
   `light_list.cfg.bin` de la scène via le décodeur `.cfg.bin` déjà branché (§ 4) : c'est une
   simple agrégation de routes existantes, pas un nouveau décodeur.

6. **`GET /api/v1/event/lua/{chemin}.lua.bin`** — déjà couvert par la route existante
   `/api/v1/lua/scripts/{chemin}` (§ 4) : à documenter côté catalogue « événements » plutôt qu'à
   recréer.

**Manquant en amont** (aucune route possible avant un parseur) : `.r41152`/`.r41000`/`.g4vs`/
`.g4la`/`.r47929`/`.r47819`/`.r51528`/`.r51265` — il faudrait d'abord identifier ces formats
(22 fichiers au total, faible priorité vu le volume).

## 7. Ce que le mode de jeu attend

- `data/common/script/lua/` alimente le runtime Lua du jeu (`nie-lua`), déjà partiellement
  câblé pour le **menu** (`menu_host.rs`, mode histoire/dialogue — cf. mémoire
  `mode-histoire-scene-dialogue.md` : la scène de dialogue livrée lit `inagle_event_subtitles`
  + police + composition PNG, mais le lien avec les fichiers `event/evNN_XXXXX/` de ce domaine
  n'est **pas prouvé** dans le dépôt actuel — la scène de dialogue actuelle ne consomme aucun
  `.g4pk`/`.g4cm`/`.mevbin`).
- Les préfixes `evNN` correspondent vraisemblablement aux cinématiques de mode Histoire / scènes
  de match (l'ampleur de `ev60`–`ev63`/`ev71`–`ev74`, soit 70 % du volume, suggère des cutscenes
  de match denses) — **aucune table du dépôt ne nomme ces préfixes à ce jour** ; c'est une
  hypothèse, pas un fait vérifié. Une vérification possible mais non faite ici : croiser les
  cmdId `funcLuaMenuCommand` (`data/re/funclua-cmdid-handlers.json`) avec les chaînes trouvées
  par `strings`/`re_strings` dans `nie.exe` autour des identifiants `evNN`.
- Le mode Victory Road (`docs/…`, mémoire `mode-victory-road-page-complete.md`) et Kizuna Town
  (mémoire `kizuna-town-pillar-re.md`) sont les deux modes déjà cartographiés dans ce dépôt ; ni
  l'un ni l'autre ne documente explicitement le rôle des scènes `event/evNN_XXXXX/` — **à
  vérifier** avant d'écrire une route qui prétendrait les rattacher à un mode précis.
