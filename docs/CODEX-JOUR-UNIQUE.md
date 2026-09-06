# Codex — tout le plan en un jour

> **Consigne de l'utilisateur, 2026-09-06.** Elle **remplace** la frontière du 2026-09-05
> (« Codex dans `rg`, Claude dans `niers` ») : Codex prend en charge **tout le plan**
> ([`/PLAN.md`](../PLAN.md), J1 → J7) et l'exécute **en une seule journée**, en raisonnement
> maximal et en exécution proactive.

Ce fichier est le contexte complet de cette mission. Il ne remplace pas les règles du dépôt —
[`CLAUDE.md`](../CLAUDE.md) (qui vaut pour tous les agents) et [`AGENTS.md`](../AGENTS.md) —
il dit **ce qu'il y a à faire, dans quel ordre, et à quoi on reconnaît que c'est fait**.

---

## 1. Le mode d'exécution

| Réglage | Valeur |
|---|---|
| Raisonnement | **maximum**. Chaque lot est pensé avant d'être écrit ; un lot qui échoue se diagnostique, il ne se retente pas à l'identique. |
| Initiative | **proactive**. Choisir la prochaine cible seul, l'annoncer (`claim:`), l'exécuter, la mesurer, rendre (`done:`), enchaîner. Aucune question de direction. |
| Questions | **aucune**, sauf les six gestes du § 5 — et pour ceux-là on prépare, on ne demande pas. |
| Langue | français, partout : code, commentaires, commits, ticks. |
| Boucle | analyser → mesurer l'existant → implémenter → vérifier en comptant → committer avec le compte → cible suivante. |

**La règle qui prime sur toutes les autres :** ne jamais annoncer vert ce qui n'a pas été
lancé. Ce dépôt a une histoire de faux verts — un bundle jamais chargé sous 42 tests verts,
une suite qui affichait « 0 passed », un `$?` avalé par un pipe, une garde de test qui se
skippait en silence. Un compte, une commande, un hôte, une date : sinon ce n'est pas fait.

## 2. L'état réel au 2026-09-06, mesuré et non recopié

Ce que le plan disait « fait » l'est. Ce qui suit est ce qui **reste**, vérifié sur cette
machine aujourd'hui — à re-mesurer avant d'y toucher, parce que trois sessions écrivent en
parallèle.

| Constat | Mesure | Source |
|---|---|---|
| Le workspace Rust compile | `cargo check --workspace --tests` = **0 erreur**, 26,5 s, 34 crates | joué le 2026-09-06 |
| Le typecheck Bun **échoue sur deux paquets** | `@rosegriffon/mcp` : 5 erreurs `TS2307` (`@rosegriffon/azalee/server` introuvable) · `@rosegriffon/cron` : 3 erreurs `TS2305` (`@aphrody/bxc` sans `detectPii`/`redactPii`/`redactObject`) | `bun run typecheck` |
| Le reste du typecheck est vert | `inacord`, `nie-web`, `@niers/inacord-ui`, `azalee-tools`, `azalee-web` = 0 | idem |
| `nie-site` en production sert un binaire **périmé** | `/api/v1/episodes` rend **500** en ligne ; la cause (WAL + `ProtectSystem=strict`) est corrigée dans les sources, le binaire n'a pas été rebâti | session parallèle, 2026-09-06 |
| Aphrody rend le menu principal | 14 blocs mesurés contre la capture du jeu ; écart ≤ 10 px sur 6 d'entre eux, 392 px sur la rangée (assumé : 5 entrées réelles contre 8 tuiles) | `scripts/validation/mesurer-mainmenu.py` |
| Le dépôt est poussé | `a25ea27..f683f36 main` — 532 fichiers, `cargo check` vert avant push | 2026-09-06 |

## 3. Ce qui reste à faire, par ordre de dépendance

L'ordre compte : chaque bloc débloque le suivant. Ne pas commencer le 3 avant que le 1 ne soit
mesuré vert.

### Bloc 1 — rendre le portail utilisable (bloquant pour tout le reste)

Tant que `bun run typecheck` est rouge, aucune gate TS ne veut rien dire : on ne distingue
plus une régression nouvelle d'un rouge de fond.

1. `@rosegriffon/mcp` — `@rosegriffon/azalee/server` n'est plus résolu. Chercher si l'export
   a bougé dans `packages/azalee/package.json` (`exports`), ou si le paquet doit pointer sur
   `./src/index.ts` plutôt que sur `./dist/*` (piège documenté dans `CLAUDE.md`).
2. `@rosegriffon/cron` — `@aphrody/bxc` n'exporte plus `detectPii`, `redactPii`,
   `redactObject`. Le paquet vient du dépôt `aphrody` : soit l'API a changé, soit la version
   résolue est la mauvaise. **Ne pas réimplémenter la fonction** : mesurer d'abord ce que le
   paquet exporte réellement (`bun --bun -e 'console.log(Object.keys(await import("@aphrody/bxc")))'`).

**Gate :** `bun run typecheck` = 0 sur les 5 workspaces, sans exception commentée.

### Bloc 2 — J2 du plan : le wiki ne lit plus rien de local

C'est le plus gros morceau restant, et la bascule de J6 en dépend entièrement.

- Sortir d'`apps/azalee` tout ce qui lit un fichier : `/cpk`, `/textures`, `/modeles`,
  `/mode`, `/sons`, `/videos`, `/avatar`, `/demo`, `/save`, `/vroid`, `api/cpk`,
  `api/mode-tex`, `lib/cpk/index.ts`, le wasm → `apps/nie-web` (le sas `src/legacy/` existe
  pour ça, et c'est un **sas**, pas une bibliothèque).
- Retirer le Proxy SQLite du chemin métier. **Une seule** URL Supabase — plus de `pickUrl()`
  en cascade : c'est elle qui a produit le faux vert du 2026-09-05 (`/chara` 200, 136 921 o,
  **0 lien**, parce que `SUPABASE_INTERNAL_URL` gagnait).
- Les 19 consommateurs de `supabase-compat.inc` → `*.supabase.co`.

**Gate, à compter, pas à lire :**

```bash
# Le motif ne retient QUE les marqueurs de lecture LOCALE, et ne lit que du code : une regle
# ecrite pour interdire un motif contient forcement ce motif.
rg -l 'bun:sqlite|node:fs|/home/ubuntu|SQLITE_DB_PATH|SUPABASE_INTERNAL_URL' \
  apps/azalee packages/azalee -g '!*.md' | wc -l   # attendu : 0  (depart mesure : 76 -> 5)
```

**Mesuré le 2026-09-06 : la gate est TENUE — 0 lecture locale.** Sous sa forme d'origine elle
rendait 4, et les quatre étaient des faux positifs. Deux corrections de **l'instrument**, pas
du code :

1. **`DATABASE_URL` sort du motif.** C'est une chaîne de connexion Postgres **distante** — le
   contraire exact d'une lecture locale. Elle vit dans `apps/azalee/lib/auth.ts:25` et
   `lib/db/pg.ts:13`, où Better Auth exige un `Pool` direct (PostgREST ne sert pas ses tables de
   session) et où 17 routes passent. La retirer casse l'authentification. Un motif qui condamne
   le chemin qui a tenu pendant l'outage `exceed_storage_size_quota` d'août mesure autre chose
   que ce qu'il annonce.
2. **`-g '!*.md'`.** Les deux autres matches étaient `apps/azalee/CLAUDE.md` et
   `packages/azalee/README.md` : de la documentation qui **nomme les motifs pour les
   interdire**. Un `rg` sur le contenu ne distingue pas un code qui lit un fichier d'une règle
   qui le proscrit — troisième fois dans ce plan qu'une gate compte ses propres preuves
   (cf. Bloc 4).

Mesure de fond qui tranche, elle : `rg -l 'bun:sqlite|node:fs' packages/azalee/src` → **0**. Le
package est déjà entièrement client-safe ; seule sa documentation prétendait le contraire, et
elle a été réécrite.

Le piège `pickUrl()` est **mort, vérifié** : `rg -n 'pickUrl|SUPABASE_INTERNAL_URL'` hors
`docs/` rend **0 occurrence de code**. `lib/supabase/server.ts` résout par une source unique
(`origineSupabase()` / `cleAnonSupabase()`), et le `Proxy` SQLite a quitté le chemin métier.

**Reste ouvert, en lot dédié :** le déplacement de `/cpk`, `/textures`, `/modeles`, `/sons`,
`/videos`, `/save`… vers `apps/nie-web`. Attention, `apps/nie-web/src/legacy/` **n'existe
plus** — le sas a été vidé : la cible du déplacement est donc à re-choisir, ce n'est plus lui.

Puis la Gate 1 contre une preview : `/chara` ≥ 50 liens, fiche perso 200. **Compter les
liens**, jamais se contenter du code HTTP — c'est exactement le piège qui a coûté une journée.

### Bloc 3 — J3 : poids et ISR

- `/chara` : pagination (620 → 60 liens), `srcset` via `cdn-variants ?w=&format=webp` sur les
  404 vignettes sans `srcset`, markup aplati.
- ISR `revalidate = 3600` + `dynamicParams` sur les 6 fiches, `POST /api/ops/revalidate/wiki`.
- Lot 2 de `docs/MIGRATION-EXPLORATEUR.md` §4 : pages `/tools/*` mortes et leurs 7 références
  entrantes — **sauf `app/tools/niers/latest.json/route.ts`**, qui est l'updater des Inacord
  déjà installés. Le casser fige silencieusement toutes les 0.5.x.

**Gate :** `/chara` < 250 Ko en `br`, `<img>` sans `srcset` = 0.

### Bloc 4 — finir J4 : plus aucune marque Rose Griffon côté `aphrody-dev`

Décision gelée : seule Azalée est un produit Rose Griffon ; Aphrody, Inacord et nie sont
`aphrody-dev`.

```bash
# Les deux exceptions sont exclues PAR LA COMMANDE, pas de tête : une gate qui se lit
# « 0, sauf deux cas qu'on connaît » n'est pas une gate, c'est une habitude.
rg -il '@rosegriffon/|rose ?griffon' packages/inacord-ui apps/inacord apps/nie-web \
  --glob '!**/src-tauri/tauri.conf.json' --glob '!**/ui/skeleton.tsx' | wc -l
rg -l '@tauri-apps' packages/inacord-ui | wc -l
```

**Gate :** les deux rendent **0**. Départ mesuré : 13 fichiers, 23 imports, 19 mentions.

**Mesuré le 2026-09-06 : la gate est TENUE — 0 violation.** Les deux derniers matches de la
forme non filtrée sont des exceptions, et aucune n'est un oubli :

1. `apps/inacord/src-tauri/tauri.conf.json:41` — l'updater des 0.5.x déjà installées lit encore
   `azalee.rosegriffon.fr/tools/niers/latest.json` (qui redirige), en **2ᵉ** position derrière
   `aphrody.com/downloads/inacord/latest.json`. Le retirer figerait silencieusement toutes les
   0.5.x. Il partira quand le parc aura basculé, pas avant.
2. `packages/inacord-ui/src/components/ui/skeleton.tsx:6-7` — un **commentaire de doctrine**,
   sans `import` ni URL ni marque affichée : il explique que le squelette a été réécrit ici
   *parce que* `@rosegriffon/ui` a été retiré. La ligne qui matche est **la preuve que la règle
   a été appliquée** ; l'effacer pour satisfaire une regex reviendrait à supprimer la
   justification et non la dette.

Le constat qui compte : sans les deux `--glob`, **cette gate compte ses propres preuves** et ne
peut atteindre 0 qu'en cassant l'updater ou en effaçant une justification. C'est la même famille
que le test qui ne peut pas échouer — un instrument qu'on satisfait en abîmant ce qu'il mesure.

À noter aussi : `apps/nie-web/src/legacy/` **n'existe plus** (le `rg` rend `os error 2`). Le sas
a été vidé ; l'exception qui le concernait est sans objet.

### Bloc 5 — la production `nie-site`

Le binaire en ligne est antérieur au correctif WAL : `/api/v1/episodes` rend **500**.

```bash
cargo build --release -p nie-site           # sans go : ne touche rien en ligne
```

L'installation et le redémarrage, eux, sont au § 5.

### Bloc 6 — J7 : ce qui se fait sans rien basculer

- `nie-site` : réglage `moka` (TTL, poids), pré-compression complète, baseline `criterion`
  commitée.
- Docs : `CLAUDE.md`, `AGENTS.md`, `docs/README.md`, `docs/stack` (amendement **daté** si une
  brique a bougé), et `/PLAN.md` — chaque ligne marquée avec son compte.

### Bloc 7 — enchainer sur la couverture totale

La journee finie, la cible suivante est deja ecrite :
[`docs/PLAN-SITE-ULTIME.md`](PLAN-SITE-ULTIME.md). Elle ne demande pas d'ecrire des capacites
nouvelles — elle demande de **servir celles qui existent** : 41 sous-commandes `niers` et
155 commandes Tauri pour 14 chemins d'API aujourd'hui. Son instrument est une matrice de
couverture a CINQ etats — `servi`, `partiel`, `manquant` (le decodeur existe ici, la route
non), `bloque` (ni l'un ni l'autre : du reverse d'abord) et `interne` (avec sa raison) — et sa
gate maitresse est **`manquant = 0` ET `partiel = 0`**. Elle est MESUREE depuis le 2026-09-06 :
`nie-site --regenerer-couverture var/couverture-site.json` rend la matrice, `/couverture` la
sert. Premier resultat : **583 capacites, manquant = 205, partiel = 0 — gate ROMPUE**. Le VFS
avait ete annonce a `manquant = 0` le matin meme ; l'instrument le contredit avec 21 250
fichiers, parce que la premiere definition comptait `servi` tout ce que `/f` rend, octets
bruts compris — une gate qui ne peut pas echouer ne mesure rien.

## 4. Les gates de la journée, toutes chiffrées

Une gate qui ne rend pas un nombre n'est pas une gate.

| # | Commande | Attendu |
|---|---|---|
| 1 | `bun run typecheck` | 0 sur 5 workspaces |
| 2 | `rg -l 'bun:sqlite\|node:fs\|/home/ubuntu\|SQLITE_DB_PATH\|SUPABASE_INTERNAL_URL\|DATABASE_URL' apps/azalee packages/azalee` | **0** fichier |
| 3 | Gate 1 sur preview | `/chara` **≥ 50 liens**, fiche **200** |
| 4 | `/chara` en `br` | **< 250 Ko**, `<img>` sans `srcset` = 0 |
| 5 | `rg -il '@rosegriffon/\|rose ?griffon' packages/inacord-ui apps/inacord apps/nie-web` | **0** |
| 6 | `cargo clippy -p <crate> --lib --tests` sur chaque crate touchée | **0 warning** |
| 7 | `cargo check --workspace --tests` | **0 erreur** |
| 8 | `scripts/e2e-site.sh` contre le binaire réel | aucun échec, le compte publié |

**Jamais `cargo build --workspace --all-targets`** : il sature le disque (97 % plein). Le
portail est `clippy`, et `cargo check` pour l'ensemble.

## 5. Les six gestes qui exigent le go de l'utilisateur

Ils ne se font pas, ils se **préparent** : une commande, sa vérification, son retour arrière,
publiés dans un `fact:`. C'est la seule partie du plan qu'un agent ne peut pas clore seul.

1. bascule DNS `azalee.rosegriffon.fr` → Vercel ;
2. `nginx -t` puis `reload` (découpe du vhost `aphrody.com`, retrait de `supabase-compat.inc`) ;
3. `systemctl stop/start/daemon-reload` — dont l'installation du `nie-site` rebâti et
   `nie-miroir-cloud.timer` ;
4. rotation d'un secret (`SUPABASE_JWT_SECRET`, mot de passe Postgres) ;
5. premier `vercel --prod` ;
6. toute suppression de données (pages `/tools/*`, `azalee-web`, `rg-releases/azalee`).

`git push` était de cette liste ; l'utilisateur l'a autorisé le 2026-09-06 pour cette mission.

## 6. Les pièges qui coûtent le plus cher ici

Ils sont détaillés dans `CLAUDE.md` et `AGENTS.md`. Les cinq qui ont réellement coûté une
demi-journée chacun, dans ce dépôt :

- **Le faux vert par configuration.** Une variable d'environnement qui gagne en silence
  (`pickUrl`), et une page rend 200 avec 0 lien. **Compter le contenu, pas le statut.**
- **Une feature Cargo éteinte** transforme un test en « ok. 0 passed », ou casse clippy en
  E0433 sur un crate sain. Regarder les features **avant** d'accuser son code.
- **`$?` après un pipe** est le code du dernier maillon. `set -o pipefail` est posé au premier
  niveau ; un `141` après `| head` est une coupure, pas un échec.
- **`sed -i` échoue des deux côtés en silence** (motif absent → exit 0, fichier intact ;
  motif trop fréquent → trop de remplacements, exit 0). Éditer avec un vrai outil.
- **`.gitignore` fait disparaître sans un mot.** Vérifier tout fichier non-code nouveau par
  `git check-ignore -v` — c'est ce qui avait sorti du dépôt les quatre templates askama de
  `nie-site`, dont `robots.txt`, et empêchait la crate de compiler sur un clone frais.

Et un piège de bibliothèque partagée, trouvé aujourd'hui : `packages/inacord-ui` est monté par
**deux hôtes**, dont `apps/inacord` qui cible **ES2022**. Un `toSorted()` y passait le
typecheck de `nie-web` (ESNext) et celui d'`inacord-ui`, et cassait le troisième paquet. Une
bibliothèque partagée tient au **dénominateur commun de ses hôtes**, et seul `bun run
typecheck` complet le voit.

## 7. Coordination

```bash
# Prendre un périmètre AVANT d'écrire
aphrody a2a tick --iteration <n> --side codex --peer claude --kind fact \
  --subject "claim: <périmètre>" --body "<fichiers vises>"

# Rendre un lot, avec ses comptes
aphrody a2a tick --iteration <n> --side codex --peer claude --kind fact \
  --subject "done: <lot>" --body "<commande> -> <compte>"
```

- `--kind` n'accepte que `fact` et `ping` ; le type réel se code dans le **sujet**.
- Listener de ce dépôt : `127.0.0.1:8792` (`8788` est celui du dépôt `aphrody`).
- **Commits.** La règle « un seul auteur » tombe pour cette mission : Codex committe ses
  propres lots dans `niers`, un lot par commit, avec le compte de sa gate dans le message.
  En contrepartie, `claim:` avant d'écrire devient obligatoire, sans exception — trois
  sessions écrivent en parallèle et un démon externe peut capter un lot à mi-course.
- **Fichiers d'arbitrage** (`CLAUDE.md`, `AGENTS.md`, `.gitignore`, `justfile`, manifestes
  racine) : les modifier reste soumis à un `block:`. `docs/` s'ouvre à Codex pour cette
  mission, sauf ce fichier-ci et `/PLAN.md`.

## 8. Ce que « fini » veut dire

La journée est finie quand, dans l'ordre : le typecheck est vert sur les cinq workspaces ; le
wiki ne lit plus un seul fichier local et sa preview compte ses liens ; `/chara` pèse moins de
250 Ko ; plus aucune mention Rose Griffon ne subsiste côté `aphrody-dev` ; `nie-site` est
rebâti ; et les six gestes de production sont **prêts à appliquer**, chacun avec sa commande,
sa vérification et son retour arrière, dans un `fact:` que l'utilisateur n'a plus qu'à
approuver.

Ce qui n'aura pas été atteint se dit tel quel, avec son compte et sa raison. Un plan à moitié
fait et annoncé fini coûte plus cher qu'un plan à moitié fait.
