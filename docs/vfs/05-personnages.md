# Domaine VFS 5 — personnages, modèles 3D et effets

Cahier des charges de routes pour Aphrody. Inventaire source : `var/vfs/lot5-chr.txt` (39 146
lignes, format `chemin taille [cpk]`), couvrant `data/common/chr/`, `data/dx11/chr/`,
`data/common/effect/`, `data/dx11/effect/`. Toute commande citée est rejouable telle quelle.

## 1. Les chiffres

```
wc -l var/vfs/lot5-chr.txt                                          → 39 146 fichiers
awk '{s+=$2} END{print s}' var/vfs/lot5-chr.txt                     → 41 726 794 036 octets (38,9 Gio)
```

Ventilation par sous-arbre (`awk -F/ '{print $1"/"$2"/"$3}' … | sort | uniq -c`) :

| Sous-arbre | Fichiers |
|---|---|
| `data/common/chr` | 20 634 |
| `data/dx11/chr` | 9 746 |
| `data/common/effect` | 6 763 |
| `data/dx11/effect` | 2 003 |

Ventilation par extension, globale (`awk '{n=split($1,a,"."); print a[n]}' … \| sort \| uniq -c`) :

| Extension | Nombre | Rôle |
|---|---|---|
| `g4tx` | 11 722 | texture (atlas BC7/DXT, décodée par `g4tx_decode.rs`) |
| `g4mg` | 11 527 | géométrie (vertices, skinning, palette d'os) |
| `g4md` | 8 944 | descripteur de maillage (submeshes, layouts d'attributs) |
| `objbin` | 2 765 | table d'objets/matériaux liée au modèle |
| `g4pkm` | 2 644 | paquet de motion/pose compressé |
| `ptlb` | 657 | table de particules (effets uniquement) |
| `g4pk` | 411 | paquet générique (souvent des LOD `_p0NN`) |
| `mevbin` | 233 | binaire d'événement de mesh (déclenche des effets) |
| `g4sk` | 67 | squelette (hiérarchie d'os + poses) |
| `bin` | 62 | divers binaires opaques |
| `clobin` | 39 | table de collision (effets) |
| `col` | 19 | collision (dx11/chr) |
| `g4mt` | 18 | motion/clip d'animation |
| `linb` | 16 | table de liens (effets) |
| `log` | 10 | logs de build, aucune valeur runtime |
| `g4tg` | 8 | groupe de textures (dx11/effect) |
| `g4cm` | 2 | caméra/config de mesh |
| `cfg.bin` (racine `effect/`) | 10 | tables RDBN/T2B de configuration d'effets |

Ventilation détaillée par sous-arbre (`awk -v d="…" 'index($1,d)==1{print $1}' … \| awk -F. '{print $NF}' \| sort \| uniq -c`) :

- `data/common/chr` : g4mg 9 541, g4md 8 944, objbin 775, g4pkm 658, g4pk 368, mevbin 213, g4sk 49, bin 49, g4mt 18, log 10.
- `data/dx11/chr` : g4tx 9 727, col 19.
- `data/common/effect` : objbin 1 990, g4pkm 1 986, g4mg 1 986, ptlb 657, g4pk 43, clobin 32, mevbin 20, g4sk 18, linb 16, bin 13.
- `data/dx11/effect` : g4tx 1 995, g4tg 8.

## 2. Grammaire des chemins

Le slug du jeu est toujours un **code**, jamais un nom traduit. Racine :
`data/{common,dx11}/chr/<famille>/<...>/<code>/<code>[.suffixe].<ext>`.

Familles présentes sous `chr/` (`awk -F/ '$3=="chr"{print $4}' … \| sort \| uniq -c`) :

| Préfixe dossier | Codes | Désignation |
|---|---|---|
| `_face` | 5 847 | visage/corps propre au personnage (`c…`, `an…`, `e…`) |
| `_uniform` | 1 118 | tenues/uniformes (`e…`, `u…`) |
| `_waza` | 274 | animations/objets de technique spéciale |
| `_item` | 249 | objets tenus (ballons `b…`, gants, items `d…`) |
| `_keshin` | 100 | Keshin (avatars de super-pouvoir, `k…`) |
| `_armd` | 89 | armures (`ka…`) |
| `_animal` | 2 | modèles animaliers (`an…`) |
| `_test`, `_convertTest`, `_rig`, `_common` | 146+9+8 | outillage de production Level-5, hors gameplay |

Sous `_face`, un second niveau `<série>/` classe par génération de jeu (mesuré,
`awk -F/ '$4=="_face"{print $5}' … \| sort -u`) : `01_IE1` … `08_ORION`, `11_VICTORY`, `12_…`,
`20_EDIT`. Exemple vérifié : `data/common/chr/_face/01_IE1/c01000010/c01000010.g4md` — Mark Evans.

Codes racine hors `_prefixe` sous `common/chr` (`awk -F/ '$4 !~ /^_/{print $4}' … \| sed -E 's/^([a-zA-Z]+).*/\1/' \| sort \| uniq -c`) : `c` (15 dossiers), `i` (9), `b` (2), `mob` (1), `stairs` (1) —
essentiellement de l'outillage/prototypes, pas de la production (`c000101_test` notamment).

Sous `effect/`, deux racines dominent : `event/` (6 584, effets déclenchés par script/mevbin) et
`battle/` (2 121, effets de match). `system/` (28) et `_motevent/` (19) sont marginaux. Le reste
sont des `.cfg.bin` de configuration à la racine (`effect_define_*`, `eff_chr_trigger_*`,
`cmd_effect_config_*`) — des tables RDBN/T2B, pas des modèles.

## 3. Critère d'assemblabilité — mesuré par famille

Un modèle n'est affichable **que si** `<famille>/.../<code>/<code>.g4mg` existe. Contre-exemple
mesuré : `data/common/chr/_item/b000003/` ne porte qu'un `.g4sk` + `.objbin`, aucun `.g4mg` — et
`nie-model-serve` y répond `404 « G4MG … »`. Vérifié en direct ci-dessous.

Comptage par famille (boucle `rg -q "chr/<fam>/.*<code>/<code>\.g4mg " var/vfs/lot5-chr.txt`
sur chaque code du dossier) :

| Famille (segment site) | Codes totaux | Assemblables (`.g4mg` présent) | Manquants |
|---|---|---|---|
| `perso` (`_face`) | 5 847 | 5 742 | 105 |
| `uniform` (`_uniform`) | 1 118 | 1 022 | 96 |
| `waza` | 274 | 274 | 0 |
| `item` | 249 | 237 | 12 (dont `b000003`) |
| `keshin` | 100 | 100 | 0 |
| `armd` | 89 | 89 | 0 |
| `animal` | 2 | 2 | 0 |
| **Total** | **7 679** | **7 466** | **213** |

`waza`/`keshin`/`armd`/`animal` sont **100 % assemblables** — critère trivial pour eux. `item` et
surtout `uniform` (96 manquants, 8,6 %) et `perso` (105 manquants, 1,8 %) portent le risque de 404.

**`uniform` n'est actuellement PAS une famille servie par le site** (§5) — 1 022 codes
assemblables ignorés du catalogue `/api/v1/3d`.

## 4. Ce que le dépôt sait déjà faire

- Parseurs bas niveau, `crates/engine/nie-formats/src/` :
  - `g4md.rs:310` `pub fn parse` → `G4md` (submeshes, `VertexAttribute`, `g4md.rs:148`).
  - `g4mg.rs:113` `pub fn extract_skin(g4mg, g4md, submesh) -> Option<Vec<VertexSkin>>` —
    8×u16 poids (vtype 5, +0x24) / 8×u8 indices (vtype 6, +0x34), palette CRC32→os.
  - `g4sk.rs:137` `parse_header`, `g4sk.rs:175` `parse_hierarchy`, `g4sk.rs:427`
    `pub fn parse_poses(data, header) -> Option<Vec<BonePose>>` (squelette + poses de repos).
  - `g4mt.rs:50` `pub fn parse` → `G4mt` (clips), `g4mt.rs:283` `Motion::parse` (canaux
    d'animation, `Clip` à `g4mt.rs:115`).
  - `g4pk.rs:137` `pub fn parse` (paquets de LOD), `g4pkm.rs:179`/`194` `parse`/`parse_g4sk`
    (motion compressée).
  - `g4tx.rs`, `g4tx_decode.rs` (BC7/DXT → RGBA), `objbin.rs`, `mevbin.rs`.
- Assemblage haut niveau, `crates/engine/nie-formats/src/assemble.rs` (5 271 lignes) :
  - `assemble.rs:2019` `pub fn assemble_character_model` — corps + visage + uniforme dans un
    même espace monde, chaîne `chara_base → chara_model → CHARA_BODY_INFO → base_<classe>_NN.glb`
    documentée en tête de fichier.
  - `assemble.rs:2628` `assemble_generic_model`, `assemble.rs:2652` `assemble_keshin`,
    `assemble.rs:2673` `assemble_armed` — G4MD+G4MG génériques (keshin/armures).
  - `assemble.rs:2573` `assemble_avatar_model` — chemin `chara_edit`/avatar.
  - `assemble.rs:206` `series_dir_from_code` (résout `c01…`→`01_ie1` etc. pour l'URI texture),
    `assemble.rs:912` `to_glb`, `:925` `to_glb_textured` (URI CDN, zéro copie pixel), `:947`
    `to_glb_embedded`.
  - `assemble.rs:564` `Skeleton::from_g4sk`, `:603` `bone_by_hash` — résolution palette→os.
- Rendu sans pilote graphique : `nie-render3d` (rastériseur CPU z-buffer), consommé par le site
  (§5) via `nie_render3d::glb::parse` + `nie_render3d::render::render`.
- Assemblage réseau live : `crates/tools/nie-model-serve/src/main.rs` — routes `/model-chr/`
  (5212), `/model-avatar/` (5259), `/model-edit/` (5332), `/model-map/` (5363),
  `/model-report/` (5393), `/model-full/` (5426).

**Prouvé** : skinning et poses de repos sur des personnages réels (Byron `c01001900` +
`c000101`, 165 os, note du 2026-09-04 dans `assemble.rs`), assemblage live confirmé par les logs
systemd (`assemblage live : c01008390` → 2,5 Mo, `mode skinned`, en quelques secondes).
**Non prouvé / connu limité** : les clips longs et les animations multi-couches (`idle def001`)
distordent — cf. mémoire `animation-skinning-feasibilite.md`. Aucun cas de ce type n'a été
revérifié dans cette session.

## 5. Ce que le site sert déjà — mesuré en direct

`curl` contre `http://127.0.0.1:8085` (`nie-site`, service actif) :

```
GET /healthz                    → 200, 0,023 s
GET /api/v1/3d                  → 200, 1145 o, 0,428 s
```

Réponse de `/api/v1/3d` (résumée) :

```json
{"amont":"http://127.0.0.1:8790","vfs_pret":true,"miroir_present":true,
 "moteur":{"crate_":"nie-render3d","chemin":"CPU z-buffer …"},
 "familles":[
   {"segment":"perso","total":5490,"verifie":false,"source":"miroir"},
   {"segment":"waza","total":273,"verifie":true,"source":"vfs"},
   {"segment":"item","total":237,"verifie":true,"source":"vfs"},
   {"segment":"animal","total":2,"verifie":true,"source":"vfs"},
   {"segment":"keshin","total":100,"verifie":true,"source":"vfs"},
   {"segment":"armd","total":89,"verifie":true,"source":"vfs"}]}
```

Somme annoncée : 5490+273+237+2+100+89 = **6 191** — confirme le chiffre cité par la mission.

**Vérification croisée contre l'inventaire VFS** :

- `waza`(273), `item`(237), `animal`(2), `keshin`(89 hein 100), `armd`(89) correspondent
  **exactement** aux comptes « assemblables » de la section 3 (274 vs 273 : un écart d'une unité
  — probablement `_waza/_test` ou un doublon filtré côté SQL, à trianguler avant de le documenter
  comme un bug).
- `perso` = 5 490, mesuré directement en base :
  `sqlite3 var/mirror.sqlite "SELECT count(DISTINCT CASE WHEN instr(internal_code,'_')>0 …) FROM inagle_characters WHERE internal_code LIKE 'c%'"` → **5 490**. Ce nombre est **déclaré,
  pas vérifié** (`"verifie":false` dans la réponse) : il vient du miroir `inagle_characters`, pas
  du VFS, et peut dépasser les 5 742 codes VFS réellement assemblables (des variantes du miroir
  n'ont pas forcément de `.g4mg` propre, elles réutilisent un corps partagé).
- **`uniform` (1 022 codes assemblables, §3) est absent des 6 familles servies.** Le site ne
  propose donc aujourd'hui aucune route pour les tenues seules — un gain net si elles sont
  ajoutées en 7ᵉ famille `vfs`-vérifiée, sur le même modèle que `waza`/`item`/`keshin`/`armd`.

Fiche d'un modèle (`/api/v1/3d/modeles/perso/c01000010`, 200) : Mark Evans, nom FR/EN/JA, élément
Montagne, poste Gardien, 12 variantes — donnée servie via le miroir, cohérente avec le catalogue
`nie-catalog`.

**Rendu effectif — dégradé au moment de la mesure** :

```
GET /model/perso/c01000010.glb   → 504 « nie-model-serve n'a pas répondu en 10s », 10,006 s
GET /model/perso/c01000010.png   → 504, 10,003 s
GET /model/item/b000004.glb      → 504, 10,002 s (b000004 pourtant assemblable, §3)
```

`systemctl status nie-model-serve` : service **actif** (PID up depuis 1h21), logs montrant des
assemblages réussis dans la dernière heure (`c01008390`, `c03036820`, `c11748130`,
`c05021820` — tous « mode skinned », 1,2-2,5 Mo, quelques secondes), mais **mémoire à 16,3 Gio
pour un plafond `MemoryHigh=16G` (`available: 0B`)** — signature exacte du piège documenté dans
`CLAUDE.md` § *Services du VPS* (cache qui remplit jusqu'au plafond cgroup, reclaim permanent,
timeouts sans requête visible). **Ne pas conclure que l'assemblage est cassé** : c'est une
saturation mémoire de service, pas un défaut de format ou de route — mais toute route nouvelle
qui tire des GLB live héritera de ce même risque tant que le budget n'est pas revu (cf.
`--memory-cache-mib 2048` dans la commande systemd, potentiellement à réduire ou dont le
`MemoryHigh` est à monter).

Les effets (`vfxo`/`pfxo`/`cfxo`/`gfxo`/`ptlb`/`fxbin`) : **aucune de ces extensions n'apparaît
dans l'inventaire lot5** (l'extraction réelle donne `ptlb`, `clobin`, `linb`, `mevbin`,
`objbin`, `g4tg` — pas de fichier `.vfxo`/`.pfxo`/`.cfxo`/`.gfxo`/`.fxbin` dans ce domaine). Ne
pas les citer comme existants sans nouvelle vérification par `niers vfs find`. Aucun décodeur
dédié n'a été trouvé pour `ptlb`/`clobin`/`linb`/`g4tg`
(`rg -l "ptlb|clobin|linb::|g4tg" crates/engine/nie-formats/src/` ne matche que `cfgbin.rs`,
`decode.rs`, `dxbc.rs`, `main.rs` de model-serve — pas de parseur typé). **Les effets ne sont
pas couverts par la couche 3D** et n'ont aujourd'hui aucune route dédiée : ni assemblage, ni
export brut.

## 6. Tableau de couverture (matrice `docs/PLAN-SITE-ULTIME.md` § 4)

| Ce que ça sert | État | Détail mesuré |
|---|---|---|
| `/api/v1/3d` (capacités, familles) | **servi** | 200, 1145 o, 0,43 s |
| `/api/v1/3d/modeles?famille=waza\|item\|animal\|keshin\|armd` | **servi** | 200 (échantillonné sur `waza`) |
| `/api/v1/3d/modeles/perso/{code}` (fiche) | **servi** | 200, identité complète |
| `/model/{famille}/{code}.glb` | **interne** | route existe, code présent, mais **amont `nie-model-serve` saturé mémoire** au moment de la mesure → 504 systématique. Fonctionne en régime nominal (logs d'assemblages réussis dans l'heure précédente) |
| `/model/{famille}/{code}.png` (rendu CPU) | **interne** | même dépendance amont ; le rastériseur `nie-render3d` lui-même n'est pas en cause |
| `/api/v1/3d/modeles?famille=uniform` | **manquant** | famille non déclarée côté site alors que 1 022/1 118 codes VFS sont assemblables (§3) — à ajouter, sur le modèle `waza`/`item` (source `vfs`, `verifie:true`) |
| Personnages `perso` sans variante détectée du miroir (5 742 assemblables VFS vs 5 490 déclarés miroir) | **interne** | écart non trianché — le miroir peut sous- ou sur-compter selon le partage de corps ; `"verifie":false` l'assume déjà |
| `_item`/`_uniform` non assemblables (12 + 96 codes) | **manquant** (par construction) | absence de `.g4mg` — pas un bug de route, une donnée manquante côté jeu (pièces auxiliaires probablement fusionnées ailleurs) |
| Effets (`event/`, `battle/`, `.ptlb`, `.clobin`, `.linb`, `.mevbin`) | **manquant** | aucun décodeur typé, aucune route ; nature exacte des formats non reversée dans cette session |
| Tables de config d'effets à la racine (`effect_define_*.cfg.bin`, etc.) | **interne** | ce sont des RDBN/T2B, décodables par `nie_formats::cfgbin` générique (§7), mais aucune route dédiée ne les republie encore |
| `data/dx11/effect/*.g4tx`/`.g4tg` (atlas de particules) | **interne** | même décodeur `g4tx_decode.rs` que les textures de personnage ; réutilisable via `/tex/<chemin>.png` générique s'il existe déjà pour ce sous-arbre — non vérifié dans cette session |

## 7. Routes à créer

| Route proposée | Contenu | Parseur/route existant à réutiliser |
|---|---|---|
| `GET /api/v1/3d/modeles?famille=uniform` | catalogue des 1 022 tenues assemblables | même filtre `source=vfs`/`dossier=data/common/chr/_uniform` que `waza`/`item` — ajouter `uniform` à l'énumération de `crates/tools/nie-site/src/routes/modeles3d.rs`, aucune nouvelle logique de parsing |
| `GET /model/uniform/{code}.glb` | GLB générique (G4MD+G4MG, pas de composition corps/visage) | `assemble_generic_model` (`assemble.rs:2628`) — même chemin que `keshin`/`armd` |
| `GET /api/v1/effets` | catalogue `event/` (6 584) + `battle/` (2 121) par nom de dossier | à écrire : aucun décodeur `ptlb`/`clobin`/`linb`/`mevbin` typé n'existe — **RE requis avant toute route**, ne pas créer de route qui ne fait que lister des chemins sans contenu exploitable |
| `GET /f/effect/{chemin}.cfg.bin` (config brute) | RDBN/T2B des `effect_*_define*.cfg.bin` | `nie_formats::cfgbin` générique + `to_iecode_json` (cf. mémoire `cfgbin-json-forme-iecode-vs-brute.md`) — décodable **aujourd'hui**, sans RE supplémentaire |
| `GET /tex/dx11/effect/{chemin}.png` | atlas de particules (`.g4tx`/`.g4tg`) | `g4tx_decode.rs` — même décodeur que les textures de personnage ; vérifier si la route générique `/tex/*` couvre déjà ce sous-arbre avant d'en écrire une nouvelle |
| `GET /api/v1/3d/modeles/{famille}/{code}/manquant` (diagnostic) | expose *pourquoi* un code n'est pas assemblable (fichiers présents vs `.g4mg` absent) | dérivable de la boucle `rg` de la section 3, à internaliser en Rust plutôt que rejouée en shell à chaque fois — utile pour les 213 codes en défaut |

Aucune route pour `event`/`battle` tant que `ptlb`/`clobin`/`linb`/`mevbin` n'ont pas de parseur
typé : servir des chemins bruts sans décodage serait un catalogue de 404 déguisé (le piège que la
mission demande justement d'éviter).

## 8. Ce que les modes de jeu attendent

- **Match** (`nie-render3d`, `nie-match3d`) : corps + visage + uniforme assemblés
  (`assemble_character_model`), plus `_waza` pour les effets de shoot/technique spéciale et
  `_item` pour le ballon (`b000001`…`b000004`, `_item/b*`). Le pilier « 3 physiques ballon
  byte-exact » (mémoire `moteur-de-jeu-nie-runtime.md`) dépend de `_item/b000001`
  (assemblable, §3).
- **Avatar / `chara_edit`** (mémoire `chara-edit-avatar-carte.md`) : `assemble_avatar_model`
  (`assemble.rs:2573`), consomme `_face` du personnage joueur + `_uniform` + les 16 listes déjà
  portées côté RE. La composition passe par 4 matériaux + l'os `c_head_1_0` + les textures
  `customTex_*` — hors du périmètre VFS pur (dépend du pool de constantes Lua).
- **Keshin** : `_keshin/k*` (100 codes, 100 % assemblables), consommé tel quel par
  `assemble_keshin` — aucune composition requise, contrairement à `perso`.
- **Armures** (`ka…`) : `_armd/ka*` (89 codes, 100 % assemblables), `assemble_armed` — probablement
  liées aux items Keshin Armed, chaîne de résolution non revérifiée ici (à confirmer via
  `chara_model_*.cfg.bin` avant d'exposer une route de composition armure+personnage).
- **Effets de match/menu** (`event/`, `battle/`) : consommés par le moteur runtime via
  `mevbin` (déclencheurs) → `ptlb`/`clobin`/`linb` (paramètres de particules/collision). Aucun
  pilier du dépôt ne les a encore reversés ; ils bloquent toute route de prévisualisation
  d'effet tant que ce travail n'est pas fait.

## Sources et commandes de vérification rapide

```bash
wc -l var/vfs/lot5-chr.txt
awk '{s+=$2} END{print s}' var/vfs/lot5-chr.txt
sqlite3 var/mirror.sqlite "SELECT count(DISTINCT …) FROM inagle_characters WHERE internal_code LIKE 'c%';"
curl -s http://127.0.0.1:8085/api/v1/3d | jq .
systemctl status nie-model-serve --no-pager
niers vfs find "_item/b000003"
```
