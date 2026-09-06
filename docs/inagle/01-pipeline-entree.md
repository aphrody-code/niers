# `packages/inagle` — la moitié ENTRÉE du pipeline

> Périmètre de ce document : `packages/inagle/src/{adapters,core,data,entities,entries,lib,parsers,schemas,types,utils}`
> plus `index.ts`, `service.ts`, `stat-calculator.ts`.
> La moitié SORTIE (`push*`, `cli*`, `api/`, et les domaines `basara/ characters/ skills/ teams/ items/
> quests/ drops/ menu/ search/ rag/ zukan/ analysis/ constellation/ lua/`) est cartographiée ailleurs.
> **Aucune modification de code n'a été faite.** Mesures du 2026-09-06 sur le VPS Linux.

## 0. Taille du périmètre

| Dossier | Fichiers `.ts` | Lignes `.ts` | `export function` | `export interface` |
|---|---:|---:|---:|---:|
| `parsers/` | 73 | 13 142 | 199 | 182 |
| `core/` | 20 | 5 197 | 90 | 56 |
| `types/` | 4 | 1 857 | 0 | 34 |
| `entities/` | 5 | 1 334 | 12 | 4 |
| `schemas/` | 7 | 787 | 2 | 22 |
| `adapters/` | 2 | 162 | 1 | 0 |
| `utils/` | 2 | 121 | 5 | 0 |
| `lib/` | 1 | 84 | 2 | 0 |
| `data/` | 3 (+4 JSON) | 238 | — | — |
| `entries/` | 0 (37 JSON) | — | — | — |

Commandes : `fd -e ts . src/<d> | wc -l` ; `fd -e ts . <d> -x cat | wc -l` ;
`rg -cN 'export (async )?function ' <d> -g '*.ts' | awk '{s+=$1}END{print s+0}'` (idem `export interface`).
Contrôle global `tokei` sur le périmètre : **120 fichiers TypeScript, 16 543 lignes de code**
(23 533 lignes brutes) et **41 fichiers JSON, 1 529 350 lignes**.

Tests présents dans le périmètre : **4** — `core/errors.test.ts`, `parsers/unlock-condition.test.ts`,
`stat-calculator.test.ts`, `azalee-inagle-integration.test.ts`
(`fd -e ts . <périmètre> -x basename {} | grep test`). Ils n'ont **pas** été exécutés ici.

---

## 1. Les formats d'entrée réellement lus

### 1.1 Le format dominant : `.cfg.bin.json` (déjà décodé, pas le binaire)

Le pipeline **ne lit pas le `.cfg.bin` du jeu**. Il lit son **dump JSON**, produit en amont par
un autre outil (le C# `IECODE` / `iecode cfgbin-db build`, hors de ce paquet).

- Occurrences du littéral `.cfg.bin.json` dans le périmètre : **193**
  (`rg -oN '\.cfg\.bin\.json' <périmètre> | wc -l`), contre **6** pour `.cfg.bin` nu.
- Le catalogue de fichiers visés est en dur : `core/paths.ts:137-203` (`FILES`, 41 noms **versionnés**,
  ex. `chara_base_1.03.98.00.cfg.bin.json`) et **34** couples `(catégorie, préfixe)` résolus
  dynamiquement par `findConfigFile` (`rg -oN 'findConfigFile\("([^"]+)",\s*"([^"]+)"' ... | sort -u | wc -l`).
- La résolution de version est faite deux fois, différemment :
  - `core/data-loader.ts:105` — glob `<préfixe>*.cfg.bin.json` puis `.sort().pop()` (**tri
    lexicographique**, faux dès que les segments ne sont plus zéro-paddés) ;
  - `core/paths.ts:217-260` (`resolveGameDataFile`) — tri **numérique segment par segment**, avec
    une regex qui refuse les collisions de préfixe (`chara_param` ne capte pas `chara_param_table_config`).
  Les deux coexistent ; c'est le premier qui sert dans `loadConfig*`.
- La forme lue est un arbre `{ entries: ConfigNode[] }`, `ConfigNode = { name, variables[], children[] }`,
  chaque variable typée `"Int" | "Float" | "String" | "List"` avec sa **valeur en chaîne**
  (`core/config-parser.ts:10-24`). C'est la forme « iecode », pas la forme brute de `niers decode`.

État mesuré du corpus ici : `data/common` porte **71 080** `.cfg.bin.json`
(`fd -t f -g '*.cfg.bin.json' data/common | wc -l`), dont **5 863** sous `common/gamedata` et
**44 241** sous `common/text` (12 sous-dossiers : `ja en fr de es it pt zh_hans zh_hant common event map`).

### 1.2 Deuxième source : SQLite `cfgbin.sqlite` (voie rapide, optionnelle)

`core/cfgbin-db.ts` ouvre en lecture seule une base produite par `iecode cfgbin-db build`
(chemin `IEVR_CFGBIN_DB`, défaut `~/data/cache/cfgbin.sqlite`, ~2,3 Go), table
`cfgbin(path PK, cpk, size, parsed, format, error, json)`. `loadConfig*` l'essaie **d'abord**,
retombe silencieusement sur le filesystem en cas d'échec (`core/data-loader.ts:106-127, 133-157`).

- **Sur cette machine la base est ABSENTE** (`ls /home/ubuntu/data/cache/cfgbin.sqlite` → absent) :
  la voie SQLite ne s'exerce pas ici, tout passe par le filesystem.
- Piège documenté dans le code : le chemin `text/*` de la DB est **désactivé par défaut** parce que
  le parseur qui l'a remplie corrompt les chaînes (`vars[N] = null` au lieu du nom localisé),
  `core/cfgbin-db.ts:175-189`. Réactivation opt-in par `IEVR_CFGBIN_DB_TEXT=1`.

### 1.3 JSON pré-agrégés

- `all-gamedata/` — `core/gamedata.ts:52` (`GamedataLoader`), un fichier par liste, chacun
  `{ _meta, data[] }`, index attendu `_summary.json`. **Ici : 27 fichiers et `_summary.json` ABSENT**
  (`ls data/all-gamedata | wc -l`), donc `getSummary()` rend `{0, 0, []}` — un état vide qui ne lève rien.
- `entries/` — **37 JSON, 38 Mo** : ce sont les **sorties** figées d'une génération antérieure
  (`characters.json` 17 Mo / 5 991 entrées, `icon_inventory.json` 8,4 Mo / 36 341,
  `capsules.json` 5,0 Mo, `skills.json` 3,0 Mo / 2 697, `items.json` 2,0 Mo / 4 153).
  **7 sont vides** (`awakenings`, `keshins`, `souls`, `miximax`, `mode_changes`, `media_assets` = 0
  élément). Comptes : `for f in *.json; do jaq 'length' $f; done` dans `src/entries/`.
- `data/zukan/db_consolidated.json` (2,1 Mo) — le catalogue **scrapé** de `zukan.inazuma.jp`, chargé
  par `entities/chara-json.ts:36`, indexé par `modelId`. Le scraping lui-même est **hors** de ce
  paquet (`@aphrody-code/zukan`) : ici on ne lit que son résultat sur disque.
- `data/zukan/zukan_mapping.json` — chargé depuis `DATA_ROOT/zukan/` par `core/data-loader.ts`
  (`loadZukanMapping`, `loadZukanOrder`). **Non vérifié** : `src/data/zukan_mapping.json` (233 Ko)
  n'est référencé par aucun `import` du périmètre — probable doublon mort, à confirmer côté SORTIE.

### 1.4 Bases communautaires embarquées dans le paquet (`src/data/`)

| Fichier | Taille | Lu par |
|---|---:|---|
| `item-bonus-db.json` | 57 Ko | `parsers/item-config.ts` (`join(here, "../data/item-bonus-db.json")`) |
| `spirits-db.json` | 76 Ko | aucun import mesuré dans le périmètre — **non vérifié** |
| `simulation-constants.json` | 64 Ko | aucun import mesuré dans le périmètre — **non vérifié** |
| `zukan_mapping.json` | 228 Ko | aucun import mesuré (cf. §1.3) |
| `data/drops/{battles,tables,treasures}.ts` | 6,5 Ko | `api/drops.ts`, `drops/index.ts` (moitié SORTIE) |

`item-bonus-db.json` n'est **pas** une donnée du jeu : c'est un relevé communautaire in-game
(commentaire `parsers/item-config.ts`). C'est de la donnée **exogène**, pas du décodage.

### 1.5 Formats binaires — présents mais NON APPELÉS

| Fichier | Ce qu'il décode | Appelé ? |
|---|---|---|
| `parsers/binary/cfgbin-parser.ts` (8,8 Ko) | `.cfg.bin` binaire, portage de `IECODE.Core/Formats/Level5/CfgBin/CfgBin.cs` | **non** |
| `parsers/binary/g4tx-parser.ts` (12,8 Ko) | G4TX / NXTCH → DDS | **non** |
| `parsers/hash/crc32.ts` (2,0 Ko) | CRC32 `0xEDB88320` | oui, par `cfgbin-parser.ts` seul |
| `core/lua-bytecode.ts` (14,9 Ko) | bytecode Lua 5.2 (`.lua.bin`) | **non** |

Mesure : `rg -nN "parseCfgBin\(|parseTextures\(|parseLuaBytecode\(" packages apps -g '*.ts'` ne rend
**qu'une** ligne, la définition elle-même. Ces quatre modules ne sont réexportés que par
`parsers/index.ts:26-59` (fichier lui-même en `@ts-nocheck`). **Le pipeline d'entrée ne lit aucun
octet de format Level-5 ; il lit du JSON.**

### 1.6 Ce que le périmètre ne fait PAS

- **Aucun accès réseau.** `rg -nN "fetch\(|https?://"` sur le périmètre rend **5** occurrences, toutes
  inertes : une URL de doc en commentaire, `CDN_URL` (`core/paths.ts:50`, une base d'URL d'images),
  deux URLs d'`openapi.ts`, un exemple en commentaire. Zéro `fetch` exécuté.
- **Aucun scraping.** `cheerio`, `@aphrody-code/bxc`, `fengari`, `web-tree-sitter`,
  `@supabase/supabase-js`, `commander`, `jsonwebtoken`, `picocolors`, `dotenv` sont déclarés dans
  `package.json` mais **jamais importés** depuis le périmètre entrée (cf. §4).

---

## 2. Les entités produites

### 2.1 L'entité pivot : `BaseCharacter` / `CharacterVariant`

`core/types.ts` déclare **29 items exportés** (`rg -nN '^export (interface|type|enum|const) \w+'`),
dont le modèle de personnage à deux étages :

| Entité | Identifiant | Champs notables |
|---|---|---|
| `BaseCharacter` | `charaId` (hash hex, ex. `0xA41870E9`) + `internalCode` (`c01000100`) | `names: LocalizedNames` (9 langues), `romanized`, `gender`, `uniformNumber`, `variants[]`, `bestRarity`/`bestRarityCode`, `slug`/`baseSlug`, `isBasara`, `image`, `icons`, `descriptions`, `series{id,type,name}`, `teamId`/`teamName`, `constellation{index,names}`, `nickname`/`ageGroup`/`schoolYear` (origine **zukan**, pas jeu) |
| `CharacterVariant` | `charaParamId` (hash hex) | `position`/`positionRaw`/`subPosition`, `element`/`elementRaw`, `rarity`/`rarityCode`, `growthPattern`, `skills[{learnLevel, skillId}]`, `stats: MultiLevelStats`, `heroType`, `zukanHash`/`zukanOrder`, `constellation` |
| `CharacterStats` | — | `kick control technique pressure physical agility intelligence` (7 stats) |
| `MultiLevelStats` | — | `lv1? lv30? lv50? lv99` |
| `LocalizedNames` | — | `ja en fr de es it pt zhHans zhHant` |

Autres entités de `core/types.ts` : `Skill`, `Team`, `Item`, `Basara extends Character`,
`BasaraBuild`, `HeroVariants`, `CharaParamRaw`, `DataManifest`, plus les tables `ElementId`,
`PositionId`, `RarityId` et leurs `*Names`.

Un second jeu d'entités, plus plat, vit dans `core/entities.ts:31-72` (`SkillEntity`,
`CharacterEntity`, `TeamEntity`, `FormationEntity`, `QuestEntity`, `ItemEntity`, tous
`extends BaseEntity {id, nameHash?, names?}`) et lit un dossier `data/entities` **absent ici**
— chemin d'ailleurs codé en dur à `join(__dirname, "../../../../../data/entities")`
(`core/entities.ts:14`), soit cinq niveaux au-dessus du module : une relique de l'ancienne
arborescence, à ne pas porter tel quel.

### 2.2 Les bases construites par `parsers/`

**39 fonctions `build*Database`** et **38 `load*Async`**
(`rg -oN 'export async function (build\w+)' parsers -g '*.ts' | wc -l`, idem `load\w+`), une par
famille : `buildActivityPhotoDatabase`, `buildBoostPlayerGroupDatabase`, `buildCapsuleDatabase`,
`buildChangeAuraSkillDatabase`, `buildCharaCostumeDatabase`, `buildCharaExpTableDatabase`,
`buildCharaMenuResourceDatabase`, `buildChatEmoteDatabase`, `buildCtrlCharaDatabase`,
`buildDictionaryDatabase`, `buildEmblemDatabase`, `buildEnjoyModeTeamDatabase`,
`buildExtendStoryDatabase`, `buildGalleryDatabase`, `buildGrowthTableDatabase`,
`buildInacodeDatabase`, `buildItemDatabaseAsync`, `buildNameplateDatabase`,
`buildNfcLotteryDatabase`, `buildOpponentTeamDatabase`, `buildOverrideSkillDatabase`,
`buildPassiveDatabaseAsync`, `buildPassiveSkillEffectDatabase`, `buildPerformanceDatabase`,
`buildPhaseTitleDatabase`, `buildRealSkillDatabase`, `buildSceneArchiveDatabase`,
`buildSkillDatabaseAsync`, `buildSkillTechnicDatabase`, `buildSpecialTacticsDatabase`,
`buildStadiumDatabase`, `buildSuperTacticsBaseDatabase`, `buildTeamBuildDatabase`,
`buildTelopWazaDatabase`, `buildTextDatabaseAsync`, `buildTextMapsAsync`, `buildTrickDatabase`,
`buildUniformDatabase`, `buildVideoWazaDatabase`.

`service.ts:34` (`createInagleService`) est le point d'assemblage : 8 de ces bases y sont
**paresseuses** (`service.ts:64-105`), le reste est construit à chaud.

### 2.3 Les identifiants — trois familles qui ne se recouvrent pas

1. **Hash hex** `0xXXXXXXXX` — CRC32 d'un nom, forme canonique de tout ce qui vient du jeu
   (`core/parser.ts:173`, `toHex`). C'est la clé de jointure principale.
2. **Code interne** `c01000010`, `whs00010` — la chaîne du jeu. `schemas/zod.ts:22`
   (`StringId = /^[a-z]{1,3}\d+$/`) la contraint.
3. **Slug** — dérivé du nom **anglais** (`entities/character.ts`, `generateSlug`). Ce n'est pas
   un identifiant du jeu : c'est une décision d'URL, et c'est là que naissent les collisions
   documentées ailleurs (969 collisions de `base_slug` pour 6 168 lignes).

---

## 3. Décodage (déjà en Rust) vs logique métier (nulle part ailleurs)

### 3.1 Ce qui est une RÉIMPLÉMENTATION pure et simple

| Module TS | Lignes | Équivalent Rust | Verdict |
|---|---:|---|---|
| `parsers/binary/cfgbin-parser.ts` | 8,8 Ko | `crates/engine/nie-formats/src/cfgbin.rs` | réimplémentation, **et code mort** (§1.5) |
| `parsers/binary/g4tx-parser.ts` | 12,8 Ko | `crates/engine/nie-formats/src/{g4tx_decode,nxtch}.rs` | réimplémentation, **code mort** |
| `parsers/hash/crc32.ts` + `core/hash.ts` | 4,0 Ko | `nie-formats/src/cfgbin.rs`, `nie-formats/tests/level5_hash.rs` | **doublon interne** (deux CRC32 dans le même paquet) + réimplémentation |
| `core/lua-bytecode.ts` | 14,9 Ko | `crates/engine/nie-lua/src/bytecode.rs` | réimplémentation, **code mort** |
| `core/config-parser.ts` | 4,3 Ko | forme iecode, cf. `nie_formats::cfgbin::to_iecode_json` | adaptateur de la forme JSON, pas un décodeur |

### 3.2 Recoupement des 73 parsers avec les 120 modules `nie-data`

Croisement par nom normalisé (`-`→`_`, suffixes `_config`/`_parser` retirés) :

```
fd -e rs . crates/engine/nie-data/src -x basename {} .rs | sort -u                       # 120
fd -e ts . packages/inagle/src/parsers -d 1 -x basename {} .ts | grep -v '\.test$' \
  | sed 's/-/_/g;s/_config$//;s/_parser$//' | sort -u                                    # 69
comm -12 …                                                                               # 41
comm -13 …                                                                               # 28
```

- **41 parsers TS ont un module `nie-data` du même concept** : `ability_learning basara belong_team
  capsule chara_base chara_costume chara_description chara_details chara_menu_resource chara_param
  chara_text chat_emote ctrl_chara dictionary emblems enjoy_mode_team extend_story formation gallery
  help inacode item mission opponent_team override_skill playstyle post quest scene_archive shop
  skill skill_technic special_tactics stadium telop_waza text trick trophy uniform unlock_condition
  video_waza`. Sur ces 41, un portage serait une **réécriture d'un travail déjà golden-testé côté Rust**.
- **28 n'ont pas d'homonyme dans `nie-data`** : `activity_photo boost_player_group change_aura_skill
  chara_exp_table constellation drop_rates drops event_subtitles gameplay growth_table hero index lua
  music nameplate nfc_lottery passive_skill passive_skill_effect performance phase_title real_skill
  star_sign story_text super_tactics_base system team_build universal_gamedata universal_text`.
  **Attention** : `nie-data` nomme ses modules *par concept*, pas par fichier ; plusieurs de ces 28
  sont vraisemblablement couverts sous un autre nom (`change_aura_skill_config`,
  `real_skill_config`, `team_build_config`, `super_tactics`, `exp`, `passives`, `event_subtitle`,
  `user_name_plate`, `nfc`, `music`, `system_unlock` existent côté Rust sous une orthographe voisine).
  **Le croisement par nom ne tranche pas** : il faut le refaire par **marqueur de fichier**
  (`grep -rl "<MARKER_LIST>" crates/engine/nie-data/src/`), comme le prescrit `CLAUDE.md`.
  Ce document ne l'affirme donc pas.

### 3.3 Ce qui est de la LOGIQUE MÉTIER et n'existe nulle part ailleurs

C'est la seule partie qui justifie un portage plutôt qu'une suppression.

| Domaine | Où | Ce que c'est |
|---|---|---|
| **Calcul de stats par interpolation** | `stat-calculator.ts` (`calculateSingleStat`, `calculateStats`, `generateGrowthCurve`, `calculateTotalPower`) | `lerp` + `Math.floor` sur 4 points d'ancrage (lv1/30/50/99) issus de trois tables (`GrowthTableLv1/Lv30/Main`), sélectionnées par `(mainPosition, subPosition, growthPattern, charaRank, playStyle)`. Le jeu ne stocke que les ancres : la courbe est une **reconstruction**, pas une lecture. |
| **Résolution rareté → rang de croissance** | `lib/rarity.ts` (`rarityCodeToName`, `rarityToGrowthRank`) | table de correspondance documentée (`0/1→Normal`, `2→Expérimenté`, `3→Émérite`, `5/6/7→Légendaire`, `20→BASARA`, `10→Héros posé par un script d'enrichissement`, pas par le jeu). Factorisé après divergence entre trois copies. |
| **Assemblage du personnage** | `entities/character.ts` (`buildCharacter`, `buildVariant`, 1 334 l. dont 36,9 Ko pour ce seul fichier) | **jointure de 13 sources** : `chara_base`, `chara_param`, `chara_text`, `chara_description`, `chara_details`, `belong_team`, `constellation`, `ctrl_chara`, `growth_table`, `hero_config`, `star_sign`, `zukan`, `universal_text`. C'est le cœur métier. |
| **Série depuis le préfixe de code** | `entities/character.ts` (`SERIES_BY_CODE_PREFIX`, `getSeriesFromInternalCode`) | `c01`→IE1, `c02`→IE2, `c03`→IE3… — une **convention observée**, validée contre 5 640 personnages de zukan, absente des fichiers du jeu. |
| **Layout de `chara_param`** | `parsers/chara-param.ts:71-118` | lecture des 9 slots de technique en **niveau d'abord** (index pairs 10..26, hash aux impairs 11..27). Le commentaire dit que l'ancienne lecture « 6 slots hash-first @9 » désalignait toutes les variantes. Ce genre de savoir n'est nulle part ailleurs — il est **payé en debug**, pas déduit du format. |
| **Nettoyage de texte du jeu** | `core/data-loader.ts` (`sanitizeText`) | furigana `[Kanji/Reading]`→Kanji, balises `<FLA:…>`/`<VAL:…>`→valeur, `<COL:…>`/`<CLO>` supprimées, séquences d'échappement, caractères de contrôle, normalisation des blancs. |
| **Romanisation japonaise** | `utils/romaji.ts` | Kuroshiro + analyseur morphologique Kuromoji (dictionnaire MeCab), Hepburn, `toRomajiTitleCase` découpe sur `・`. **Aucun équivalent Rust dans le dépôt.** |
| **Recherche floue** | `core/search.ts` | index uFuzzy (`intraMode: 1`, 1 insertion tolérée). |
| **Résolution d'images** | `core/images.ts` | index en mémoire (`faces uniforms skills items emblems auras classes common`) construit par scan récursif, plus une réécriture d'URL zukan → CDN. |
| **Normalisation des flottants localisés** | `core/config-parser.ts` (`parseFloatVar`) | remplace la **virgule décimale** française par un point — le dump JSON porte des flottants localisés. |

---

## 4. Dépendances externes réellement utilisées

Mesure : `rg -oN 'from "([^."][^"]*)"' -r '$1' <périmètre> -g '*.ts' | sort | uniq -c`.

| Paquet | Occurrences | Où / pourquoi |
|---|---:|---|
| `node:path` | 80 | construction de chemins, partout |
| `node:fs` | 43 | `existsSync`/`readFileSync`/`readdirSync` (chemins **synchrones** hérités) |
| `node:url` | 5 | `fileURLToPath(import.meta.url)` pour se localiser dans le paquet |
| `node:fs/promises` | 3 | `core/gamedata.ts`, `core/extract-lua-commands.ts` |
| `zod` | 2 | `schemas/zod.ts` — validation à l'exécution |
| `vitest` | 2 | tests |
| `zod-to-json-schema` | 1 | génération du schéma OpenAPI |
| `kuroshiro` + `kuroshiro-analyzer-kuromoji` | 1 + 1 | `utils/romaji.ts` — **seule dépendance vraiment irremplaçable** |
| `hono` + `hono/cors` | 1 + 1 | `adapters/hono.ts` — 9 routes HTTP |
| `bun:sqlite` | 1 | `core/cfgbin-db.ts`, via `createRequire` |
| `@streamparser/json` | 1 | `core/async-loader.ts` — parsing en flux au-delà de 5 Mo |
| `@leeoniya/ufuzzy` | 1 | `core/search.ts` |
| `node:os` / `node:module` / `node:buffer` | 1 chacun | `homedir()`, `createRequire`, tampons |

API Bun : **12 `Bun.file`** et **12 `Bun.Glob`**. `Bun.Glob.scanSync` sert de **shim de `stat`
synchrone** (`core/paths.ts:10-20`, `core/cfgbin-db.ts:59-72`) parce que `Bun.file().exists()` est
asynchrone — un contournement, pas une intention.

Déclarées dans `package.json` mais **jamais importées depuis ce périmètre** : `@aphrody-code/bxc`,
`@aphrody-code/zukan`, `cheerio`, `commander`, `dotenv`, `fengari`, `jsonwebtoken`, `picocolors`,
`web-tree-sitter`, `@supabase/supabase-js`. Elles appartiennent à la moitié SORTIE.

Variables d'environnement lues : `DATA_PATH` (×2), `NEXT_PUBLIC_ASSET_URL` (×2),
`NEXT_PUBLIC_USE_RAW_IMAGE_PATHS`, `INAGLE_DEBUG`, `IEVR_CFGBIN_DB`, `IEVR_CFGBIN_DB_TEXT`.

---

## 5. Les pièges d'un portage Rust

1. **La résolution de racine est implicite et silencieuse.** `resolveDataRoot()`
   (`core/paths.ts:28-48`) essaie `DATA_PATH`, puis `<paquet>/data`, puis `<cwd>/data`, et **retourne
   `<cwd>/data` même s'il n'existe pas**. Mesuré ici : `packages/inagle/data` est **absent** et
   `DATA_PATH` n'est **pas posée** → la racine dépend du **répertoire courant**. Un service lancé
   ailleurs lit un arbre vide et rend des listes vides sans une seule erreur. Le dépôt a déjà la
   bonne forme côté Rust : `nie_formats::vfs::resolve_game_dir()` / `NIE_GAME_DIR`. Ne pas reporter
   `DATA_PATH`.
2. **Le pipeline est adossé à un dump JSON, pas au VFS.** Il ne lit ni CPK ni `.cfg.bin`. Porté tel
   quel en Rust, il resterait dépendant d'une étape amont C#/iecode — alors que `nie-formats` sait
   ouvrir le VFS directement. Un portage fidèle **fossiliserait la mauvaise entrée**.
3. **`.sort().pop()` sur les noms versionnés.** `findConfigFile` (`core/data-loader.ts:115-123`)
   choisit la « dernière » version par tri **lexicographique** ; `resolveGameDataFile`
   (`core/paths.ts:248`) le fait numériquement. Deux sémantiques dans le même paquet : un portage
   doit choisir la seconde et le dire, sinon un patch du jeu fait basculer silencieusement le
   fichier lu.
4. **17 caches globaux mutables et un ordre d'exécution obligatoire.** `rg -nN '^let ' <périmètre>`
   rend **23** variables de module, dont 17 caches (`ctrlCharaCache`, `growthLv1Cache`,
   `growthTableCache`, `starSignCache`, `teamCache`, `seriesCache`, `constellationCache`,
   `_imageIndex`, `zukan*Cache`, `_db`/`_enabled`/`_*Stmt`…). `service.ts:36` doit appeler
   `preloadCharacterEnrichment()` **avant** `createCharactersAPI()` : sans cela
   `getCtrlCharaMapSync()` rend une carte vide et les personnages sortent appauvris **sans erreur**.
   En Rust cela devient un `struct` de contexte construit explicitement — c'est un gain, mais c'est
   une **réécriture d'architecture**, pas une traduction.
5. **`readFileSync` + `existsSync` partout (43 usages).** Le pipeline est synchrone dans son cœur
   et asynchrone en surface (`loadJSONAsync`, `@streamparser/json` au-delà de 5 Mo). Un portage
   Rust doit trancher : tout synchrone (simple, `rayon` pour le parallélisme) ou tout `async`.
   Le mélange actuel est la source des deux jeux de fonctions `loadX` / `loadXAsync` dupliquées.
6. **Kuroshiro/Kuromoji n'a pas d'équivalent Rust dans le dépôt.** La romanisation exige un
   analyseur morphologique japonais et son dictionnaire (~50 Mo de MeCab-IPADIC). C'est le seul
   point du périmètre qui **ne se porte pas** sans choisir une crate tierce (`lindera`, `vibrato`)
   et accepter des sorties potentiellement différentes. Les romaji déjà calculés valent mieux
   qu'une réimplémentation approximative.
7. **Les échecs sont avalés.** `loadJSON` rend `{data: [], _meta: {count: 0}}` sur `catch`
   (`core/data-loader.ts:69-71`) ; `loadConfigAsync` retombe du SQLite au disque sur `catch {}`
   sans trace ; `GamedataLoader` se contente d'un `console.warn` si sa racine manque. Trois
   variantes du faux vert : **un `Result` Rust changerait le comportement observable** de tous les
   consommateurs. À traiter comme une décision, pas comme un détail d'implémentation.
8. **`@ts-nocheck` sur `parsers/index.ts` et `parsers/item-config.ts`** : le type-checker ne couvre
   pas le fichier qui réexporte 73 parsers. Les signatures n'y sont donc pas vérifiées — ne pas s'en
   servir comme spécification.
9. **`core/entities.ts:14` code en dur `../../../../../data/entities`**, un dossier **absent ici**.
   Un portage qui recopie ce chemin produit un module qui ne trouve jamais rien.
10. **`entries/*.json` (38 Mo) sont des artefacts, pas des sources.** 7 sur 37 sont vides. Les
    porter comme entrée reviendrait à figer un instantané de mai/juin 2026.

---

## 6. Ce que je n'ai PAS pu vérifier

- **Le pipeline n'a pas été exécuté.** Aucune commande `bun` n'a été lancée : les comptes
  d'entités ci-dessus viennent des artefacts `entries/*.json`, pas d'une reconstruction.
- **Les 4 tests du périmètre n'ont pas été lancés.**
- **Le recoupement des 28 parsers « sans homonyme » avec `nie-data` n'est pas tranché** (§3.2) :
  il faut le refaire par marqueur de fichier, pas par nom de module.
- **`spirits-db.json`, `simulation-constants.json` et `src/data/zukan_mapping.json` n'ont aucun
  importateur mesuré dans le périmètre entrée** ; ils sont peut-être lus par la moitié SORTIE.
- **La base `cfgbin.sqlite` est absente d'ici** : le chemin SQLite de `core/cfgbin-db.ts` n'a donc
  pas pu être exercé, et le bug de corruption des chaînes `text/*` qu'il documente n'a pas été
  reproduit.
- **Je n'ai pas mesuré la conformité des valeurs** produites par un parser TS face au module
  `nie-data` correspondant. Dire « équivalent » ci-dessus veut dire « même concept », jamais
  « mêmes octets ».
