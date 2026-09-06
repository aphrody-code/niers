# AGENTS.md — contexte commun à tous les agents de `niers`

Ce dépôt est travaillé par **plusieurs agents à la fois** (Claude Code, Codex, et ce qui
viendra). Ce fichier est le contexte que **tout** agent lit en premier, quel que soit son
moteur. Il tient sur un écran : le détail est ailleurs, et dit où.

| Pour | Lire |
|---|---|
| Les règles complètes du dépôt | `CLAUDE.md` — vaut pour **tous** les agents, pas seulement Claude |
| **La mission en cours** | **`docs/CODEX-JOUR-UNIQUE.md`** — tout le plan en une journée |
| **Le niveau d'exigence** | **`docs/PLAN-SITE-ULTIME.md`** — couverture de TOUTE la surface du dépôt |
| Le protocole de coexistence | `docs/A2A-CODEX.md` |
| La carte machine (A2A v1.0) | `ai.json` à la racine |

Communiquer en **français**.

> **Mission en cours — consigne de l'utilisateur du 2026-09-06.** Codex prend en charge
> **tout** `/PLAN.md` (J1 → J7) et l'exécute **en une journée**, en raisonnement maximal et en
> exécution proactive. Cela **remplace** la frontière du 2026-09-05 (« Codex dans `rg`, Claude
> dans `niers` ») : Codex écrit désormais dans `niers`, et y committe ses propres lots. L'ordre
> des blocs, les huit gates chiffrées et les six gestes qui exigent un go sont dans
> [`docs/CODEX-JOUR-UNIQUE.md`](docs/CODEX-JOUR-UNIQUE.md).

---

## 1. Se parler

```bash
# Emettre (depuis la racine, qui porte ai.json)
aphrody a2a tick --iteration <n> --side <moi> --peer <lui> --kind fact \
  --subject "<type>: <sujet>" --body "<fait mesure>"

# Lire ce que l'autre a ecrit
tail -5 .coord/inbox-from-<lui>.jsonl | jq -c '{ts,topic,body}'
```

- **`--kind` n'accepte que `fact` et `ping`.** Mesuré : `claim`, `done`, `block`, `status`
  retombent sur `ping` **en silence**, l'intention est perdue. Le type se code donc dans le
  sujet : `claim:`, `done:`, `block:`, `goal:`.
- Le listener JSON-RPC de ce dépôt est `127.0.0.1:8792` (`aphrody a2a serve`).
  **`8788` est celui du dépôt `aphrody`** — ne pas confondre.
- MCP partagés : `aphrody` (docs, RE) et `niers-game` (VFS, assets, KB). MCP sert à **agir**,
  jamais à se coordonner : un appel MCP ne laisse aucune trace lisible par l'autre agent.

## 2. Ne pas s'écraser

1. **Annoncer son périmètre avant d'écrire** (`claim:`), et n'écrire rien en dehors.
2. **Fichiers d'arbitrage réservés à Claude** : `CLAUDE.md`, `AGENTS.md`, `.gitignore`,
   `justfile`, manifestes racine, `/PLAN.md`, `docs/CODEX-JOUR-UNIQUE.md`. Besoin d'un
   changement ? Envoyer un `block:`. Le reste de `docs/` est ouvert pendant la mission.
3. **Un auteur par lot** (amendé le 2026-09-06 ; c'était « un seul auteur de commits »). Un
   lot = un commit, par celui qui l'a écrit, avec le compte de sa gate dans le message. La
   règle d'origine supposait un seul agent en écriture ; à deux, elle transforme le travail de
   l'un en commit anonyme de l'autre — c'est arrivé ici même, `188e409` a capté trois fichiers
   à mi-course. La contrepartie est que `claim:` avant d'écrire n'a plus d'exception.
4. **Rien de destructif ni de production sans accord** : pas de `rm -rf`, pas de
   `git reset --hard`, pas de redémarrage de service, pas d'écriture hors du dépôt.
   **`pkill -f` est interdit** — il tue les sessions d'agent. Cibler un PID.

La boucle autonome (`scripts/a2a-loop.sh <claude|codex>`) fait un tour : lire le dernier
`goal:` du pair, l'exécuter, rendre un `done:`, puis **fixer au pair le `goal:` suivant**.
Seul un sujet préfixé `goal:` vaut ordre de travail.

## 3. Vérifier — et ce qui ment

Le portail est **clippy**, jamais un build complet.

```bash
cargo clippy -p <crate> --lib --tests     # 0 warning exige
bun run typecheck                          # cote TS
```

- **`cargo build --workspace --all-targets` sature le disque** (92 % plein). Ne jamais le lancer.
- **Une suite qui affiche « 0 passed » n'est pas verte** : elle n'a pas tourné.
- **Une feature éteinte transforme un test en faux vert** — ou en erreur de compilation.
  `nie-formats` n'active que `std` et `lua` ; `nie-data` n'active pas `serde`. Un test qui
  utilise un item gaté **sans** `[[test]] required-features` casse le portail (E0433) au lieu
  de sauter. Vérifié ici sur 24 tests de `nie-data`.
- **Une garde de test qui teste un chemin en dur au lieu de `NIE_GAME_DIR` se skippe
  toujours**, en silence, et la suite s'annonce verte sans rien exécuter.
- **`dotnet` est ABSENT de ce VPS** : `csharp/` ne se compile ni ne se teste ici. Un lot C#
  ne peut être que **relu** — le dire, ne jamais l'annoncer vérifié.

## 4. Les pièges qui coûtent le plus cher

**Git ne descend jamais dans un répertoire exclu.** `!data/oc/` seul ne ramène rien si
`data/` est ignoré. Il faut ré-inclure le parent, ré-exclure son contenu direct, puis
ré-inclure la cible — et écrire `.agents/**`, jamais `.agents/`, quand on veut ré-inclure
dedans. Vérifier chaque cas par `git check-ignore -v <fichier>`, jamais au raisonnement.

**Un `.gitignore` ne s'applique plus à un fichier déjà suivi.** Un fichier d'instructions
peut donc « exister » chez vous et être absent d'un clone frais, sans le moindre signal.
C'est ce qui a fait disparaître `AGENTS.md`, les sous-agents du plugin `niers` et les README
des OC. Tout le markdown du dépôt est désormais versionné, sans liste d'exceptions.

**Un chemin machine en dur court-circuite la résolution du jeu.** Aucun chemin de machine
n'est compilé dans un binaire : `nie_formats::vfs::resolve_game_dir()` côté Rust,
`dansLeDepot()` côté TS, `TestDataPaths`/`ResolveDefaultGamePath()` côté C#. Chercher le
helper existant avant d'en écrire un.

**Un chemin VFS cité de mémoire est presque toujours faux** — les fichiers du jeu portent un
numéro de version (`chara_base_1.03.98.00.cfg.bin`). Le résoudre par `niers vfs find` avant
de l'écrire. C'est la mesure qui tranche une revue de code, dans un sens comme dans l'autre.

**`sed -i` échoue en silence des deux côtés** : motif absent → 0 remplacement, exit 0,
fichier intact ; motif trop fréquent → trop de remplacements, exit 0. Éditer avec un vrai
outil d'édition. Même logique pour Python : `uv run` toujours, et **un fichier** au-delà de
deux lignes (le shell mange `$(…)` et les backslashes avant Python).

**`rg`, jamais `grep -r` à la racine** : `grep` descend dans `node_modules` et part en
timeout à 60 s, quand `rg` répond en 0,06 s.

## 5. Ce qui casse la production

Cette machine porte **18 services** et un état partagé. Avant de déplacer ou de renommer,
chercher qui pointe dessus **hors du dépôt** :

- `/etc/systemd/system/nie-miroir.service` cible en dur
  `scripts/donnees/miroir-inagle.sh`, son timer est actif, et son `ExecStartPost` redémarre
  `nie-model-serve`. Le renommer casse la rotation nocturne. Le réparer demande un
  `daemon-reload`, donc l'accord de l'utilisateur.
- Un démon externe commit périodiquement sous `chore(sync): checkpoint <horodatage>`. Il ne
  distingue pas les auteurs et **peut capter un lot à mi-course**. Relire `git log` avant de
  conclure qu'un commit est le sien.

## 6. Contrainte produit — Aphrody sur `aphrody.com`, Inacord, nie

Décision **gelée le 2026-09-05** (`docs/stack/`, plan d'exécution `/PLAN.md`). Trois noms :
**Azalée** le wiki (`azalee.rosegriffon.fr`, Vercel, DA Rose Griffon), **Aphrody** le site
d'outils (`aphrody.com`), **Inacord** l'application de bureau et mobile (`apps/inacord`, ex
`nie-explorer`) ; le jeu s'appelle **nie** et les crates gardent leur préfixe.

**Aphrody n'est ni un wiki ni un explorateur de fichiers** (précision de l'utilisateur, le
2026-09-05) : le wiki est Azalée, l'explorateur est Inacord. Son interface **reproduit le menu
principal du jeu**, et pas de mémoire : `nie-game --runtime --menu <écran> --export-layout`
rend la disposition réelle — pour `mainmenu01`, un canevas de 1280×720 et 34 objets avec leur
`transform`, leur `drawPriority`, leur sprite et leurs textes déjà traduits. Une interface
d'Aphrody qui présente des listes de fichiers a dérivé vers le métier d'Inacord.

Aphrody est servi par la crate `nie-site` **100 % Rust**, `publish = false`, sous
`crates/tools/` : Axum 0.8, Tokio 1.x, Tower, `askama`, `moka`, `rusqlite` en lecture seule ;
aucun serveur Bun/Node ; pas de Leptos, pas de SQLx. Écoute **uniquement** sur
`127.0.0.1:8085`, derrière nginx qui termine le TLS. Fournir `/healthz`, `/robots.txt`,
`/.well-known/security.txt` ; API sous `/api/v1/`, paginée, sans détail d'infrastructure ;
`nie-model-serve` n'est atteint **que** par son proxy. Tests de routes qui **comptent** +
clippy sans avertissement avant d'activer nginx. Aphrody et Inacord sont **la même
interface** (`packages/inacord-ui`, contrat `packages/asset-source`) : rien ne se réécrit d'un
côté.

**État au 2026-09-05 — ces paquets EXISTENT, ils ne sont plus à faire.** `nie-site` sert
13 routes (44 tests, clippy 0), `scripts/e2e-site.sh` rend 65 vérifications sans échec contre le
binaire réel, et Aphrody monte l'interface partagée : 4 catalogues cherchables sur
143 246 fichiers, navigation `/b`, lecture audio et vidéo, `/api/v1/episodes`. Deux règles en
découlent pour qui reprend :

- **Ne jamais écrire de condition sur l'hôte dans un composant.** Sur les 147 commandes du
  desktop, ~66 sont portables et 81 ne le seront jamais ; c'est le contrat qui porte
  l'asymétrie, et `capacites()` qui la MESURE au lieu de l'affirmer.
- **`apps/nie-web/src/legacy/` est un sas, pas une bibliothèque.** Exclu du `tsconfig`, il
  garde le code sorti du wiki jusqu'à sa réécriture contre `/f`, `/b` et `/api/v1`. Ses
  mentions Rose Griffon disparaîtront avec lui — les toiletter avant serait travailler sur du
  code condamné.

**Propriété.** Seule Azalée est un produit **Rose Griffon**. Aphrody, Inacord et nie sont des
projets **`aphrody-dev`** : aucune marque, mention, URL `rosegriffon.fr`, paquet
`@rosegriffon/*` ni compte partagé dans `nie-site`, `nie-web`, `inacord-ui`, `apps/inacord`.
Seule exception, temporaire : l'updater des installations 0.5.x lit encore
`azalee.rosegriffon.fr/tools/niers/latest.json`, qui redirige vers
`aphrody.com/downloads/inacord/latest.json`.

Les contenus Inazuma Eleven sont exploitables au titre de l'Accord Commercial Officiel
N° RG-L5-VR-2026-001 — **signé par Rose Griffon** : la base légale de leur exploitation sur
un site `aphrody-dev` est **à confirmer par l'utilisateur**, aucun agent ne la présume.
**Jamais** de donnée personnelle, de secret, de credential, de chemin machine ni de dump hors
périmètre.

Sur `aphrody.com`, seuls `aphrody.com` et `www` passent à `nie-site` ; `api.`, `downloads.`,
`cdn.`, `bot.`, `admin.`, `mcp.`, `bxc.`, `n2b.` restent au dépôt `aphrody` (`aphrody-site`,
:8083), dont `docs/SITES-PLATFORM.md` prévoyait « Niers » sur `nie.aphrody.com` : à amender
par son propriétaire.
