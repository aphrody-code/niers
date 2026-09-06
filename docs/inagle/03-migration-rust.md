# `packages/inagle` — migration Rust de la logique de jeu pure

> Suite de [`01-pipeline-entree.md`](01-pipeline-entree.md) et
> [`02-sortie-et-domaines.md`](02-sortie-et-domaines.md). Les deux cartes mesuraient ;
> ce document **exécute** — et s'arrête, en le disant, devant tout ce qui demande un arbitrage.
>
> Travaux du **2026-09-06**, VPS Linux. Aucun fichier TypeScript n'a été modifié : le portage est
> une **addition** en Rust, pas une suppression côté Bun. Ce qui rend le TS retirable est un
> arbitrage — cf. § 6.

## 0. Le résultat en cinq lignes

| | |
|---|---|
| Périmètre visé (cartes 01 et 02) | **1 820 lignes** de TypeScript sur 6 fichiers |
| Déjà porté **avant** cette session | `stat-calculator.ts` (**344 l.**), pour l'essentiel — trouvé, pas réécrit |
| Porté par cette session | **1 341 l.** de TS remplacées (+ 5 items oubliés de `stat-calculator.ts`), en **2 850 lignes** de Rust |
| Laissé de côté, documenté | **~135 l.** (résolution d'API dans `optimizer.ts`) |
| Tests ajoutés | **86**, tous prouvés par falsification ; clippy **0 warning** sur les deux crates |

Commandes de contrôle :

```bash
wc -l packages/inagle/src/{stat-calculator.ts,lib/rarity.ts,characters/comparison-engine.ts,\
analysis/optimizer.ts,zukan/matcher.ts,zukan/audit.ts}     # 1820 total
cargo clippy -p nie-core  --lib --tests                     # 0 warning
cargo clippy -p nie-zukan --lib --tests                     # 0 warning
cargo test   -p nie-core  --lib                             # 297 passed
cargo test   -p nie-zukan --lib                             #  53 passed
```

---

## 1. Ce qui existait DÉJÀ — la moitié du périmètre

**La première mesure a invalidé la moitié de la mission.** `stat-calculator.ts`, présenté comme
« le cœur, 358 l. à porter », était **déjà porté**, et bien — deux fois même :

| Item TS | Rust existant | Vérifié |
|---|---|---|
| `calculateSingleStat` | `nie-core/src/stats.rs:107` `calculate_single_stat` | 3 segments `/29`, `/20`, `/49`, `floor` |
| `calculateStats` | `nie-core/src/growth.rs:293` `calculate_stats` | entrée manquante → bloc à zéro, parité TS |
| `findLv1Entry` / `findLv30Entry` / `findMainEntry` | `growth.rs:152` / `:193` / `:248` | les 4, 5 et 4 niveaux de repli, cités ligne à ligne |
| `rarityToGrowthRank` | `stats.rs:216` | table complète, `20 → 5` compris |
| `calculateTotalPower` | `stats.rs:47` `StatBlock::total` | |
| `GrowthTables` + chargement des vraies tables | `growth.rs:108`, `:349` `load_embedded` | lv1=36, lv30=144, main=48, sub=48 |

`nie-data/src/growth.rs` (424 l.) en porte une **seconde** implémentation, en `i64`, avec le
parseur `parse_growth_tables` depuis le `cfg.bin.json` brut. Les deux coexistent — c'est un
doublon interne au dépôt, signalé au § 6, **pas** traité ici (fusionner deux `StatBlock` de
largeurs différentes touche `match_sim`, `nie-wasm`, `nie-app`, `nie-wiki` et `nie-play`).

**Quatre items de `stat-calculator.ts` manquaient réellement**, et ont été ajoutés :
`rarityCodeToName`, `POSITION_LABELS`, `RANK_LABELS`, `STAT_LABELS`, plus `generateGrowthCurve`.

---

## 2. Ce qui est porté, et où

### 2.1 Choix de destination

| Destination | Pourquoi | Ce qui y entre |
|---|---|---|
| **`crates/engine/nie-core`** | La crate de **logique de jeu pure** : zéro I/O disque (uniquement `include_str!`), `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, et elle porte déjà `StatBlock` et les tables de croissance | stats, comparaison de variantes, builds BASARA, synergie d'équipe |
| **`crates/tools/nie-zukan`** | Elle porte déjà `ZukanChara` et `cross.rs`, qui apparie zukan et le miroir inagle **par égalité exacte** de `game_id`. Le matcher d'inagle est la voie **floue** du même problème, sur les mêmes données | appariement flou + audit |

**Aucune crate nouvelle n'a été créée.** `nie-data` a été écartée : elle est `no_std`, et son rôle
est *modèles + parseurs de `cfg.bin.json`*, pas la logique. `nie-wiki` a été envisagée pour le
zukan (elle modélise déjà les lignes `inagle_characters`) puis écartée au profit de `nie-zukan`,
qui possède **les deux** côtés de la jointure.

**Aucune signature publique existante n'a été modifiée.** Les 5 lots sont des ajouts : nouveaux
modules, ou nouveaux items dans un module existant.

### 2.2 Le détail, lot par lot

| Lot | Source TS | Destination Rust | l. TS | Tests |
|---|---|---|---:|---:|
| 1 | `lib/rarity.ts` (84 l.) + 4 items de `stat-calculator.ts` | `nie-core/src/stats.rs`, `nie-core/src/growth.rs` | 84 | 13 |
| 2 | `characters/comparison-engine.ts` | `nie-core/src/comparaison.rs` (553 l.) | 161 | 14 |
| 3 | `analysis/optimizer.ts`, moitié pure | `nie-core/src/optimisation.rs` (867 l.) | ~360 | 22 |
| 4 | `zukan/matcher.ts` + `zukan/audit.ts` | `nie-zukan/src/appariement.rs` (1 430 l.) | 736 | 37 |
| | | **total** | **~1 341** | **86** |

Le rapport Rust/TS est d'environ **2,2** — l'écart est de la documentation (`tokei` : 2 102 lignes
de code pour 452 de commentaires et 292 blanches sur les 3 fichiers neufs) et des tests, absents
du décompte TS.

### 2.3 La fusion demandée : un seul Spearman

`matcher.ts:169` (`spearmanCorrelation`) et `audit.ts:122` (`auditSpearmanCorrelation`) sont **le
même algorithme, dupliqué** — `audit.ts` le dit lui-même en en-tête : « Doublon assumé du
précédent ». Les deux corps sont identiques instruction par instruction (rangs moyens sur les ex
æquo, `1 - 6Σd²/n(n²-1)`).

**Fusionnés en une seule `correlation_spearman`, sous le nom de l'antériorité** — celui du
matcher, dont l'audit se déclare le doublon. Les tables d'ères (`ERAS` / `AUDIT_ERAS`), elles
aussi rigoureusement identiques, sont fusionnées en `ERES`.

### 2.4 Ce que la traduction a fait apparaître

Cinq comportements du TS ne sont documentés nulle part côté inagle. Ils sont portés **tels
quels** et **figés par un test** : si l'un d'eux se met à rougir, c'est qu'un arbitrage a été pris
en silence.

1. **`POSITION_LABELS` contredit le reverse-engineering.** Le TS dit `2 = DF` et `4 = FW` ; le
   dépôt sait, depuis `refs/iecode-re/cli/include/iecode/gamedata/types.h:28` et
   `.../loader.cpp:178`, que c'est `FW=2` et `DF=4` — et `growth.rs` s'appuie déjà sur cette
   convention-là dans ses goldens. Table portée sous le nom `LIBELLES_POSITION_INAGLE`, avec la
   divergence en doc et en test. **Non corrigée : c'est un arbitrage** (§ 6).
2. **`RANK_LABELS` et `rarityCodeToName` ne décrivent pas la même échelle.** L'une est indexée 1-5
   et anglaise (`R (Rare)`), l'autre 0-20 et française (`Expérimenté`). Portées séparément.
3. **Asymétrie du `Void` dans la synergie d'équipe.** La garde `dominantElement !== "Void"`
   protège la synergie « Cohésion Élémentaire » (`optimizer.ts:341`) mais **pas** les 30 points de
   score (`optimizer.ts:465`, qui ne teste que `elementRatio >= 0.55`). Une équipe intégralement
   `Void` n'affiche donc aucune cohésion et encaisse quand même son bonus. Deux de mes tests
   affirmaient l'inverse : c'est le code qui avait raison.
4. **`Series Evolution` est une classification morte.** `comparison-engine.ts:105` pose
   `seriesChanged = false` en constante et ne s'en sert jamais. La variante est portée pour que le
   type reste complet ; un test prouve qu'aucune combinaison d'entrées ne la produit.
5. **Un `name_en` vide rend l'audit aveugle sur le nom.** `evaluateRow` termine son test par
   `zukanName.includes(dbFirst)` ; si `dbFirst` est la chaîne vide, l'inclusion est toujours vraie
   et la ligne ne peut **pas** être signalée en `NAME_MISMATCH`.

Et quatre divergences que le TS documente, lui, comme volontaires (`audit.ts:14-19`) — donc
**préservées, non fusionnées** : l'audit connaît `Defenseur` sans cédille, le matcher connaît
`Entraîneur`/`Coach`, les caractères japonais et `Aucun`, et seul le matcher traduit `Orion`.

### 2.5 La preuve par falsification

Le portage ne se prouve pas par une suite verte : il se prouve par une suite qui **peut** rougir.
Chaque lot a été falsifié — la valeur gardée cassée volontairement, la suite relancée :

| Lot | Falsification appliquée | Tests devenus rouges |
|---|---|---:|
| 1 | `rarityCodeToName(3)` → `"Normal"` ; total de la courbe `+1` | 3 |
| 2 | signe de l'écart de stat inversé | 5 |
| 3 | multiplicateur du striker, `>` → `>=` sur l'élément dominant, plafond des passifs retiré | 3 |
| 4 | signe de `rho`, table d'éléments, bonus d'ère des Héros, détection MixiMax | 7 |

Chaque falsification a ensuite été annulée et la suite revérifiée verte.

---

## 3. Ce qui N'EST PAS porté, et pourquoi

### 3.1 Hors périmètre par consigne — I/O, réseau, base

| Surface | Volume | Raison |
|---|---:|---|
| 47 importeurs (`cli-push.ts`, `push-categories.ts`, `lua/pusher.ts`) + 18 runners `scripts/push-*.ts` | 3 353 l. | Postgres / PostgREST / JWT. Et la carte 02 y relève **16 importeurs jamais appelés** et **6 tables ciblées qui n'existent pas en base** : câbler ou supprimer est un arbitrage, pas un portage |
| `push-adapter.ts` — signature d'un JWT `service_role` | — | Réseau + secret. La carte 02 signale au passage **une clé service-role en dur dans le code versionné** : à faire tourner, geste irréversible, laissé à l'utilisateur |
| `zukan/order.ts`, `zukan/scraper.ts`, `zukan/scripts/*` | ~2 429 l. | Navigateur headless (`@aphrody-code/bxc`) — le contenu zukan est rendu côté client |
| `utils/romaji.ts` | — | Kuroshiro + Kuromoji. **Aucun équivalent Rust dans le dépôt** ; `lindera`/`vibrato` rendraient des sorties différentes. Les romaji déjà calculés valent mieux qu'une réimplémentation approximative |

### 3.2 Écarté après mesure — c'est de la résolution, pas du calcul

**`calculateTeamSynergy` : la moitié « chargement » (~135 l.).** Le TS y appelle
`createCharactersAPI`, `createBasaraAPI` et `createSkillsAPI`, qui lisent le disque. Seule la
**notation** est portée, sur des joueurs déjà résolus (`JoueurCharge`).

Ce découpage n'invente rien : le TS construit lui-même l'enregistrement intermédiaire
`loadedPlayers` (`optimizer.ts:305-313`) avec exactement ces champs. Les deux prédicats **purs**
de cette boucle — « cette technique est-elle un passif » et « quel nom lui donner » — ont bien été
portés (`est_passif`, `nom_passif`).

**`characters/evolution.ts` (354 l.), `rag/*` (229 l.), `search/fuzzy.ts` (332 l.).** Trois
candidats de la carte 02 écartés, chacun pour une raison mesurée :
- `evolution.ts` est `async` et importe `DATA_ROOT` + `loadAllCharacters` — pas pur ;
- `rag/*` est assis sur le service complet ;
- `search/fuzzy.ts` repose sur `@leeoniya/ufuzzy`, un algorithme propriétaire au paquet. Le
  réécrire ou le remplacer (`nucleo`) **changerait le classement** : ce serait une décision
  produit, pas une traduction.

**Assemblage de l'entité personnage (`entities/character.ts`, 1 334 l.).** Jointure de 13 sources
de disque. C'est le cœur métier, et c'est de l'I/O de bout en bout. Les deux modules portés
déclarent donc leur propre sous-ensemble minimal d'entrée (`VarianteComparable`, `EntreeZukan`,
`LigneInagle`) — la méthode qu'inagle applique déjà lui-même côté zukan avec `ZukanMatchEntry`.

### 3.3 Déjà couvert ailleurs, à ne pas re-porter

`parsers/binary/cfgbin-parser.ts`, `parsers/binary/g4tx-parser.ts`, `core/lua-bytecode.ts`,
`parsers/hash/crc32.ts` : réimplémentations de `nie-formats` et `nie-lua`, **et code mort** (carte
01 § 1.5 : aucun appelant). Rien à porter, tout à supprimer — arbitrage.

---

## 4. Cartographie finale ligne par ligne

| Fichier TS | l. | État | Rust |
|---|---:|---|---|
| `stat-calculator.ts` | 344 | **porté** (avant + cette session) | `nie-core::stats`, `nie-core::growth` |
| `lib/rarity.ts` | 84 | **porté** | `nie-core::stats::{rarity_to_growth_rank, rarity_code_to_name}` |
| `characters/comparison-engine.ts` | 161 | **porté** | `nie-core::comparaison` |
| `analysis/optimizer.ts` | 495 | **porté à ~73 %** | `nie-core::optimisation` — reste la résolution d'API |
| `zukan/matcher.ts` | 434 | **porté** | `nie-zukan::appariement` |
| `zukan/audit.ts` | 302 | **porté, Spearman fusionné** | `nie-zukan::appariement` |
| **total** | **1 820** | **~93 % porté** | |

---

## 5. API Rust produite

```rust
// nie-core::stats  (ajouts)
pub fn rarity_code_to_name(code: u8) -> String;
pub const LIBELLES_POSITION_INAGLE: [(u8, &str); 4];   // contredit le RE — cf. § 2.4
pub const LIBELLES_RANG_INAGLE:     [(u8, &str); 5];
pub const LIBELLES_STATS:           [(&str, &str, &str); 7];

// nie-core::growth  (ajouts)
pub struct PointCroissance { pub niveau: u8, pub stats: StatBlock, pub total: u32 }
pub fn generate_growth_curve(&GrowthTables, &GrowthParams, u8, u8) -> Vec<PointCroissance>;

// nie-core::comparaison
pub fn comparer_variantes(&VarianteComparable, &VarianteComparable,
                          &HashMap<String, String>) -> ResultatComparaison;
pub fn analyser_variantes(&[VarianteComparable],
                          &HashMap<String, String>) -> Vec<ResultatComparaison>;

// nie-core::optimisation
pub const BUILDS_BASARA: [ProjectionBuild; 6];
pub fn projeter_stats_build(StatBlock, u8) -> StatBlock;
pub fn builds_basara_classes(StatBlock) -> Vec<ResultatBuild>;
pub fn est_passif(Option<&str>, Option<&str>, Option<&str>, Option<&str>) -> bool;
pub fn nom_passif(Option<&str>, Option<&str>) -> String;
pub fn calculer_synergie_equipe(&[JoueurCharge], Option<&Entraineur>) -> RapportSynergie;

// nie-zukan::appariement
pub fn correlation_spearman(&[f64], &[f64]) -> f64;      // les DEUX Spearman fusionnés
pub fn similarite_description(&str, &str) -> f64;
pub fn score_appariement(&EntreeZukan, &LigneInagle) -> i32;   // -1 = rejet dur
pub fn apparier_groupes_strict(&mut ContexteAppariement, &[EntreeZukan],
                               &[LigneInagle], &mut HashMap<String, String>);
pub fn assigner_meilleur(&mut ContexteAppariement, &LigneInagle, &[EntreeZukan],
                         &mut HashMap<String, String>, i32);
pub fn evaluer_ligne(&LigneInagle,
                     &HashMap<String, Vec<EntreeZukan>>) -> Option<AnomalieAudit>;
pub fn detecter_hashes_dupliques(&[LigneInagle]) -> HashMap<String, HashSet<String>>;
```

Aucun de ces items n'est encore appelé par une commande `niers` ni par une route `nie-site` :
**le portage est disponible, il n'est pas branché.** Le brancher est le § 6.3.

---

## 6. Les arbitrages qui restent à l'utilisateur

Six décisions ont été **rencontrées** pendant ce portage. Aucune n'a été prise : chacune change un
comportement observable ou touche un fichier hors périmètre.

### 6.1 `POSITION_LABELS` : le TS ou le RE ?

Le TS affiche `2 = DF`, le reverse-engineering du binaire dit `2 = FW`. Les deux sont dans le
dépôt, l'un contredit l'autre, et la table du TS sert à l'affichage sur des pages publiques.
**Trancher, c'est décider ce que voit l'utilisateur final.** Le test
`libelles_position_inagle_contredisent_le_re` fige l'état actuel pour que le changement ne puisse
pas passer inaperçu.

### 6.2 Le doublon `nie-core::growth` / `nie-data::growth`

Deux portages du même `stat-calculator.ts` coexistent : `nie-core` en `u16` avec les tables
embarquées et des goldens, `nie-data` en `i64` avec le parseur `cfg.bin.json`. Chacun a ce que
l'autre n'a pas. Les fusionner suit la doctrine (« garder le meilleur de chacun sous le nom de
l'antériorité ») mais touche `match_sim`, `nie-wasm`, `nie-app`, `nie-wiki`, `nie-play`,
`nie-headless` — **une signature partagée, six appelants, et d'autres sessions au travail**. À
planifier, pas à improviser.

### 6.3 Brancher le Rust, et retirer le TS

Le code porté n'a pas d'appelant. Deux gestes possibles, dans cet ordre :
1. exposer une sous-commande `niers` ou une route `/api/v1/` par module ;
2. **puis seulement** faire déléguer le TS (via `packages/nie` / FFI) ou le supprimer.

Supprimer le TS avant d'avoir un appelant Rust ferait perdre la seule référence de comparaison.

### 6.4 Les 16 importeurs morts et les 6 tables fantômes

Carte 02 § 1.3 et § 1.4 : 16 importeurs sont importés et jamais appelés (14 sans aucun point
d'entrée), et 6 tables ciblées par du code n'existent pas en base — leur migration,
`20260813_inagle_couverture_parseurs.sql`, est introuvable. **Les câbler ou les supprimer** est un
choix produit. Porter 16 fonctions que personne n'appelle serait une perte sèche.

### 6.5 La clé `service_role` en dur dans le code versionné

`packages/inagle/src/cli-push.ts` porte, dans sa branche de repli « local dev », un JWT
`service_role` écrit en clair et versionné. Une clé `service_role` contourne RLS. **Elle est à
faire tourner et à sortir du dépôt** — geste irréversible côté production, donc non fait ici.

### 6.6 La recherche floue et le romaji

`@leeoniya/ufuzzy` et Kuroshiro/Kuromoji n'ont pas d'équivalent Rust dans le dépôt. Les remplacer
(`nucleo`, `lindera`) rendrait des **résultats différents** — un autre classement de recherche,
d'autres romaji. C'est une décision produit, mesurable avant d'être prise : comparer les deux
sorties sur le corpus complet avant de basculer quoi que ce soit.

---

## 7. Ce que je n'ai PAS vérifié

- **Les tests TS n'ont pas été exécutés.** Les cas de `stat-calculator.test.ts` ont été **lus et
  portés** un à un (bornes lv1/30/50/99, courbe 1→99 = 99 points, plage 10→20 = 11 points, total
  70 à lv1, position invalide → zéros) ; leurs valeurs attendues sont reproduites à l'identique
  côté Rust. Mais `bun test` n'a pas tourné : la comparaison est **de code à code**, pas de sortie
  à sortie.
- **Aucune comparaison sur données réelles.** Les modules portés n'ont pas été exécutés sur les
  6 166 lignes d'`inagle_characters` ni sur les 5 640 entrées zukan. Dire « fidèle » ci-dessus veut
  dire « même algorithme, mêmes constantes, mêmes cas de bord testés » — jamais « mêmes sorties sur
  le corpus ». C'est la gate qui manque, et elle demande d'abord un relevé de référence
  (carte 02 § 6, point 1).
- **Les trois autres implémentations du dépôt** (C++ `src/`, C# `csharp/`) n'ont pas été
  inspectées pour ces cinq fonctions. `dotnet` est absent du VPS : un lot C# n'y est que relu.
- **`nie-data::growth` n'a pas été comparé valeur à valeur** avec `nie-core::growth`. Dire
  « doublon » veut dire « même concept, même source TS citée », pas « mêmes octets ».
