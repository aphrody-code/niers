# Vérification, gates et définition de fini

Un gate est une commande, un compte attendu et un verdict. Ce document dit lesquels ; il ne
transforme jamais une cible en fonctionnalité livrée. Les propriétaires et les jours sont
dans [`/PLAN.md`](../../PLAN.md).

## La règle qui précède toutes les autres

**Un code de sortie et un code HTTP ne mesurent pas la présence de données.** Le 2026-09-05,
deux fois dans la même journée, un build vert et des pages en 200 ont caché un site vide :
RLS sans policy (PostgREST rend 200 et `[]`), puis `SUPABASE_INTERNAL_URL` testé avant
`NEXT_PUBLIC_SUPABASE_URL` par `pickUrl()`. Dans les deux cas, `/chara` répondait 200 en
moins de 90 ms — avec **0** lien de personnage. Depuis, chaque gate de rendu **compte les
éléments attendus dans la réponse**, et un `0 passed` ou un `0 élément` est un échec.

## Gate 1 — serverless du wiki (`scripts/ops/gate-serverless.sh`)

```bash
scripts/ops/gate-serverless.sh              # build + next start + assertions
scripts/ops/gate-serverless.sh --no-build   # rejoue sur le .next existant
```

Il force `SQLITE_DB_PATH=/nonexistent/mirror.sqlite` **et** les deux URL Supabase sur le
Cloud, lit un vrai `base_slug` dans l'origine visée au lieu d'en inventer un, puis compte :

| Page | Assertion | **MESURÉ 2026-09-05 07:05, VPS → Cloud** |
|---|---|---|
| `/chara` | ≥ 50 `href="/chara/…"` distincts | **200** |
| `/skill` | ≥ 50 `href="/skill/…"` | **60** |
| `/item` | ≥ 20 `href="/item/…"` | **48** |
| `/equipe` | ≥ 5 `href="/equipe/…"` | **208** |
| `/chara/<base_slug réel>` | HTTP 200 | `mark-evans` → **200** |
| Build | `EXIT_REEL` lu dans le log, pas le code du harnais | **0**, 120/120 pages, 1 114 replis `SQLITE_CANTOPEN` |

TTFB au même run : `/` 17 ms, `/chara` 52 ms (2 708 582 o non compressés), fiche 6 ms
(215 102 o). Verdict : **le wiki rend ses données sans aucun fichier local.** Cette mesure
est faite depuis le VPS ; la latence Vercel → eu-west-3 reste à mesurer sur la preview (J2).

## Gate 2 — le wiki ne lit plus rien de local (J2)

```bash
rg -l 'bun:sqlite|node:fs|from "fs"|/home/ubuntu|process\.cwd\(\)|SQLITE_DB_PATH|SUPABASE_INTERNAL_URL|DATABASE_URL' \
   apps/azalee packages/azalee --glob '!node_modules' --glob '!*.md'
```

Attendu : **0 fichier** (départ MESURÉ : 41 `bun:sqlite`, 44 `node:fs`, 15 `/home/ubuntu`).
Ce qui reste légitimement local part dans `apps/nie-web` — il n'est pas « corrigé » sur place.
Puis `vercel deploy` (preview) et le Gate 1 rejoué **contre l'URL preview**, avec en plus
`curl -H 'Accept-Encoding: br' -w '%{size_download}'` pour le poids réel (sans cet en-tête,
`/chara` paraît peser 2,36 Mo au lieu de ~104 Ko).

## Gate 3 — poids et latence du wiki (J3, puis avant/après chaque lot)

| URL | Départ MESURÉ (prod, 2026-09-05) | Objectif |
|---|---|---|
| `/chara` HTML | 2 355 397 o (81 % de DOM : 620 liens, 404 `<img>`) | **< 250 Ko** |
| `<img>` sans `srcset` | 404 / 404 | **0** |
| `/` TTFB | 30 ms | ≤ 50 ms depuis Vercel |
| `/chara/<id>` TTFB p95 sans miroir | 6 ms (VPS → Cloud, n = 1) | **< 800 ms** depuis Vercel, n ≥ 20 |

Outil : `curl -w '%{http_code} %{time_starttransfer} %{size_download}'` sur les mêmes URL,
`hyperfine --warmup 3` pour les séries. La matrice se rejoue **avant et après chaque lot** ;
un lot sans matrice n'est pas promu.

## Gate 4 — extraction de l'interface partagée, Inacord (J4)

```bash
rg -l '@tauri-apps' packages/inacord-ui/           # attendu : 0
rg -l 'apps/inacord' --glob '!docs/**' --glob '!*.md'   # attendu : 0 (l'app est apps/inacord)
bun run typecheck                                  # 5 workspaces + inacord-ui + asset-source
( cd apps/inacord/src-tauri && cargo check )       # l'hôte Tauri compile toujours
bun run --filter inacord build                     # le bundle de bureau se construit
jq -r '.productName, .identifier' apps/inacord/src-tauri/tauri.conf.json   # Inacord / dev.niers.explorer
```

Puis **lancer** Inacord : seule l'exécution trouve une ressource jamais lue ou une table
vide — ni `tsc`, ni clippy, ni la taille du bundle ne les voient. Départ MESURÉ : 158 fichiers,
34 avec Tauri, `api.ts` 630 lignes, `productName: "niers"` 0.5.9. La mise à jour depuis une
installation 0.5.9 réelle (Windows) vers la première release « Inacord » est **À VÉRIFIER**
avant publication : elle doit mettre à jour, pas installer à côté.

## Gate 5 — `nie-site` et `nie-web` (J5)

```bash
cargo clippy -p nie-site --all-targets -- -D warnings   # 0 avertissement
cargo test -p nie-site                                   # tests de routes : ils COMPTENT
cargo bench -p nie-site                                  # criterion, routage et cache
hyperfine --warmup 3 'curl -s http://127.0.0.1:8085/api/v1/textures?page=1'
```

Attendu : `/healthz` 200 JSON, `/robots.txt`, `/.well-known/security.txt`, `/` sert
`index.html` du bundle avec ses balises `og:`, `/api/v1/textures?page=1` rend `per_page`
éléments **comptés** dans le test, `/assets/…` répond via le proxy avec `ETag` et `br`, un
amont absent rend **503 en moins de 10 s** et jamais un 504 à 30 s. Bundle initial de
`nie-web` **< 300 Ko gz**. TTFB `/textures` **< 50 ms** (départ : 392 ms), `/modeles`
**< 50 ms** (départ : 229 ms).

Chaque test de route affirme un contenu (`assert!(count >= n)`), pas seulement un statut.

**DA du jeu** : `niers design tokens --out packages/inacord-ui/src/theme/game-tokens.css`
écrit **70** variables — le compte du fichier `font_color.cfg.bin`, pas un chiffre choisi —
et le test le vérifie ; la page d'accueil d'Aphrody est capturée par `bxc` (rendu réel, CSP
comprise) et posée à côté de la référence `data/design/aphrody-ui-ref-mainmenu-7.1.2.png`
pour revue ; aucune couleur, icône ou texture du thème n'est écrite « de mémoire ».

**Vhost `aphrody.com`** (go de l'utilisateur) : `nginx -t` ; `curl -sI` sur les dix hôtes
**avant et après** le `reload` — `aphrody.com` et `www` répondent `nie-site`, les huit autres
répondent comme avant ; `nie.aphrody.com` rend 308 vers `aphrody.com` ; l'en-tête CSP vu par
le navigateur est **celui de `nie-site`**, pas `default-src 'none'`.

## Gate 6 — bascule (J6, go de l'utilisateur)

1. `dig +short azalee.rosegriffon.fr` pointe Vercel ; Gate 1 rejoué **contre la production**.
2. Les dix préfixes (`/cpk`, `/textures`, `/modeles`, `/mode`, `/sons`, `/videos`, `/avatar`,
   `/demo`, `/save`, `/vroid`) rendent **308** vers `aphrody.com` — et **`/tools/niers/latest.json`
   rend 200** depuis `azalee.rosegriffon.fr` : c'est l'updater de toutes les installations d'Inacord.
3. Les 19 consommateurs de `/rest/v1|/realtime/v1|/storage/v1` visent `*.supabase.co` ;
   handshake WebSocket Realtime **101** ; une URL signée Storage télécharge.
4. `aphrody.com/healthz` 200 servi par `nie-site` ; `nie-model-serve` n'a plus de vhost public.
5. `azalee-web.service` arrêté, non supprimé, 7 jours ; rollback = DNS + `systemctl start`.

## Gates gelés — hors semaine

Inchangés et **non exécutés cette semaine** : moteur (replay headless, captures RGBA8, bump
wgpu 29 → 30), WASM (`cargo build --target wasm32-unknown-unknown`, `cargo test -p nie-wasm`),
Tauri mobile (`bun tauri android dev`, `ios dev`, appareil réel), Steam (`RestartAppIfNecessary`,
`Init`, SteamPipe beta, `steam_appid.txt` absent de l'artefact). Ils redeviennent actifs avec le
lot qui les concerne, jamais par ce document.

## Gate données et confidentialité — permanent

- aucune migration de `auth.users` ; `profiles`, `account`, `two_factor`, `audit_logs` fermés ;
- secrets en variables d'environnement Vercel / fichiers `~/.config/niers/*.env` à 0600, jamais
  dans le dépôt, jamais affichés ;
- rôles Postgres minimaux : `anon` lit `inagle_*`, n'écrit rien, n'exécute aucun RPC ;
- URLs Storage signées et expiration testée ; CORS explicite par origine ;
- `/vfs`, `/raw`, `/export`, `/depot` et `/assets` testés contre traversal et taille.

## Définition de fini

Un lot est fini quand **sa gate a tourné, a compté, et que le compte est dans le commit** —
commande, hôte, date, valeur. « Ça compile », « 200 », « 0 passed » et « exit 0 » ne sont pas
des états : ce sont les quatre formes du faux vert que ce dépôt a déjà payées.
