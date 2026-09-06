# @rosegriffon/azalee

Bibliothèque Bun du wiki **Azalée** (Inazuma Eleven: Victory Road) : la logique
métier **client-safe** de l'encyclopédie, utilisable **sans aucun framework**.

## Ce package ne lit plus rien de local

C'est le contrat, et il est structurel : `packages/azalee/src` ne contient
**aucun** `bun:sqlite`, `node:fs` ni chemin machine.

```bash
rg -l 'bun:sqlite|node:fs' packages/azalee/src | wc -l   # 0  (mesuré 2026-09-06)
```

Tout ce qui ouvre un fichier — miroir SQLite, index CPK, index de texte de jeu,
API HTTP headless, client distant, CLI `azalee` — vit désormais dans
**`@niers/azalee-tools`** (`packages/azalee-tools`), qui est de l'outillage
**hors ligne** et n'est jamais déployé. Ce README ne le documente pas : voir ce
package.

Correspondance des anciens sous-chemins, retirés du champ `exports` le
2026-09-06 parce qu'ils **pointaient sur des fichiers absents** (`src/config.ts`,
`src/server/`, `src/remote/`, `src/cpk/index.ts`, `src/game-text/index.ts`,
`src/icon-index/`) :

| Ancien | Aujourd'hui |
|--------|-------------|
| `@rosegriffon/azalee/config` | `@niers/azalee-tools/config` |
| `@rosegriffon/azalee/server`, `/server/*` | `@niers/azalee-tools/server/*` |
| `@rosegriffon/azalee/remote`, `/remote/*` | `@niers/azalee-tools/remote` |
| `@rosegriffon/azalee/cpk` (racine) | `@niers/azalee-tools/cpk/*` |
| `@rosegriffon/azalee/game-text` (racine) | `@niers/azalee-tools/game-text` |
| `@rosegriffon/azalee/icon-index` | `@niers/azalee-tools/icon-index` |
| CLI `azalee` | `bun --filter @niers/azalee-tools cli` |

Les sous-chemins **`/cpk/*` et `/game-text/*` restent ici** : ce sont les modules
purs (`cpk/shared`, `cpk/live`, `cpk/audio`, `cpk/models`, `cpk/tree`,
`game-text/format`, `game-text/shared`) — des types et des constructeurs d'URL,
sans accès disque.

## Installation

Dans le monorepo, c'est une dépendance workspace :

```jsonc
// apps/<app>/package.json
{
  "dependencies": {
    "@rosegriffon/azalee": "workspace:*"
  }
}
```

Hors monorepo (registre GitHub Packages, cf. `publishConfig`) :

```bash
bun add @rosegriffon/azalee
```

Runtime requis : **Bun ≥ 1.3.14** (`engines`). La racine étant pure, elle se
bundle aussi pour le navigateur.

## Racine — client-safe

```ts
import {
  FORMATIONS,
  translateEffect,
  getCharacterFaceUrl,
  getCharacterModelFullGlbUrl,
} from "@rosegriffon/azalee";

FORMATIONS.length;                            // 91 (83 formations du jeu + 8 legacy)
translateEffect("Shot AT +10%");              // "Tir ATT +10%"
getCharacterFaceUrl("c01000010");
// https://cdn.rosegriffon.fr/dx11/menu/icon_chr/face/c01000010_l_c01000010_1_l00.png
getCharacterModelFullGlbUrl("c01000010");
// https://cdn.rosegriffon.fr/model-full/c01000010.glb?v=5
```

La racine ré-exporte aussi les modules `*-shared` (types + helpers purs) :
`cpk/shared`, `cross/assets-shared`, `cross/data-shared`, `game-text/format`,
`game-text/shared`, et
`wiki/{chara-stats,drops,gacha,game-text,invocation,shops,teams,trophies}-shared`.
Un composant client peut ainsi typer et afficher une donnée sans jamais importer
le module qui la lit.

## Sous-chemins d'export

Table exacte du champ `exports` de `package.json`.

| Sous-chemin | Cible | Nature |
|-------------|-------|--------|
| `@rosegriffon/azalee` | `src/index.ts` | **client-safe** — jeu, images, recherche, texte, types `*-shared` |
| `@rosegriffon/azalee/images` | `src/images/index.ts` | client-safe — résolution d'URLs CDN |
| `@rosegriffon/azalee/search` | `src/search/index.ts` | client-safe — fuzzy match + recherche intelligente |
| `@rosegriffon/azalee/search/*` | `src/search/*.ts` | client-safe — `utils`, `fuzzy-match`, `smart-search`, `search-ui-config` |
| `@rosegriffon/azalee/text` | `src/text/index.ts` | client-safe — glossaire FR, descriptions, romaji, gaiji |
| `@rosegriffon/azalee/text/*` | `src/text/*.ts` | client-safe |
| `@rosegriffon/azalee/game` | `src/game/index.ts` | client-safe — formations, genre, règles d'équipe, cut-ins, emblèmes |
| `@rosegriffon/azalee/game/*` | `src/game/*.ts` | client-safe |
| `@rosegriffon/azalee/data/*` | `src/data/*` | client-safe — JSON figés (manifestes) |
| `@rosegriffon/azalee/cpk/*` | `src/cpk/*.ts` | client-safe — `shared`, `live`, `audio`, `models`, `tree` |
| `@rosegriffon/azalee/game-text/*` | `src/game-text/*.ts` | client-safe — `format`, `shared` |
| `@rosegriffon/azalee/cross` | `src/cross/index.ts` | Inazuma Eleven Cross (Unity/Addressables) |
| `@rosegriffon/azalee/cross/*` | `src/cross/*.ts` | dont `data-shared`, `assets-shared` |
| `@rosegriffon/azalee/db` | `src/db/index.ts` | injection de fournisseur de données (aucun pilote embarqué) |
| `@rosegriffon/azalee/db/*` | `src/db/*.ts` | `provider` |
| `@rosegriffon/azalee/wiki` | `src/wiki/index.ts` | sections du wiki, lues via le fournisseur injecté |
| `@rosegriffon/azalee/wiki/*` | `src/wiki/*.ts` | `service`, `teams`, `shops`, `quests`, `coaches`, `stadiums`, `drops`, `gacha`, `trophies`, `invocation`, `chara-stats`, `game-text` (+ leurs `*-shared`) |
| `@rosegriffon/azalee/rag` | `src/rag.ts` | recherche sémantique (embeddings + Redis) |
| `@rosegriffon/azalee/package.json` | `package.json` | métadonnées |

`src/net.ts` (récupération HTTP avec cache TTL de processus) est interne : il
n'est pas exporté, seulement consommé par `wiki/chara-stats` et `wiki/invocation`.

## Injection du client de données

Les modules `wiki/*` n'ouvrent **aucune** source : ils lisent celle que l'hôte
injecte. C'est ce qui a permis de sortir le miroir SQLite du chemin d'une page.

```ts
import { setDatabaseProvider, hasDatabaseProvider } from "@rosegriffon/azalee/db";
import { createClient } from "@/lib/supabase/server";

setDatabaseProvider(createClient); // fabrique sync ou async
hasDatabaseProvider();             // true
setDatabaseProvider(null);         // plus aucune source
```

Ils n'utilisent que la surface `.from(table).select(...)`, commune au client
Supabase et au client miroir de `@niers/azalee-tools` — c'est ce qui rend
l'injection transparente.

> **Le piège du 2026-09-05, à ne pas rejouer.** Le wiki web enveloppait ce
> fournisseur dans un `Proxy` qui détournait `from("inagle_*")` vers un cache
> SQLite local, avec repli. Deux sources de vérité pour les mêmes tables :
> `/chara` répondait **200 en 136 921 octets avec 0 lien**. Une seule source, et
> on compte le contenu, jamais le statut.

## Données embarquées et régénération

`src/data/` (~6,7 Mo) suit le package. Chaque fichier a une provenance connue ;
les scripts de régénération vivent dans `@niers/azalee-tools` (`scripts/`,
`scripts-app/`) et écrivent ici.

| Fichier | Régénération |
|---------|--------------|
| `character-face-manifest.json` | `bun packages/azalee-tools/scripts/build-character-face-manifest.ts` |
| `character-model-manifest.json` | `…/build-character-model-manifest.ts` |
| `chr-model-manifest.json` | `…/build-chr-model-manifest.ts` |
| `item-image-manifest.json` | `…/build-item-image-manifest.ts` |
| `keshin-model-manifest.json` | `…/build-keshin-model-manifest.ts` |
| `menu-gallery-manifest.json` | `…/build-menu-gallery-manifest.ts` |
| `miximax-icon-manifest.json` | `…/build-miximax-icon-manifest.ts` |
| `skills-cutin-served.json` | `…/build-skills-cutin-served.ts` |
| `change-aura-skills.json` | `…/sync-inagle-entries.ts` (copie depuis `packages/inagle/src/entries`) |
| `passives-full.json`, `formations-full.json`, `skills-cutin.json`, `item-enrichment.json`, `emblem-crc-map.json`, `chr-model-names.json`, `cross/*.json` | produits hors package (exports niers / inagle) — pas de générateur embarqué |

Les manifestes sont des **gates anti-404** : un code absent du manifeste fait
renvoyer `null` ou un placeholder au lieu d'une URL CDN morte.

Ces JSON sont **gitignorés** (`.gitignore:26`, contenu de jeu © LEVEL-5) : sur un
clone frais il faut les régénérer avant `bun run typecheck`. Ce n'est pas un
défaut de code.

## Tests, build, publication

```bash
bun test                    # depuis packages/azalee
bun run typecheck           # tsc --noEmit (vrais types workspace)
bun run lint                # oxlint
bun run build               # tsc -p tsconfig.build.json + copie de src/data → dist/data
```

Le build de publication substitue les types de `@rosegriffon/db` par les shims de
`types/workspace-shims.d.ts` : ce package workspace exporte ses **sources TS** (pas
de `dist`), et `tsc` refuse alors d'émettre des déclarations pour des fichiers hors
`rootDir` (TS6059). Conséquence assumée : dans les `.d.ts` publiés, les types venant
de `@rosegriffon/db` sont dégradés ; le comportement runtime est identique.

Publication : registre `npm.pkg.github.com`, `prepublishOnly` déclenche le build.

Rappel de discipline, valable pour toute contribution : **la racine du package
doit rester client-safe**, et depuis 2026-09-06 c'est le package **entier** qui
l'est. Tout ce qui ouvre un fichier va dans `@niers/azalee-tools`, sans quoi le
bundle navigateur casse sur `Can't resolve 'fs'`/`'tls'`. La vérification tient
en une commande :

```bash
bun build --target=browser src/index.ts --outdir /tmp/azalee-browser
```
