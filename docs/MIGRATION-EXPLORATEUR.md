# Migration du wiki vers l'explorateur — outils et galerie

Depuis la fusion (`docs/FUSION.md`), le site du wiki (`apps/azalee`), sa bibliothèque
(`packages/azalee`) et l'explorateur Tauri (`apps/inacord`) vivent sous la même racine. Ce
document décrit le passage des **six outils** (`/tools`) et de la **galerie** (`/gallery`) du web
vers l'application de bureau : ce qui a été refait, où, sur quelle source de données — et ce qui,
côté `apps/azalee`, devient redondant.

Mesures datées du **2026-09-02**, relevées sur cette machine (miroir `var/mirror.sqlite`, VFS
monté). Aucune n'est recopiée d'un document antérieur.

---

## 1. Ce qui change : la source, pas le dossier

|  | `apps/azalee` (web) | `apps/inacord` (bureau) |
|---|---|---|
| Fichiers du jeu | HTTP vers `cdn.rosegriffon.fr` | **en direct**, `nie-formats` via `src/lib/api.ts` |
| Données extraites | Supabase / PostgREST (`wikiService`) | `src/lib/wikiDb.ts` — `tauri-plugin-sql` sur le miroir |
| Rendu | Next.js 16.3.0-canary.37, Server Components | React + Vite, tout client |
| Session | `getServerSession` (Better Auth) | **aucune** — application locale |

Un composant migré change donc de **source de données**. Trois conséquences mesurées, chacune en
faveur du bureau :

1. **la galerie compte juste.** Le web réunit deux fonds qui ne se rejoignent jamais : la table
   `inagle_gallery` (360 lignes) et un manifeste figé `packages/azalee/src/data/menu-gallery-manifest.json`
   (3 579 entrées). D'où le défaut visible : la pastille « Toutes » annonce 3 939 items pour une
   liste qui n'en rend que 360. Le bureau liste le VFS : **17 085 `.g4tx`** sous
   `data/dx11/menu/220_img/`, **45 catégories** découvertes (le manifeste en connaissait 6), et
   chaque total est relevé par `nie_explore::listing::find_paged`, qui compte avant de paginer.
2. **le traducteur cherche vraiment le romaji.** La *server action* du web pré-filtre par `ilike`
   PostgREST sur `name_fr`/`name_en`/`name_ja` ; le romaji est dérivé de `name_ja` et n'existe dans
   aucune colonne, donc une saisie qui ne correspond QU'au romaji ne remontait jamais. Le bureau
   charge l'index complet (**9 403 lignes → 8 328 entrées** après dédoublonnage par nom) et score
   tout localement, romaji compris. **5 131 entrées** portent un romaji dérivé.
3. **le comparateur calcule au lieu d'interpoler.** `interpolateStats()` du wiki reconstruit une
   courbe entre `lv1`, `lv30`, `lv50` et `lv99` — sauf que `$.stats.lv30` est **nul sur 6 166
   lignes sur 6 166** du miroir : la branche « paliers complets » n'est jamais prise, et le wiki
   affiche une droite lv1→lv99. Le bureau appelle `api.gameDataCalculateStats`, c'est-à-dire
   `nie_core::growth::calculate_stats` sur les tables de croissance IEVR embarquées, à partir du
   `chara_param_id` — qui est exactement `inagle_characters.id` (`0x23DC2602`, vérifié).

---

## 2. Ce qui a été construit

### Vues

| Vue | Fichier | Remplace |
|---|---|---|
| **Outils** (5 onglets) | `apps/inacord/src/components/ToolsView.tsx` | `apps/azalee/app/tools/page.tsx` |
| Traducteur | `src/components/tools/TranslatorPanel.tsx` | `components/tools/TranslatorClient.tsx` + `app/actions/translate.ts` |
| Calculateur de stats | `src/components/tools/StatCalculator.tsx` | `components/wiki/StatCalculator.tsx` |
| Comparateur | `src/components/tools/ComparatorPanel.tsx` | `components/wiki/CharacterComparator.tsx` + `app/tools/compare/page.tsx` |
| Équipe aléatoire | `src/components/tools/RandomTeamPanel.tsx` | `components/wiki/RandomTeamGenerator.tsx` |
| Mon équipe | `src/components/tools/TeamBuilderPanel.tsx` | `components/tools/my-team/` (6 fichiers) |
| **Galerie** | `src/components/GalleryView.tsx` | `app/gallery/page.tsx`, `components/wiki/GalleryGrid.tsx`, `GalleryLightbox.tsx`, `GalleryCard.tsx`, `filters/GalleryFilterBar.tsx` |

Câblage : `src/App.tsx` (deux entrées de barre latérale — `gallery` sous « Données », `tools` sous
« Outils » — et deux `TabsContent`), `src/components/AppMenu.tsx` (`VIEW_TABS`).

**Une vue à onglets, pas cinq entrées de barre latérale.** Trois raisons, aucune esthétique :
la barre porte déjà douze entrées et son groupe « Outils » désigne les outils *du dépôt* (mods, RE,
Viola, Live mod, Lua) ; `AppMenu` n'attribue un accélérateur qu'aux **neuf premières** vues
(`Ctrl+1…9`), donc cinq entrées de plus rendraient muettes cinq vues existantes ; et les trois
outils d'équipe partagent le même roster de 6 166 lignes, chargé une seule fois par `ToolsView`.

### Modules

| Module | Rôle |
|---|---|
| `src/lib/traduction.ts` | Normalisation, Levenshtein, barème de score, dédoublonnage. **Pur.** |
| `src/lib/wikiQueries.ts` | Le SQL du miroir et la forme des lignes. **Pur.** |
| `src/lib/galerie.ts` | Catégories, titres, vignettes dédiées, assemblage. **Pur.** |
| `src/lib/equipe.ts` | Roster → joueur, tirage, auto-remplissage, filtres. **Pur.** |
| `src/lib/filtrage.ts` | `useFiltered`, extrait de `GameDataView` (deux appelants désormais). |
| `src/lib/wikiDb.ts` | +4 méthodes : `chargerIndexNoms`, `chargerRoster`, `chargerEncadrement`, `techniquesDuPersonnage`. |
| `src/lib/teamsDb.ts` | Compositions locales (table `teams`, `mods.db`). |
| `src/lib/verification-migration.ts` | Le contrôle exécutable (§5). |

Le SQL vit dans un module **pur** parce que `@tauri-apps/plugin-sql` n'existe qu'à l'intérieur de
la webview : tout module qui l'importe est invérifiable hors application. Ainsi le contrôle rejoue
*exactement* les requêtes que les vues envoient.

### Le constructeur d'équipe sans session

Le wiki enregistre côté serveur (`app/actions/teams.ts`, derrière `getServerSession`) : sans compte
connecté, le bouton « Créer » n'apparaît pas et il ne reste qu'un brouillon en `localStorage`
(`azalee-my-team`). Le bureau n'a ni compte ni session — mais il a un disque.

**Choix retenu : table `teams` de `mods.db`**, migration **v4** ajoutée à
`apps/inacord/src-tauri/src/lib.rs` (`mods_migrations`), même convention que `modsDb`,
`jobsDb` et `vfsIndexDb`, qui partagent déjà ce fichier — une seule base à migrer, une seule à
sauvegarder. `src/lib/teamsDb.ts` en est la façade : plusieurs compositions **nommées**,
persistées, listées, rechargées, supprimées, hors ligne. C'est plus que ce que le web offre à un
visiteur non connecté.

Le pont avec le site n'est pas perdu : le code de partage est celui de
`@rosegriffon/azalee/game/team-code`, **le même des deux côtés**. Une composition faite dans
l'explorateur se colle dans l'URL du wiki, et réciproquement (boutons « Copier le code » /
« Coller un code »).

---

## 3. Ce qui a été supprimé — doublons

Règle appliquée : **une seule implémentation survit, et c'est la meilleure**, pas la plus proche.

| Doublon | Ce qui survit | Ce qui disparaît |
|---|---|---|
| `useFiltered` | `src/lib/filtrage.ts` | la copie locale de `GameDataView.tsx` |
| `StatsCalculator` (calculateur de stats) | `src/components/tools/StatCalculator.tsx`, monté par `ToolsView` **et** `GameDataView` | la définition locale de `GameDataView.tsx` (148 lignes) |
| `RARITY_LABELS` | exporté par `tools/StatCalculator.tsx` | la constante locale de `GameDataView.tsx` |
| Formations, facteur de poste, synergies, code de partage | `@rosegriffon/azalee/game/*` (**importé**, 91 formations dont 83 décodées du jeu) | aucune réécriture côté explorateur — les 12 formations écrites à la main du générateur web ne sont pas reprises |
| Barème de score du traducteur | `src/lib/traduction.ts` | la copie de `app/actions/translate.ts` (686 lignes) devient redondante |
| Fabrique de vignettes | `src/lib/thumbs.ts` (déjà unique) | la galerie **n'en crée pas une seconde** |
| Conversion ligne miroir → joueur | `equipe.versJoueur` | trois copies évitées (comparateur, générateur, constructeur) |
| Tri par rareté | `equipe.trier` (un seul point de tri du module) | deux `.sort()` dispersés |

### Doublons repérés mais NON traités ici

Ils vivent hors du périmètre d'écriture de ce chantier (`apps/azalee`, `packages/azalee`) et sont
consignés pour la suite :

* **`humanSize` existe cinq fois.** `apps/azalee/app/cpk/CpkBytesPreview.tsx:34` et
  `apps/azalee/app/cpk/CpkPackageViewer.tsx:16` sont strictement identiques (copier-coller) **et
  plafonnées au mébioctet** : un CPK de 3 Go s'y affiche « 3072.00 Mo ». `formatBytes` de
  `apps/azalee/components/save/SaveUploader.tsx:35` est une quatrième forme, `formatOctets` de
  `@niers/catalog/jeu` (réexporté par `packages/azalee/src/cpk/video.ts:48`) une cinquième.
  **La meilleure est `apps/inacord/src/lib/bytes.ts:14`** : elle gère les gibioctets et adapte
  le nombre de décimales. C'est elle qui devrait survivre — mais elle vit dans l'application de
  bureau, donc le web devrait plutôt converger vers `formatOctets` de `@niers/catalog`, déjà
  client-safe et déjà la source unique des conventions du serveur.
* **Le rendu hexadécimal existe quatre fois.** `hexLines` de `apps/inacord/src/lib/bytes.ts`
  d'un côté ; `CpkHexViewer.tsx`, `CpkBytesPreview.tsx` et `CpkFormationViewer.tsx` d'azalée de
  l'autre. Aucune n'a été touchée : la galerie et les outils n'affichent pas d'octets.

---

## 4. Ce qui devient redondant côté `apps/azalee`

**Fait le 2026-09-06** : les pages, composants et *server actions* listés ci-dessous ont été
retirés d'`apps/azalee`, et les références entrantes de §4.2 corrigées dans le même commit
(menu, barre latérale mobile, plan de site, redirections, boutons « Comparer », ossature média).
Ce qui suit reste la liste exacte de ce qui est parti, et de ce qui ne devait pas partir (§4.3,
respecté : `app/tools/niers/` et son endpoint de mise à jour sont intacts).

Rédaction d'origine, conservée : **rien n'avait été supprimé dans `apps/azalee` par ce chantier.** Le site est en production, un autre
agent y travaillait en parallèle (27 fichiers modifiés non commités, dont `app/videos`, `app/skill`
et `packages/azalee/src/cpk`), et retirer `/tools` et `/gallery` du web est une décision de mise en
ligne. Cette section est la liste exacte qui permet de le faire proprement.

### 4.1 Fichiers dont la fonction est intégralement reprise

Pages :

```
apps/azalee/app/tools/page.tsx                    (index des outils)
apps/azalee/app/tools/translator/page.tsx
apps/azalee/app/tools/stats/page.tsx
apps/azalee/app/tools/compare/page.tsx
apps/azalee/app/tools/random-team/page.tsx
apps/azalee/app/tools/my-team/page.tsx
apps/azalee/app/tools/my-team/TeamBuilderLoader.tsx
apps/azalee/app/tools/my-team/opengraph-image.tsx
apps/azalee/app/gallery/page.tsx
```

Composants — vérifié : **aucun autre importateur** (`StatCalculator` n'est cité que dans des
commentaires de `CourbeExperience.tsx` et `demo/DemoStatsPanel.tsx`, jamais importé) :

```
apps/azalee/components/tools/TranslatorClient.tsx          454 l.
apps/azalee/components/wiki/StatCalculator.tsx             239 l.
apps/azalee/components/wiki/CharacterComparator.tsx        816 l.
apps/azalee/components/wiki/RandomTeamGenerator.tsx      1 337 l.
apps/azalee/components/tools/my-team/ (6 fichiers)       2 966 l.
apps/azalee/components/wiki/GalleryGrid.tsx                 80 l.
apps/azalee/components/wiki/GalleryLightbox.tsx            274 l.
apps/azalee/components/wiki/GalleryCard.tsx
apps/azalee/components/wiki/filters/GalleryFilterBar.tsx    73 l.
```

Backend (*server actions*) :

```
apps/azalee/app/actions/translate.ts   686 l.  — importé seulement par TranslatorClient
apps/azalee/app/actions/teams.ts               — importé seulement par my-team/{TeamBuilder,PlayerDetailPanel}
```

### 4.2 Références entrantes à corriger dans le même mouvement

Les retirer sans ces corrections laisse le site pointer vers des routes mortes **depuis sa propre
navigation** :

| Fichier | Ce qui pointe vers les pages retirées |
|---|---|
| `apps/azalee/components/Shell.tsx:124` et `:392` | `pathname.startsWith("/tools")` — libellé et état actif du menu |
| `apps/azalee/components/wiki/MediaShell.tsx:37` | onglet `{ path: "/gallery", label: "Illustrations" }` de la barre média |
| `apps/azalee/app/sitemap.ts:90,103-120` | `/gallery`, `/tools`, `/tools/my-team`, `/tools/stats`, `/tools/compare`, `/tools/random-team`, `/tools/translator` |
| `apps/azalee/next.config.ts:170,175` | redirections vers `/tools/compare` et `/tools/random-team` |
| `apps/azalee/app/chara/players-client.tsx:261` | bouton « comparer » |
| `apps/azalee/app/chara/[id]/page.tsx:571` | `/tools/compare?char1=…` |
| `apps/azalee/components/wiki/CharacterSheet.tsx:622` | `/tools/compare?char1=…` |
| `apps/azalee/app/textures/[[...path]]/page.tsx:4` | commentaire d'en-tête citant `/gallery` |
| `apps/azalee/app/skill/(liste)/page.tsx:180` | commentaire citant la ligne de compte partagée avec `/gallery` |

### 4.3 Ce qui reste vrai côté web — à NE PAS toucher

* **`apps/azalee/app/tools/niers/latest.json/route.ts` est l'endpoint de mise à jour de
  l'explorateur.** `apps/inacord/src-tauri/tauri.conf.json:35` le déclare en **premier**
  endpoint (`https://azalee.rosegriffon.fr/tools/niers/latest.json`), GitHub Releases n'étant qu'un
  repli. Le déplacer, le renommer ou casser la route coupe la mise à jour automatique de **toutes
  les installations déjà déployées**.
* **`apps/azalee/app/tools/niers/`** est la page de téléchargement de l'application. Migrer dans
  l'application la page qui permet de la télécharger n'aurait aucun sens.
* **`wikiService.getGalleryList` et `getGalleryCategoryCounts` doivent rester.** Elles ne servent
  pas que la page : `packages/azalee/src/server/serve.ts:216` expose `api/gallery` et
  `packages/azalee/src/remote/types.ts:52,146` en dérive ses types. Les supprimer casserait l'API
  headless et ses consommateurs distants.
* **`GALLERY_CATEGORIES` et `menu-gallery-manifest.json`** restent utilisés par ces mêmes routes et
  par `packages/azalee/scripts/build-menu-gallery-manifest.ts`.
* `components/wiki/StatHeptagon.tsx` et `components/wiki/CharacterSearchDialog.tsx` sont partagés
  avec `CharacterSheet` : ils survivent au retrait des outils.

---

## 5. Preuves

Le contrôle qui compte n'est pas le build : c'est de savoir si **la source de données répond**.
Un `ilike` PostgREST transposé en SQLite, une table renommée entre Supabase et le miroir, une
colonne vide — tout cela compile parfaitement et rend un cadre vide.

```sh
bun --bun apps/inacord/src/lib/verification-migration.ts
```

Le script rejoue sur les vraies données exactement ce que les vues envoient. Sortie du
2026-09-02 : **31 réussis, 0 échec, 0 sauté** (un contrôle qui ne peut pas s'exécuter est annoncé
« SAUTÉ », jamais compté comme réussi).

Il a trouvé **deux vrais défauts**, tous deux invisibles à la compilation :

1. **la clé de jointure des techniques était fausse.** `inagle_skills.id` vaut `whk00010` (code
   interne), alors que `inagle_characters.skills` porte des hachages `0x8C382852`. Le hachage vit
   dans `json_extract(data, '$.skillID')` — renseigné sur les 1 002 lignes ; la colonne `hash_id`,
   elle, est vide sur les 1 002. Joindre sur `id` rendait **toujours** zéro ligne : le comparateur
   aurait affiché « Aucune technique » pour chaque personnage, sans qu'aucune erreur ne remonte.
   Corrigé dans `wikiQueries.sqlTechniquesParIds` → 30 techniques résolues sur 5 personnages.
2. **le seuil flou du traducteur.** Deux fautes de frappe sur un nom de dix caractères repassent
   sous le seuil de 0,62 (la passe floue est plafonnée à 0,7). C'est le barème du wiki, repris tel
   quel ; le contrôle le dit maintenant explicitement au lieu de le laisser croire.

Autres mesures relevées par le contrôle :

```
requêtes de l'index de noms   6 025 chara · 1 002 waza · 1 737 objets · 70 tactiques
                              208 équipes · 305 keshin · 56 totems
dédoublonnage                 9 403 lignes → 8 328 entrées distinctes
roster                        6 025 lignes, rarity_code extrait sur 6 025/6 025
postes / éléments FR          tous mappés, aucun inconnu
encadrement                   102 lignes (Coach, Coordinator, Manager)
galerie                       17 085 .g4tx, 45 catégories
vignettes dédiées             363/363 couples img_/thumb_ résolus dans le VFS
gallery_config ↔ VFS          360/360 illustrations retrouvées
```

Contrôles de conformité :

```sh
cd apps/inacord && bunx tsc --noEmit          # propre
cd apps/inacord && bunx vite build            # ✓ built
cd apps/inacord/src-tauri && cargo check      # Finished, exit 0
bunx oxlint -c .oxlintrc.json -A style -A pedantic -A restriction apps/inacord/src
                                                   # 49 avertissements, exactement le compte
                                                   # d'avant le chantier — aucun nouveau
```

---

## 6. Ce qui reste ouvert

* **La passe romaji→kana n'est pas portée.** La *server action* du web utilise
  `wanakana.toHiragana`/`toKatakana` pour transformer une saisie latine en kana avant de comparer.
  `wanakana` n'est pas une dépendance de l'explorateur. Le chemin utile (taper « endou », trouver
  えんどう) reste couvert par le romaji DÉRIVÉ de `name_ja`, comparé à chaque ligne ; ce qui
  disparaît, c'est la recherche d'un kana par un autre kana translittéré. **Dépendance à installer
  si on veut la parité complète : `wanakana` (celle d'`apps/azalee`).**
* **Le glossaire local n'est pas porté.** `app/actions/translate.ts` lit `data/glossary.json` par
  `node:fs`. C'est un fichier du dépôt, pas de l'installation du jeu, et la portée `fs:scope` de
  Tauri ne couvre que `$APPDATA`. Le porter demanderait une commande Rust dédiée.
* **Le filtre « style de jeu » du générateur n'est pas porté, à dessein.** Il lit
  `sheetData.playstyle`, nul sur **6 166 lignes sur 6 166** du miroir : la garde `minCount` du wiki
  l'annule en silence, ce qui donne l'illusion d'un filtre actif. Le style de jeu existe côté jeu
  (`chara_param`, `var[5]`, cf. `nie_data::playstyle`) mais n'est exposé par aucune commande Tauri.
  L'exposer serait la vraie correction — pour les deux surfaces.
* **L'export PNG du terrain n'est pas porté.** Le wiki utilise `html-to-image` (`toPng`), absente
  des dépendances de l'explorateur. Le partage passe par le code d'équipe, qui est de toute façon
  plus fidèle qu'une capture.
* **Le glisser-déposer est natif** (`draggable` + `dragover`/`drop`), pas `@dnd-kit`. Suffisant
  pour des cases ; il n'y a ici ni liste triable ni capteur tactile. `@dnd-kit/core` est au
  catalogue de la racine si on veut la parité de geste.
* **Le retrait côté `apps/azalee` est fait** (2026-09-06) — cf. §4. Les URL retirées sont
  redirigées en 308 vers **deux destinations distinctes**, mesurées et non supposées :
  `/textures`, `/modeles`, `/sons` et `/videos` vers `https://aphrody.com` (que `nie-site` sert
  réellement, avec titre et canonique propres) ; `/gallery` et les cinq outils vers
  **`/tools/niers`**, la page de téléchargement de l'explorateur — `apps/nie-web` ne connaît que
  `medias` et `explorateur`, toute autre route y retombe sur l'accueil, et servir l'accueil
  d'Aphrody sous l'URL d'un outil serait pire qu'un 404.
