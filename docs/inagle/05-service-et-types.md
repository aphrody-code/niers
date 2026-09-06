# La couche de service et les types du wiki

> Périmètre exclusif : `packages/azalee`, `packages/types`, `packages/db`,
> `packages/nie-catalog`, `packages/nie-game`.
> `apps/azalee` est audité en parallèle par un autre agent : il n'est cité ici que comme
> **consommateur** (qui appelle quoi), jamais comme sujet.
> Mesures du **2026-09-06** sur le VPS Linux. **Aucune modification de code n'a été faite.**
> Chaque compte porte la commande qui l'a produit.

## 0. Volumétrie du périmètre

```bash
tokei packages/<p> -t TypeScript
```

| Paquet | Fichiers `.ts` | Lignes brutes | Lignes de code | Rôle déclaré |
|---|---:|---:|---:|---|
| `packages/azalee` | 67 | 11 547 | **7 992** | la bibliothèque du wiki (service, images, CDN, recherche) |
| `packages/db` | 16 | 8 819 | 8 331 | socle base ; **7 054 des 8 331 lignes sont `types.gen.ts`** |
| `packages/types` | 19 | 3 168 | 1 829 | types du site communautaire — **rien du jeu** (§ 2.4) |
| `packages/nie-catalog` | 10 | 2 142 | 1 349 | la façade des quatre gisements |
| `packages/nie-game` | 14 | 1 161 | 838 | la logique de jeu pure |

---

## 1. `packages/azalee` — les fonctions de service

### 1.1 La surface exportée, et qui la consomme

```bash
rg -n --no-heading '^export (async )?(function|const|class) ' packages/azalee/src -g '*.ts'   # 187
# puis, par symbole : rg -l -w <sym> packages apps -g '*.ts' -g '*.tsx' -g '!node_modules' -g '!<son propre fichier>'
```

**187 symboles exportés** (`function` / `const` / `class`, les `interface`/`type` exclus).
Ventilés par la **zone** qui les référence :

| Zone qui consomme | Symboles | Lecture |
|---|---:|---|
| un hôte réel (`apps/azalee`, `packages/azalee-tools`, `packages/mcp`, `apps/nie-bot`…) | **116** | vivants |
| **personne, nulle part** | **28** | code mort franc (§ 1.2) |
| seulement `apps/nie-web/src/legacy/` | **16** | le SAS `legacy` est **exclu du `tsconfig`** — mort en pratique |
| seulement un autre fichier de `packages/azalee/src` | 20 | interne : exporté sans raison publique |
| seulement `packages/azalee/test/` | 7 | n'existe que pour son propre test |

**44 exports sur 187 (23,5 %) ne sont atteints par aucun hôte** (28 + 16), et **71 sur 187
(38,0 %) ne sortent pas du paquet** (28 + 16 + 20 + 7).

### 1.2 Les 28 exports que personne ne référence

Vérification individuelle sur tout le dépôt (`rg -l -w <sym> apps packages crates` → **1 seul
fichier, sa propre définition**) pour `getSkillVideoUrl`, `STAT_KEYS`, `fetchAudioBank`,
`getCrossTable`, `resolveCrossAssetUrl`.

| Fichier | Symboles morts |
|---|---|
| `packages/azalee/src/cross/data.ts:32,36,81,90,105,121,126` | `getCrossStatus`, `getCrossAudio`, `getCrossTable`, `getCrossEnums`, `getCrossDataCounts`, `getCrossTypeFields`, `generateCSharpClass` |
| `packages/azalee/src/cross/data-shared.ts:15,93,114,158` | `CROSS_CYAN`, `KIND_TONE`, `TABLE_METADATA`, `humanizeClassName` |
| `packages/azalee/src/cross/assets-shared.ts:11,24,59,71` | `CROSS_CDN_BASE`, `getCrossCharacterVoices`, `getCrossLocalAssetUrl`, `resolveCrossAssetUrl` |
| `packages/azalee/src/images/utils.ts:132,136,579,700,707,928,1088` | `CDN_ASSETS`, `MENU_BUCKET_URL`, `getTeamEmblemUrl`, `ZUKAN_3D_MIRROR_AVAILABLE`, `getSkillVideoUrl`, `getCharacterModel360Url`, `listServedChrModelCodes` |
| `packages/azalee/src/cpk/live.ts:111,274` | `VfsError`, `findOrNull` |
| `packages/azalee/src/cpk/audio.ts:43` | `fetchAudioBank` |
| `packages/azalee/src/cpk/tree.ts:18` | `TREE_FILE_CAP` |
| `packages/azalee/src/net.ts:52` | `clearFetchCache` |
| `packages/azalee/src/wiki/chara-stats-shared.ts:36` | `STAT_KEYS` |

**Le sous-arbre `cross/` est mort à 15/17.** Ses trois fichiers exportent 17 symboles ; seuls
`getCrossTables` et `getCrossCatalogStats` sont lus, par
`packages/azalee-tools/src/server/serve.ts:15`. Il traîne avec lui **6,5 Mo** de JSON
(`du -sh packages/azalee/src/data` — dont `data/cross/*`), embarqués dans le paquet publié
(`"files": ["src", …]`, `packages/azalee/package.json`).

### 1.3 Les 16 exports que seul le SAS `legacy` atteint

`apps/nie-web/src/legacy/` est explicitement **hors du `tsconfig`** (CLAUDE.md : « un SAS — le
code sorti du wiki qui attend d'être réécrit »). Ce qui n'est référencé que de là n'est compilé
par personne : `lsOrNull`, `texContainer` (`cpk/live.ts:179,256`), les 4 de `cpk/models.ts`
(`MODEL_FAMILIES`, `modelFamily`, `modelTextureDir`, `modelHref`), les 5 de `cpk/tree.ts`
(`isUnloaded`, `fetchChildren`, `setChildren`, `findNode`, `ancestorPaths`), les 4 de
`cpk/audio.ts` (`fetchAudioBankOrNull`, `audioBankKind`, `audioBankKindLabel`,
`voiceBankCharacterCode`), `getChrModelTextureUrl` (`images/utils.ts:1104`).

### 1.4 Ce que chaque module interroge

```bash
rg -o '\.from\("([a-z_]+)"' -r '$1' packages/azalee/src/<f> | sort -u
```

| Module | Tables lues (via le client injecté) | Autre source |
|---|---|---|
| `wiki/service.ts` (3 005 l, **le cœur**) | `inagle_characters`, `inagle_skills`, `inagle_items`, `inagle_auras`, `inagle_keshins`, `inagle_passives`, `inagle_teams`, `inagle_tactics`, `inagle_coordinators`, `inagle_gallery`, `inagle_override_skills`, `inagle_special_tactics`, **`tweets`** | 4 JSON figés : `change-aura-skills`, `menu-gallery-manifest`, `item-enrichment`, `passives-full` |
| `wiki/teams.ts` | `inagle_teams`, `inagle_characters`, `inagle_uniforms` | `data/emblem-crc-map.json` |
| `wiki/drops.ts` | `inagle_drops`, `inagle_drops_battles`, `inagle_drop_rates`, `inagle_items` | — |
| `wiki/coaches.ts` | `inagle_characters`, `inagle_coordinators`, `inagle_manager_passives` | — |
| `wiki/gacha.ts` | `inagle_capsules`, `inagle_costumes` | — |
| `wiki/shops.ts` | `inagle_shops`, `inagle_items` | — |
| `wiki/quests.ts` / `trophies.ts` / `stadiums.ts` / `invocation.ts` | `inagle_quests` / `inagle_trophies` / `inagle_stadiums` / `inagle_constellations` | `stadiums.ts` et `invocation.ts` tapent aussi le CDN en dur |
| `wiki/chara-stats.ts` | — | `https://cdn.rosegriffon.fr/cfg` (fichier du jeu) |
| `cpk/live.ts`, `cpk/audio.ts`, `cpk/models.ts`, `cpk/shared.ts` | — | HTTP, chemins construits par **`@niers/catalog/jeu`** (les seuls du paquet à le faire) |
| `cross/data.ts` | — | 6 JSON de `src/data/cross/` |
| `rag.ts` | — | `@rosegriffon/db/redis` (embeddings + vector store) |
| `game/roster-resolver.ts` | — | `fetch("/api/save/resolve-roster")` — une route d'`apps/azalee` |
| `images/utils.ts` (42 exports) | — | `https://cdn.rosegriffon.fr/...` **en dur** (§ 3.4) |

**`tweets` n'existe pas dans le miroir SQLite** :
`sqlite3 var/mirror.sqlite "select count(*) from sqlite_master where name='tweets'"` → **0**,
alors que la table existe bien en Postgres (`/tmp/gen_live.ts:286`). `wikiService.getTweets` et
`getTweetById` (`wiki/service.ts:2816,2828`) répondent donc sur le wiki et **échouent hors ligne**,
sur le seul backend que l'outillage et les tests savent monter.

### 1.5 `wikiService` : 35 méthodes, 5 mortes

```bash
rg -n '^\t(async )?[a-zA-Z0-9]+\(' packages/azalee/src/wiki/service.ts   # 35
rg -c "wikiService\.<m>" apps packages -g '*.ts' -g '*.tsx' -g '!node_modules'
```

| Appels | Méthodes |
|---:|---|
| **0** | `getCharactersLearningSkill:1918`, `getKeshinModelGallery:1027`, `getPassiveFamily:2933`, `getTeam:2485`, `getTeamPassives:2928` |
| 1 | `findAuraByName`, `getAllBaseSlugParams`, `getAllTeams`, `getCharacterForms`, `getCoordinatorPools`, `getGalleryCategoryCounts`, `getOverrideSkillsForSkill`, `getRandomTeamPools`, `getTweetById`, `getTweets`, `groupVariants`, `mapDbCharacterToBase` (12) |
| 2 à 12 | 16 méthodes |
| 20 / 25 | `getSkill`, `getCharactersList` |

`getTeam` est mort alors que `wiki/teams.ts` exporte un `getTeamDetail` vivant : deux chemins
pour la même entité, un seul emprunté.

### 1.6 Quatre sous-chemins déclarés dans `exports` ne résolvent vers rien

```bash
jq -r '.exports|keys[]' packages/azalee/package.json   # puis test d'existence de la cible
```

`packages/azalee/package.json` déclare `./config`, `./server`, `./remote`, `./icon-index` —
**aucun des quatre fichiers n'existe** dans `src/` (ils ont migré vers `@niers/azalee-tools` au
lot J2). `src/index.ts:12` renvoie d'ailleurs le lecteur vers
`@rosegriffon/azalee/server`, qui n'est plus servable.

Trois spécificateurs sont **importés** sans cible, tous depuis le SAS `legacy` :
`@rosegriffon/azalee/cpk/video` (5 fichiers, `apps/nie-web/src/legacy/app/videos/*` — le fichier
vit maintenant en `packages/azalee-tools/src/cpk/video.ts`), `@rosegriffon/azalee/cpk`,
`@rosegriffon/azalee/game-text`, `@rosegriffon/azalee/wiki/game-text`. Ils ne cassent aucun build
**parce que `legacy` est hors du `tsconfig`** — c'est-à-dire pour une raison sans rapport.

---

## 2. Les types

### 2.1 Générés contre écrits à la main

| Origine | Compte | Où |
|---|---:|---|
| **Générés** depuis Postgres | **1 fichier, 7 054 lignes** — 147 tables, 6 vues, ~1 640 colonnes | `packages/db/src/types.gen.ts`, produit par `packages/db/scripts/types-gen.ts` (endpoint `/generators/typescript` de pg-meta, `127.0.0.1:8813`) |
| **Écrits à la main** dans la lib du wiki | **89** `export interface` / `export type` | `rg -c '^export (interface\|type) ' packages/azalee/src` |
| Écrits à la main, hors jeu | 19 fichiers, 1 829 l | `packages/types` (§ 2.4) |

**Le type généré n'est presque pas utilisé.** Seuls **2 fichiers** de `packages/azalee/src`
importent `@rosegriffon/db` (`wiki/service.ts`, `rag.ts`), et `Database["public"]["Tables"][…]`
n'y apparaît que **8 fois** (`service.ts:34-42` : `inagle_items`, `inagle_skills`,
`inagle_auras`, `inagle_keshins`, `inagle_souls`, `inagle_awakenings`, `inagle_miximax`,
`inagle_mode_changes`). Les 89 interfaces manuscrites décrivent le reste — `Trophy`, `Quest`,
`Coach`, `ShopDetail`, `Stadium`, `CapsulePrize`… — sans aucun lien vérifié avec la base.

### 2.2 La dérive, mesurée contre la base vivante

```bash
curl -s 'http://127.0.0.1:8813/generators/typescript?included_schemas=public' -o /tmp/gen_live.ts
tail -n +16 packages/db/src/types.gen.ts > /tmp/gen_committed.ts   # retire l'en-tête ajouté par le script
diff /tmp/gen_committed.ts /tmp/gen_live.ts | wc -l                # 3 412
```

| Mesure | Fichier commité | Base vivante |
|---|---:|---:|
| Tables typées | **147** | **301** |
| Vues | 6 | 6 |
| Colonnes (sur les 147 tables communes) | 1 640 | 1 642 |

- **154 tables existent en base et ne sont pas typées** : **153 `inagle_cross_*`** (le jeu
  *Inazuma Eleven Cross*, poussé après la dernière régénération) et `niers_schema_migrations`.
- **0 table typée n'a disparu de la base.**
- **2 colonnes manquent au type** : `inagle_characters.uniform_number` et
  `inagle_characters.wiki_sections`. Une page qui les lit doit passer par un `as` — le type ne les
  connaît pas.
- Une seule divergence purement cosmétique : le générateur local rend `Relationships: []` là où le
  fichier commité porte les clés étrangères (ex. `avatar_saves_user_id_fkey`). C'est une
  différence de **générateur**, pas de schéma.

### 2.3 Y a-t-il un type qui ment ?

```bash
# extraction des blocs Row des deux fichiers, comparaison colonne par colonne
awk -f /tmp/rows.awk … ; awk 'NR==FNR{t[$1"|"$2]=$3;next}{…}'
# → communes=1640 divergences=0
```

**Sur la nullabilité et le type scalaire : non, aucun.** Les **1 640 colonnes communes** portent
exactement la même déclaration des deux côtés — aucun `string` là où la base dit nullable, aucun
`| null` de trop. C'est le mérite direct de la doctrine inscrite en tête du fichier (« toute
correction se fait EN BASE puis par régénération ») : il n'y a **aucune retouche manuelle** à
mentir.

**Le mensonge est ailleurs, et il est par omission** :
1. **154 tables absentes du type** — une page qui interroge `inagle_cross_*` n'a pas un type faux,
   elle n'a **pas de type du tout**, et `.from("inagle_cross_…")` ne compile qu'en `as any`.
2. **Les 89 interfaces manuscrites d'`azalee`** ne sont contrôlées par rien : elles décrivent des
   lignes de la base sans en dériver. C'est là que se trouve le risque de divergence réel, et il
   n'est mesurable qu'en lisant chaque `select` — le générateur ne les voit pas.
3. **`tweets` est typée et n'existe pas dans le miroir** (§ 1.4) : le type est vrai en production
   et faux hors ligne.

### 2.4 `packages/types` n'est pas dans le chemin des données du jeu

Ses 19 fichiers décrivent le **site communautaire** : `bot.ts` (659 l), `cron.ts` (506 l),
`experience.ts` (333 l), `instagram.ts` (284 l), `profil.ts`, `roles.ts`, `article.ts`, `stream.ts`.
Aucun n'évoque un personnage, une technique ou un objet. Ses consommateurs
(`rg -l '@rosegriffon/types'`) sont `packages/cron` (5 fichiers), `packages/ui`, `packages/auth`,
`packages/db/src/services/stats.ts` et 5 fichiers d'`apps/azalee`, tous côté rédaction/Discord.
**Ce paquet ne concerne pas l'audit du wiki de données ; il ne doit pas être mêlé au portage.**

---

## 3. `packages/nie-catalog` — la façade des quatre gisements

### 3.1 Ce qu'elle sait faire aujourd'hui

| Gisement | Module | Fonctions publiques | Support |
|---|---|---|---|
| `jeu` | `src/jeu.ts` (757 l) | **52 constructeurs d'URL/chemin** (`cheminFiche`, `cheminTexture`, `cheminModeleComplet`, `urlExport`…), `jeuJoignable`, 5 formateurs, 8 DTO (`FilmDto`, `AudioBank`…) | HTTP vers `nie-model-serve` |
| `extrait` | `src/extrait.ts` (168 l) | `tables`, `requete`, `ligne`, `personnage`, `chercherPersonnages`, `technique`, `assets` | `var/mirror.sqlite` en lecture seule |
| `re` | `src/re.ts` (134 l) | `fonctions`, `fonctionA`, `fonctionsCitant`, `classes`, `couverture`, `BINAIRE_REFERENCE` | `var/niers.sqlite` |
| `anime` | `src/anime.ts` (130 l) | `episode`, `saison`, `chercherEpisodes`, `etatAnime` | `data/anime/episodes.db` |
| **jointures** | `src/synergie.ts` (282 l) | `fichiersDe`, `personnage`, `personnageComplet`, `film`, `technique`, `chercher` — chaque lien porte sa `Confiance` (`cle`/`prefixe`/`texte`) | les quatre |
| résolution | `src/sources.ts` (153 l) | `sources`, `racineDepot`, `oublierSources` — chaque source rend `emplacement: null` **et** la liste de ce qui a été essayé | — |
| état | `src/index.ts` | `etat()` — **mesure le contenu**, pas l'existence du fichier | — |

C'est une façade **complète en lecture** et **muette en écriture** : aucune fonction n'écrit.

### 3.2 Qui l'emprunte

```bash
rg -l '@niers/catalog' packages apps crates -g '!node_modules'
```

**6 fichiers de code** l'importent : `packages/azalee/src/cpk/{live,audio,models,shared}.ts`,
`packages/asset-source/src/{url-conventions,web-source}.ts`,
`packages/azalee-tools/src/{config.ts,cpk/video.ts}` — et `apps/inacord/src-tauri/src/lib.rs`
côté Rust.

**`apps/azalee` ne l'importe pas une seule fois** (`rg -c '@niers/catalog' apps/azalee` → aucun
résultat). Le wiki n'emprunte donc pas la façade : il a son **troisième chemin**, un client
Supabase injecté dans `packages/azalee/src/db/provider.ts` par
`apps/azalee/lib/azalee-runtime.ts:19` (`setDatabaseProvider(createClient)`).

### 3.3 Les violations, comptées

CLAUDE.md interdit de « rouvrir une de ces bases à la main ».

| Forme | Compte | Où |
|---|---:|---|
| **Ouverture directe d'un SQLite dans `apps/azalee`** | **0** | le wiki ne touche aucun fichier de base — le lot J2 a réussi |
| Ouvertures directes ailleurs dans le dépôt | **7 paquets** | `packages/mcp/src/{resources.ts:120,171, tools/db.ts:68}`, `apps/nie-mcp/src/kb.ts:62`, `packages/ietv/src/cache.ts:123`, `packages/wonderbot/src/progression.ts:335`, `apps/nie-web/src/legacy/app/api/ietv/route.ts:70`, `packages/inagle/src/core/cfgbin-db.ts` |
| **`.from("inagle_…")` en direct dans une page ou une action du wiki** | **71 occurrences, 17 fichiers** | contournement de `wikiService`, pas de la façade |

Détail des 71 (`rg -o '\.from\("inagle_[a-z_]+"' apps/azalee | cut -d: -f1 | sort | uniq -c`) :
`app/dashboard/page.tsx` (13), `app/actions/search.ts` (10), `app/actions/translate.ts` (7),
`app/page.tsx` (6 — l'accueil lit six tables en direct), `scripts/get-test-entities.ts` (5),
`app/dashboard/zukan-review/page.tsx` (5), `app/dashboard/database/images/page.tsx` (5),
`app/dashboard/database/verification/page.tsx` (4), `app/tools/compare/page.tsx` (3),
`app/sitemap.ts` (3), `lib/api-client.ts` (2), `app/skill/[id]/page.tsx` (2),
`app/dashboard/database/page.tsx` (2), `lib/wiki/exp-table.ts` (1),
`app/api/save/resolve-roster/route.ts` (1), `app/actions/zukan-admin.ts` (1),
`app/actions/teams.ts` (1).
Le total tous schémas confondus est de **278** `.from(` dans `apps/azalee` — les 207 autres visent
le site communautaire (`articles`, `profiles`, `notifications`…), hors périmètre.

**La lecture juste : il n'y a pas de violation de la façade, il y a une façade que le wiki
ignore.** `@niers/catalog` sait lire `extrait` (le même miroir), et le wiki lit Postgres par un
quatrième chemin. Deux façades — `wikiService` et `catalogue` — pour un seul gisement.

### 3.4 La duplication d'URL, chiffrée

`packages/nie-catalog/src/jeu.ts` expose **52** constructeurs de chemin/URL et est le seul endroit
qui connaît `BASE_JEU_DEFAUT`. En parallèle, `packages/azalee/src/images/utils.ts` exporte
**42 symboles** dont `CDN_URL` (`:131`, lu de `NEXT_PUBLIC_ASSET_URL`) et une trentaine de
constructeurs (`getCharacterFaceUrl`, `getSkillImageUrl`, `getKeshinModelGlbUrl`, …), et trois
modules de `wiki/` écrivent `https://cdn.rosegriffon.fr/...` **en dur**
(`stadiums.ts`, `service.ts`, `invocation.ts`, plus `text/gaiji.ts` et `images/utils.ts`).
Les mêmes conventions d'URL sont donc écrites **trois fois** : `@niers/catalog/jeu`,
`azalee/images/utils.ts`, et à la main dans `wiki/*`.

---

## 4. `packages/nie-game` — ce qui est déjà en Rust

### 4.1 Ce qu'il y a exactement

**14 fichiers, 1 161 lignes brutes, 838 lignes de code, 49 symboles exportés**
(`rg -n '^export (async )?(function|const|interface|type|class) ' packages/nie-game/src`).

| Module | Lignes | Ce qu'il porte |
|---|---:|---|
| `game/formations.ts` | 287 | `FORMATIONS`, `GAME_FORMATIONS`, `LEGACY_FORMATIONS`, `BENCH_SLOTS`, `ROLE_COLORS`, `ROLE_LABELS`, types `Formation`/`PositionCoord` — **des coordonnées de placement en dur** |
| `text/translations.ts` | 191 | `SHOP_FR`, `TACTIC_FR`, `EFFECT_FR`, `translateEffect`, `tacticSlug` |
| `text/format-description.ts` | 171 | `formatDescription`, `formatJapaneseName`, `hasUnresolvedTags` — le balisage `[…]` du texte du jeu |
| `game/team-rules.ts` | 163 | `getPositionMatchFactor`, `recalculateMemberStats`, `calculateElementSynergies` |
| `text/gaiji.ts` | 88 | `GAIJI`, `GAIJI_ATLAS`, `GAIJI_ATLAS_VFS`, `GROWTH_TYPE_GLYPHS`, `GROWTH_TYPE_LABEL` |
| `text/aura-translations.ts` | 69 | `HISSATSU_TYPE_FR`, `HISSATSU_ELEMENT_FR`, `BUFF_EFFECT_FR`, `translatePassiveEffect` |
| `game/team-code.ts` | 63 | `encodeTeamCode`/`decodeTeamCode` (base64 UTF-8) |
| `game/team-types.ts` | 39 | 4 interfaces (`TeamMember`, `TeamData`, `SavedTeam`, `TeamMemberStats`) |
| `text/japanese-detect.ts` + `japanese-romaji.ts` | 58 | `containsJapanese`, `stripRubyAnnotations`, `japaneseToRomaji` (wanakana) |
| `text/download-filename.ts` | 13 | `downloadName` |

Il est **réellement partagé** : `apps/inacord` (7 fichiers), `packages/inacord-ui`,
`apps/nie-web`, et `packages/azalee/src/{game,text}/*.ts` qui ne sont plus que **12 shims**
d'une ligne (`export * from "@niers/game/…"`, cf. `packages/azalee/src/game/team-code.ts:2`).

### 4.2 Ce que le Rust sait déjà faire

| Besoin TS | Équivalent Rust | État |
|---|---|---|
| calcul de stat par niveau (`inagle/stat-calculator.ts`) | `crates/engine/nie-core/src/stats.rs:107,183` (`calculate_single_stat`, `calculate_stat_block`), `growth.rs:293,355` (`calculate_stats`, `generate_growth_curve`) | **porté** — commit `8d1f521` du jour |
| rareté → rang de croissance, libellés | `nie-core/src/stats.rs:216,257,286,304,317` (`rarity_to_growth_rank`, `rarity_code_to_name`, `LIBELLES_POSITION_INAGLE`, `LIBELLES_RANG_INAGLE`, `LIBELLES_STATS`) | **porté** (même commit) |
| moteur de comparaison de variantes | `nie-core/src/comparaison.rs` (553 l) | **porté** — commit `d77d34f` du jour |
| table d'expérience | `nie-core/src/exp.rs` (`ExpTable`) | porté |
| tactiques (mode, priorités, IA) | `nie-core/src/tactics.rs` (372 l), `nie-data/src/{special_tactics,super_tactics,ai}.rs` | porté |
| formations | `nie-data/src/formation.rs:379` (`parse_formation_config` → `SoccerFormationInfo`, `SoccerFormPlacementInfo`, courbes de ligne) — **lit les vraies coordonnées du jeu** | porté, et **meilleur que le TS** |
| balisage gaiji du texte | `nie-data/src/text.rs:261-300` (`split_markup` sépare le texte et rend les noms de gaiji) | porté partiellement — le **rendu** du glyphe est ailleurs (`nie-formats`) |
| libellés de formation « 4-4-2 » | `crates/tools/nie-wiki/src/query.rs:1075` | doublon TS/Rust déjà existant |

### 4.3 Ce qui reste APRÈS les portages en cours

Le lot parallèle prend `stat-calculator.ts`, `rarity.ts`, `comparison-engine.ts`, `optimizer.ts`,
`zukan/matcher.ts` — **tous dans `packages/inagle`, aucun dans `packages/nie-game`**. Mon
périmètre est donc **intact après eux**. Ce qui reste, par ordre de valeur :

| Reste | Lignes | Pourquoi ce n'est pas encore en Rust |
|---|---:|---|
| `game/team-rules.ts` — `getPositionMatchFactor`, `recalculateMemberStats`, `calculateElementSynergies` | 163 | **aucune trace en Rust** (`rg -l 'position_match\|element_synerg' crates` → 0). C'est de la **règle de jeu**, exactement ce que `nie-core` doit porter, et c'est reversable |
| `text/translations.ts` + `aura-translations.ts` — 8 tables FR | 260 | ce sont des **traductions faites main**, pas des données du jeu : à porter en **données** (table servie), pas en code |
| `game/formations.ts` — coordonnées en dur | 287 | **le vrai gisement est `nie-data/src/formation.rs`** ; le TS est une copie manuelle à supprimer une fois la route servie (§ 5) |
| `text/format-description.ts` | 171 | le balisage est déjà parsé côté Rust (`nie-data/src/text.rs`) ; ce qui manque est le **rendu HTML**, propre au front |
| `text/gaiji.ts` — atlas + glyphes de croissance | 88 | l'atlas est une **ressource** (VFS), pas du code : à servir |
| `game/team-code.ts` — codec base64 du code d'équipe | 63 | format **inventé par le wiki**, pas par le jeu : rien à porter, il doit rester en TS |
| `japanese-romaji.ts`, `japanese-detect.ts`, `download-filename.ts` | 71 | dépendent de `wanakana` ; **aucune valeur** à porter |

**Verdict** : sur 838 lignes de code, **163 méritent le Rust** (`team-rules.ts`), **375 doivent
devenir des données servies** (formations + traductions + gaiji), et **300 restent
légitimement en TypeScript** (codec, romaji, rendu de balisage).

---

## 5. La frontière — ce que `nie-site` devrait exposer

### 5.1 Ce qui existe déjà, vérifié

```bash
sed -n '/^declarer_routes! {/,/^}/p' crates/tools/nie-site/src/app.rs | rg -c '^\s*"'   # 44
```

**44 routes** sont déclarées dans la source (la macro `declarer_routes!` monte **et** liste, cf.
`crates/tools/nie-site/src/app.rs:72`). Dont, pour ce qui nous concerne :
`/api/v1/chara` (table `inagle_characters`, **12 colonnes**, 4 facettes, 6 tris —
`routes/api_v1.rs:30,33,241,251`), `/api/v1/{vue}` (4 vues : textures, modèles, sons, vidéos —
`vfs_index.rs:46`), `/f/{*chemin}`, `/b`, `/b/{*prefixe}`, `/api/v1/recherche`,
`/api/v1/donnees/{familles,famille/{cle},*chemin}` (**121 familles**, `routes/donnees.rs:70`),
`/api/v1/formats/{capacites,decode}`, `/api/v1/lua/*`, `/api/v1/3d/*`, `/api/v1/episodes`,
`/api/v1/couverture`.

> **Le service en production est en retard sur la source.** Le process écoutant sur `8085` a
> démarré à **10:00:08** ; le binaire a été relié à **12:23:46**.
> Mesuré : `/couverture` → 200, `/b` → 200, mais `/api/v1/recherche` → **404** et
> `/api/v1/donnees/familles` → **404** (`{"genre":"introuvable"}`). Les routes du jour existent
> dans le dépôt et **ne sont pas servies**. Aucune proposition ci-dessous ne doit être jugée
> « manquante » sur la foi d'un `curl` avant un redémarrage.

### 5.2 Les routes qui manquent vraiment

Règle appliquée : une route n'est proposée que si **aucune** des 44 ne la couvre, et que la
donnée existe déjà dans un gisement joignable (le miroir, ou le VFS). Les comptes viennent de
`sqlite3 var/mirror.sqlite`.

| Route proposée | Ce qu'elle rendrait | Ce qu'elle remplacerait | Gisement (lignes mesurées) |
|---|---|---|---|
| `/api/v1/entites/{table}` + `/api/v1/entites/{table}/{id}` | liste paginée + fiche, facettes, tri — **générique sur les 220 tables du miroir**, colonnes nommées par table | **28 des 35 méthodes de `wikiService`**, et les 71 `.from("inagle_…")` des pages | `var/mirror.sqlite`, 220 tables |
| `/api/v1/skills` (ou la forme générique ci-dessus) | technique + variantes + vidéos + override | `getSkill` (20 appels), `getSkillsList` (12), `getSkillsByIds`, `getOverrideSkillsForSkill` | `inagle_skills`, `inagle_skill_videos`, `inagle_override_skills` (33) |
| `/api/v1/items` | objet + enrichissement + catégorie | `getItem` (8), `getItemsList` (11) | `inagle_items` |
| `/api/v1/auras` | keshin / soul / awakening / miximax / mode-change **unifiés** | `getAura` (5), `getAurasList` (9), `findAuraByName`, `getCharacterAuras` — 6 tables aujourd'hui jointes en TS | `inagle_auras` + 5 |
| `/api/v1/equipes` | équipe, effectif, uniforme, emblème | `wiki/teams.ts` (3 fn) **et** `wikiService.getTeam`/`getAllTeams` — les deux chemins | `inagle_teams`, `inagle_uniforms` (627) |
| `/api/v1/formations` | les formations **lues dans le fichier du jeu**, pas recopiées | `packages/nie-game/src/game/formations.ts` (287 l) | `nie-data/src/formation.rs`, déjà écrit — **c'est du câblage** |
| `/api/v1/regles/stats` | stat par niveau, courbe, rang de rareté, facteur de poste, synergie d'élément | `azalee/src/game/stats-interpolation.ts`, `wiki/chara-stats-shared.ts`, `nie-game/src/game/team-rules.ts` | `nie-core::{stats,growth}` **déjà porté**, sauf le facteur de poste et la synergie (§ 4.3) |
| `/api/v1/texte/{langue}/{cle}` | le texte du jeu traduit, gaiji **résolus** | `text/translations.ts`, `aura-translations.ts`, `format-description.ts`, `text/gaiji.ts` | `nie-data/src/text.rs` (`split_markup`) + `common/text/` (44 241 fichiers) |
| `/api/v1/boutiques`, `/quetes`, `/trophees`, `/stades`, `/gacha`, `/drops`, `/coachs` | les sept familles sans aucune route | `wiki/{shops,quests,trophies,stadiums,gacha,drops,coaches}.ts` (~20 fn) | 2 331 / 182 / 347 / 81 / 740+577 / 98+177 / 102+80 lignes |
| `/api/v1/recherche/entites` | recherche **dans les données**, pas dans les chemins du VFS | `search/smart-search.ts` (mort), `app/actions/search.ts` (6 tables en direct) | miroir |
| `/api/v1/assets/urls` (ou un manifeste de conventions) | la table des conventions d'URL, **une seule fois** | les 3 copies du § 3.4 | `@niers/catalog/jeu` fait déjà autorité |

### 5.3 Ce qu'il ne faut PAS router

- **`cross/*`** (§ 1.2) : 15 exports morts sur 17 et 153 tables non typées. Le sujet n'est pas
  une route, c'est une décision — garder ou supprimer le hub *Cross*.
- **`team-code.ts`** : format du wiki, pas du jeu.
- **`rag.ts`** : dépend de Redis et d'un sidecar d'embeddings, hors de la stack gelée d'Aphrody.
- **`tweets`** : donnée éditoriale du site communautaire, absente du miroir — elle n'a rien à
  faire dans une API du jeu.

### 5.4 L'ordre qui fait tomber le plus de code

1. `/api/v1/entites/{table}` — générique, une seule route, **28 méthodes de `wikiService` et 71
   accès directs** deviennent inutiles.
2. `/api/v1/regles/stats` — `nie-core` est **déjà** écrit ; c'est du câblage, et il supprime la
   dernière logique de jeu écrite en TypeScript.
3. `/api/v1/formations` — supprime 287 lignes de coordonnées recopiées à la main, remplacées par
   la lecture du fichier du jeu.
4. `/api/v1/texte/{langue}/{cle}` — supprime 348 lignes de traduction et de balisage, et rend le
   texte au gisement qui le porte.

---

## 6. Ce qu'il faut retenir

| Constat | Chiffre | Preuve |
|---|---:|---|
| Exports d'`azalee` que personne n'atteint | **44 / 187** | § 1.1-1.3 |
| Sous-arbre `cross/` mort | 15 / 17 exports, 6,5 Mo de JSON embarqués | § 1.2 |
| Méthodes de `wikiService` jamais appelées | 5 / 35 | § 1.5 |
| Sous-chemins d'`exports` sans cible | 4 déclarés + 3 importés | § 1.6 |
| Tables en base et non typées | **154 / 301** | § 2.2 |
| Colonnes divergentes en type ou nullabilité | **0 / 1 640** | § 2.3 |
| Ouvertures de base à la main dans `apps/azalee` | **0** | § 3.3 |
| Accès `.from("inagle_…")` contournant `wikiService` | **71**, 17 fichiers | § 3.3 |
| Fichiers important `@niers/catalog` | 6 (+1 Rust) ; `apps/azalee` : **0** | § 3.2 |
| Logique de `nie-game` qui mérite le Rust | **163 l / 838** | § 4.3 |
| Routes déclarées par `nie-site` | 44 (dont **2 en 404 en production**) | § 5.1 |
