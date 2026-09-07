# Fusion — tout ce qui touche Inazuma Eleven vit dans ce dépôt

Le travail était réparti sur quatre dépôts et une douzaine de services. Un même personnage
existait quatre fois : une fiche dans la base du wiki, des fichiers dans le VFS, des chaînes dans
le binaire reversé, un épisode dans le catalogue de la série. Rien ne reliait ces quatre
existences, et chaque outil réimplémentait sa moitié du chemin.

Ce document dit ce qui a été rapatrié, ce qui reste dehors et pourquoi, et où en est la bascule
des services.

## Ce qui est entré

| Origine | Devenu | Contenu |
|---|---|---|
| `rg/packages/inagle` | `packages/inagle` | le pipeline de données `inagle_*` — parsers, entités, push, 38 Mo d'entrées JSON |
| `rg/packages/inagle-cross` | `packages/inagle-cross` | les recoupements entre entités |
| `rg/packages/cron` | `packages/cron` | le démon de tâches, **dont `src/tasks/ie-crawl/`** (43 modules : X, RAG, zukan, Level-5, news, Reddit) |
| `rg/packages/db` `types` `auth` `config` | idem | ce dont `cron` et `inagle` dépendent |
| `rg/apps/azalee/scripts/ops/backup-supabase-to-sqlite.ts` | `scripts/donnees/dump-inagle-sqlite.ts` | le dump Postgres → SQLite |
| `rg/apps/azalee/scripts/ops/mirror-sync.sh` | `scripts/donnees/miroir-inagle.sh` | la republication du miroir, **vers `var/` d'ici** |
| `bxc/packages/{ietv,ietv-client,wonderbot,zukan}` | `packages/*` | le catalogue d'épisodes et son bot Discord |
| `bxc/` (app) | `apps/bxc` | l'automatisation de navigateur dont dépend le crawler |
| `~/.cache/ietv/episodes.db` | `data/anime/episodes.db` | 355 épisodes, 10 saisons, 3 chaînes |
| `rg/apps/azalee` | `apps/azalee` | le site du wiki (Next.js 16.3.0-canary.37) — sans `.next` ni `data/` |
| `rg/packages/azalee` | `packages/azalee` | sa bibliothèque — sans `bin/azalee`, 79 Mo de binaire recompilable |
| `rg/packages/{ui,assets,mcp}` | `packages/*` | le socle d'interface, les images, le serveur MCP |
| *(généré depuis la base)* | `supabase/migrations/` | le schéma des 66 tables `inagle_*`, qui n'existait nulle part |
| *(nouveau)* | `packages/nie-catalog` | **la façade** — voir plus bas |
| `rg/apps/cdn` | `apps/cdn` | le serveur d'images de `cdn.rosegriffon.fr` |
| `rg/apps/realtime` | `apps/realtime` | la diffusion SSE des changements Postgres |
| `rg/apps/storage` | `apps/storage` | l'API Storage compatible Supabase |
| `rg/scripts/ops/deploy.ts` | `scripts/ops/deploy.ts` | le déploiement bleu/vert — **une racine par app** (voir plus bas) |
| `rg/scripts/next-build.sh` | `scripts/next-build.sh` | le build Next standalone sous Node, dont dépend `apps/azalee` |
| `rg/happydom.ts` | `packages/nie-plugin/src/happydom.ts` | les globals DOM des tests, préchargés par `bunfig.toml` |

Le catalogue de versions de `rg` (183 entrées) a été fusionné dans celui d'ici. Deux conflits,
tranchés en faveur de niers pour ne pas faire cohabiter deux TypeScript :

* `typescript` : **5.9.3** (rg voulait `^6.0.3`) ;
* `@types/bun` : **1.3.14**.

`@aphrody-code/x` est mappé sur `npm:@aphrody/x` par les `overrides`, comme `bxc` et `zukan`
l'étaient déjà : le registre GitHub Packages exige un jeton que ce dépôt n'a pas à porter.

## La façade — `@niers/catalog`

C'est la pièce qui manquait, et la seule qui soit neuve. Elle résout les quatre gisements à
l'exécution, les interroge en lecture seule, et surtout **les joint** :

```bash
bun --bun packages/nie-catalog/src/cli.ts etat
```

```
Gisements Inazuma Eleven
  ✓ jeu      https://cdn.rosegriffon.fr
  ✓ extrait  66 tables, 165 244 lignes    var/miroir/inagle-2026-09-02T05-54-13.sqlite
  ✓ re       108 650 fonctions, 13 653 nommées    var/niers.sqlite
  ✓ anime    355 épisodes, 10 saisons, 3 chaînes  data/anime/episodes.db
```

Chaque jointure porte sa `confiance` : `cle` quand les deux gisements partagent un identifiant,
`prefixe` quand un chemin commence par un code, `texte` quand seul le nom relie — le cas du jeu et
de la série, qui n'ont aucune clé commune. Détail dans `packages/nie-catalog/README.md`.

## Le miroir a déménagé

Il vivait sous `rg/apps/azalee/data/backups/mirror.sqlite`. Tout ce qui n'était pas le site web
devait aller le chercher là-bas par un chemin absolu vers un autre dépôt.

Il est maintenant republié dans `var/miroir/` d'ici, avec `var/mirror.sqlite` comme lien daté
basculé atomiquement — `scripts/donnees/miroir-inagle.sh`, planifié par
`deploy/systemd/nie-miroir.{service,timer}` à 04:10 UTC, dix minutes après le créneau de
`azalee-mirror-sync`. Ces unités sont **installées et armées** depuis le 2026-09-02. Le script
refuse de basculer sur un dump invalide : un dump vide laisse l'ancien miroir en place, au lieu de
faire répondre « aucun résultat » à tout le site.

`@niers/catalog` résout `var/mirror.sqlite` **en premier**, et retombe sur celui de `rg` s'il
n'existe pas encore : les deux dépôts peuvent coexister pendant la bascule.

## Le schéma SQL, qui n'existait nulle part

Les 66 tables `inagle_*` avaient été créées par le pipeline de push, au fil des familles portées :
une base neuve n'était pas reconstructible, et rien ne disait quel schéma le code attend.
`supabase/migrations/` le pose — **généré depuis la base réelle**, pas écrit de mémoire.

Trois propriétés, mesurées :

* **rejouables à froid** — les quatre fichiers passent sur une base vide, dans l'ordre ;
* **idempotentes** — ils repassent sur la base qu'ils viennent de créer. Les séquences et les vues
  manquaient aux deux premiers essais ; c'est le rejeu qui l'a dit, pas la relecture ;
* **fidèles** — le schéma reconstruit porte **les 811 colonnes de la production, sans exception**
  (comparaison de `information_schema.columns`).

Les politiques RLS sont à part : elles interrogent `public.profiles` et `auth.uid()`, donc le
socle Supabase. Une base qui ne porte que les tables du jeu se construit sans lui — la migration
le détecte et passe son tour en le disant. Détail dans `supabase/README.md`.

## La bibliothèque du wiki lit maintenant le miroir du dépôt

`resolveMirrorPath()` ne cherchait que sous `apps/azalee/data/backups`. Elle remonte désormais
jusqu'au dossier qui porte `Cargo.toml` **et** `crates/` — la même signature que côté Rust, pour
qu'un `var/` homonyme rencontré en chemin ne soit jamais pris pour la racine — et y lit
`var/mirror.sqlite`. Vérifié : 6 166 personnages, 1 002 techniques, lus depuis le miroir d'ici.

## Ce qui reste dehors, et pourquoi

* **`rg/apps/website`** (le site vitrine Rose Griffon), **`rg/apps/bot`** (le bot Discord de la
  communauté) et **`rg/packages/patreon-bun`** ne portent pas sur Inazuma Eleven. Ce sont les
  **seules** surfaces encore servies depuis `rg`, et deux tâches du cron les visent — d'où
  `depotRoseGriffon()` (`packages/cron/src/lib/racine.ts`) et le `repoRoot` par application de
  `scripts/ops/deploy.ts`.
* **`/home/ubuntu/rg-releases`** garde son nom : c'est l'arborescence des **versions publiées**,
  pas un dépôt, et le site vitrine y publie les siennes. La renommer aurait cassé `website-web`
  pour un gain cosmétique.
* **`aphrody/`** — bibliothèques Material Design 3, sans rapport avec Inazuma Eleven.
* **Les autres services `bxc`** (`bxc.service`, les deux crawlers, `bxc-x-*`) rendent des
  services au-delà d'Inazuma Eleven : ils restent où ils sont.

## Bascule des services

| Service | État | Ce qui a changé |
|---|---|---|
| `bxc-wonderbot.service` | **désarmé** | `niers-wonderbot.service`, **actif** — même guilde, même jeton |
| `nie-miroir.timer` | **armé** | republie `var/mirror.sqlite` à 04:10 UTC ; `azalee-mirror-sync` a été **retiré de la machine** |
| `rg-cron.service` | **désarmé** | `nie-cron.service`, **actif** depuis `packages/cron` |
| `rg-storage.service` | **basculée** | `WorkingDirectory=/home/ubuntu/niers`, sert `apps/storage` |
| `rg-realtime.service` | **basculée** | idem, sert `apps/realtime` |
| `rg-cdn.service` | **basculée** | `apps/cdn`, avec son `.env` rapatrié |
| `rg-mcp.service` | **basculée** | `packages/mcp` ; le jeton d'administration est désormais **déclaré** (voir plus bas) |
| `azalee-web` | **construit et servi depuis ici** | bascule bleu/vert sans coupure ; la version publiée porte le commit de **niers** |
| `azalee-api`, `cdn-variants`, `nie-model-serve` | déjà ici | basculés lors de la première vague |

La preuve que le wiki vient bien d'ici tient dans un champ : `slot-a/RELEASE.json` porte
`commit: ab20824`, le `HEAD` de **niers** — et non `e2c271b`, celui de `rg`. C'est exactement ce
que le `repoRoot` par application rend possible : avant lui, `deploy.ts` lançait son `git
rev-parse` depuis une racine unique, et la version publiée aurait été étiquetée avec la révision
d'un dépôt qu'elle ne contient pas.

Deux bascules ont eu lieu, toutes deux vérifiées.

`bxc-wonderbot` est désarmé, `niers-wonderbot` tourne depuis ce dépôt, connecté à la même guilde
avec le même jeton (les secrets vivent dans `~/.config/niers/wonderbot.env`, en 600, hors du
dépôt). **Un seul bot par jeton Discord** : deux instances sur le même jeton se battent et
répondent en double — c'est la raison pour laquelle on désarme l'ancienne avant d'armer la
nouvelle, jamais l'inverse.

`nie-miroir` a été lancé à la main avant tout armement, et son résultat contrôlé par la façade :
`var/mirror.sqlite` pointe sur un instantané frais (165 244 lignes, `quick_check` à `ok`) et les
quatre gisements répondent. Son `DATABASE_URL` vient de `~/.config/niers/donnees.env`, en 600,
hors du dépôt.

`azalee-mirror-sync` a depuis été **retiré de la machine** : les quatre services qui l'épinglaient
lisent maintenant `niers/var/mirror.sqlite`, republié par `nie-miroir.timer`.

Les autres ont suivi, **une unité à la fois, chacune vérifiée avant la suivante** : on installe la
nouvelle définition, on redémarre, on attend que le service réponde sur `/health`, et on contrôle
qu'aucun de ses chemins ne vise plus `/home/ubuntu/rg`. Pour `nie-cron`, l'ordre s'inverse — le
démon est un **singleton** (ports 3005/3006/4001 et socket `/var/lib/rg/cron.sock`) : on désarme
`rg-cron` d'abord, jamais les deux ensemble, avec retour arrière automatique si le nouveau ne tient
pas.

> **Une vérification de bascule se fait sur le PID du service, pas sur un port.** La première
> tentative a désarmé `nie-cron` alors qu'il démarrait normalement : le contrôle cherchait
> `ss -lnt | grep :4001`, or `10.8.0.1:4001` et `10.8.0.1:3005` (interface VPN) écoutent en
> permanence, sans processus associé. Le motif matchait donc **cron arrêté**, la boucle d'attente
> sortait au premier tour et le retour arrière tuait un service sain après 488 ms. Compter les
> ports ouverts **par le `MainPID` de l'unité**, sur `127.0.0.1`.

**Deux secrets ont dû être déclarés**, parce qu'ils étaient chargés *implicitement* par le
répertoire de travail :

* `RG_MCP_ADMIN_TOKEN` était lu dans `/home/ubuntu/rg/.env`, que Bun charge tout seul depuis le
  cwd. En déplaçant le service, il aurait disparu **en silence** — le serveur MCP aurait démarré,
  répondu, et refusé toute écriture sans dire pourquoi. Il vit maintenant dans
  `~/.config/niers/mcp.env` (600), **déclaré** par `EnvironmentFile=`.
* Le build du wiki lit `../../.env.local` : `/home/ubuntu/niers/.env.local` a été posé en 600,
  hors de git, à côté de `apps/azalee/.env` qui avait suivi la fusion.

**L'état réel de la machine, unité par unité, avec ce qui reste dehors et pourquoi, vit dans
`docs/EXPLOITATION.md`.**

## `@rosegriffon/inagle` : Bun lit les sources, Next lit `dist/`

Le paquet exposait ses **sources** (`main: ./src/index.ts`) — nécessaire, parce que Bun lit le
TypeScript directement et qu'aucun build ne tourne avant `bun run`. Mais le build du wiki, lui,
passe par **Turbopack sous Node**, et il a échoué sur les **21 ré-exports** du barrel
`src/index.ts` : ils portent le suffixe `.js` (`export … from "./adapters/hono.js"`), que
TypeScript remappe vers le `.ts` et que Turbopack, non — `Module not found`. Le défaut ne se
voyait pas dans `rg` : là-bas le paquet est **construit**, `@rosegriffon/inagle` résout vers
`dist/index.js`, du JavaScript où ces `.js` existent pour de bon.

Les `exports` portent donc désormais une **condition par runtime** :

```json
".": { "bun": "./src/index.ts", "types": "./dist/index.d.ts", "default": "./dist/index.js" }
```

* Bun (CLI, tests, `packages/mcp`, `nie-cron`) prend `bun:` → **les sources**, sans build ;
* Node et Next prennent `default:` → **`dist/`**, comme dans `rg`.

Vérifié : `Bun.resolveSync` rend `src/index.ts`, `require.resolve` de Node rend `dist/index.js`.

`dist/` est ignoré par git : il doit donc être **reconstruit avant chaque build du wiki**. C'est
le rôle du `prebuild` d'`apps/azalee` (`bun run --filter @rosegriffon/inagle build`), que Bun
exécute avant `build`. Sans lui, un `dist/` absent ou périmé se déploierait en silence.

## Ce que la migration a mis au jour

Trois défauts préexistants, invisibles tant que le wiki vivait dans `rg` :

* **La vérification de types ne couvrait que 12 paquets sur 32.** `bun run typecheck` fait
  `--filter '*' typecheck`, et les 9 paquets du wiki déclarent `type-check` (avec tiret) : ils
  n'étaient donc jamais vérifiés. C'est ce qui a laissé passer un `ignoreDeprecations: "6.0"`
  (valeur de TypeScript 6) sous le TypeScript **5.9.3** du catalogue, qui la refuse — 8 tsconfig
  étaient concernés. Les paquets migrés déclarent maintenant les **deux** noms (`type-check` est
  gardé : `scripts/ops/deploy.ts` l'appelle comme gate), et la couverture est passée à 24.
* **`packages/db` résolvait le store RAG vers un chemin mort.** Les candidats étaient retenus sur
  l'existence de leur *dossier*, et `data/` existe toujours ici — c'est le VFS du jeu. `var/rag`
  passe désormais en premier.
* **Douze faiblesses de typage dans `packages/inagle`**, masquées par les `.d.ts` construits :
  `Object.entries()` sur un `Record<number, T>` rend une clé `number` et une valeur `never`, ce
  qui cassait six recherches inversées « libellé → identifiant » (`entreesDe`, dans
  `src/utils/tables.ts`), et `kuroshiro` n'a aucune déclaration amont (`types/kuroshiro-shim.d.ts`).

## Vérifier

```bash
bun install                                   # depuis la racine, jamais dans un sous-paquet
bun test packages/nie-catalog                 # 13 cas, dont les jointures réelles
bun --bun packages/nie-catalog/src/cli.ts etat
bun --bun packages/nie-catalog/src/cli.ts personnage mark-evans-0x06E25622
```

Les tests qui exigent un gisement peuplé **s'annoncent quand ils se sautent** : un test muet qui
ne s'exécute pas est un faux vert.

## Synchronisation et ressources

La synchronisation inter-dépôts est centralisée dans
`/home/ubuntu/rg/scripts/ops/repo-sync.ts` et son registre
`rg/docs/REPO-SYNC.md`. Niers ne maintient pas une seconde implémentation : le
script traite ce dépôt comme source des formats, manifests et outils, refuse
les données/artefacts avant `git add`, et ne lance aucun déploiement. Le chemin
`apps/azalee/data` reste une compatibilité consommée par les services tant que
la migration de ces références n'est pas prouvée.

Le cache GLB (`var/model-cache`) et `target/debug` sont régénérables mais ne
sont pas supprimés par Git. Leur purge bornée passe par
`rg/scripts/ops/prune-runtime.ts`, après arrêt contrôlé de
`nie-model-serve.service`. Les unités systemd de Niers restent la source de
vérité installable ; toute modification doit être copiée dans `/etc/systemd`
et suivie de `daemon-reload` et d'une sonde `/health`.
