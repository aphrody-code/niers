# `packages/inagle` — moitié « SORTIE ET DOMAINES »

> Cartographie mesurée le **2026-09-06**, périmètre restreint aux 15 dossiers de domaine
> (`analysis api basara characters constellation drops items lua menu quests rag search skills
> teams zukan`), aux 7 fichiers racine de la CLI, à `scripts/`, `supabase/` et `package.json`.
> La moitié « ENTRÉE » (`parsers/`, `entities/`, `data/`, `schemas/`, `core/`) est hors périmètre
> et n'a **pas** été lue : tout ce qui la concerne ici est signalé comme non vérifié.
>
> Chaque compte porte la commande qui l'a produit. Aucune modification de code n'a été faite.

## 0. Volumétrie du périmètre

```bash
for d in analysis api basara characters constellation drops items lua menu quests rag search skills teams zukan; do \
  n=$(fd -e ts . src/$d | wc -l); l=$(fd -e ts . src/$d -x cat \; | wc -l); echo "$d $n $l"; done
```

| Dossier | Fichiers `.ts` | Lignes |
|---|---:|---:|
| `analysis` | 3 | 935 |
| `api` | 1 | 166 |
| `basara` | 3 | 622 |
| `characters` | 7 | 1 761 |
| `constellation` | 2 | 206 |
| `drops` | 1 | 4 |
| `items` | 3 | 578 |
| `lua` | 1 | 50 |
| `menu` | 3 | 282 |
| `quests` | 2 | 130 |
| `rag` | 3 | 229 |
| `search` | 2 | 357 |
| `skills` | 7 | 3 147 |
| `teams` | 2 | 405 |
| `zukan` | 19 | 2 429 |
| **total domaines** | **56** | **11 301** |

Chaîne de push (`cat src/cli-push.ts src/push-categories.ts src/push-adapter.ts src/lua/pusher.ts
scripts/*.ts | wc -l`) : **3 353 lignes**, dont **502** pour les 18 runners de `scripts/`.

---

## 1. Les importeurs vers Postgres

### 1.1 Combien, et où

| Fichier | Importeurs écrivant en base | Commande |
|---|---:|---|
| `packages/inagle/src/cli-push.ts` | **14** | `rg -n '^async function import' src/cli-push.ts` → 16 dont `importExpTable`/`importGrowthTables` (14 écrivent, `exportStoryTextDatabase` écrit un JSON, pas la base) |
| `packages/inagle/src/push-categories.ts` | **32** | `rg -n '^export async function' src/push-categories.ts` |
| `packages/inagle/src/lua/pusher.ts` | **1** | `importLuaScripts` |
| **Total** | **47** | |
| Runners standalone `scripts/push-*.ts` | **18** | `fd . packages/inagle/scripts \| wc -l` |

> **Le « 18 importeurs » de `PLAN.md:299` compte les runners de `scripts/`, pas les importeurs.**
> Il y en a **47**. Chaque runner de `scripts/` ne fait que construire l'adaptateur et appeler un
> importeur de `src/push-categories.ts` (source unique) — cf. `packages/inagle/scripts/push-uniforms.ts:13`.
> Seul `packages/inagle/scripts/push-drop_rates.ts:23` construit ses lignes lui-même.

### 1.2 L'ordre du push, tel qu'il est écrit

`packages/inagle/src/cli-push.ts:188-228` — 32 appels `await` **strictement séquentiels**, dans
cet ordre (`rg -n '^\t*await (import|export)[A-Za-z]+\(' src/cli-push.ts`) :

| # | Appel | ligne | Table(s) écrite(s) | Clé de conflit | Régime |
|---:|---|---:|---|---|---|
| 1 | `importCharacters` | 188 | `inagle_characters` | `id` | **delete-all + upsert** (`cli-push.ts:267`), snapshot préalable de `sheet_data`/`zukan_order` (`:261`) |
| 2 | `importSkills` | 189 | `inagle_skills`, `inagle_skill_videos` | `id` / `skill_id,position` | **delete-all + upsert** (`:474`), snapshot de `video_url/poster_url/thumbnail_url/created_at` (`:454`) + restauration des variantes vidéo (`:551`) |
| 3 | `importItems` | 190 | `inagle_items` | `id` (défaut) | **delete-all + upsert** (`:570`, `deleteAllExcept(…, "id", "0x00000000")`), snapshot `sheet_data` (`:563`) |
| 4 | `importPassives` | 191 | `inagle_passives` | `id` | upsert |
| 5 | `importAuras` | 192 | `inagle_keshins`, `inagle_souls`, `inagle_awakenings`, `inagle_mode_changes`, `inagle_miximax`, `inagle_auras` | `id` | upsert ×6 (`:789-794`) |
| 6 | `importTeams` | 193 | `inagle_teams` | `id` | upsert |
| 7 | `importFormations` | 194 | `inagle_formations` | `id` | upsert par lots de 200 |
| 8 | `importQuests` | 195 | `inagle_quests` | `id` | upsert |
| 9 | `importDrops` | 196 | `inagle_drops_tables`, `inagle_drops_battles`, `inagle_drops_treasures` | `table_id` / `battle_group_id` / — | les deux premières en upsert ; **`inagle_drops_treasures` = delete-all puis `insert`** (`:900`, `:911`) |
| 10 | `importGallery` | 199 | `inagle_gallery` | `id` | upsert |
| 11 | `importCostumes` | 200 | `inagle_costumes` | `id` | upsert par lots de 100 |
| 12 | `importOpponentTeams` | 201 | `inagle_opponent_teams` | `id` | upsert |
| 13 | `importCapsules` | 202 | `inagle_capsules` | `id` | upsert par lots de 100 |
| 14 | `importUniforms` | 205 | `inagle_uniforms` | `name_id` | upsert par lots de 200 |
| 15 | `importShops` | 206 | `inagle_shops` | `id` (`<shopId>:<itemId>`) | upsert |
| 16 | `importTricks` | 207 | `inagle_tricks` | `id` | upsert + `dedup` |
| 17 | `importSpecialTactics` | 208 | `inagle_special_tactics` | `id` | upsert |
| 18 | `importTelopWaza` | 209 | `inagle_telop_waza` | `skill_id` | upsert |
| 19 | `importVideoWaza` | 210 | `inagle_video_waza` | `id` | upsert |
| 20 | `importEmblems` | 211 | `inagle_emblems` | `emblem_id` | upsert |
| 21 | `importSuperTactics` | 212 | `inagle_super_tactics` | `id` | upsert + `dedup` |
| 22 | `importSkillTechnic` | 213 | `inagle_skill_technic` | `id` | upsert |
| 23 | `importTeamBuild` | 214 | `inagle_team_build` | `id` | upsert |
| 24 | `importBoostGroups` | 215 | `inagle_boost_groups` | `id` | upsert + `dedup` |
| 25 | `importConstellations` | 216 | `inagle_constellations` | `id` | upsert |
| 26 | `importStarSigns` | 217 | `inagle_star_signs` | `chara_param_id` | upsert |
| 27 | `importTrophies` | 218 | `inagle_trophies` | `trophy_id` | upsert |
| 28 | `importMissions` | 219 | `inagle_missions` | `mission_id` | upsert |
| 29 | `importLuaScripts` | 220 | `inagle_lua_scripts` | `id` | upsert par lots de 100 |
| 30 | `importExpTable` | 224 | `inagle_exp_table` | `level` | upsert, source = `data/all-gamedata/exp_table.json` |
| 31 | `importGrowthTables` | 225 | `inagle_growth_tables` | `section,main_position,sub_position,play_style,growth_pattern,chara_rank` (**6 colonnes**) | upsert, source = `data/all-gamedata/growth_tables.json` |
| 32 | `exportStoryTextDatabase` | 228 | *(aucune)* | — | écrit `story_text_database.json` ; tolère `EROFS`/`EACCES` (`:1104`) |

**Contraintes générales**
- Tous les `upsert` passent par `INSERT … ON CONFLICT (<cols>) DO UPDATE SET` construit à la main
  (`packages/inagle/src/push-adapter.ts:104-136`), chunké à **50 lignes** côté Postgres direct
  (limite des 65 535 paramètres, `:95`) et **200** côté `pousserParLots`
  (`packages/inagle/src/push-categories.ts:64`).
- `PostgresAdapter.insert()` **n'est pas un insert** : il délègue à `upsert(table, records, "id")`
  (`push-adapter.ts:139-142`). Les deux adaptateurs n'ont donc pas la même sémantique sur
  `inagle_drops_treasures`.
- `dedup()` (`push-adapter.ts:33`) déduplique par `id` **avant** l'envoi : le dernier gagne.
  C'est une perte silencieuse d'information si deux entités partagent une clé.
- Aucun importeur n'est transactionnel. Une erreur de lot est `console.error` puis **le push
  continue** (`push-categories.ts:74-79`). Le processus rend **0** même après des lots en échec.

### 1.3 Seize importeurs importés et jamais appelés

```bash
# importés depuis push-categories, puis appelés dans la séquence
comm -23 <(LC_ALL=C sort imported.txt) <(LC_ALL=C sort called.txt)   # 32 importés, 31 appelés
```

`importAbilityLearning`, `importActivityPhotos`, `importCharaMenuResource`, `importChatEmotes`,
`importDropRates`, `importEnjoyModeTeams`, `importEventSubtitles`, `importExpRarityRates`,
`importNameplates`, `importNfcLottery`, `importOverrideSkills`, `importPassiveSkillEffects`,
`importPerformances`, `importPhaseTitles`, `importSceneArchives`, `importStadiums` — **16
importeurs figurent dans le bloc `import { … } from "./push-categories.js"` de
`cli-push.ts:26-58` sans jamais être appelés** dans l'action `push`.

Deux d'entre eux gardent une porte d'entrée (`scripts/push-drop_rates.ts`,
`scripts/push-event_subtitles.ts`). **Les 14 autres n'ont aucun point d'entrée** : leur code est
écrit, testé nulle part, et ne s'exécute jamais. Les tables correspondantes de production ne sont
donc à jour que par accident (elles portent des données d'un push antérieur ou d'un script
disparu — c'est ce que dit le commentaire `push-categories.ts:860-866`).

### 1.4 Six tables écrites par du code qui n'existent pas en base

```bash
for t in inagle_ability_effects inagle_ability_boards inagle_enjoy_mode_teams \
         inagle_exp_rarity_rates inagle_nfc_lottery inagle_passive_skill_effects; do
  sqlite3 var/mirror.sqlite "select count(*) from sqlite_master where name='$t';"; done
# → 0 0 0 0 0 0
```

Les six sont ciblées par `push-categories.ts:1328,1359,1362,1395,1430,1454` et **absentes du
miroir de production** (`var/mirror.sqlite`, instantané `inagle-2026-09-06T04-41-28`). Le
commentaire `push-categories.ts:1307` renvoie à une migration
`20260813_inagle_couverture_parseurs.sql` — **elle n'existe pas** dans
`supabase/migrations/` ni dans `packages/inagle/supabase/migrations/`
(`rg -l inagle_couverture_parseurs supabase packages/inagle/supabase` → aucun résultat).

---

## 2. La CLI

Point d'entrée : `packages/inagle/src/cli.ts` (28 lignes), `commander`, binaire `inagle` →
`dist/cli.js` (`package.json`), et `bun run cli` → `bun src/cli.ts`.

**8 commandes de premier niveau, 13 commandes feuilles** (`rg -n '\.command\(' src/cli*.ts`) :

| Commande | Déclarée en | Ce qu'elle fait |
|---|---|---|
| `stats` | `cli-commands.ts:9` | Monte le service complet, affiche 4 compteurs (personnages, techniques, objets, équipes) |
| `search <query>` | `cli-commands.ts:36` | Recherche floue globale (uFuzzy), `-l/--limit` (défaut 5), ventile par type |
| `character <name>` | `cli-commands.ts:88` | Fiche du premier personnage correspondant |
| `menu list [path]` | `cli-menu.ts:11` | Liste un répertoire d'images de menu |
| `menu find <pattern>` | `cli-menu.ts:38` | Recherche de fichiers, `-p/--path` |
| `menu scan [path]` | `cli-menu.ts:68` | Statistiques récursives, `-e/--ext`, `-d/--depth` |
| `menu map [outputDir]` | `cli-menu.ts:112` | Génère un JSON par dossier de premier niveau (défaut `./menu-maps`) |
| `items list` | `cli-commands.ts:131` | Liste d'objets, `-c/--category`, `-l/--limit` (20) |
| `items find <query>` | `cli-commands.ts:163` | Recherche d'objets par nom, avec URL d'image |
| `skills find <query>` | `cli-commands.ts:195` | Recherche de techniques (coût TP, puissance min/max, élément) |
| `teams list` | `cli-commands.ts:236` | Liste des équipes |
| `teams get <id>` | `cli-commands.ts:250` | Détail d'une équipe + URL d'emblème |
| `push` | `cli-push.ts:119` | **Le seul écrivain.** Options : `--env <path>` (défaut `.env`), `--url`, `--jwt-secret`, `--key`, `--db-url` |

**Ce que la CLI n'expose pas** : `analysis/`, `rag/`, `constellation/`, `basara/`, `zukan/`,
`quests/`, `drops/` n'ont **aucune** sous-commande. `zukan/scripts/*.ts` (7 fichiers) et
`scripts/push-*.ts` (18) se lancent à la main, en `bun <fichier>`.

Un second point d'entrée existe hors `commander` : `packages/inagle/src/cli-tool.ts` (59 l.),
`search|character|skill|item <query>`, sortie JSON brute. Son message d'usage dit encore
`tsx cli-tool.ts` (`cli-tool.ts:10`) alors que le dépôt interdit Node.

---

## 3. Le schéma

### 3.1 Ce qui existe en base

```bash
sqlite3 var/mirror.sqlite "select count(*) from sqlite_master where type='table' and name like 'inagle_%';"        # 219
sqlite3 var/mirror.sqlite "select count(*) from sqlite_master where type='table' and name like 'inagle_cross_%';"  # 153
```

**219 tables `inagle_*`**, dont **153 `inagle_cross_*`** (domaine du jeu mobile, hors périmètre de
ce paquet et explicitement non décidé par `PLAN.md:307`) et **66 hors `cross`**.

Colonnes et lignes des 66 (`for t in …; do sqlite3 var/mirror.sqlite "select count(*) from
pragma_table_info('$t')"; select count(*) from "$t"; done`) — la colonne **Écrite par ce code**
distingue ce que le push produit de ce dont il hérite :

| Table | Cols | Lignes | Écrite par | Régime |
|---|---:|---:|---|---|
| `inagle_activity_photos` | 6 | 115 | `importActivityPhotos` — **jamais appelé** | héritée |
| `inagle_auras` | 12 | 9 | `importAuras` | upsert |
| `inagle_awakenings` | 15 | 3 | `importAuras` | upsert |
| `inagle_basara` | 17 | 63 | *(aucun importeur)* | héritée |
| `inagle_boost_groups` | 7 | 5 | `importBoostGroups` | upsert |
| `inagle_capsules` | 4 | 740 | `importCapsules` | upsert |
| `inagle_chara_menu_resource` | 5 | 92 | `importCharaMenuResource` — **jamais appelé** | héritée |
| `inagle_characters` | **59** | **6 166** | `importCharacters` | delete+upsert |
| `inagle_chat_emotes` | 9 | 57 | `importChatEmotes` — **jamais appelé** | héritée |
| `inagle_constellations` | 13 | 30 | `importConstellations` | upsert |
| `inagle_coordinators` | 16 | 102 | *(aucun)* | héritée |
| `inagle_costumes` | 8 | 577 | `importCostumes` | upsert |
| `inagle_custom_passives` | 4 | 37 | *(aucun)* | héritée |
| `inagle_drop_rates` | 8 | 177 | `importDropRates` — appelé **seulement** par `scripts/push-drop_rates.ts` | hors flux |
| `inagle_drops` | 9 | 98 | *(aucun)* | héritée |
| `inagle_drops_battles` | 4 | 6 | `importDrops` | upsert |
| `inagle_drops_tables` | 3 | 26 | `importDrops` | upsert |
| `inagle_drops_treasures` | 6 | **0** | `importDrops` | **delete-all + insert — table vide** |
| `inagle_emblems` | 10 | 2 | `importEmblems` | upsert |
| `inagle_event_subtitles` | 17 | 2 093 | `importEventSubtitles` — hors flux (`scripts/push-event_subtitles.ts`) | hors flux |
| `inagle_events` | 10 | 4 708 | `importEventSubtitles` | hors flux |
| `inagle_exp_table` | 2 | 100 | `importExpTable` | upsert |
| `inagle_formations` | 14 | 115 | `importFormations` | upsert |
| `inagle_gallery` | 7 | 360 | `importGallery` | upsert |
| `inagle_game_assets` | 9 | 40 471 | *(aucun)* | héritée — **n'est pas l'index des fichiers du jeu** |
| `inagle_growth_tables` | 8 | 276 | `importGrowthTables` | upsert 6 colonnes |
| `inagle_heroes` | 15 | 126 | *(aucun)* | héritée |
| `inagle_icon_inventory` | 8 | 38 222 | *(aucun)* | héritée |
| `inagle_img_inventory` | 8 | 12 787 | *(aucun)* | héritée |
| `inagle_items` | 24 | 1 807 | `importItems` | delete+upsert |
| `inagle_keshins` | 16 | 306 | `importAuras` | upsert |
| `inagle_kizuna_items` | 5 | 125 | *(aucun ici)* | héritée |
| `inagle_lua_scripts` | 10 | 666 | `importLuaScripts` | upsert |
| `inagle_manager_passives` | 8 | 80 | *(aucun)* | héritée |
| `inagle_media_assets` | 9 | 314 | *(aucun)* | héritée |
| `inagle_missions` | 8 | **1** | `importMissions` | upsert |
| `inagle_miximax` | 17 | 74 | `importAuras` | upsert |
| `inagle_mode_changes` | 15 | 12 | `importAuras` | upsert |
| `inagle_nameplates` | 7 | 54 | `importNameplates` — **jamais appelé** | héritée |
| `inagle_opponent_teams` | 8 | 17 | `importOpponentTeams` | upsert |
| `inagle_override_skills` | 10 | 33 | `importOverrideSkills` — **jamais appelé** | héritée |
| `inagle_passive_generation` | 4 | 34 | *(aucun)* | héritée |
| `inagle_passive_scaling` | 13 | 60 | *(aucun)* | héritée |
| `inagle_passives` | 15 | 128 | `importPassives` | upsert |
| `inagle_performances` | 6 | 16 | `importPerformances` — **jamais appelé** | héritée |
| `inagle_phase_titles` | 5 | 9 | `importPhaseTitles` — **jamais appelé** | héritée |
| `inagle_quests` | 13 | 182 | `importQuests` | upsert |
| `inagle_rag_edges` | 6 | 41 491 | *(aucun ici)* | héritée |
| `inagle_scene_archives` | 8 | 112 | `importSceneArchives` — **jamais appelé** | héritée |
| `inagle_shops` | 15 | 2 331 | `importShops` | upsert |
| `inagle_skill_technic` | 11 | 14 | `importSkillTechnic` | upsert |
| `inagle_skill_videos` | 8 | 1 211 | `importSkills` (restauration) + `zukan/sync-skill-videos.ts` | mixte |
| `inagle_skills` | **35** | 1 002 | `importSkills` | delete+upsert |
| `inagle_souls` | 15 | 56 | `importAuras` | upsert |
| `inagle_special_tactics` | 16 | 86 | `importSpecialTactics` | upsert |
| `inagle_stadiums` | 6 | 81 | `importStadiums` — **jamais appelé** | héritée |
| `inagle_star_signs` | 11 | 5 082 | `importStarSigns` | upsert |
| `inagle_super_tactics` | 7 | 23 | `importSuperTactics` | upsert |
| `inagle_tactics` | 21 | 70 | *(aucun)* | héritée |
| `inagle_team_build` | 13 | 86 | `importTeamBuild` | upsert |
| `inagle_teams` | 19 | 208 | `importTeams` | upsert |
| `inagle_telop_waza` | 8 | 928 | `importTelopWaza` | upsert |
| `inagle_tricks` | 11 | 9 | `importTricks` | upsert |
| `inagle_trophies` | 10 | 347 | `importTrophies` | upsert |
| `inagle_uniforms` | 7 | 627 | `importUniforms` | upsert |
| `inagle_video_waza` | 14 | 4 | `importVideoWaza` | upsert |

**Bilan du flux nominal `inagle push`** : **31 importeurs**, **38 tables** écrites, **17 tables
hors `cross` que le push ne touche jamais**, **6 tables ciblées par du code sans exister en base**.

### 3.2 Migrations

- `packages/inagle/supabase/migrations/` ne contient **qu'un seul** fichier :
  `20260605_event_subtitles.sql` (`fd . packages/inagle/supabase`), qui crée `inagle_events`
  (10 colonnes) et `inagle_event_subtitles` (17 colonnes, PK `(event_id, line_index)`), 6 index,
  RLS + policy `Public Read`. Il est doublé à la racine par
  `supabase/migrations/20260605000000_inagle_event_subtitles.sql`.
- Le schéma réel vit **hors du paquet** : `supabase/migrations/20260902000000_inagle_schema_reference.sql`,
  `…_inagle_cross_core.sql`, `…_inagle_policies.sql`, `…_inagle_public_read.sql`
  (`rg -l inagle_ supabase/migrations`). **Non lus ici** — dire lesquelles des 59 colonnes de
  `inagle_characters` sont typées `jsonb` ou générées demanderait de les ouvrir.

### 3.3 Ce que je n'ai pas pu vérifier

- Les comptes de lignes ci-dessus viennent du **miroir SQLite** (`var/mirror.sqlite`), pas de
  Postgres en direct. Le miroir est l'instantané de 04:41 le 2026-09-06.
- Les faibles comptes (`inagle_missions` = 1, `inagle_emblems` = 2, `inagle_video_waza` = 4,
  `inagle_tricks` = 9) peuvent refléter le contenu réel du jeu **ou** un parseur à vide dont
  l'erreur a été avalée. Trancher exige de lancer les parseurs — moitié « ENTRÉE », hors périmètre.
- `inagle_drops_treasures` est **vide** alors qu'un importeur l'écrit. La cause (parseur à 0
  résultats, ou `db.insert()` en erreur non testée à `cli-push.ts:911`) n'est **pas** établie.
- Les types de colonnes Postgres (`jsonb`, `text[]`, contraintes `NOT NULL`, index) ne se lisent
  pas dans le miroir SQLite : le typage réel est dans `supabase/migrations/`, non lu.

---

## 4. Ce qui dépend d'un service externe

| Surface | Fichiers | Dépendance | Portable en calcul pur ? |
|---|---|---|---|
| **Écriture en base** | `push-adapter.ts`, `cli-push.ts`, `push-categories.ts`, `lua/pusher.ts`, 18 `scripts/push-*.ts` | `@supabase/supabase-js` (REST/PostgREST) **ou** `pg` (Postgres direct) | Non — c'est de l'I/O. `sqlx` 0.8 couvre le second cas ; le premier (PostgREST + JWT service-role) n'a pas d'équivalent Rust dans le dépôt |
| **Authentification** | `push-adapter.ts:176-215`, `cli-push.ts:143-181` | `jsonwebtoken` : signe un JWT `role: service_role` à partir de `SUPABASE_JWT_SECRET`/`JWT_SECRET` | Non — à reproduire (`jsonwebtoken` crate) si l'on garde la voie PostgREST |
| **Scraping zukan.inazuma.jp** | `zukan/order.ts:21` (`Browser`/`Page` de `@aphrody-code/bxc`), `zukan/scripts/scrape-skills-pages.ts:1`, `zukan/scraper.ts:20` (`@aphrody-code/zukan`) | Navigateur headless — le contenu est rendu côté client | Non — nécessite un navigateur, pas un `reqwest` |
| **HTTP simple** | `zukan/skill-videos.ts:47` (`fetch` vers `zukan.inazuma.jp`), `:55` (repli `azalee.rosegriffon.fr`) | `fetch` | Oui (`reqwest`), mais reste du réseau |
| **Parsing HTML** | `zukan/parser.ts` (483 l.), `zukan/order.ts:54` | `cheerio` | Oui — `scraper`/`lol_html` ; le HTML d'entrée reste externe |
| **Lecture du dump** | `items/api.ts`, `characters/mapper*.ts`, `basara/api.ts`, `teams/api.ts`, `skills/{api,mapper,mapper-aura,mapper-passive}.ts`, `menu/{explorer,maps}.ts`, `analysis/matcher.ts`, `zukan/{parser,library}.ts` (21 fichiers, `rg -ln 'node:fs\|readFileSync\|Bun.file'`) | Disque : `DATA_PATH` / `DATA_ROOT` (défaut `/home/ubuntu/niers/data`) | Oui — c'est du fichier local, et c'est le terrain de `nie-data` |
| **Romanisation** | `package.json` : `kuroshiro` + `kuroshiro-analyzer-kuromoji` | Dictionnaire morphologique japonais | Non trivial — aucun équivalent Rust dans le dépôt (à vérifier : `lindera`) |
| **Serveur HTTP** | `adapters/hono.ts` (ré-exporté par `index.ts:8`, hors périmètre) | `hono` | — |

Deux variables d'environnement gouvernent tout : **`DATA_PATH`** (`push-categories.ts:54`,
`cli-push.ts:63`, défaut `/home/ubuntu/niers/data`) et le triplet
**`SUPABASE_URL` / `SUPABASE_SERVICE_ROLE_KEY` / `JWT_SECRET`**, avec un repli en dur sur
`http://127.0.0.1:8811`.

> **Risque relevé au passage, non corrigé** : `packages/inagle/src/cli-push.ts` porte, dans la
> branche « local dev fallback », une **clé service-role JWT écrite en dur dans le code
> versionné**. Une clé service-role contourne RLS. À faire tourner et à sortir du dépôt —
> geste irréversible côté production, donc laissé à l'arbitrage de l'utilisateur.

---

## 5. Ce qui est de la logique de jeu pure

Portable tel quel en Rust : aucun `fs`, aucun `fetch`, aucun client base.

| Module | Lignes | Contenu | Vérification |
|---|---:|---|---|
| `src/stat-calculator.ts` | 358 | **Le cœur.** `calculateSingleStat` (interpolation par paliers 1→30→50→99, `Math.floor` sur chaque segment, `:108`), `findLv1Entry`/`findLv30Entry`/`findMainEntry` (recherche avec **4 niveaux de repli** successifs, `:131`, `:167`, `:222`), `calculateStats` (`:266`), `calculateTotalPower` (`:294`), `generateGrowthCurve` (`:301`), 3 tables de libellés | Un seul import : `./lib/rarity.js` (`rarityToGrowthRank`). Couvert par `src/stat-calculator.test.ts` |
| `src/characters/comparison-engine.ts` | 161 | `compareVariants` (`:29`), `analyzeCharacterVariants` (`:152`) | Imports **type-only** + `ELEMENT_NAMES` |
| `src/analysis/optimizer.ts` | 495 | `BASARA_BUILD_PROJECTIONS` (`:48`), `projectBasaraBuildStats` (`:121`), `getOptimizedBasaraBuilds` (`:148`), `calculateTeamSynergy` (`:176`) | Pur, **mais** importe `createCharactersAPI`/`createBasaraAPI`/`createSkillsAPI`, qui lisent le disque : le calcul est isolable, l'assemblage non |
| `src/zukan/matcher.ts` | 434 | `spearmanCorrelation` (`:169`), `descriptionSimilarity` (`:241`), `matchScore` (`:264`), `matchGroupsStrict` (`:387`), `assignBest` (`:417`), tables `POS_MAP`/`ELEM_MAP`/`ERAS`/`STOP_WORDS` | Aucun import externe — 100 % calcul |
| `src/zukan/audit.ts` | 302 | Doublon assumé du précédent : `auditSpearmanCorrelation` (`:122`), `evaluateRow` (`:179`), `detectDuplicateHashes` (`:294`) | Pur. **Deux implémentations de Spearman coexistent** — à fusionner, pas à porter deux fois |
| `src/search/fuzzy.ts` | 332 | `createSearchIndex` (`:92`), `createGlobalSearch` (`:175`) | Pur **sauf** `@leeoniya/ufuzzy` : algorithme de recherche floue propriétaire au paquet, à réécrire ou à remplacer (`nucleo`, `fuzzy-matcher`) — **le classement ne sera pas identique** |
| `src/characters/evolution.ts` | 354 | `getCharacterEvolution` (`:246`), `compareEvolutions` (`:336`) | `async`, importe `DATA_ROOT` et `loadAllCharacters` → **pas pur** ; la logique d'évolution l'est, son chargement non |
| `src/rag/{context-builder,match-context}.ts` | 229 | Construction de contexte pour un LLM | Pur en calcul, mais assis sur le service complet |

**Ce qui n'est pas portable en calcul** dans les domaines : `zukan/` (2 429 l., dont 19 fichiers
majoritairement scraping/HTML), `menu/` (282 l. de parcours disque), `analysis/matcher.ts`
(435 l. — analyse de **source C** et de fichiers JSON du dépôt, outil de RE, pas de jeu).

---

## 6. La gate de l'amendement A2 est-elle atteignable ?

> `PLAN.md:299-302` — « `niers push --dry-run` annonce les lignes table par table, puis un push
> réel rend **le même total qu'aujourd'hui, écart 0** ».

**Atteignable, mais pas telle qu'elle est formulée.** Ce que la mesure impose :

1. **« Aujourd'hui » n'a pas de valeur définie.** Il n'existe **aucun `--dry-run`** dans le paquet
   (`rg -n 'dry.?run' packages/inagle` → 0). Chaque importeur imprime `✅ X imported (n/m)` en
   texte libre, non machine-lisible, et le total actuel n'est enregistré nulle part. **La gate
   exige d'abord un relevé de référence**, table par table, avant de porter quoi que ce soit —
   sinon on comparera à un souvenir.

2. **Le périmètre à égaler est de 38 tables, pas 66.** 17 tables hors `cross` ne sont écrites par
   personne, 6 autres n'existent pas en base. Un `niers push` qui rendrait 66 totaux ne serait pas
   comparable ; un qui rendrait 38 le serait. **Il faut geler cette liste avant de commencer.**

3. **Écart 0 est impossible sur trois tables tant que le régime n'est pas reproduit** :
   - `inagle_characters`, `inagle_skills`, `inagle_items` sont en **delete-all + réinsertion**
     avec **préservation explicite de colonnes curatées hors pipeline** (`sheet_data`,
     `zukan_order`, `video_url`, `poster_url`, `thumbnail_url`, `created_at`). Un portage qui
     oublie le snapshot (`cli-push.ts:261`, `:454`, `:563`) rend le même **nombre** de lignes et
     détruit du contenu — l'écart 0 serait un faux vert. `cli-push.ts:430-450` documente
     précisément la production perdue le 17/08/2026 par cette voie.
   - `inagle_skills` a une clé étrangère `on delete cascade` vers `inagle_skill_videos` : le
     delete emporte 1 211 lignes qu'il faut réinsérer (`:551`). **La gate doit compter les deux
     tables ensemble**, sinon elle valide un push qui a vidé les vidéos.
   - `inagle_drops_treasures` est vide aujourd'hui : « le même total qu'aujourd'hui » y vaut 0,
     ce qui valide la panne au lieu de la révéler.

4. **`dedup()` fait que le total dépend de l'ordre d'itération.** Deux entités de même `id` →
   une ligne, la dernière. Un portage Rust qui itère dans un autre ordre rendra le même compte
   mais un contenu différent. **Le compte seul ne prouve rien** : la gate doit aussi comparer une
   empreinte (somme de contrôle par table sur les colonnes non volatiles — `updated_at` change à
   chaque push et doit être exclu).

5. **Deux chemins d'écriture, deux sémantiques.** `SupabaseAdapter` (PostgREST + JWT) et
   `PostgresAdapter` (`pg` direct) ne se comportent pas pareil (`insert` ≠ `insert`,
   `push-adapter.ts:139`). `sqlx` couvre le second. **Décider lequel est la référence** est un
   préalable, pas un détail : la gate ne peut pas comparer un push PostgREST à un push `sqlx`
   sans avoir tranché.

6. **Ce que `nie-db` doit reprendre est plus gros que le plan ne le dit.** Non pas
   « 18 importeurs, 2 575 lignes » mais **47 importeurs / 3 353 lignes** dans le seul chemin de
   sortie — et cela suppose que la moitié « ENTRÉE » (`parsers/`) est déjà couverte par
   `nie-data`, ce que je **n'ai pas vérifié** (hors périmètre). Sans les parseurs, il n'y a rien
   à pousser.

7. **`nie-db` n'existe pas** (`ls crates/*/nie-db` → aucun) et `niers` n'a pas de sous-commande
   `push`. Le point de départ est zéro.

**Recommandation de séquence** : (a) instrumenter le push actuel pour qu'il émette un JSON
`{table, lignes, empreinte}` — modification minime, aucune régression possible ; (b) figer ce
relevé comme référence ; (c) supprimer ou câbler les 16 importeurs morts **avant** le portage,
sinon on porte 16 fonctions que personne n'appelle ; (d) porter d'abord les **28 tables en upsert
pur** (aucune préservation, aucune cascade) — c'est la moitié facile et elle vaut une vraie gate ;
(e) traiter `characters` / `skills` / `items` en dernier, avec une gate qui compare le contenu et
pas le compte.

