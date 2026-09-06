# Domaine 6 — Monde, données de jeu et rendu

Cahier des charges de routes pour Aphrody, sur le périmètre :
`data/common/map/`, `data/common/gamedata/`, `data/common/craft/`, `data/common/action/`,
`data/common/system/`, `data/common/input/`, `data/common/camera/`, `data/dx11/map/`,
`data/dx11/shader/`, `data/dx11/event/`.

Source : `var/vfs/lot6-monde.txt` (chemin, taille, `[cpk]`), extrait du VFS live (255 308 entrées).
Toutes les commandes ci-dessous sont rejouables telles quelles depuis `/home/ubuntu/niers`.

## 1. Les chiffres

```
$ wc -l var/vfs/lot6-monde.txt
27685 var/vfs/lot6-monde.txt   # ECART RESOLU : 27 685 est le bon compte. Le 29 688 de l'enonce
                                # datait d'un decoupage anterieur ou `data/dx11/effect/` (2 003
                                # fichiers) apparaissait DEUX fois — ici et dans le lot 05
                                # (personnages/effets). Il a ete retire d'ici, pas du lot 05 :
                                # 27 685 + 2 003 = 29 688. La somme des six lots fait bien
                                # 255 308, verifie par `wc -l var/vfs/lot*.txt`.

$ awk '{s+=$2} END{printf "%d\n", s}' var/vfs/lot6-monde.txt
22 140 059 801 octets ≈ 22,14 Go
```

Ventilation par extension (`awk '{print $1}' … | sed -E 's/.*\.([a-z0-9_]+)$/\1/' | sort | uniq -c | sort -rn`) :

| Extension | Fichiers | Nature |
|---|---:|---|
| `objbin` | 9 411 | géométrie de map (éclairage/light-map, cf. §2) |
| `bin` | 7 160 | dernier segment de `.cfg.bin` (RDBN ou T2B, cf. §3) |
| `g4pkm` | 2 629 | pack de modèle (motion incluse, `g4pkm_motion.rs`) |
| `g4mg` | 2 629 | géométrie skinnée (mesh + squelette) |
| `vfxo` | 1 335 | effet visuel (particules) — **aucun parseur** |
| `g4tx` | 1 256 | texture (déjà servi par le domaine texture) |
| `col` | 1 131 | collision (parseur présent, cf. §7) |
| `pfxo` | 1 113 | effet visuel (variante) — **aucun parseur** |
| `fxbin` | 372 | shader compilé — **aucun parseur** |
| `g4sk` | 270 | squelette |
| `g4nv` | 160 | inconnu (non exploré ici) |
| `g4pk` | 155 | pack (sans motion) |
| `cfxo` | 29 | effet visuel — **aucun parseur** |
| `gfxo` | 20 | effet visuel — **aucun parseur** |
| `g4md` | 11 | modèle (peu nombreux ici, la masse est dans le domaine perso) |
| `cfg` | 2 | à part (2 fichiers en `.cfg` nu, hors `.cfg.bin`) |

Ventilation par sous-arbre (`awk '{n=split($1,a,"/"); print a[1]"/"a[2]"/"a[3]}' … | sort | uniq -c`) :

| Sous-arbre | Fichiers |
|---|---:|
| `data/common/map` | 12 613 |
| `data/common/gamedata` | 9 788 |
| `data/dx11/shader` | 2 870 |
| `data/dx11/map` | 2 371 |
| `data/dx11/event` | 16 |
| `data/common/craft` | 10 |
| `data/common/action` | 10 |
| `data/common/system` | 3 |
| `data/common/input` | 3 |
| `data/common/camera` | 1 |

`gamedata` (9 788) et `map` (12 613) portent à eux seuls 81 % des fichiers du domaine — c'est
là que la priorité de routage doit aller.

## 2. La grammaire des chemins

- **Cartes par code** : `s10g001`, `b10g001`, `w16` — préfixe de type (`s`=scénario/stage,
  `b`=bâtiment, `w`=monde ouvert) + numéro de zone + suffixe de sous-zone (`g001`). Vérifié
  (`niers vfs find s10g001 -n 5`, `niers vfs find w16 -n 5`) : 124 fichiers sous `s10g001`, 138
  sous `w16`, répartis entre `data/common/gamedata/map/<code>/` (config) et
  `data/common/map/_light/<code>_g1/` (géométrie d'éclairage, suffixes `_su00`…`_su12`, `_cl00`…).
- **Shaders versionnés par dossier unique** : tous les 2 870 fichiers de `data/dx11/shader/`
  vivent sous `1.00.41/` — une seule version de shader dans tout le dump, pas de multi-version
  observée sur ce domaine.
- **Fichiers versionnés par suffixe numérique** : `chara_act_cfg.1.03.91.00.cfg.bin` (16 880
  puis 70 352 octets selon la version), `w16_encount_config_1.01.13.00.cfg.bin` /
  `…_1.01.31.00…` / `…_1.01.80.00…` (trois versions coexistantes du même fichier logique, tailles
  944/1056/1072 o) — **ne jamais deviner un numéro de version**, toujours `niers vfs find`.
- **`event_bustup_talk_data_config_c16_3.00.06.cfg.bin`** : le code personnage (`c16`, `c20`,
  `c21`, `c22`…) est encodé dans le nom, pas dans un sous-dossier.
- `data/common/map/` niveau 4 : dominé par `s` (4 919), `w` (3 155), `ar` (1 740, arène ?),
  `_light` (1 593), `k` (712, ville/kizuna ?), `b` (372, bâtiment) — préfixes cohérents avec
  `data/dx11/map/` (mêmes lettres : `w` 779, `ar` 705, `s` 543, `k` 274, `b` 21).

## 3. `gamedata` — le cœur des données de jeu

9 788 fichiers, ventilation par sous-dossier
(`grep '^data/common/gamedata/' … | awk -F/ '{print $4}' | sort | uniq -c | sort -rn`) :

| Sous-dossier | Fichiers | Sous-dossier | Fichiers |
|---|---:|---|---:|
| `menu` | 3 866 | `system` | 29 |
| `map` | 3 115 (dont 493 `.cfg.bin`) | `rpg_battle` | 27 |
| `event` | 1 438 | `skill` | 23 |
| `soccer` | 873 | `team` | 6 |
| `quest` | 131 | `chat_emote` | 6 |
| `phase` | 88 | `post` | 5 |
| `staffroll` | 56 | `mission` | 5 |
| `character` | 46 (dont 46 `.cfg.bin`) | `item` | 5 |
| … | | `craft` | 2 |

Extension : **6 415 `.bin`** (dernier segment de `.cfg.bin`) et **3 373 `.objbin`** — `gamedata`
porte donc aussi de la géométrie, pas seulement de la config (ex. `data/common/gamedata/map/`
mêle `.cfg.bin` de config et objets).

### RDBN vs T2B — mesuré, pas supposé

Deux tests réels via `nie_formats::cfgbin` (par le site, §5) :

- **RDBN** (`is_rdbn`, `crates/engine/nie-formats/src/cfgbin.rs:248`) — forme `{"lists":[{"name":…,
  "typeName":…,"values":[…]}]}`. Confirmé sur
  `data/common/gamedata/skill/override_skill_config_3.00.21.00.cfg.bin` : sortie
  `{"lists":[{"name":"m_OverrideConditionSkillInfoList","typeName":"OverrideConditionSkillInfo",…}]}`.
- **T2B** (`cfgbin_parse`, `crates/engine/nie-formats/src/cfgbin.rs:804`) — arbre `CfgEntry`,
  forme `{"entries":[{"children":[…],"name":…,"variables":[…]}]}`. Confirmé sur
  `data/common/input/dbg_input_ctrl.cfg.bin` : racines nommées `INPUT_PAD_INFO_0`,
  `INPUT_CTRL_BGN` (vu aussi via `?forme=structure`, type_nom `t2b_entry`).
- **`common/property/**` est T2B par doctrine** (CLAUDE.md) — hors périmètre direct de ce
  domaine (`property` n'apparaît pas dans l'inventaire lot6), mais `input`, `system`, `action`,
  `camera` sont structurellement proches (config statique moteur plutôt que liste de données) :
  à confirmer fichier par fichier avant d'écrire un décodeur dédié — ne pas supposer T2B par
  analogie seule.

### Déjà porté dans `nie-data`

`crates/engine/nie-data/src/` compte **117 fichiers** (`ls … | wc -l`), **116 `pub mod`**
déclarés dans `lib.rs`, et **130 golden tests** (`ls tests/*golden* | wc -l`). Recherche par
marqueur (jamais par nom de fichier — les modules sont nommés par concept) :

| Famille gamedata | Marqueur cherché | Porté ? |
|---|---|---|
| `skill/override_skill_config` | `override_skill_config` | oui (2 refs) |
| `skill/aura_skill_config` | `aura_skill_config` | oui (4 refs, `aura.rs`) |
| `skill/passive_skill_config` | `passive_skill_config` | oui (5 refs) |
| `skill/ability_learning_config` | `ability_learning_config` | oui (`ability_learning.rs`) |
| `team/*_team_config` | `team_config` | oui (5 refs, `belong_team.rs`, `enjoy_mode_team.rs`) |
| `rpg_battle/rpg_battle_cmd_config` | `rpg_battle_cmd_config` | oui (1 ref) |
| `rpg_battle/rpg_battle_ai_config` | `rpg_battle_ai_config` | **non** (0 ref) |
| `event/event_bustup_talk` | `event_bustup_talk` | oui (`event_bustup.rs`) |
| `system/activity_config` | `activity_config` | oui (`activity.rs`) |
| `character/belong_team_config` | `belong_team_config` | oui |
| `character/basara_chara_config` | `basara_chara_config` | oui (`basara.rs`) |
| `character/add_model_config` | `add_model_config` | oui (`add_model.rs`) |
| `character/academic_year_config` | `academic_year_config` | oui (`academic_year.rs`) |
| `character/chara_action` | `chara_action` | **non** |
| `system/behavior_trigger_common` | `behavior_trigger` | **non** |
| `system/add_content_config` | `add_content_config` | **non** (proche : `add_content_equip.rs` existe) |
| `input/*` | `key_assign`, `input_ctrl` | **non** — `input.rs` existe mais ne cible pas ces marqueurs (à vérifier au cas par cas) |
| `camera/external_camera_config` | `external_camera_config` | **non** |
| `action/chara_act_cfg`, `base_act` | `chara_act_cfg`, `base_act` | **non** |
| `map/map_data`, `map_light_set`, `map_minimap`, `map_col_shape` | idem | **non** — aucun module `nie-data` ne couvre les configs de map |

**Verdict** : les familles *personnage/skill/team/event* de `gamedata` sont largement portées
(logique métier), mais **tout le sous-système monde** (`map`, `action`, `camera`, `craft`,
`input`, `system` bas niveau) est **non porté** dans `nie-data` — seul le décodage générique
RDBN/T2B (§5) les rend lisibles, sans schéma typé.

## 4. Formats binaires 3D/monde — parseurs `nie-formats`

Présents dans `crates/engine/nie-formats/src/` :

| Fichier module | Formats couverts | Fonctions clé |
|---|---|---|
| `col.rs` | `.col` (collision) | `crates/engine/nie-formats/src/col.rs:28` `is_size_consistent`, `:34` `data_offset`, `:41` `is_pxcl`, `:49` `parse` |
| `g4mg.rs` | `.g4mg` (géométrie skinnée) | `crates/engine/nie-formats/src/g4mg.rs:113` `extract_skin`, `:156` `extract_geometry`, `:344` `material_base_name` |
| `g4md.rs` | `.g4md` (modèle) | utilisé par `nie-render3d`, `nie-app/character.rs` |
| `g4pk.rs`, `g4pkm.rs`, `g4pkm_motion.rs` | packs de modèle + motion | — |
| `g4sk.rs` | squelette | — |
| `objbin.rs` | géométrie/light-map de map | — |
| `mevbin.rs` | événements (`crates/engine/nie-formats/src/mevbin.rs:99` `motion_count`, `:105` `parsed_event_count`, `:136` `parse`) | |

**Aucun parseur pour `fxbin`/`vfxo`/`pfxo`/`cfxo`/`gfxo`** (shaders et effets visuels) : `rg -i
"fx|shader" crates/engine/nie-formats/src/` ne rend aucun module dédié. 2 909 fichiers du
domaine (372 + 1 335 + 1 113 + 29 + 20 + shaders `1.00.41/`) sont donc **illisibles** en l'état —
seul un dump hexadécimal ou une extraction brute est possible aujourd'hui.

`nie-render3d` (`crates/engine/nie-render3d/src/`) consomme `g4md`/`g4mg` (via `glb.rs`,
`scene.rs`) pour produire du GLB — c'est la voie déjà branchée sur le site (§5, routes `/api/v1/3d`).

## 5. Ce que le site sert déjà — mesuré

Service actif (`systemctl is-active nie-model-serve` → `active`), écoute `127.0.0.1:8085`.

```
$ curl -s -o /dev/null -w "code=%{http_code} taille=%{size_download} temps=%{time_total}s\n" \
    http://127.0.0.1:8085/api/v1/formats
code=200 taille=1258 temps=0.001s
```

`GET /api/v1/formats` liste les familles servies (extrait) :

| Suffixe | Décodage | Route | Sortie | Fichiers | Octets |
|---|---|---|---|---:|---:|
| `.cfg.bin` | en_process | `/api/v1/formats/decode/{chemin}` | JSON | 71 101 | 216 141 904 |
| `.lua.bin` | en_process | `/api/v1/lua/scripts/{chemin}` | JSON | 1 197 | 10 694 973 |
| `.g4tx` | délégué | `/assets/tex/{chemin}.png` | PNG | 54 203 | 82 004 879 296 |
| `.g4md` | délégué | `/api/v1/3d` | glTF binaire | 8 956 | 17 862 924 |
| `.g4mg` | délégué | `/api/v1/3d` | glTF binaire | 15 876 | 3 315 054 016 |
| `.acb` | délégué | `/assets/audio-info/{chemin}` | JSON | 5 512 | — |

**`.col`, `.objbin`, `.g4pkm`, `.g4sk`, `.g4pk`, `.fxbin`, `.vfxo`, `.pfxo`, `.cfxo`, `.gfxo`,
`.g4nv`** — **absents de la liste `/api/v1/formats`**, donc non servis, y compris ceux qui ont
un parseur (`col`, `g4mg`(*), `g4pkm`, `g4sk`).

(*) `.g4mg` EST servi mais seulement via `/api/v1/3d` (assemblage modèle+squelette+skin), pas en
décodage brut individuel.

### Décodage RDBN mesuré

```
$ curl -s -o /tmp/rdbn.json -w "code=%{http_code} taille=%{size_download} temps=%{time_total}s\n" \
  "http://127.0.0.1:8085/api/v1/formats/decode/data/common/gamedata/skill/override_skill_config_3.00.21.00.cfg.bin"
code=200 taille=6029 temps=0.081s
→ {"donnees":{"lists":[{"name":"m_OverrideConditionSkillInfoList","typeName":"OverrideConditionSkillInfo",
   "values":[{"num":1,"skillId":"0x2CFC3E63"},…]}]}}
```

### Décodage T2B mesuré

```
$ curl -s -o /tmp/t2b.json -w "code=%{http_code} taille=%{size_download} temps=%{time_total}s\n" \
  "http://127.0.0.1:8085/api/v1/formats/decode/data/common/input/dbg_input_ctrl.cfg.bin"
code=200 taille=128986 temps=0.022s
→ {"donnees":{"entries":[{"children":[{"children":[…],"name":"INPUT_PAD_INFO_0",
   "variables":[{"type":"Int","value":"2"},…]}],"name":"INPUT_CTRL_BGN"}…]}}

$ curl -s "…/decode/data/common/input/dbg_input_ctrl.cfg.bin?forme=structure"
code=200 taille=454 temps=0.004s
→ {"structure":{"chaines":0,"entete":null,"racines":[{"nom":"INPUT_CTRL_BGN","type_nom":"t2b_entry",
   "lignes":4,"ligne_octets":10},…]}}
```

Le générique `.cfg.bin` (RDBN + T2B) fonctionne **déjà** sur tout ce domaine, sans routage
spécifique : `input`, `system`, `camera`, `action`, `craft` et tout `gamedata` sont décodables
via cette seule route générique, typée ou non selon `nie-data` (§3).

## 6. Matrice de couverture (§4 `docs/PLAN-SITE-ULTIME.md`)

| Format / famille | État | Raison / route |
|---|---|---|
| `gamedata/**/*.cfg.bin` (générique, RDBN+T2B) | **servi** | `/api/v1/formats/decode/{chemin}`, code 200 mesuré |
| `input/*.cfg.bin`, `system/*.cfg.bin`, `camera/*.cfg.bin`, `action/*.cfg.bin`, `craft/*.cfg.bin` | **servi** | même route générique — pas de schéma typé mais lisible |
| `skill/*`, `team/*`, `event_bustup*`, `character/*` (config, cf. §3) | **interne** | typé dans `nie-data` (`aura.rs`, `belong_team.rs`, `event_bustup.rs`…) mais **non exposé** en route dédiée — passe encore par le générique, sans nom de champ métier |
| `.g4md`, `.g4mg` (modèles/géométrie) | **servi** | `/api/v1/3d`, glTF binaire, délégué |
| `.g4tx` (textures de map/event) | **servi** | `/assets/tex/{chemin}.png` |
| `.acb` (audio de map/event) | **servi** | `/assets/audio-info/{chemin}` |
| `.objbin` (géométrie/light-map de map, 9 411 fichiers) | **manquant** | pas de route ; format non listé dans `/api/v1/formats` (parseur présent : `objbin.rs`) |
| `.col` (collision, 1 131 fichiers) | **manquant** | parseur présent (`col.rs`), 0 route |
| `.g4pkm`/`.g4pk`/`.g4sk` (packs modèle + squelette, 3 054 fichiers) | **manquant** | parseurs présents, 0 route |
| `.fxbin`/`.vfxo`/`.pfxo`/`.cfxo`/`.gfxo` (shaders/effets, 2 869 fichiers) | **manquant** | **aucun parseur** dans `nie-formats` — travail de RE avant toute route |
| `dx11/shader/1.00.41/*` (2 870 fichiers) | **manquant** | aucun parseur ; format shader compilé (DXBC probable, non vérifié) |
| Schémas typés `map_data`/`map_light_set`/`map_minimap`/`map_col_shape` | **manquant** | aucun module `nie-data` ; RDBN/T2B brut seulement |

## 7. Routes à créer

Aucune route sans décodeur existant — classées par ce qui est immédiatement faisable :

1. **`/api/v1/gamedata/map/{code}`** — agrège tous les `.cfg.bin` d'un code de carte
   (`s10g001`, `w16`…) via le décodeur générique (§5) déjà en service ; pas de nouveau parseur,
   juste un agrégateur de chemins (`niers vfs find <code>`).
2. **`/api/v1/collision/{chemin}`** — décoder `.col` via `col::parse` (existe,
   `crates/engine/nie-formats/src/col.rs:49`) et rendre un JSON de mesh de collision. **À
   écrire** : le endpoint HTTP seul manque, le parseur est prêt.
3. **`/api/v1/model-pack/{chemin}`** — décoder `.g4pkm`/`.g4pk`/`.g4sk` (parseurs présents) pour
   exposer le contenu d'un pack sans reconstruire tout le glTF (utile pour l'inspection, pas le
   rendu).
4. **`/api/v1/map-geometry/{chemin}`** — décoder `.objbin` (`objbin.rs`, parseur présent) : la
   géométrie de light-map des 9 411 fichiers `data/common/map/_light/**` n'est exposée nulle part.
5. **Shaders et effets (`fxbin`/`vfxo`/`pfxo`/`cfxo`/`gfxo`, `dx11/shader/1.00.41/`)** —
   **aucune route possible avant RE** : pas de parseur, format non identifié dans ce domaine.
   Première étape : `hexyl` sur un échantillon + comparaison à un format DXBC/effet Level-5
   connu, avant d'écrire quoi que ce soit dans `nie-formats`.
6. **Schémas typés manquants** (`rpg_battle_ai_config`, `chara_action`, `behavior_trigger`,
   `add_content_config`, `key_assign`, `input_ctrl`, `external_camera_config`, `chara_act_cfg`,
   `base_act`, `map_data`, `map_light_set`, `map_minimap`, `map_col_shape`) — porter dans
   `nie-data` avant d'exposer un champ nommé ; en attendant, le générique RDBN/T2B (§5) suffit
   pour un affichage brut.

## 8. Ce que les modes de jeu consomment

- **Match (soccer)** : `gamedata/soccer/game/*_trigger_0.04.78.cfg.bin` (873 fichiers, RDBN
  probable — même famille que `quest`/`phase`), `action/chara_act_soccer_cfg.1.02.80.00.cfg.bin`
  (84 208 o), `action/base_act.cfg.bin`, `camera/config/external_camera_config.cfg.bin`. Corrélé
  avec le pilier physique/moteur 3D déjà FAIT (mémoire : `moteur-de-jeu-nie-runtime.md`).
- **Kizuna Town** : `gamedata/map/<code_ville>/` (préfixe `k`, 712 fichiers `data/common/map/k`),
  `craft/asset/area/*` et `craft/asset/map/*` (placement d'objets — `craft.rs` déjà porté côté
  `nie-data`, cf. mémoire `kizuna-town-pillar-re.md`).
- **Craft** (placement) : les 10 fichiers `data/common/craft/asset/{area,map}/*.cfg.bin` sont
  minuscules (176 à 12 448 o) — vraisemblablement des layouts de test (`pl_test_layout`,
  `peak_load_test`) plutôt que du contenu final ; à confirmer avant de les présenter comme
  représentatifs.
- **Victory Road (histoire)** : `gamedata/event/event_bustup_talk_*` (1 438 fichiers, déjà porté
  `event_bustup.rs`), `gamedata/phase/c*/c*_trigger_0.04.78.cfg.bin` (88 fichiers, un par
  chapitre `c01`…), `gamedata/quest/qsa*_trigger_0.04.78.cfg.bin` (131 fichiers). Cohérent avec
  le pilier « Mode Victory Road » déjà porté (mémoire `mode-victory-road-page-complete.md`).
- **Input/système bas niveau** : `input/key_assign_3.00.13.00.cfg.bin`,
  `input/input_ctrl_3.00.18.00.cfg.bin` — consommés par tous les modes indistinctement (mapping
  manette), pas de route dédiée nécessaire au-delà du décodage générique T2B.
