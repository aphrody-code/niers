# `apps/azalee` — audit page par page

> Mesuré le **2026-09-06**, périmètre **exclusif** `apps/azalee/` (81 `page.tsx`, 26 `route.ts`).
> `packages/azalee` et les types partagés sont audités en parallèle par un autre agent : ils ne
> sont lus ici que pour **résoudre une table** derrière une façade, jamais audités ni modifiés.
> **Aucune modification de code n'a été faite.** Chaque compte porte sa commande.
>
> S'appuie sur `docs/inagle/01-pipeline-entree.md` et `docs/inagle/02-sortie-et-domaines.md`
> (notamment le tableau des 66 tables `inagle_*` hors `cross` et leur régime d'écriture).

```bash
cd apps/azalee && rg --files -g 'page.tsx' app | wc -l   # 81
cd apps/azalee && rg --files -g 'route.ts'  app | wc -l   # 26
```

---

## 0. Les quatre chemins d'accès aux données — et un seul manquant

Toutes les pages passent par **l'un de ces quatre** chemins, jamais un cinquième :

| # | Chemin | Point d'entrée | Qui l'emprunte |
|---|---|---|---|
| 1 | **Façade `packages/azalee`** (`lib/wiki-service.ts:13`, `lib/wiki/*.ts:13`) | `export * from "@rosegriffon/azalee/wiki/…"` + `import "@/lib/azalee-runtime"` | 24 pages wiki |
| 2 | **Supabase direct** (`lib/supabase/server.ts:36` anon, `lib/supabase/admin.ts:6` service-role) | `createClient()` / `createAdminClient()` puis `.from(…)` **dans la page** | 17 pages (13 dashboard + `/`, `/skill/[id]`, `/tools/compare`, …) |
| 3 | **Postgres direct** (`lib/db/pg.ts:26`) | `getPgPool().query("SELECT …")` — SQL brut, contourne PostgREST | 6 pages news/accueil + 5 routes |
| 4 | **Fichier / service externe** | `@/data/*.json`, wasm (`lib/nie-engine.ts`), `api.github.com` (`lib/niers-releases.ts:49`), CDN `cdn.rosegriffon.fr` | 8 pages |

**`@niers/catalog` n'est utilisé nulle part** :

```bash
cd apps/azalee && rg -n '@niers/catalog' app components lib config types   # aucun résultat
cd apps/azalee && rg -n 'NIE_CDN|model-serve' app lib                       # 1 seul (commentaire, lib/cpk-wasm.ts:5)
```

Azalée ne connaît **pas** la façade des quatre gisements : elle parle à Postgres. C'est la
première conséquence pratique de tout portage vers Aphrody — il n'y a aucun code à réutiliser
côté accès données, seulement la forme des vues.

### Deux docstrings périmées qui décrivent un chemin qui n'existe plus

`lib/azalee-runtime.ts:5-11` annonce « on injecte le client Supabase **enveloppé du `Proxy`
maison** : les tables `inagle_*` partent sur le miroir » et `lib/wiki/exp-table.ts:12-15` cite
« routées vers le miroir SQLite (`lib/supabase/server.ts:105`) ».

Le `Proxy` a été retiré (lot J2, 2026-09-05) et `lib/supabase/server.ts` **fait 70 lignes** — il
n'a pas de ligne 105, et `rg -n 'Proxy|mirror|sqlite' lib/supabase/server.ts` ne rend que le
commentaire d'historique (`:27`). **Toutes** les lectures `inagle_*` partent aujourd'hui sur
PostgREST/Supabase. `apps/azalee/data/mirror.sqlite` existe encore sur le disque mais n'est plus
dans le chemin d'une page.

---

## 1. Les 81 pages

Légende de la colonne **Portage** : `portable` = ne lit que de la donnée du jeu, reproductible
depuis le VFS ; `reste` = éditorial / comptes / admin / modération, Azalée est le wiki de
référence, produit Rose Griffon ; `trancher` = la question est posée dans la colonne *Note*.

### 1.1 Wiki du jeu — 33 pages

| Page | Backend (chemin:ligne) | Tables lues | Types | Rendu | Portage | Note |
|---|---|---|---|---|---|---|
| `app/aura/(liste)/page.tsx` | aucun — 4 constantes en dur (`:16-…`) | — | local | `dynamic="force-dynamic"` (`:1`) — inutile, page statique | **portable** | index de catégories, aucune donnée |
| `app/aura/[category]/(liste)/page.tsx` | façade `wikiService.getAurasList` (`:98`, via `lib/wiki-service.ts:13`) | `inagle_auras` (`packages/azalee/src/wiki/service.ts:686`, sélection explicite `:795`) | `any` | `force-dynamic` (`:8`) | **portable** | `sheet_data` des auras est **écrit par le push** (`packages/inagle/src/cli-push.ts:764`), pas curaté |
| `app/aura/[category]/[id]/page.tsx` | `wikiService.getAura` (`:37`, `:70`) | `inagle_auras`, `inagle_skills` (`service.ts:893,903`), `inagle_characters` (`:930`) | `any` | `force-static` + `revalidate=3600` + `dynamicParams` (`:3-5`) | **portable** | |
| `app/boutique/page.tsx` | `getShopsList` (`:17`, `lib/wiki/shops.ts:13`) | `inagle_shops` (`packages/azalee/src/wiki/shops.ts:167`) | `ShopCard` local | `force-dynamic` (`:7`) | **portable** | |
| `app/boutique/[id]/page.tsx` | `getShop` (`:31`, `:64`) | `inagle_shops` (`shops.ts:82`), `inagle_items` (`:105`) | local | `force-dynamic` (`:8`) | **trancher** | `inagle_items.sheet_data` est **mixte** : parser si `item.shops`, sinon snapshot curaté (`cli-push.ts:652`). Le prix/les attributs affichés viennent-ils du jeu ou de la feuille ? |
| `app/capsule/page.tsx` | `getCapsuleList`/`getCostumeList`/`getGachaCounts` (`:48,52,40`) | `inagle_capsules`, `inagle_costumes` (`gacha.ts:116,218,258`) | local | `force-dynamic` (`:1`) | **portable** | les deux tables sont en upsert pur depuis le dump |
| `app/capsule/[id]/page.tsx` | `getCapsulePrize`/`getCapsulePoolPrizes` (`:16,41,48`) | `inagle_capsules` (`gacha.ts:153,168`) | local | `force-dynamic` (`:1`) | **portable** | |
| `app/chara/(liste)/page.tsx` | `wikiService.getCharactersList` (`:98`), `getAllTeams` (`:115`) | `inagle_characters` (`service.ts:1398`), `inagle_teams` (`:717`) | `BaseCharacter` (`packages/azalee`) | `force-dynamic` (`:1`) | **trancher** | l'**ordre** de toutes les listes est `zukan_order` (`service.ts:1228,1252,1286,1334`) — colonne **curatée**, scrapée de `zukan.inazuma.jp`, préservée hors pipeline (`cli-push.ts:261`). Sans elle, quel ordre ? |
| `app/chara/[id]/page.tsx` (814 l.) | `wikiService.*` ×8 (`:53,69,158,201,206,218,285,291,417`), `resolveCharaStats` (`:17`), `compareVariants` de `@rosegriffon/inagle` (`:19`) | `inagle_characters`, `inagle_skills`, `inagle_auras` | `BaseCharacter` + **45 `any`** | `force-static` + `revalidate=3600` (`:4-6`) | **trancher** | le **moveset affiché** vient de `sheetData.moveset` / `altMoveset` / `firstMoves` (`:380-383`, `:454`) — `inagle_characters.sheet_data` est dit **« 100 % curaté hors-pipeline »** (`cli-push.ts:373`). Les stats, elles, sont game-pures (`chara-stats.ts` lit `growth_table_config…cfg.bin` par le CDN). Deux origines dans une seule fiche |
| `app/drops/page.tsx` | `getDropsData` (`:15`, `lib/wiki/drops.ts`) | `inagle_drop_rates` (`drops.ts:92,145`), `inagle_items` (`:104`), `inagle_drops_battles` (`:151`), `inagle_drops` (`:213`) | local | `force-dynamic` (`:1`) | **trancher** | `inagle_drops` (98 l.) n'a **aucun importeur** et `inagle_drop_rates` est **hors flux** (doc 02 §3.1). D'où viennent ces lignes ? |
| `app/entraineur/page.tsx` | `getCoachesList` (`:32`, `lib/wiki/coaches.ts:13`) | `inagle_coordinators`, `inagle_manager_passives`, `inagle_characters` (`coaches.ts:281-283`) | local | `force-dynamic` (`:1`) | **trancher** | `inagle_coordinators` (102 l.) et `inagle_manager_passives` (80 l.) sont **héritées, aucun importeur** |
| `app/entraineur/[id]/page.tsx` | `getCoach` (`:6`) | idem (`coaches.ts:330,331,346`) | local | `force-dynamic` (`:1`) | **trancher** | idem |
| `app/equipe/page.tsx` | `getTeamsList` (`:5`) | `inagle_teams` (`teams.ts:302`) | local | `force-dynamic` (`:1`) | **portable** | |
| `app/equipe/[id]/page.tsx` | `getTeamDetail` (`:22`, `:127`) | `inagle_teams` (`teams.ts:154,217`), `inagle_characters` (`:161,260`), `inagle_uniforms` (`:241`) | `TeamKit`, `TeamRosterMember` (`lib/wiki/teams.ts`) | `force-dynamic` (`:1`) | **portable** | 3 tables en upsert pur |
| `app/gallery/page.tsx` | `wikiService.getGalleryList` (`:59`), `getGalleryCategoryCounts` (`:65`) | `inagle_gallery` (`service.ts:2774,2801`) | local | `force-dynamic` (`:1`) | **portable** | |
| `app/invocation/page.tsx` | `getInvocationRates` (`:27`) | `inagle_constellations` (`invocation.ts:60`) | local | `force-dynamic` (`:1`) | **portable** | |
| `app/item/(liste)/page.tsx` | `wikiService.getItemsList` (`:94`) | `inagle_items` (`service.ts:1787`) | `Item` | `force-dynamic` (`:1`) | **trancher** | même question `sheet_data` que `/boutique/[id]` |
| `app/item/[id]/page.tsx` | `wikiService.getItem` (`:20`, `:52`) | `inagle_items` (`service.ts:1711`) | `Item` | `force-static` + `revalidate=3600` (`:10-12`) | **trancher** | idem |
| `app/keshin/page.tsx` | aucun — `permanentRedirect("/modeles/keshin")` (`:15`) | — | — | statique | **portable** | ⚠️ **la cible n'existe pas** : `ls app/modeles` → *No such file or directory*. Redirection 308 vers un 404 |
| `app/niveau/page.tsx` | `getExpTable` (`:49`, `lib/wiki/exp-table.ts:56`) | `inagle_exp_table` | `ExpLevelEntry`, `ExpTableData` (`lib/wiki/exp-table-shared.ts`) | `force-dynamic` (`:1`) | **portable** | 100 lignes ; le calcul (`exp-table-shared.ts`, 270 l.) est pur |
| `app/passive/(liste)/page.tsx` (563 l.) | **fichiers JSON** : `@/data/passive-sheets.json` (`:28`) + `@rosegriffon/azalee/data/passives-full.json` (`:29`) | *(aucune table)* | local | *(défaut — statique)* | **trancher** | `passive-sheets.json` est un **fichier de feuille communautaire** versionné dans l'app. Est-il régénérable depuis le jeu ? Non vérifié |
| `app/passive/[id]/page.tsx` | `wikiService.getPassive` (`:22`, `:59`) | `inagle_passives` (`service.ts:2908`) | `PassiveDetail` | `force-static` + `revalidate=3600` (`:12-14`) | **portable** | |
| `app/quete/page.tsx` | `getQuestsList` (`:8`) | `inagle_quests` (`quests.ts:123`) | `QuestKind` | `force-dynamic` (`:1`) | **portable** | |
| `app/quete/[id]/page.tsx` | `getQuest`, `getQuestNeighbors` (`:7`) | `inagle_quests` | `Quest` | `force-dynamic` (`:1`) | **portable** | |
| `app/search/page.tsx` | délègue à `./search-client` → server actions `app/actions/search.ts` | `inagle_characters` (`actions/search.ts:101,501,674`), `_skills` (`:127,685`), `_items` (`:136,700`), `_teams` (`:172`), `_tactics` (`:693`), `_passives` (`:709`) + `getPgPool()` (`:5`) | `GlobalSearchResult` (`packages/azalee/search`) | `force-dynamic` (`:1`) | **trancher** | mélange PostgREST **et** SQL brut ; le classement flou (`smart-search`) n'a pas d'équivalent Rust — cf. doc 02 §5 (`ufuzzy`) |
| `app/skill/(liste)/page.tsx` | `wikiService.getSkillsList` (`:104`) | `inagle_skills` (`service.ts:2185`), `inagle_override_skills` (`:2247`) | `Skill` | `force-dynamic` (`:1`) | **trancher** | `inagle_override_skills` (33 l.) : importeur **jamais appelé** (doc 02 §1.3) |
| `app/skill/[id]/page.tsx` | `wikiService.getSkill` (`:133,222`) **+ Supabase direct dans la page** (`:44`, `:104`) | `inagle_skills` (`:46`), `inagle_skill_videos` (`:106`), `inagle_override_skills` | `Skill`, `SkillVideoVariant`, `OverrideSkillData` | `force-static` + `revalidate=3600` (`:15-17`) | **trancher** | `inagle_skill_videos` (1 211 l.) et `video_url/poster_url/thumbnail_url` sont **scrapés de zukan.inazuma.jp**, restaurés à chaque push (`cli-push.ts:454,551`). Les vidéos ne sortiront **jamais** du VFS |
| `app/stade/page.tsx` | `getStadiumsList` (`:9`) | `inagle_stadiums` (`stadiums.ts:151`) | local | `force-dynamic` (`:1`) | **trancher** | `importStadiums` **jamais appelé** — 81 lignes héritées, origine non établie |
| `app/stade/[id]/page.tsx` | `getStadium` (`:8`) | `inagle_stadiums` (`stadiums.ts:122`) | local | `force-dynamic` (`:1`) | **trancher** | idem |
| `app/succes/page.tsx` | `getTrophiesList` (`:9`) | `inagle_trophies` (`trophies.ts:90`) | `Trophy` (`@rosegriffon/azalee/wiki/trophies-shared`) | `force-dynamic` (`:1`) | **portable** | 347 lignes, upsert pur |
| `app/succes/[id]/page.tsx` | `getTrophy`, `getTrophyNeighbors` (`:7`) | `inagle_trophies` | `Trophy` | `force-dynamic` (`:1`) | **portable** | |
| `app/tactic/(liste)/page.tsx` | `wikiService.getTacticsList` (`:31`) | `inagle_tactics` + `inagle_special_tactics` (`service.ts:2418,2419`) | `Item` | `force-dynamic` (`:1`) | **trancher** | `inagle_tactics` (70 l.) : **aucun importeur** ; `inagle_special_tactics` (86 l.) : upsert. Une page, deux régimes |
| `app/tactic/[id]/page.tsx` | `wikiService.getTactic` (`:19`, `:41`) | idem (`service.ts:2364,2371,2379,2388,2389`) | `any` | `force-static` + `revalidate=3600` (`:3-5`) | **trancher** | idem |

*(33 lignes — `/keshin` et `/aura/(liste)` comptées ici bien qu'elles ne lisent aucune table.)*

### 1.2 Éditorial et actualités — 8 pages · **reste sur Azalée**

| Page | Backend | Tables | Rendu |
|---|---|---|---|
| `app/news/(liste)/page.tsx` | `getPgPool()` (`:5`) + 4 server actions (`:1-4`) | `articles`, + réactions/commentaires/historique | `force-dynamic` (`:11`) |
| `app/news/[slug]/page.tsx` (825 l.) | `getPgPool()` (`:41`) + 5 actions (`:19-23`) | `articles`, `profiles`, séries, favoris, réactions | `force-dynamic` (`:1`) |
| `app/news/tag/[tag]/page.tsx` | `getPgPool()` (`:10`) | `articles` | `force-dynamic` (`:1`) |
| `app/news/tweet/[id]/page.tsx` (571 l.) | `getPgPool()` (`:9`), `getAuthorInfo` (`:10`) | `tweets` | `force-dynamic` (`:12`) |
| `app/patch-notes/(liste)/page.tsx` | `getPatchNotes` (`:1`, `app/actions/news.ts:28`) | `patch_notes` | `force-dynamic` (`:4`) |
| `app/patch-notes/[id]/page.tsx` | `getPatchNoteDetail` (`:3`, `actions/news.ts:68`) | `patch_notes` | `force-dynamic` (`:6`) |
| `app/cross/page.tsx` (1 036 l.) | `@/data/inazuma-cross.json` via `app/cross/data.ts:5` | *(fichier)* | statique |
| `app/soutenir/page.tsx` | `apiClient.shop.getProducts` (`:3`) → `getPgPool()` (`lib/api-client.ts:432`) | `merch_products` (SQL brut, `:434`) | `force-dynamic` (`:5`) |

`app/cross/page.tsx` mérite un mot : c'est de la **traduction éditoriale** d'un site officiel
japonais (`app/cross/data.ts:1-4`), pas de la donnée de jeu. Elle reste sur Azalée sans ambiguïté.

### 1.3 Dashboard, administration, modération — 20 pages · **reste sur Azalée**

Toutes gardées en amont par `app/dashboard/layout.tsx:17,29,36` (session Better Auth →
`profiles.role` → `ADMIN_ROLES`), plusieurs redoublent avec `requireAdmin()`.

| Page | Backend | Tables lues |
|---|---|---|
| `app/dashboard/page.tsx` (787 l.) | `createClient()` (`:19`) + `getServerSession` (`:10`) | `articles` ×7, `profiles` ×2, **`inagle_characters`, `_skills`, `_passives`, `_items`, `_teams`, `_formations`, `_auras`, `_keshins`, `_coordinators`** (`:72-104`) — comptes seulement |
| `app/dashboard/admin/users/page.tsx` | `createAdminClient()` (`:10`) + `requireAdmin` (`:3`) | `profiles` (`:14`) |
| `app/dashboard/audit/page.tsx` | `createClient()` (`:28`) + `requireAdmin` | `audit_logs` (`:31`) |
| `app/dashboard/database/page.tsx` | `createClient()` (`:16`) + `apiClient` (`:11`) | `inagle_keshins_clean` (`:25`), `inagle_characters.element` (`:33`) |
| `app/dashboard/database/images/page.tsx` (338 l.) | `createClient()` (`:51`) | `inagle_characters` ×5 (`:62-109`) |
| `app/dashboard/database/[table]/page.tsx` | `createClient()` (`:21`) | **table dynamique** (`:49`, `:54`) — `.from(table)` non typé |
| `app/dashboard/database/[table]/[id]/page.tsx` | `createClient()` (`:13`) | **table dynamique** (`:15`, `(supabase as any)`) |
| `app/dashboard/database/verification/page.tsx` | `createClient()` (`:12`) + `requireAdmin` | `inagle_characters/_skills/_teams/_items` (`:19-41`) |
| `app/dashboard/zukan-review/page.tsx` | `createClient()` (`:22`) + `requireAdmin` | `inagle_characters` ×4 (`:36-47`), colonnes `series`, `zukan_hash` |
| `app/dashboard/news/page.tsx` | `createAdminClient()` (`:38`) | `articles` (`:80-85`), `profiles` (`:108`) |
| `app/dashboard/news/[id]/page.tsx` | `createAdminClient()` (`:11`) | `articles` (`:14`) |
| `app/dashboard/news/[id]/versions/page.tsx` | `createClient()` (`:21`) + actions | `articles` (`:24`) |
| `app/dashboard/news/new/page.tsx` | aucun (monte `NewsEditorLoader`) | — |
| `app/dashboard/news/stats/page.tsx` | `createClient()` (`:183`) + `getArticleStats` | `articles` (`:186`) |
| `app/dashboard/tweets/page.tsx` | `createClient()` (`:8`) | `tweets` (`:10`) |
| `app/dashboard/tweets/[id]/page.tsx` | `createClient()` (`:13`) | `tweets` (`:14`, `:17`) |
| `app/dashboard/users/page.tsx` | `createAdminClient()` (`:42`) | `profiles` (`:48`, `:60`), `articles` (`:75`) |
| `app/dashboard/import-sheet/page.tsx` | action `importGoogleSheet` (`actions.ts:60`) — **Google Sheets** | `profiles`, `articles` |
| `app/dashboard/import-google-doc/page.tsx` | action → `googleapis` (`actions.ts:3,24`) — **Google Docs/Drive** | `articles` |
| `app/dashboard/settings/page.tsx` | `redirect("/settings")` (`:13`) | — |

Deux pages (`database/[table]`, `database/[table]/[id]`) sont un **éditeur de table générique**
qui accepte n'importe quel nom de table en paramètre d'URL et fait `.from(table).select("*")`
(`[table]/page.tsx:49`, `[id]/page.tsx:15`). C'est de l'outillage d'administration, pas une vue
publique : non portable par nature, et à surveiller côté sécurité (non audité ici).

### 1.4 Compte, authentification, profil — 7 pages · **reste sur Azalée**

| Page | Backend | Rendu |
|---|---|---|
| `app/login/page.tsx` | `useAuth` (`:7`), client | `"use client"` (`:1`) |
| `app/2fa/page.tsx` | `authClient` (`:15`) | `"use client"` (`:1`) + `force-dynamic` (`:17`) |
| `app/auth/reset-password/page.tsx` | `authClient` (`:8`) | `"use client"` (`:1`) |
| `app/auth/auth-code-error/page.tsx` | aucun | statique |
| `app/settings/page.tsx` | `createAdminClient()` (`:37`), `auth.api.listSessions` (`:72`) | `profiles` (`:39`), `account` (`:52`) — `force-dynamic` (`:12`) |
| `app/profil/[username]/page.tsx` | `getProfileByUsername` (`lib/db/profiles.ts`) + `getServerSession` (`:10`) | `force-dynamic` (`:1`) |
| `app/maintenance/page.tsx` | aucun | statique |

### 1.5 Outils — 7 pages

| Page | Backend | Tables | Rendu | Portage |
|---|---|---|---|---|
| `app/tools/page.tsx` | aucun | — | `revalidate=86400` (`:13`) | **portable** (index) |
| `app/tools/stats/page.tsx` | **wasm** `lib/nie-engine.ts` (`components/wiki/StatCalculator.tsx:15`) | *(aucune)* | statique + `"use client"` dans le composant | **portable** — déjà 100 % moteur `nie-wasm`, aucun accès base |
| `app/tools/random-team/page.tsx` | `wikiService.getRandomTeamPools` (`:16`), `getCoordinatorPools` (`:17`) | `inagle_characters` (`service.ts:2028`), `inagle_coordinators` (`:1527`) | `force-dynamic` (`:1`) | **trancher** (coordinators sans importeur) |
| `app/tools/my-team/page.tsx` | `wikiService.getCharactersList` (`:32`), `getCoordinatorsList` (`:37`) + `getServerSession` (`:3`) | `inagle_characters`, `inagle_coordinators` (`service.ts:1545`) ; la **sauvegarde** passe par `app/actions/teams.ts:21` → `user_teams` | `force-dynamic` (`:15`) | **trancher** — le *builder* est portable, la **sauvegarde d'équipe est un compte utilisateur** |
| `app/tools/compare/page.tsx` (356 l.) | `createClient()` (`:51`, `:146`) **+** `wikiService` (`:35,82,167-176,216`) | `inagle_characters` (`:53,148,154`), `inagle_skills` | `force-dynamic` (`:7`) | **trancher** — compare des `sheetData` (`:193,221`), donc de la donnée curatée |
| `app/tools/translator/page.tsx` | `searchTranslations` (`app/actions/translate.ts`) | `inagle_characters/_skills/_items/_teams/_keshins_clean/_souls_clean` (`translate.ts:416-581`) | statique + client | **portable** — noms JP/EN/FR du jeu, `japaneseToRomaji` pur |
| `app/tools/niers/page.tsx` | `getLatestNiersDesktopRelease` (`lib/niers-releases.ts:49`) → **api.github.com** | *(aucune)* | `revalidate=3600` (`:10`) | **reste** — page de téléchargement de l'app desktop, propre à Azalée |

### 1.6 Accueil, statique, légal — 6 pages

| Page | Backend | Portage |
|---|---|---|
| `app/page.tsx` | `createClient()` (`:41`) + `getPgPool()` (`:11`) ; comptes de `inagle_characters/_skills/_items/_keshins_clean/_souls_clean/_passives` (`:81-86`) + derniers articles | **trancher** — la moitié « carrousel wiki » est portable, la moitié « actualités » non. `revalidate=300` (`:38`) |
| `app/charte/page.tsx` | aucun | **reste** (charte éditoriale) |
| `app/contact/page.tsx` | aucun, `revalidate=86400` (`:9`) | **reste** |
| `app/legal/cgu`, `legal/confidentialite`, `legal/mentions-legales` | aucun | **reste** (mentions Rose Griffon) |

*(`/aura/(liste)`, `/tools`, `/maintenance` sont statiques mais comptées dans leur section
thématique — §1.1, §1.5, §1.4 — pour ne pas être décomptées deux fois.)*

---

## 2. Les 26 routes `route.ts`

| Route | Backend | Nature |
|---|---|---|
| `app/api/auth/[...all]/route.ts` | Better Auth (`:1`) | auth |
| `app/api/auth/magic-login/route.ts` | `profiles` (`:76,98,110`) | auth |
| `app/api/supabase-token/route.ts` | `mintSupabaseJwt` (`:4`) | auth |
| `app/auth/post-login/route.ts` | `createClient()` (`:48`), `profiles` (`:50`) | auth |
| `app/api/common/route.ts` | `createAdminClient()` (`:12`), `profiles` (`:14`) | compte |
| `app/api/health/route.ts` | `profiles` (`:12`) | ops |
| `app/api/admin/news/draft/route.ts` (447 l.) | `createAdminClient()` (`:374`), `profiles`, `articles` | admin |
| `app/api/admin/discord/role/[roleId]/members/route.ts` | `profiles` (`:120`) + Discord | admin |
| `app/api/articles/export/route.ts` | `requireAdmin()` (`:7`) | admin |
| `app/api/cron/publish-scheduled/route.ts` | `articles` (`:25,44`) | cron |
| `app/api/news/feed/route.ts`, `share-count`, `app/news/feed.xml`, `app/api/tags/popular` | `getPgPool()` | éditorial |
| `app/api/og/character/route.tsx` | `runtime="edge"` (`:4`), image OG | **portable** |
| `app/api/graphql/route.ts` (310 l.) | `wikiService` (`:7`) + `ragSearch` (`:6`) | mixte |
| `app/api/rag/search/route.ts` | `ragSearch` (`@rosegriffon/db/rag`) | RAG |
| `app/api/llm/[model]/route.ts` | statique, `revalidate=86400` (`:7`) | robots/agents |
| `app/api/save/resolve-roster/route.ts` | `createClient()` (`:103`), `inagle_characters` (`:111`) | **portable** — lit une sauvegarde du jeu |
| `app/api/vroid/*` (7 routes) | OAuth VRoid Hub (`lib/vroid/*`) | **reste** — compte tiers |
| `app/tools/niers/latest.json/route.ts` | GitHub Releases (`lib/niers-releases.ts`) | updater Tauri |

---

## 3. Synthèse — donnée du jeu contre donnée curatée

### 3.1 Le partage, table par table

Croisement des tables lues par `apps/azalee` avec le régime d'écriture mesuré en
`docs/inagle/02-sortie-et-domaines.md` §3.1.

**Tables `inagle_*` que le push produit depuis le dump** (donc reproductibles par `nie-data`) :
`inagle_characters`, `_skills`, `_items`, `_passives`, `_auras`, `_keshins`, `_souls`,
`_capsules`, `_costumes`, `_constellations`, `_exp_table`, `_formations`, `_gallery`, `_quests`,
`_shops`, `_special_tactics`, `_teams`, `_trophies`, `_uniforms`, `_drops_battles` — **20**.

**Tables `inagle_*` lues par une page mais qu'aucun importeur n'alimente** (héritées, ou dont
l'importeur n'est jamais appelé) : `inagle_tactics` (70 l.), `inagle_coordinators` (102 l.),
`inagle_manager_passives` (80 l.), `inagle_stadiums` (81 l.), `inagle_drops` (98 l.),
`inagle_drop_rates` (177 l., hors flux), `inagle_override_skills` (33 l.) — **7**.
Leur origine n'est **pas établie** : elles peuvent venir d'un parseur mort, d'un script disparu
ou d'une saisie manuelle. Rien dans le dépôt ne permet aujourd'hui de les régénérer.

**Colonnes explicitement curatées, préservées hors pipeline** (`cli-push.ts:261,373,454,551,563`) :
`inagle_characters.sheet_data` (« 100 % curaté »), `.zukan_order`, `.zukan_hash`, `.series` ;
`inagle_skills.video_url/poster_url/thumbnail_url` + toute la table `inagle_skill_videos`
(1 211 l., scrapée) ; `inagle_items.sheet_data` (mixte).

**Fichiers curatés versionnés dans l'app** : `apps/azalee/data/passive-sheets.json`,
`data/inazuma-cross.json` (crawl + traduction éditoriale), `data/zukan-audit.json`.
Seul `data/aphrody-dossier.json` est **généré par niers** (`components/wiki/AphrodyDossierSection.tsx:5`).

### 3.2 Le compte demandé

**40 des 81 pages** touchent au moins une table `inagle_*` (par façade ou en direct) ; **41** n'en
touchent aucune. Les quatre paniers ci-dessous sont **listés nommément** pour être revérifiables ;
ils se recouvrent, la somme dépasse donc 40.

| Panier | Pages | Membres |
|---|---:|---|
| **A** — ne lisent que des tables `inagle_*` **que le push produit depuis le dump** | **16** | `/aura/[cat]` (liste), `/aura/[cat]/[id]`, `/boutique`, `/capsule`, `/capsule/[id]`, `/equipe`, `/equipe/[id]`, `/gallery`, `/invocation`, `/niveau`, `/passive/[id]`, `/quete`, `/quete/[id]`, `/succes`, `/succes/[id]`, `/tools/translator` |
| **B** — lisent une table `inagle_*` **sans importeur** (origine non établie) | **11** | `/drops`, `/entraineur`, `/entraineur/[id]`, `/stade`, `/stade/[id]`, `/tactic` (liste), `/tactic/[id]`, `/skill` (liste), `/skill/[id]`, `/tools/random-team`, `/tools/my-team` |
| **C** — lisent une **colonne curatée** (`sheet_data`, `zukan_order`, `zukan_hash`, `video_url`) | **9** | `/chara` (liste), `/chara/[id]`, `/item` (liste), `/item/[id]`, `/boutique/[id]`, `/skill/[id]`, `/tools/compare`, `/dashboard/zukan-review`, `/dashboard/database/images` |
| **D** — lisent un **fichier curaté** versionné | **2** | `/passive` (liste) (`data/passive-sheets.json`), `/cross` (`data/inazuma-cross.json`) |

Restent, dans les 40, six pages qui ne lisent `inagle_*` que pour des **comptes** ou une
**recherche** et qu'aucun panier ne décrit bien : `/` (accueil), `/search`, `/dashboard`,
`/dashboard/database`, `/dashboard/database/verification`, `/dashboard/database/[table]`
(table dynamique).

C'est **la** distinction qui décide de ce qu'Aphrody peut servir nativement : le panier A n'a
besoin que du VFS ; les paniers B, C et D (20 pages distinctes) dépendent d'un contenu qui n'est
**pas** dans `nie.exe` ni dans les CPK.

### 3.3 Portage — le verdict

| Verdict | Pages |
|---|---:|
| **portable** (ne lit que de la donnée du jeu) | **20** |
| **reste sur Azalée** (éditorial, comptes, admin, modération, légal) | **41** |
| **à trancher** | **20** |

Détail des 20 **portables** : `/aura` (liste, index sans données), `/aura/[cat]` (liste),
`/aura/[cat]/[id]`, `/boutique`, `/capsule`, `/capsule/[id]`, `/equipe`, `/equipe/[id]`,
`/gallery`, `/invocation`, `/keshin` (redirection — **cassée**, cf. §1.1), `/niveau`,
`/passive/[id]`, `/quete`, `/quete/[id]`, `/succes`, `/succes/[id]`, `/tools`, `/tools/stats`,
`/tools/translator`.

Ventilation des 41 qui **restent** : 8 éditorial/news, 20 dashboard/admin, 7 compte/auth,
5 légal-statique, 1 `/tools/niers` (téléchargement de l'app desktop).

Ventilation des 20 **à trancher** : 16 pages wiki (§1.1), 3 outils (`/tools/random-team`,
`/tools/my-team`, `/tools/compare`), 1 accueil.

Les 20 **à trancher**, avec leur question : cf. la colonne *Note* de §1.1. Elles se ramènent à
**cinq** questions seulement :

1. `inagle_characters.sheet_data` et `.zukan_order` sont curatés — que devient une fiche
   personnage sans son moveset ni son ordre officiel ? (`/chara`, `/chara/[id]`, `/tools/compare`,
   `/tools/my-team`)
2. `inagle_items.sheet_data` est mixte — le prix et les attributs affichés viennent-ils du parseur
   ou de la feuille ? (`/item`, `/item/[id]`, `/boutique/[id]`)
3. 7 tables sans importeur (`tactics`, `coordinators`, `manager_passives`, `stadiums`, `drops`,
   `drop_rates`, `override_skills`) — sont-elles régénérables depuis le VFS, ou perdues ?
   (`/tactic`, `/tactic/[id]`, `/entraineur`, `/entraineur/[id]`, `/stade`, `/stade/[id]`,
   `/drops`, `/skill`, `/tools/random-team`)
4. Les vidéos de techniques (`inagle_skill_videos`) sont scrapées — Aphrody les sert-elle, ou
   affiche-t-elle le cut-in décodé du jeu à la place ? (`/skill/[id]`)
5. La recherche floue (`ufuzzy`, `smart-search`) n'a pas d'équivalent Rust : classement identique
   exigé ou non ? (`/search`, `/` accueil)

---

## 4. Composants React réutilisés par ≥ 3 pages

```bash
cd apps/azalee && rg -o --no-filename 'from "@/components/[^"]+"' -g 'page.tsx' app \
  | sort | uniq -c | sort -rn | awk '$1>=3'
```

| Composant | Pages | Candidat `packages/inacord-ui` ? |
|---|---:|---|
| `@/components/ui/Icon` | **19** | **oui** — mais `Icon` rend `null` sur un nom absent de sa table (piège connu, `CLAUDE.md`) ; une fusion doit d'abord réconcilier les deux tables de noms |
| `@/components/ui/fade-in` (`FadeInItem`, `FadeInStagger`) | **10** | **oui** — pure animation, zéro donnée |
| `@/components/wiki/WikiSearchToolbar` | **9** | **oui** si la recherche est côté URL ; elle dépend de `parseSearchParams` (`lib/validations.ts`) |
| `@/components/wiki/WikiPagination` | **8** | **oui** — pure présentation |

Seuls **4** composants passent le seuil de 3 pages. Tous les autres sont mono-page ou bi-page :
`packages/inacord-ui` n'a donc **presque rien** à récupérer d'`apps/azalee` en l'état — les 45
primitives d'`inacord-ui` couvrent déjà ce registre. Le seul apport réel serait la **paire
toolbar + pagination** liée aux `searchParams`, et elle traîne `lib/validations.ts` avec elle.

Hors `@/components`, **25 pages** importent `@rosegriffon/ui` (`AdminPageHeader`, `Badge`,
`Button`, `Card`, `Skeleton`, `Avatar`…). C'est la vraie bibliothèque partagée d'Azalée — un
paquet Rose Griffon, donc **hors** du périmètre `aphrody-dev` (`CLAUDE.md`, § propriété).

---

## 5. Ce qui dépend de l'authentification ou d'un rôle

```bash
cd apps/azalee && rg -ln 'requireAdmin|getServerSession|requireAuth|useAuth|authClient|auth\.api' -g 'page.tsx' app   # 12
```

| Surface | Mécanisme | Fichier:ligne |
|---|---|---|
| **Tout `/dashboard/*` (20 pages)** | garde de layout : session → `profiles.role` → `ADMIN_ROLES` | `app/dashboard/layout.tsx:17,24,29,32,36` |
| `/dashboard/{admin/users,audit,database/verification,zukan-review}` | `requireAdmin()` **en plus** de la garde de layout | `lib/auth-helpers.ts` |
| `/settings` | session obligatoire + `auth.api.listSessions` | `app/settings/page.tsx:72` |
| `/profil/[username]` | lecture publique, mais `getServerSession` pour distinguer « son » profil | `:10` |
| `/news/[slug]` | session pour favoris / réactions / historique de lecture | `:21-23` |
| `/tools/my-team` | session pour **sauvegarder** une équipe (`user_teams`) | `:3`, `app/actions/teams.ts:21` |
| `/login`, `/2fa`, `/auth/reset-password` | `authClient` côté navigateur | `:7`, `:15`, `:8` |
| Routes | `requireAdmin` (`api/articles/export:7`), `ADMIN_ROLES` (`api/admin/news/draft:1`), Better Auth (`api/auth/[...all]`), session VRoid (`api/vroid/*`) | |

Trois **clients Supabase distincts** cohabitent, avec trois niveaux de privilège :
`createPublicClient` (anon, `lib/supabase/public.ts:32`), `createClient` (anon + pont JWT
**désactivé par défaut**, `lib/supabase/server.ts:18`) et `createAdminClient` (**service-role**,
`lib/supabase/admin.ts:6`). Sept pages et trois routes utilisent la voie service-role, qui
contourne RLS. Je n'ai **pas** audité si chacune la justifie.

---

## 6. Ce que je n'ai pas vérifié

- **Les colonnes exactes** de la plupart des `select` : `packages/azalee/src/wiki/service.ts` fait
  très majoritairement `select("*")` (`:1221,1279,1398,1545,1787,2138,2388…`). Les seules
  sélections explicites relevées sont `service.ts:795-796` (auras), `:1324` (formes de perso),
  `:698` (slugs) et `coaches.ts:283`. Le reste est non énumérable sans lire la base.
- **Les vues `inagle_keshins_clean` / `inagle_souls_clean`** (lues par `app/page.tsx:84-85`,
  `dashboard/database/page.tsx:25`, `actions/translate.ts:554,581`) : elles **n'existent pas dans
  le miroir** (`sqlite3 var/mirror.sqlite "select name from sqlite_master where name like
  '%clean%'"` → vide) mais sont définies dans `supabase/migrations/20260902000000_inagle_schema_reference.sql:708,1213`.
  Je n'ai **pas** lu leur `WHERE` : ce qu'elles filtrent est inconnu.
- **Si les 7 tables sans importeur sont régénérables depuis le VFS** : trancher demande de lancer
  les parseurs de `packages/inagle/src/parsers/` — moitié « ENTRÉE », hors périmètre des deux
  documents précédents comme du mien.
- **Si `apps/azalee/data/passive-sheets.json` est dérivable du jeu.** Il est versionné dans l'app
  et lu en dur par `app/passive/(liste)/page.tsx:28`. Origine non établie.
- **Les composants** : je n'ai compté que les imports depuis un `page.tsx`. Un composant importé
  par trois autres composants n'apparaît pas dans mon décompte.
- **La sécurité** de l'éditeur de table générique (`.from(table)` sur un paramètre d'URL) et la
  justification de chaque `createAdminClient()` : signalés, non audités.
- **Les 20 pages « à trancher » ne sont pas un verdict d'impossibilité** : ce sont des pages dont
  je n'ai pas pu établir, en lecture seule, que **toute** leur donnée vient du jeu. Aucune n'a été
  déclarée portable sans avoir remonté la chaîne page → façade → table → régime d'écriture.
