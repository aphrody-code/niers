# Filtres — état mesuré des quatre surfaces, et ce qui manque à Aphrody

> Recensement en **lecture seule** du 2026-09-06, sur le VPS Linux. Aucun code n'a été modifié.
> Chaque affirmation porte son `chemin:ligne`. Ce qui n'a pas été vérifié est dit tel quel.
>
> Périmètre : les pages qui **listent** quelque chose. Un formulaire, un lecteur, un éditeur
> n'entrent pas dans le compte — une page sans filtre est en revanche recensée comme telle,
> parce que « aucun filtre » est une information au même titre qu'une liste de facettes.

## Commandes de la mesure

```bash
# Le serveur d'Aphrody, lu ligne à ligne
cat -n crates/tools/nie-site/src/vfs_index.rs
cat -n crates/tools/nie-site/src/routes/api_v1.rs
cat -n crates/tools/nie-site/src/routes/vfs.rs
cat -n crates/tools/nie-site/src/routes/mod.rs
rg -n "route\(" crates/tools/nie-site/src/app.rs
rg -n "PER_PAGE|per_page" crates/tools/nie-site/src/config.rs

# Ce que le VFS sait faire
niers vfs --help ; niers vfs find --help ; niers vfs chara --help ; niers vfs waza --help
niers vfs extract --help ; niers vfs formats --help ; niers find --help ; niers grep --help
rg -n "pub fn |pub struct " crates/engine/nie-formats/src/vfs.rs
timeout 300 niers vfs stats --top 25

# Le gisement `extrait`
sqlite3 var/mirror.sqlite "PRAGMA table_info(inagle_characters);"
sqlite3 var/mirror.sqlite "select 'element',count(distinct element) from inagle_characters …"
```

Résultats structurants :

- VFS : **255 308 fichiers, 936 CPK, 5 loose** (`niers vfs stats`).
- `inagle_characters` : **6 166 lignes, 40 colonnes**. Cardinalités des facettes candidates —
  `element` 6, `position` 5, `rarity` 4, `series` 9, `gender` 2, `constellation` 30,
  `team_id` 199.
- Les quatre vues d'Aphrody couvrent **143 246 fichiers sur 255 308** (recompté depuis
  l'histogramme : g4tx 54 203 ; g4md 8 956 + g4mg 15 876 + g4sk 339 + g4mt 71 + g4pk 45 591 +
  g4pkm 6 992 ; acb 5 512 + awb 5 512 ; usm 194). Le chiffre concorde avec celui déjà écrit dans
  `crates/tools/nie-site/src/vfs_index.rs:172`. **112 062 fichiers** (`.bin` 72 308, `.p3lip`
  21 047, `.objbin` 12 190, `.vfxo`, `.g4cm`, `.col`, `.ptlb`, `.mevbin`…) n'entrent dans
  **aucune** vue et ne sont atteignables que par le parcours `/b`.

---

## 1. Azalée — `apps/azalee/app/**` et `packages/azalee/src/**`

### 1.1 Le socle partagé

| Élément | Fichier:ligne | Ce qu'il porte |
|---|---|---|
| Schéma des `searchParams` | `apps/azalee/lib/validations.ts:3-52` | 24 clés : `ageGroup, category, element, gender, grade, has_video, overdrive, page, pcat, perPage, playstyle, position, power_max, power_min, q, rarity, role, series, show_aura, sort, status, tab, team, type` |
| `perPage` | `apps/azalee/lib/validations.ts:17-23` | whitelist **`10 \| 20 \| 50 \| 200`**, défaut 20 |
| `page` | `apps/azalee/lib/validations.ts:11-15` | `Math.max(1, parseInt)`, défaut 1 |
| Barre de recherche | `apps/azalee/components/wiki/WikiSearchToolbar.tsx:67-89` | param `q`, debounce 400 ms, `page` remis à 1 (`:83`) |
| Compteur de filtres actifs | `apps/azalee/components/wiki/WikiSearchToolbar.tsx:97-99` | tous les params sauf `q` et `page` |
| Pagination | `apps/azalee/components/wiki/WikiPagination.tsx:35-76` | `page` + `perPage` ; fenêtre 1,2 ± 4 + 2 dernières au-delà de 20 pages |
| Puces de filtre | `apps/azalee/components/wiki/FilterChips.tsx:25-37` | `paramName` libre, **mono-valeur**, toggle |
| Clés canoniques SEO | `apps/azalee/lib/seo.ts:65-76` et `:93-100` | 10 clés, + 5 pour les techniques |

### 1.2 Page par page

| Page | Fichier:ligne | Filtres (param → domaine) | Tri | Pagination | Recherche |
|---|---|---|---|---|---|
| `/chara` | `app/chara/(liste)/page.tsx:70-163`, `components/wiki/filters/CharacterFilters.tsx:12-201` | `element` (Fire/Wind/Forest/Mountain, `:22-32`) · `position` (GK/DF/MF/FW, `:42-68`) · `gender` (1/2, `:78-86`) · `playstyle` (Justice/Bond/Counter/Breach/Rough Play/Tension, `:96-112`) · `ageGroup` (8 valeurs, `:124-137`) · `rarity` (Normal/Expérimenté/Héros/BASARA, `filters/RarityFilterChips.tsx:9-14`) · `role` (Coordinator/Coach, `:155-162`) · `series` (9 valeurs, `:170-184`) · `team` (id, combobox, `filters/TeamFilter.tsx:28-89`) · `status` (`players-client.tsx:43-67`) | **aucun param** ; fixe `zukan_order ASC, internal_code ASC` — `packages/azalee/src/wiki/service.ts:1489-1492` | `page` + `perPage`, défaut 60, whitelist `{50,60,100,200}` — `page.tsx:67-68` | `q` → `ilike` sur `name_fr`+`name_en` — `service.ts:1400-1404` |
| `/skill` | `app/skill/(liste)/page.tsx:81-218`, `components/wiki/filters/SkillFilterBar.tsx:62-353` | `type` (shoot/block/dribble/catch, `page.tsx:64-70`) · `element` (5 + Void, `page.tsx:72-79`) · `has_video` (`SkillFilterBar.tsx:187-202`) · `show_aura` (`:205-220`) · `overdrive` (`:223-238`) · `power_min`/`power_max` (slider 0→880 pas 5, `:111-143`) | **`sort`** = `power` \| `tension` \| `tension_asc` — `service.ts:2276-2283` | `page`, **60 en dur** — `page.tsx:62` | `q` → `ilike` `name_fr`+`name_en` — `service.ts:2190-2194` |
| `/item` | `app/item/(liste)/page.tsx:80-170`, `filters/ItemFilterBar.tsx:19-65` | `category`, **19 valeurs** (`page.tsx:58-78`), **défaut imposé `shoes`**, pas d'option « toutes » | aucun ; `.order("id")` — `service.ts:1798` | `page`, 48 en dur | `q` → `ilike` `name_fr`+`name_en` |
| `/tactic` | `app/tactic/(liste)/page.tsx:21-112` | **aucune facette** ; `category` codée en dur `special_tactics` (`:32`) | aucun | `page`, 48 | `q` |
| `/passive` | `app/passive/(liste)/page.tsx:168-563`, `filters/PassiveFilters.tsx:50+`, `filters/PassivePlayerFilters.tsx:34+` | `type` (player/custom/coordinator) · `playstyle` (6) · `category` (shot/focus/scramble/castle/team/own) · `element` (5) · `pcat` (6) · `rarity` (numérique dynamique) | aucun | **aucune** | `q` en mémoire, `normalizeText` |
| `/aura` (hub) | `app/aura/(liste)/page.tsx:59-148` | **aucun** — 5 liens statiques | — | — | — |
| `/aura/[category]` | `app/aura/[category]/(liste)/page.tsx:78-116`, `components/wiki/AuraList.tsx:68-100` | `element` (toggle) ; la catégorie est un **segment de route** | aucun ; `icon_code DESC` puis `name_fr` — `service.ts:1159-1165` | `page`, 48 | `q` → `ilike` sur 4 champs — `service.ts:1148-1153` |
| `/equipe` | `app/equipe/page.tsx:16-32`, `components/wiki/TeamsListClient.tsx:30-42` | **aucun param d'URL** ; recherche + série `all/v/go/ie/aresOrion` en `useState` — non partageable | aucun | **aucune** | locale, `name`+`nameEn`+`nameJa` |
| `/boutique` | `app/boutique/page.tsx:16-77` | **aucun** | aucun | **aucune** | **aucune** |
| `/capsule` | `app/capsule/page.tsx:29-270` | `onglet` (capsules/costumes, **hors schéma zod**) · `pool` (dynamique, `packages/azalee/src/wiki/gacha.ts:126-136`) · `type` (dynamique, `gacha.ts:227-232`) | aucun | `page`, 48, plafond 200 (`gacha.ts:110`) | `q` sur `id`/`contentRef` |
| `/entraineur` | `app/entraineur/page.tsx:24-139` | `role` (3) · `element` (4) · `playstyle` (6) — `packages/azalee/src/wiki/coaches.ts:358-377` | aucun ; `id ASC` | **aucune** (102 coachs d'un bloc) | `q` en mémoire sur 5 champs |
| `/gallery` | `app/gallery/page.tsx:46-124`, `filters/GalleryFilterBar.tsx:20+` | `category`, **11 valeurs** — `packages/azalee/src/wiki/service.ts:120-132` | aucun ; `flg_no ASC` | `page`, 48 | `q` → `ilike` sur **`img_path`** (le chemin, pas un titre) — `service.ts:2780-2782` |
| `/drops` | `app/drops/page.tsx:14-38`, `DropsExplorer.tsx:20-175` | **aucun param d'URL** ; onglets + `category` + `source` + `game` en `useState` | aucun | **aucune** | locale |
| `/news` | `app/news/(liste)/page.tsx:23-265` | `category` (5 valeurs, `:21`) | aucun param ; `published_at DESC` | `page`, 12 | `q` full-text `to_tsquery('french')` — `:91-94` |
| `/news/tag/[tag]` | `app/news/tag/[tag]/page.tsx:36-73` | tag = **segment de route** | aucun | `page` | aucune |
| `/quete` | `app/quete/page.tsx:17-97` | `kind` (all/main/side) | aucun | aucune ; `.limit(2000)` | `q` debounce 350 ms sur `titles.{fr,en,ja}`+`id` |
| `/stade` | `app/stade/page.tsx:18-63` | **aucun** | aucun | **aucune** | `q` sur `code`+`title` |
| `/succes` | `app/succes/page.tsx:82-189` | `cat` (all/trophy/activity, **hors schéma zod**) | aucun | aucune ; `.limit(2000)` | `q` sur 5 champs |
| `/patch-notes` | `app/patch-notes/(liste)/page.tsx:13-36` | **aucun param** ; plateforme en `useState` — `PatchNotesDashboard.tsx:106` | aucun | aucune | aucune |
| `/search` | `app/search/page.tsx:31-39` | **un seul param, `q`** ; aucun filtre par type | aucun | aucune | globale |
| `/invocation`, `/niveau`, `/cross` | `app/invocation/page.tsx:26-85`, `app/niveau/page.tsx:48+`, `app/cross/page.tsx` | **aucun `searchParams`** — vérifié par `rg` | — | — | — |

Dashboard (back-office, listes également facettées) :

| Page | Fichier:ligne | Filtres | Tri | Pagination |
|---|---|---|---|---|
| `/dashboard/news` | `app/dashboard/news/page.tsx:29+`, `NewsFilters.tsx:74+` | `status` (4) · `category` (4) · `author` · `view` (grid/list) | **`sort`** = `created_at`/`published_at`/`updated_at`/`view_count`, toujours DESC (`:49-51`) | `page` + `perPage` `{10,20,50}` |
| `/dashboard/users` | `app/dashboard/users/page.tsx:33+` | `role` (5) | fixe `updated_at DESC` | `page` |
| `/dashboard/audit` | `app/dashboard/audit/page.tsx:16-38` | `action` (libre) | fixe `created_at DESC` | `page`, 50 |
| `/dashboard/database/[table]` | `app/dashboard/database/[table]/page.tsx:12-40` | table en whitelist (12) | **`sort` + `dir`** (colonne libre, asc/desc) | `page`, 50 |
| `/dashboard/database/images` | `.../images/page.tsx:45-90` | `filter` (multi_versions/no_image) | fixe par branche | `page`, 50 |
| `/dashboard/{tweets,zukan-review,admin/users,news/stats}` | — | **aucun** `searchParams` | fixe | aucune |

### 1.3 Ce qu'Azalée déclare mais n'utilise pas

- `apps/azalee/components/news/AdvancedFilters.tsx:35-117` produit `tag`, `sort`
  (`recent\|popular\|commented\|trending`), `date` (`week\|month\|year`) — le composant **n'est
  importé nulle part** et `/news` ne lit aucun de ces trois params (`page.tsx:28-33`).
- `apps/azalee/app/actions/zukan.ts:8-70` déclare `q, type, element, position, gender, rank, page,
  limit` — l'action **n'est appelée nulle part**, et `rank` est déclaré puis ignoré (`:24`).
- `packages/azalee/src/wiki/service.ts:2279-2282` accepte `sort=tension_asc`, qu'aucun bouton
  n'expose.
- `apps/azalee/components/wiki/MediaShell.tsx:38-41` et `app/keshin/page.tsx:15` pointent vers
  `/textures`, `/sons`, `/videos`, `/modeles` — **ces routes n'existent pas dans `apps/azalee/app`**.
  Elles vivent dans `apps/nie-web/src/legacy/`, qui n'est pas routé (cf. §3).
- `apps/azalee/app/chara/(liste)/page.tsx:68` accepte `perPage ∈ {50,60,100,200}` mais le schéma
  zod (`lib/validations.ts:21`) ramène `60` et `100` à `20` **avant** que la page les voie.

---

## 2. Inacord (ex `nie-explorer`) — `apps/inacord/src/**`

| Vue | Fichier:ligne | Filtres / facettes | Tri | Limites |
|---|---|---|---|---|
| **Explorateur VFS** | `components/ExplorerView.tsx` | `query` (`:157`, sous-chaîne **chemin entier**, insensible à la casse — `crates/engine/nie-explore/src/listing.rs:173-178`) · **`ext`** (`:166`, **égalité stricte** sur l'extension — `listing.rs:183-187`) · `viewMode` liste/grille (`:174`) · `gridSize` 72→192 px (`:178`) · préfixe (navigation) · frontière `.cpk` (`:53-56`) | **`sortKey`** `name`\|`size` (`:169`) ; taille = DESC forcé, nom = ASC forcé (`:333`) ; dossiers toujours par nom (`:249`) | `PAGE_FICHIERS = 300` (`:75`), `PAGE_RECHERCHE = 500` (`:78`) ; **pas de virtualisation** ; état persisté en `localStorage` — `lib/explorerTabs.ts:40` |
| **Galerie** | `components/GalleryView.tsx` | `categorie` = sous-dossiers **réels** de `data/dx11/menu/220_img` (`:288`, `lib/galerie.ts:24`) · `sousDossier` = langue/variante (`:290`, langues `galerie.ts:59`) · extension figée `g4tx` (`galerie.ts:27`) · `recherche` sur **titre OU chemin** (`galerie.ts:186-196`) | **aucun** (`galerie.ts:164-165`) | `PAR_PAGE = 60` (`:46`), défilement infini ; `MAX_PAR_CATEGORIE = 30000` (`:111`) |
| **Navigateur de contenu (éditeur)** | `components/editor/ContentBrowser.tsx` | **`filter`** = `all`\|`models`\|`textures`\|`audio`\|`configs` (`:101`, jeux d'extensions `:38-40`, `.cfg.bin` `:56`) · `query` sur le **nom seul** (`:122`) | aucun | `PAGE = 300` (`:37`) |
| **Données de jeu** | `components/GameDataView.tsx` | **`famille`** — 25 familles (`:116-646`) · `filtre` texte, **champs déclarés par famille** (`recherche:` aux lignes 137, 172, 195, 232, 260, 277, 294, 331, 362, 378, 393, 410, 429, 447, 463, 479, 496, 521, 538, 555, 572, 603, 622, 637) · `vue` table/cartes (`:692`) | **`tri {key,dir}`** cycle asc→desc→aucun (`:688`, `:786-788`), numérique ou `localeCompare(fr,{numeric})`, vides relégués (`:769-784`) | cartes `limiteCartes = 120` (+240) (`:694`) ; export CSV/JSON du **filtré+trié** (`:790-812`) |
| **Atelier Lua** | `components/LuaView.tsx` | `filter` sur le chemin (`:56`) | aucun | **500 en dur** (`:106`) |
| **RE / base de connaissance** | `components/ReToolsView.tsx`, `lib/reDb.ts` | sous-onglets `functions\|classes\|forge\|live\|aob` (`:346`) · `query` : **adresse** si `/^(0x[0-9a-f]+\|\d+)$/i` → `WHERE f.vaddr` (`reDb.ts:172-177`), sinon `name\|subsystem\|role LIKE` (`:180-183`) · classes : `name\|namespace LIKE` (`:255-258`) · AOB `motif` + `limite` (`:193-194`) | fonctions : `pagerank DESC` ; classes : `name` | `LIMIT 200` (`reDb.ts:154`, `:243`), xrefs 100 (`:263`), forge 300 (`:222`) |
| **CPK brut** | `components/RawCpkView.tsx` | `query` sur le chemin (`:29`) | aucun | aucune (tout rendu) |
| **Recherche globale** | `components/SearchView.tsx` | `kind` = `chara`\|`waza` (`:32`) · `query` nom FR/EN/JA, ID ou code interne | aucun | serveur : **20 en dur** (`src-tauri/src/lib.rs:4952`, `:4965`) ; fichiers liés : 60 (`:151`) |
| **Médiathèque / Cinéma** | `components/CinemaView.tsx`, `lib/recherche.ts` | **langage de requête** : `s:`/`saison:`, `e:`/`ep:`, `lang:`/`vo`/`vf`, `type:` (jeu/serie), `chapitre:`, `st:` (sous-titres), `vu:` (`recherche.ts:46-64`) ; formes `s3e12`, `s03e12`, `3x12` (`:112`) ; booléens `oui/1/true/vrai/o/y/yes` (`:66-67`) ; 10 codes langue (`:70-96`) · Select `langue` avec **comptes réels** et jeton `__toutes__` (`:1207-1229`, `:136`) | **classement pondéré** titre 100 / titreJp 80 / romaji 70 / film 60 / origine 50 / description 30 / chemin 20 (`recherche.ts:186-201`) + bonus début de champ/mot (`:250-255`) ; repli fuzzy si la passe exacte est vide (`:292-295`) | `slice(0,30)` (`:1316`, `:1334`), `MAX_HEROS = 7` (`:165`) |
| **Planche de textures** | `components/TextureSheet.tsx` | `filtre` sur `name` (`:36`) | aucun | `PAGE = 60` (`:17`) |
| **Banque audio** | `components/AudioBankPanel.tsx` | `filtre` sur **`name` OU `awb_id`** (`:39`, `:64-66`) | aucun | `PAGE = 80` (`:12`) |
| **Caméra `.g4cm`** | `components/CameraTrackView.tsx` | **`filtre` par canal**, égalité stricte (`:156`, `:177-181`) | aucun | aucune |
| **Viola (dump)** | `components/ViolaView.tsx` | **`dumpFiltre` = un vrai GLOB** (`:86`) — listes `a,b,c`, exclusions `!motif` prioritaires, `**` traverse les `/`, ancré `^…$`, insensible à la casse — `crates/engine/nie-viola/src/filtre.rs:1-19`, `:54` | — | — |
| **Palette de commandes** | `components/CommandPalette.tsx` | `query` fuzzy **cmdk par défaut** (aucun `filter`/`shouldFilter` custom dans `packages/inacord-ui/src/components/ui/command.tsx`) ; 3 groupes : 16 vues (`lib/vues.ts`), épinglés, récents | — | `RECENTS_MAX` — `lib/places.ts:63` |
| **Outils** | `components/ToolsView.tsx` + `components/tools/*` | facette `outil` (`:66`) ; recherche roster mutualisée `useFiltered` (`lib/filtrage.ts:9-21`) sur `[nom, poste, element, rarete]` — `ComparatorPanel.tsx:80`, `TeamBuilderPanel.tsx:174`, `StatCalculator.tsx:49` | aucun | aucune |
| **Jobs** | `components/JobManager.tsx` | **aucun** | fixe | `LIMIT 50` — `lib/jobsDb.ts:49` |

**Vues d'Inacord sans aucun filtre** (vérifié) : `ModsView.tsx`, `SaveView.tsx`, `DashboardView.tsx`
(un seul `.filter(statut ∈ {produit,bloque})` en dur, `:295`), `ReForgeView.tsx` (`slice(0,60)`,
`:235`), `NavmeshView.tsx`, `SettingsView.tsx`, `DetailPane.tsx`, `PropertyEditor.tsx`,
`VideoPlayer.tsx`, `Sidebar.tsx`, `TopBar.tsx`, `SelectionBar.tsx`, `ExplorerTabsBar.tsx`,
`MemoireCard.tsx`, `ModelPreview.tsx`, `CfgbinViewer.tsx` (tri par colonne mais **aucune**
recherche).

**`packages/inacord-ui/src/**` ne contient aucune vue-liste**, donc aucun filtre : uniquement les
primitives (`toggle-group`, `select`, `slider`, `tabs`, `command`, `data-grid`, `tree-rows`), le
contrat de source (`source.tsx:39`, `:84`, `:102`) et les vignettes (`lib/thumbs.ts`).

### 2.1 Ce que les commandes Tauri acceptent

| Commande | Signature (chemin:ligne) | Paramètres de filtrage |
|---|---|---|
| `vfs_ls` | `apps/inacord/src-tauri/src/lib.rs:469-475` | `prefix, limit, offset` — **mais le front appelle `api.ls(prefix, gameDir)` sans `limit`/`offset`** (`ExplorerView.tsx:293`) : tout le dossier transite par l'IPC |
| `vfs_find` | `lib.rs:501-507` | `query, ext, limit` |
| `vfs_find_paged` | `lib.rs:521-528` | `query, ext, limit, offset` |
| `vfs_related` | `lib.rs:1661-1678` | `needle, limit` — `p.contains(&needle)` **sensible à la casse** (`:1669`), contrairement à `find_paged` |
| `list_packs_dir` | `lib.rs:2239-2250` | aucun ; `.cpk` en dur, **non trié** |
| `lua_list_scripts` | `lib.rs:4126-4148` | aucun ; suffixe `.lua.bin`/`.lua` en dur |
| `vfs_texture_list` | `lib.rs:776-780` | aucun |
| `vfs_audio_cues` | `lib.rs:4441` | aucun |
| `save_list_blobs` | `lib.rs:1148` | aucun |
| `game_data_*` (25) | `src-tauri/src/game_data.rs:111, 230, 264, 303, 348, 382, 497, 546` | **aucun** — la famille entière est rendue, tout le filtrage/tri est côté client |
| `viola_dump_start` | `src-tauri/src/viola.rs:129-138` | `filtre` **glob** |
| `re_dump_scan` | `src-tauri/src/re_trace.rs:265` | `motif` AOB, `limite` |

---

## 3. Aphrody aujourd'hui — `apps/nie-web/src/**` + `crates/tools/nie-site`

> `apps/nie-web/src/legacy/**` **n'est routé par aucune page** — vérifié : la seule mention est un
> commentaire (`pages/Catalogue.tsx:6`). Ses filtres ne sont pas actifs et ne comptent pas ici.

| Page | Fichier:ligne | Filtres | Tri | Pagination | Recherche |
|---|---|---|---|---|---|
| `/` Menu principal | `pages/MenuPrincipal.tsx:152`, `entrees.ts:71-79` | **aucun** | — | — | — |
| `/explorateur` | `pages/Explorateur.tsx:43-117` | **aucun** — un seul état, `prefixe` (`:43`), fil d'Ariane (`:35-38`). Tout `contenu.dossiers` et `contenu.fichiers` est rendu **sans troncature ni compte** | **aucun** | **aucune** | **aucune** |
| `/textures`, `/sons`, `/videos` | `pages/Catalogue.tsx:42-297` | **la vue elle-même** (jeu d'extensions figé) | **aucun** | `page`, `PAR_PAGE = 60` en dur (`:42`), Précédent/Suivant | `q` à **soumission explicite** (`saisie` vs `filtre`, `:88-89`), sous-chaîne chemin entier, insensible à la casse |
| `/modeles` | `pages/Modeles3D.tsx:54-298` | **`famille`** — 6 valeurs servies par `/api/v1/3d` (`:141`, `:203-223`) | **aucun** | `page`, `PAR_PAGE = 24` (`:54`) | `q` à soumission explicite, code **ou** nom |
| `PetAphrody.tsx`, `Ecran.tsx` | — | aucune vue-liste | — | — | — |

### 3.1 Ce que le serveur sait déjà filtrer

**L'index VFS** — `crates/tools/nie-site/src/vfs_index.rs` :

| Ce qu'il porte | Ligne | Exposé en query ? |
|---|---|---|
| `chemins: Vec<String>` **trié** | `:101` | oui, par `/b/<préfixe>` (dichotomie + balayage, `:224`) |
| `tailles: Vec<u32>` | `:102` | **rendu** dans `Fichier.taille` (`:82`) — **jamais filtrable ni triable** |
| `vues: [Vec<u32>; 4]` pré-calculées | `:104`, `:122-129` | oui, `/api/v1/{vue}` |
| Extensions par vue | `:47-54` | **figées dans le code**, pas de `?ext=` |
| Sous-chaîne insensible à la casse sur le **chemin entier** | `:174-193`, `:200-210` | oui, `?q=` |
| Sous-dossiers directs d'un préfixe | `:225-249` | oui, rendus **en entier** (jamais paginés — `:214-215`) |
| **CPK d'origine** | — | **perdu** : `state.rs:266-269` ne garde que `(chemin, file_size)` de `VfsEntry`, qui porte pourtant `cpk_filename` (`crates/engine/nie-formats/src/vfs.rs:129-134`) |

**Les routes** — `crates/tools/nie-site/src/routes/` :

| Route | Fichier:ligne | Query acceptée |
|---|---|---|
| `GET /api/v1/{vue}` | `api_v1.rs:98-129` | `page`, `per_page`, `q` (`mod.rs:25-32`) |
| `GET /b`, `GET /b/{*prefixe}` | `vfs.rs:152-182` | `page`, `per_page`, `q` **déclaré mais IGNORÉ** — `parcourir` (`vfs.rs:169-182`) ne lit que `p.offset()` et `p.per_page` |
| `GET /api/v1/chara` | `api_v1.rs:167-217` | `page`, `per_page` seuls. **`q` ignoré**, aucune facette, `ORDER BY` **figé** `zukan_order, internal_code` (`:181-183`) — alors que 12 colonnes sont lues (`:31-44`) dont `element`, `position`, `rarity`, `series` |
| `GET /api/v1/episodes` | `episodes.rs:43-50`, `:104` | **`since`** (epoch ms) et **`limit`** (défaut 5 000, plafond `LIMITE_MAX`) — c'est une API de synchronisation, pas de catalogue : ni `season`, ni `q`, ni `language`, alors que les colonnes existent (`:56-83`) |
| `GET /api/v1/3d/modeles` | `modeles3d.rs:367-378`, `:591` | `famille` (6 valeurs, `:168-175`), `page`, `per_page`, `q` (code **ou** nom, `:448-462` / `LIKE` SQL `:502`) |
| `GET /api/v1/3d/modeles/{famille}/{code}` | `modeles3d.rs:673-688` | `rapport` (`1` = joindre le rapport d'assemblage) |
| `GET /model/{famille}/{fichier}` | `modeles3d.rs:942-991` | `angle` (degrés, quantifié), `l`, `h` (bornés) |
| `GET /f/{*chemin}` | `vfs.rs:104-150` | aucune — un chemin exact |
| Bornes | `config.rs:27`, `:29`, `:189` | `PER_PAGE_MAX = 200`, `PER_PAGE_DEFAUT = 50`, `per_page.clamp(1,200)` |

**Le contrat client** — `packages/asset-source/src/contract.ts:59-64`, `:103` :
`catalogue?(vue, { page?, parPage?, q?, signal? })`. Il est **optionnel**, et
`apps/inacord/src/lib/desktop-source.ts` **ne l'implémente pas** : montée sous Inacord, la page
Catalogue affiche « Le catalogue est en cours de préparation » (`Catalogue.tsx:119-121`).

---

## 4. Ce que le VFS permettrait et que personne n'expose

| Capacité | Preuve | Qui l'utilise |
|---|---|---|
| Filtre par extension | `niers vfs find --ext <ext>` ; `niers vfs extract --ext` | Inacord (`ext`), la CLI. **Pas Aphrody** |
| CPK conteneur d'une entrée | `VfsEntry.cpk_filename` — `crates/engine/nie-formats/src/vfs.rs:129-134` ; `niers vfs stat` l'affiche | **personne** côté web |
| Nombre de CPK / entrées extra / loose | `Vfs::cpk_count()`, `extra_count()`, `loose_count()` — `vfs.rs:732-746` | `niers vfs stats` (936 CPK, 5 loose). Pas Aphrody |
| Montage dump vs packs | `Vfs::is_dump()` — `vfs.rs:417` | `/api/v1/health` (`Capacites.vfs_dump`, `state.rs:78`), affiché nulle part comme filtre |
| Histogramme des extensions | `niers vfs stats --top N` | CLI seule — **aucune route ne le publie** |
| Couverture de format **mesurée** (magic ou décodage complet) | `niers vfs formats [--parse] [--prefix] [--limit]` | CLI seule |
| Facette élément / poste sur les personnages | `niers vfs chara --element --position --limit` | CLI + Azalée. **Ni Inacord ni Aphrody** |
| Facette catégorie / élément sur les techniques | `niers vfs waza --category --element --limit` | CLI + Azalée |
| Glob (listes, exclusions `!`, `**`) | `crates/engine/nie-viola/src/filtre.rs:1-19` ; `niers find --glob` cumulable | Viola (dump) seul |
| Filtre par type `f`/`d`, profondeur, fichiers cachés | `niers find --type --depth --hidden` | CLI seule (disque, pas VFS) |
| Recherche dans le **contenu** | `niers grep` (regex, `-i`, `--glob`, `--ext`, `-l`) | CLI seule (disque, pas VFS) |
| Taille d'une entrée | `IndexVfs.taille()` — `vfs_index.rs:268-274` | rendue, jamais filtrée |

---

## 5. Matrice de couverture

Légende : ✅ présent · ◐ partiel (existe mais amputé, ou hors URL, ou codé en dur) · ❌ absent ·
`—` sans objet pour cette surface.

| # | Filtre | Azalée | Inacord | **Aphrody** | Servi par l'API |
|---|---|:--:|:--:|:--:|:--:|
| **Fichiers / VFS** |
| 1 | Recherche sous-chaîne sur le chemin | — | ✅ `ExplorerView.tsx:157` | ◐ catalogue oui, **explorateur non** | ✅ `?q=` `api_v1.rs:112` |
| 2 | Recherche **dans le parcours** `/b` | — | ✅ | ❌ | ◐ `q` déclaré (`mod.rs:30`) mais **ignoré** (`vfs.rs:169-182`) |
| 3 | Filtre par extension exacte | — | ✅ `ExplorerView.tsx:166` | ❌ | ❌ (extensions figées `vfs_index.rs:47-54`) |
| 4 | Filtre par famille d'asset (models/textures/audio/configs) | — | ✅ `ContentBrowser.tsx:101` | ◐ 4 vues, couvrant 143 246/255 308 | ✅ `/api/v1/{vue}` |
| 5 | Navigation par préfixe / dossier | — | ✅ | ✅ `Explorateur.tsx:43` | ✅ `/b/{*prefixe}` |
| 6 | Recherche **restreinte à un sous-arbre** | — | ❌ | ❌ | ❌ |
| 7 | Tri par nom | — | ✅ `ExplorerView.tsx:169` | ❌ | ❌ (ordre d'index figé) |
| 8 | Tri par taille | — | ✅ | ❌ | ❌ |
| 9 | Filtre par taille min/max | — | ❌ | ❌ | ❌ (`tailles` présent `vfs_index.rs:102`) |
| 10 | Filtre par **CPK d'origine** | — | ❌ | ❌ | ❌ (donnée jetée `state.rs:266-269`) |
| 11 | Filtre glob (`**`, `!excl`, listes) | — | ✅ Viola `ViolaView.tsx:86` | ❌ | ❌ |
| 12 | Vue liste / grille | — | ✅ `ExplorerView.tsx:174` | ❌ | — |
| 13 | Taille de vignette | — | ✅ `ExplorerView.tsx:178` | ❌ | — |
| 14 | `per_page` réglable | ✅ `WikiPagination.tsx:37` | ❌ | ❌ (60 en dur `Catalogue.tsx:42`) | ✅ `per_page` `config.rs:27` |
| 15 | **État de filtre dans l'URL** (partageable, indexable) | ✅ | ❌ (`localStorage`) | ❌ (`useState` `Catalogue.tsx:73`) | ✅ |
| 16 | Compte total affiché | ✅ | ✅ `ExplorerView.tsx:1044` | ◐ page X/Y, pas de total par dossier | ✅ `Page.total` `mod.rs:52` |
| **Catalogue de personnages** |
| 17 | Recherche par nom FR/EN/JA | ✅ `service.ts:1400` | ✅ `SearchView.tsx` | ❌ (aucune page) | ❌ `/api/v1/chara` n'a pas de `q` |
| 18 | Élément (6 valeurs) | ✅ | ◐ recherche texte `GameDataView.tsx:137` | ❌ | ❌ (colonne lue `api_v1.rs:36`) |
| 19 | Poste (5) | ✅ | ◐ | ❌ | ❌ (colonne lue `:37`) |
| 20 | Rareté (4) | ✅ | ❌ | ❌ | ❌ (colonne lue `:38`) |
| 21 | Série (9) | ✅ | ◐ | ❌ | ❌ (colonne lue `:39`) |
| 22 | Genre (2) | ✅ | ❌ | ❌ | ❌ |
| 23 | Style de jeu (6) | ✅ | ❌ | ❌ | ❌ |
| 24 | Tranche d'âge (8) | ✅ | ❌ | ❌ | ❌ |
| 25 | Équipe (199) | ✅ `TeamFilter.tsx:28` | ❌ | ❌ | ❌ |
| 26 | Rôle (coach/coordinator) | ✅ | ❌ | ❌ | ❌ |
| 27 | Tri du catalogue | ❌ (fixe) | ✅ `GameDataView.tsx:688` | ❌ | ❌ (`ORDER BY` figé `api_v1.rs:181`) |
| **Techniques / objets / autres catalogues** |
| 28 | Catégorie de technique (4) | ✅ | ◐ | ❌ | ❌ |
| 29 | Présence d'une vidéo | ✅ `SkillFilterBar.tsx:187` | ❌ | ❌ | ❌ |
| 30 | Inclure hyper/aura | ✅ `:205` | ❌ | ❌ | ❌ |
| 31 | Overdrive | ✅ `:223` | ❌ | ❌ | ❌ |
| 32 | Fourchette numérique (puissance 0→880) | ✅ `:111-143` | ❌ | ❌ | ❌ |
| 33 | Tri par puissance / coût | ✅ `sort` | ✅ (générique) | ❌ | ❌ |
| 34 | Catégorie d'objet (19) | ✅ `item/page.tsx:58-78` | ◐ | ❌ | ❌ |
| 35 | Catégorie d'illustration (11) | ✅ `service.ts:120-132` | ✅ dossiers réels `GalleryView.tsx:288` | ❌ | ❌ |
| 36 | Langue / variante d'un asset | ❌ | ✅ `GalleryView.tsx:290` | ❌ | ❌ |
| **Épisodes / médias** |
| 37 | Saison / numéro d'épisode | ❌ (pas de page) | ✅ `recherche.ts:46-64` (`s3e12`) | ❌ | ❌ (`/api/v1/episodes` n'a que `since`/`limit`) |
| 38 | Langue de piste | ❌ | ✅ `CinemaView.tsx:1207` | ❌ | ❌ (colonne `language` `episodes.rs:78`) |
| 39 | Sous-titres présents / vu | ❌ | ✅ | ❌ | ❌ |
| 40 | Classement par pertinence pondérée | ❌ | ✅ `recherche.ts:186-201` | ❌ | ❌ |
| 41 | Repli approché (fuzzy) | ❌ | ✅ `recherche.ts:292-295` | ❌ | ❌ |
| **3D** |
| 42 | Famille de modèle (6) | — | ❌ | ✅ `Modeles3D.tsx:141` | ✅ `modeles3d.rs:367` |
| 43 | Recherche code **ou** nom | — | ❌ | ✅ `Modeles3D.tsx:143` | ✅ `modeles3d.rs:448-462` |
| **Reverse / forge** |
| 44 | Recherche fonction par nom ou adresse | — | ✅ `reDb.ts:154-183` | ❌ | ❌ (aucune route RE) |
| 45 | Filtre par statut de forge | — | ◐ en dur `DashboardView.tsx:295` | ❌ | ❌ |
| **Transverse** |
| 46 | Recherche globale multi-gisements | ✅ `/search` (q seul) | ✅ `SearchView.tsx` (`kind`) | ❌ | ❌ |
| 47 | Facettes avec **comptes** | ✅ `getGalleryCategoryCounts` `service.ts:2799` | ✅ `CinemaView.tsx:592-607` | ◐ totaux de famille 3D seulement | ◐ `/api/v1/health` donne 4 totaux (`api_v1.rs:75-82`) |
| 48 | Export de la liste filtrée | ❌ | ✅ CSV/JSON `GameDataView.tsx:790-812` | ❌ | ❌ |

### Compte

- **48 filtres recensés.**
- Aphrody ✅ (pleinement) : **3** — #5 navigation par préfixe, #42 famille 3D, #43 recherche 3D.
- Aphrody ◐ (partiels) : **3** — #1 recherche par chemin (catalogue oui, explorateur non),
  #4 famille d'asset (4 vues seulement, 143 246 fichiers sur 255 308), #16 total affiché
  (page X/Y, pas de total par dossier).
- Aphrody ❌ : **42**.

> **manquant = 42** (45 si l'on compte les trois partiels comme des manques, ce qui se défend pour
> #1 et #4).

- Servis par l'API mais **non exposés par l'interface** : **1** seul — #14 (`per_page`, plafonné à
  200, `config.rs:27`). Les 41 autres exigent du code serveur en plus du client.
- Déclarés par l'API et **inopérants** : #2 (`q` sur `/b`) — un bug silencieux, pas un manque.
- Azalée ✅ sur 26 des 48 ; Inacord ✅ sur 22. **Aucun filtre n'est présent dans Aphrody et absent
  des deux autres** : Aphrody est un sous-ensemble strict, sauf sur la 3D (#42, #43) que ni Azalée
  ni Inacord n'ont.

---

## 6. Pour chaque manque : la source de données et son coût

| # | Filtre manquant | Source de données | Coût |
|---|---|---|---|
| 2 | `q` sur le parcours `/b` | déjà dans l'index | **déjà dans l'index** — `DemandePage.q` est parsé (`mod.rs:30`), il suffit de le lire dans `parcourir` (`vfs.rs:169`). ~10 lignes |
| 3 | Extension exacte | `IndexVfs.chemins` | **calculable sans passe** : un `Vec<u32>` d'index par extension se construit dans `IndexVfs::depuis` (`vfs_index.rs:122-129`), au même parcours que les 4 vues. 44 extensions distinctes mesurées |
| 7 / 8 | Tri nom / taille | `chemins` (déjà trié par nom), `tailles` (`vfs_index.rs:102`) | **déjà dans l'index**. Le tri par nom est l'ordre naturel ; le tri par taille demande un `Vec<u32>` d'index pré-trié (une passe à la construction) |
| 9 | Taille min/max | `tailles` | **déjà dans l'index** — un `filter` sur la valeur déjà lue, coût nul |
| 10 | CPK d'origine | `VfsEntry.cpk_filename` — `crates/engine/nie-formats/src/vfs.rs:131` | **déjà disponible, actuellement jeté**. Corriger `state.rs:266-269` pour porter le triplet, et interner les 936 noms de CPK (`u16` + table) : ~2 Ko de plus, pas 255 308 `String` |
| 11 | Glob | `crates/engine/nie-viola/src/filtre.rs` | **réutilisable tel quel** — le moteur existe et est testé ; il faut le déplacer d'une crate `engine` vers une dépendance de `nie-site`, ou l'appeler par `nie-formats` |
| 6 | Recherche + préfixe | index trié | **calculable** : `partition_point` sur le préfixe (`vfs_index.rs:224`) puis balayage filtré — c'est la combinaison des deux mécanismes existants |
| 12/13/15 | Grille, vignette, état en URL | — | **client seul** (`apps/nie-web/src/pages/Catalogue.tsx`) : `useSearchParams` de React Router, aucune donnée nouvelle |
| 14 | `per_page` réglable | — | **déjà servi** (`config.rs:27`) : un `<select>` à câbler côté client |
| 16 | Total par dossier | `Dossier.total_fichiers` — `vfs_index.rs:95` | **déjà rendu**, jamais affiché par `Explorateur.tsx` |
| 17–26 | Facettes personnage (nom, élément, poste, rareté, série, genre, style, âge, équipe, rôle) | `inagle_characters` — 40 colonnes mesurées, `var/mirror.sqlite` | **calculable en SQL** : ajouter des `WHERE` paramétrés à `api_v1.rs:180-184`. Cardinalités faibles (6/5/4/9/2/199), donc des facettes chiffrées par `GROUP BY` sont bon marché. `playstyle` et `ageGroup` sortent de `sheet_data->>` / `age_group` (cf. `service.ts:1458-1487`) et sont plus coûteux |
| 27 | Tri du catalogue | mêmes colonnes | **calculable** — remplacer l'`ORDER BY` figé (`api_v1.rs:181-183`) par une whitelist de colonnes. Ne **jamais** accepter un nom de colonne venu du client, la crate a déjà cette doctrine (`api_v1.rs:9-11`) |
| 28–34 | Techniques et objets | tables `inagle_*` du miroir | **calculable en SQL**, mais exige d'abord d'**écrire les routes `/api/v1/skill` et `/api/v1/item`** — elles n'existent pas (`app.rs:124-176`) |
| 35/36 | Catégorie d'illustration, langue | sous-dossiers réels du VFS (`data/dx11/menu/220_img`) + `gallery_config` | **déjà dans l'index** pour la catégorie (ce sont des préfixes) ; la langue est un segment de chemin, donc lisible sans passe. Inacord le fait déjà (`lib/galerie.ts:24-59`) |
| 37–39 | Saison, épisode, langue, vu | `data/anime/episodes.db` — colonnes `season`, `episode`, `language`, `duration` déjà lues (`episodes.rs:56-83`) | **calculable en SQL** — les colonnes sont là, seule la clause manque |
| 40/41 | Pertinence pondérée, fuzzy | — | **à écrire**, mais `apps/inacord/src/lib/recherche.ts:186-295` est un portage direct (TypeScript → Rust, ou réutilisation côté client) |
| 44/45 | RE, forge | `var/niers.sqlite` (`function`, `forge_unit`, `v_forge_function`) | **calculable en SQL**, mais exige une route neuve. **Attention** : la KB du VPS est ancrée sur le build transitoire `4c2b91fbae6f…`, pas sur la cible — toute mesure chiffrée exige un `niers rebuild` préalable |
| 46 | Recherche globale | les 4 gisements, via `@niers/catalog` | **coûteux** : une jointure inter-gisements, sans clé commune entre le jeu et la série (cf. CLAUDE.md § *Les quatre gisements*) |
| 47 | Facettes chiffrées | `GROUP BY` SQL ; `Vec<u32>` par facette côté index | **calculable** ; pour le VFS, gratuit si les listes par extension sont pré-calculées |
| 48 | Export de la liste filtrée | la page déjà rendue | **client seul** |

**Aucun des 42 manques n'exige une passe sur les 255 308 entrées.** L'index est construit une fois
au démarrage (`state.rs:260-278`) et le parcours qui pré-calcule les 4 vues (`vfs_index.rs:122-129`)
peut, dans la même boucle, produire les listes par extension, par CPK et l'ordre par taille. Le
surcoût est en mémoire (quelques `Vec<u32>` de 255 k éléments ≈ 1 Mio chacun), pas en temps.

---

## 7. Ordre de priorité

Du plus utile au moins utile, avec la raison.

1. **`q` sur `/b` (#2)** — c'est un **bug**, pas un manque : le paramètre est déclaré (`mod.rs:30`)
   et silencieusement ignoré (`vfs.rs:169-182`). Un client qui l'envoie croit filtrer. ~10 lignes.
2. **État des filtres dans l'URL (#15)** — sans lui, aucun filtre d'Aphrody n'est partageable,
   indexable, ni conservé au rechargement. C'est le préalable de tous les autres : ajouter dix
   facettes en `useState` ne ferait que dupliquer la dette d'Inacord.
3. **Recherche dans l'explorateur (#1 côté `/b`) + total par dossier (#16)** — l'explorateur
   d'Aphrody n'a **strictement aucun** filtre (`Explorateur.tsx:43-117`), et `total_fichiers` est
   déjà rendu par le serveur (`vfs_index.rs:95`). Deux gains immédiats sur la vue la plus utilisée.
4. **Filtre par extension (#3)** — c'est le filtre le plus demandé du VFS, celui qu'Inacord a en
   premier (`ExplorerView.tsx:166`) et le seul moyen d'atteindre les **112 062 fichiers hors des
   quatre vues** (`.bin`, `.p3lip`, `.objbin`…). Pré-calculable au même parcours que les vues.
5. **`per_page` réglable (#14)** — déjà servi et plafonné à 200 (`config.rs:27`), aucun code
   serveur : un `<select>` transforme 60 résultats en 200 sur la même requête.
6. **Facettes du catalogue personnages (#17–#22)** — `/api/v1/chara` lit déjà `element`,
   `position`, `rarity`, `series` (`api_v1.rs:36-39`) et **ne les expose pas** : quatre `WHERE`
   paramétrés suffisent pour transformer une liste de 6 166 lignes en catalogue navigable. C'est
   aussi ce qui manque pour qu'Aphrody ait une page personnages du tout.
7. **Tri (#7, #8, #27)** — un catalogue sans tri force à paginer pour trouver ; `tailles` et
   `zukan_order` sont déjà là. Whitelist de colonnes obligatoire, jamais un `ORDER BY` venu du
   client.
8. **CPK d'origine (#10)** — la seule facette qui répond à « d'où vient ce fichier », donnée que le
   VFS porte (`vfs.rs:131`) et que le site jette (`state.rs:266-269`). 936 valeurs, internables en
   `u16`. Utile au modding et au diagnostic, moins au visiteur.
9. **Taille min/max (#9)** — gratuit, mais d'intérêt étroit (chasse aux gros assets).
10. **Facettes chiffrées (#47)** — un filtre qui annonce « 0 résultat » après le clic est un filtre
    raté ; les comptes sont gratuits si les listes par facette existent (étapes 4 et 6).
11. **Glob (#11)** — puissant, moteur déjà écrit et testé (`nie-viola/src/filtre.rs`), mais il
    double le filtre par extension pour un public plus étroit.
12. **Épisodes : saison / épisode / langue (#37–#39)** — colonnes déjà lues (`episodes.rs:56-83`),
    mais `/api/v1/episodes` est aujourd'hui une **API de synchronisation** (`since`/`limit`) ; en
    faire un catalogue est un changement de nature, à décider avant de coder.
13. **Routes `/api/v1/skill` et `/api/v1/item` + leurs facettes (#28–#34)** — le plus gros gain
    fonctionnel, mais tout est à écrire : deux routes, deux DTO, sept facettes. À faire après que
    `chara` ait servi de patron.
14. **Vue grille / taille de vignette (#12, #13)** — confort, client seul, sans effet sur ce qui
    est atteignable.
15. **Recherche restreinte à un sous-arbre (#6)** — combinaison des deux mécanismes existants,
    mais peu réclamée tant que #1 et #3 manquent.
16. **Pertinence pondérée et fuzzy (#40, #41)** — un portage de `recherche.ts:186-295`. Utile
    seulement quand il y aura assez de champs cherchables pour que le classement compte.
17. **Export de la liste filtrée (#48)** — client seul, faible enjeu tant que les listes ne sont
    pas filtrables.
18. **RE et forge (#44, #45)** — public d'experts, et la KB du VPS n'est **pas ancrée sur la
    cible** : y brancher une route publierait des chiffres faux.
19. **Recherche globale multi-gisements (#46)** — le plus coûteux et le moins sûr : le jeu et la
    série n'ont aucune clé commune, un rapprochement par le nom ne peut pas se présenter comme un
    fait.

---

## 8. Divergences relevées au passage (non corrigées)

1. `crates/tools/nie-site/src/routes/vfs.rs:169-182` — `DemandePage.q` est accepté par `/b` et
   **jamais lu**. Silencieux.
2. `crates/tools/nie-site/src/state.rs:266-269` — `cpk_filename` est jeté à la construction de
   l'index alors que `VfsEntry` le porte.
3. `apps/inacord/src-tauri/src/lib.rs:1669` — `vfs_related` est **sensible à la casse**, contrairement
   à `vfs_find_paged` (`crates/engine/nie-explore/src/listing.rs:173-178`).
4. `apps/inacord/src/lib/reDb.ts:183` et `:258` — le `LIKE` n'échappe ni `%` ni `_` ; un `%` tapé
   par l'utilisateur agit comme un joker SQL. `vfsIndexDb.ts:127-128` fait l'inverse (`escapeLike` +
   `ESCAPE '\'`).
5. `apps/inacord/src/components/ExplorerView.tsx:293` et `editor/ContentBrowser.tsx:110` — `vfs_ls`
   accepte `limit`/`offset` (`lib.rs:469-473`) mais ils ne sont pas passés : le dossier entier
   traverse l'IPC avant d'être tronqué en React.
6. Aucune vue d'Inacord n'est virtualisée — partout un `slice(0, n)`.
7. `apps/inacord/src/lib/desktop-source.ts` n'implémente pas `catalogue` : la page Catalogue de
   `nie-web` est inopérante sous l'hôte Inacord (`Catalogue.tsx:119-121`).
8. `apps/azalee/components/wiki/MediaShell.tsx:38-41` et `app/keshin/page.tsx:15` pointent vers
   quatre routes absentes d'Azalée.
9. `apps/azalee/app/item/(liste)/page.tsx:90` — aucune option « toutes catégories » : impossible de
   lister tous les objets.
10. `apps/azalee/app/chara/players-client.tsx:80-89` — « Tout effacer » omet `ageGroup` et `status`.
11. `apps/azalee/app/capsule/page.tsx:57-62`, `app/succes/page.tsx:97-103`,
    `components/news/CategoryChips.tsx:22` — ces liens reconstruisent l'URL depuis zéro et perdent
    `q` et `page`.
12. `apps/azalee/components/news/AdvancedFilters.tsx:35` et `app/actions/zukan.ts:8` — code mort :
    filtres produits ou déclarés, lus par personne.
