@AGENTS.md

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

App `@rose-griffon/azalee-web` — base de donnees communautaire Inazuma Eleven: Victory Road (azalee.rosegriffon.fr). Next 16 App Router, React 19, Better Auth (Discord + Google), Supabase Cloud, donnees jeu via `@rose-griffon/inagle` (parser binaire CPK/G4TX).

Le `CLAUDE.md` parent (`../../CLAUDE.md`) couvre les conventions du monorepo (commits FR 1 ligne, gitignore *.md, Bun obligatoire, deploy via Vercel). **Ne pas dupliquer ici.**

## Commandes

| Commande | Usage |
|---|---|
| `bun install` | A la racine. Le `node_modules` local est resolu via le linker hoisted (cf. piege Bun isolated dans CLAUDE.md parent). |
| `bun run dev` | Next dev server Turbopack (port 3000 en dev, 3002 en prod via systemd). |
| `bun run build` | Build standalone Next 16. **Pas de `next lint`** — `eslint` direct. |
| `bun run lint` | `oxlint` puis `eslint` (config-next + better-tailwindcss + react-compiler). |
| `bun run lint:fix` | Auto-fix les deux. |
| `bun run type-check` | `tsc --noEmit`. Depuis 2026-05-17 `typescript.ignoreBuildErrors=false` (TS 6.0 + React 19.2 + zod 4 ont corrigé la drift). Erreurs TS bloquent maintenant `next build`. |
| `bun run backup:supabase` | Dump Supabase Postgres → SQLite via le pilote de Bun (WAL + batch transactions). Sortie : `data/backups/supabase-<ISO>.sqlite` (~40 MB, 18 601 rows / 77 tables, gitignored). Lancer avant tout op risqué DB. |
| `bun run test` / `test:run` | Vitest (jsdom + `tests/setup.ts`). |
| `azalee test` | Exécute la suite complète de vérifications natives (SQLite DB, API HTTP, Bxc detect/recon, DOM, rendu des pages). |
| `vercel --prod` | Deploy prod sur Vercel depuis la racine ou via Vercel CLI. |
| `bun run sync:inagle` | Sync donnees inagle (CPK/G4TX) vers Supabase. |

## Architecture

### App router et alias

- `app/` (App Router), `components/`, `lib/`, `hooks/`, `src/lib/`, `src/data/` cohabitent au meme niveau.
- **Path alias TS** `@/*` mappe vers **deux** racines : `./src/*` ET `./*` (la racine, ou vivent `lib/`, `app/`, `components/`). Un import `@/lib/auth` resout `lib/auth.ts` racine, pas `src/lib/`. Verifier l'IDE en cas d'ambiguite.
- `tsconfig.json` exclut `scripts/` du type-check.
- `eslint.config.mjs` ignore aussi `scripts/`, `data/`, `public/`, `lib/inagle/_cli_ignore/`, `lib/inagle/_scripts_ignore/`, `src/lib/inagle/{types,data,parsers,...}` — ces sous-arbres sont du code genere/legacy, ne pas y appliquer de lint/refactor automatique.
- `better-tailwindcss` config pointe vers `src/app/globals.css` (cf. `eslint.config.mjs`) mais le vrai fichier vit a `app/globals.css` — incoherence connue, tailwind v4 lit quand meme les @theme depuis le bon endroit a runtime.

### Auth (Better Auth + pont Supabase)

`lib/auth.ts` : Better Auth instance avec `pg.Pool` (chaine de connexion Postgres → table users Better Auth) + `supabaseAdmin` service-role pour upsert dans `profiles`.

- Providers : **Discord** + **Google** OAuth, plus magic-link (`app/api/auth/magic-login`).
- `databaseHooks.user.create.after` → `ensureUserProfile()` cree la ligne `profiles` Supabase. Re-appelable depuis layouts pour self-healing.
- Plugins : `bearer()` puis `nextCookies()` **doit rester DERNIER** (propage Set-Cookie depuis server actions).
- `middleware.ts` : check optimiste cookie sur `/dashboard`, `/settings`, `/api/dashboard`. Validation crypto cote pages via `auth.api.getSession()`.
- Roles definis dans `lib/auth-roles.ts` : `ADMIN_ROLES = [admin, superadmin, editor, moderator]`, `USER_MANAGEMENT_ROLES = [admin, superadmin]`. **Importer ces constantes**, jamais de hardcode.

### Pont Better Auth → Supabase JWT (DESACTIVE par defaut)

`lib/supabase/jwt.ts` mint un JWT signe avec `SUPABASE_JWT_SECRET` qui se fait passer pour `auth.uid()` dans les policies RLS. Endpoint client : `GET /api/supabase-token`.

- **Toggle** : `DISABLE_SUPABASE_JWT_BRIDGE` — defaut `true` (pont **off**). Reactiver via `DISABLE_SUPABASE_JWT_BRIDGE=false` UNE FOIS le `SUPABASE_JWT_SECRET` resync avec Supabase Cloud.
- Off → `lib/supabase/server.ts` retourne un client anon. Les lectures publiques marchent via `GRANT SELECT ON ALL TABLES TO anon, authenticated` applique 2026-04-20 (cf. piege parent : Supabase Cloud peut perdre ces GRANTs apres migration → 42501).
- **N'introduire aucune route qui depend de `auth.uid()` cote PostgREST tant que le pont est off.** Faire les writes sensibles via `lib/supabase/admin.ts` (service role) cote serveur.

### Donnees jeu : `@rose-griffon/inagle` + IEVR API

Workspace package `packages/inagle/` parse le CPK/G4TX du jeu. Acces runtime via `getVfs()` qui ouvre la VFS lazy (singleton process-local).

API exposee a `app/api/ievr/` — toutes en `runtime = "nodejs"` + `dynamic = "force-dynamic"` (FFI native, pas Edge) :

| Route | Action |
|---|---|
| `GET /api/ievr/list?pattern=&limit=` | Lister les internalPath du VFS. |
| `GET /api/ievr/asset?path=` | Bytes bruts (octet-stream, immutable 24h). |
| `GET /api/ievr/cfgbin?path=` | Decode cfgbin → JSON. |
| `GET /api/ievr/image?path=&format=webp\|png&index=&quality=&lossy=` | Decode G4TX → WebP/PNG, cache LRU bytes en memoire. |

`lib/wiki-service.ts` consomme inagle + Supabase pour servir le GraphQL (cf. ci-dessous).

### GraphQL endpoint

`app/api/graphql/route.ts` — graphql-yoga + `@envelop/graphql-jit` + `@graphql-yoga/plugin-response-cache` (TTL 15min, session globale). Schema inline (Character, Item, Skill, Aura, Tactic). Resolvers delegent a `wikiService` qui combine Supabase rows + sheetData JSONB + inagle types. **Cache global** — pas de differentiation utilisateur, ne pas y exposer de donnees user-scoped.

### Pipeline images : 3 sources

Le module `lib/images.ts` resout chaque path vers la bonne source. Critique a comprendre avant de toucher aux assets :

1. **CDN local `cdn.rosegriffon.fr/static/azalee/menu/`** (`MENU_BUCKET_URL`, ~47k webp pre-extraits). URL fixe en dur — le prerender Next ne voit pas toujours `NEXT_PUBLIC_CDN_URL`.
2. **G4TX live streaming `cdn.rosegriffon.fr/g4tx/data/dx11/menu/<path>.g4tx.webp?w=...`** (`lib/g4tx-cdn.ts`) — decode FFI `libiecode.so` cote CDN, cache disque variants. Utilise pour Keshin/Soul/Mixi auras et certains telop_waza.
3. **CloudFront zukan `azalee.rosegriffon.fr/zukan-assets-mirror/`** (`CDN_BASE_URL`) — videos `.mp4` (transformes depuis `zukan.inazuma.jp/...webm`).

`resolveAssetUrl()` rewrite les paths legacy `/storage/v1/object/public/menu/...` (encore en DB pour ~1010 skills) vers le CDN local + tente un mapping G4TX prioritaire pour les auras. **Toute image qui finit en `200_icon/10_icon_chr/aura_*` doit passer par `mapLegacyMenuPathToG4tx()`** sinon le rendu est cassé.

Migration 2026-02-06 (cf. `CHANGELOG.md`) : tous les PNG de `data/images/menu/` → WebP qualité 80. Si on rajoute des assets PNG dans ce dossier, prevoir conversion `cwebp -q 80`.

### `next.config.ts` — points sensibles

- `output: "standalone"` + `outputFileTracingRoot: ../../` (workspace root) — Next trace les deps depuis la racine du monorepo.
- `transpilePackages: ["@rose-griffon/inagle"]` — workspace TS non-bundle.
- `serverExternalPackages` exclut tous les natifs/lourds (`better-auth`, `pg`, `sharp`, `nodemailer`, `crawlee`, `apify-client`, `cheerio`, `bcryptjs`, `jsonwebtoken`, `googleapis`, `google-translate-api-x`, `web-push`, `konva`, `csv-parse`, `commander`). **Etendre cette liste** si on ajoute un module Node natif.
- `compress: false` — nginx gere gzip, eviter le double compress.
- `reactCompiler: true` (stable Next 16) — pas de `useMemo`/`useCallback` manuels sauf cas tres specifiques.
- `images.imageSizes/deviceSizes` reduits a 6 widths totaux (96/256/384/512 + 768/1024/1920) pour matcher le cache disque G4TX. Ne pas elargir sans pre-warmer le cache.
- `images.minimumCacheTTL: 86400` — 24h cache pour `_next/image` (defaut 60s).
- 2 toggles **build-time** CDN externe :
  - `NEXT_PUBLIC_CDN_URL=https://cdn.rosegriffon.fr/static/azalee` → `assetPrefix` (chunks JS/CSS/fonts servis par CDN, hostname whitelist nginx).
  - `NEXT_PUBLIC_CDN_LOADER=1` → loader Image custom (`./cdn-loader.js`) qui bypass `/_next/image`. **Perte AVIF/WebP runtime** — n'activer que si les assets sont deja en webp (notre cas).
- CSP construit dynamiquement avec l'origin CDN (cf. `headers()` dans `next.config.ts`). Ajouter une nouvelle source externe → editer `cspHeader` array.
- `compiler.removeConsole` strip les `console.log` en prod (garde `error` + `warn`).
- 3 redirects permanents : `/dashboard/news/edit → /new`, `/compare → /tools/compare`, `/random-team → /tools/random-team`.
- Rewrite `/storage/v1/:path* → SUPABASE_INTERNAL_URL` (proxy Supabase Storage par notre origin).

## Pieges

- **Auras G4TX** : la lib `getAuraImageUrl()` mappe `wks{N}` (Keshin) / `wss{N}` (Soul) / `wa{N}` vers `aura_fs|aura_soul|aura_mixi/...g4tx`. Codes hors pattern (`wap*`, `mode_change_*`, `awakening*`, `wmm*`) **n'ont pas de mapping** et retournent string vide → le caller doit fallback. Voir commits recents `bf40b28`, `0e266e2`.
- **Telop miximax FAUX (cross-namespace)** : le mapping naïf `wmm00<NNN> → aura_mixi_c05028<NNN>` pointe sur un perso légendaire d'un AUTRE set (`c05028XXX` = Fei/Ryoma/Zanark) → affichait le nom d'un autre perso (Arthur→Ryoma). Les miximax wmm n'ont **pas** de telop valide : `resolveAuraTelopUrl(wmm*)=null`, icône via `getMiximaxImageUrl` (manifeste cn/ca). Keshins/souls/wap référencent leur PROPRE code = corrects.
- **« Icône » tactique = bannière telop large** `220_img/telop_waza/fr/wht*` (1728×352, ~4.9:1), PAS une icône carrée → l'afficher en bannière (`TacticCard`, `aspect-[24/5]` ; détail = `w-full max-w-xs`), jamais écrasée dans un slot carré (sinon invisible). La liste ne doit pas passer le `wht*` à `getItemIconUrl` (→ 404).
- **Enrichissement objets DURABLE** : bonus de stats / descriptions / maxStack viennent du parser inagle (`item-config.ts` + `item-bonus-db.json` par nameId, descriptions via `item_text`) mais **ne sont pas dans Supabase** → perdus au re-sync du miroir. Figés dans `data/item-enrichment.json` (694 items, par id azalee), appliqués au runtime en repli dans `getItem`/`getItemsList` (`ITEM_ENRICHMENT[item.id]`). Régénérer depuis le miroir enrichi si le parser évolue.
- **Symlinks hors-racine = OUTAGE Turbopack** : `apps/azalee/data/` doit être un VRAI dossier (les `@/data/*.json` importés réels + trackés) ; le miroir vit IN-ROOT `data/backups/supabase-<stamp>.sqlite` (+ `mirror.sqlite` symlink RELATIF in-dir). Ne jamais symlinker `data/` vers `/home/ubuntu/niers/data` (Turbopack refuse les symlinks hors racine → build cassé → service sur build supprimé → chunks 500). Cf. CLAUDE.md parent.
- **Pages** : `/gallery` (lightbox+download+préchargement + 2394 illustrations du dossier menu via CDN live, `menu-gallery-manifest.json`), `/cross` (Inazuma Eleven Cross = jeu mobile distinct, sortie 9/6/2026), `/save` (upload save→résumé, wasm niers, en cours). Validation navigateur réel = **bxc** (curl ne voit ni le rendu JS ni la CSP : c'est bxc qui a révélé l'outage chunks-500, la CSP img-src manquante, et les telop faux).
- **Service Worker** `public/sw.js` : bumper `CACHE_NAME` apres tout update critique (assets, manifest, CSP) sinon les clients servent du stale.
- **`dynamicParams = false` sur `/gallery/[category]/page.tsx`** (cf. commit `ae367c6`) — les nouvelles categories doivent etre ajoutees dans `generateStaticParams` ou la page renvoie 404.
- **GraphQL response-cache global** : pas de bypass user-aware. Toute donnees personnelle/admin doit transiter par les routes REST sous `/api/dashboard/*`, **pas** par `/api/graphql`.
- **Supabase JWT bridge** : laisser `DISABLE_SUPABASE_JWT_BRIDGE` non defini (= true) tant que le secret n'est pas resync. Le code RLS qui depend de `auth.uid()` echouera silencieusement avec un client anon.
- **Lint paths ignores** : ne pas refactor `src/lib/inagle/{parsers,data,types}` ni `lib/inagle/_*_ignore/` — ils sont generes/legacy et exclus du lint/type-check.
- **`reactCompiler: true`** : ne pas ajouter `"use no memo"` sans raison documentee. Si un composant boucle, identifier le state mutable plutot que d'opt-out.
- **Déploiement : Vercel, décision gelée le 2026-09-05** (`docs/stack/`, `/PLAN.md`). Azalée part en full serverless sur Vercel (runtime Node, ISR) avec Supabase Cloud `kvnlbhatjqqmhhxaxlbi` pour seule source de données. L'ancienne règle « ne JAMAIS déployer sur Vercel » venait d'échecs sous Bun et d'une base en `127.0.0.1` — deux causes levées ; le gate `scripts/ops/gate-serverless.sh` a rendu ses données sans miroir le 2026-09-05 (`/chara` 200 liens). Jusqu'à la bascule DNS (J6 du plan), la production reste `azalee-web.service` sur le VPS (Next 16 standalone, port 3003, `bash scripts/ship-azalee.sh`) ; ce qui lit un fichier local part chez **Aphrody** (`aphrody.com`, `apps/nie-web`), il ne se corrige pas sur place. **Un build vert et des pages en 200 ne prouvent rien : compter les éléments rendus.**
- **`better-tailwindcss` entryPoint** : pointe sur `app/globals.css` (PAS `src/app/globals.css`). Sinon ~60 false positives `no-unknown-classes` sur les tokens M3 custom (`bg-error-container`, `text-on-surface-variant`, …). Cf. `eslint.config.mjs`.
- **`.next/` build lock** : back-to-back `bun run build` echouent ("Another next build process is already running"). Attendre la fin du précédent ou kill `node .../next build` orphelins.
- **`.claude/` gitignored** : sub-agents Claude Code avec `isolation: worktree` créent `.claude/worktrees/agent-X/` (= git embed pollue le tree). Doit rester dans `.gitignore` (commit `0351320`).
- **Auto-formatter Edit revert** : un Edit peut être silencieusement annulé/ré-indenté par un hook éditeur après le retour OK du tool (tabs/spaces, restore import retiré). Si une modif disparaît : fallback `sed -i` (bypass formatter) ou `Write`.
- **Baseline DB schema** : `data/schema-snapshot/{public-schema-*.sql, columns.json, rls-policies.json, tables.json}` (308 KB committé) = `pg_dump --schema-only` au 2026-05-17. Diff contre Supabase Cloud pour détecter schema drift.
- **`noUncheckedIndexedAccess: false` override** : activé global dans `packages/config/tsconfig-base.json`, désactivé dans `apps/azalee/tsconfig.json` car 183 erreurs pré-existantes (`SelectQueryError` Supabase + `Object is possibly undefined` sur sitemap/auth). TODO : migration progressive.
- **`"use server"` → tout export DOIT être async** : dans un fichier server actions (ex `app/actions/translate.ts`), un `export function helper()` non-async casse `next build` avec `Server Actions must be async functions` — mais **`tsc --noEmit` ne le détecte PAS** (type-check vert, build rouge). Garder les helpers purs (normalisation, Levenshtein…) **non exportés** (internes au module) ou les déplacer dans un `lib/*` séparé sans `"use server"`.
- **Cache Turbopack cache aussi les ERREURS de compile** (`turbopackFileSystemCacheForBuild` actif) : après correction d'une erreur source, un rebuild peut **toujours** échouer avec l'ancienne erreur (même ligne, déjà corrigée). Purger `rm -rf .next/cache` (⚠ `rm -f .next/cache` ne fait rien sur un dossier). Si des builds concurrents se chevauchent (orphelins `next build` racent sur `.next`), `rm -rf .next` pour repartir propre.
- **Tools — Assistant Tactique (RagAssistant) retiré des pages** (2026-06-01) : feature beta, `components/wiki/RagAssistant.tsx` conservé mais **plus aucun import/usage** (random-team, my-team, translator, comparateur, fiche perso). Ne pas le re-câbler sans validation.
- **`RandomTeamGenerator` init déterministe** : l'équipe initiale est générée via `generate(true)` (slice, pas `Math.random`) pour que SSR et 1er rendu client soient identiques → évite l'erreur d'hydratation React **#418**. Un `useEffect` mount-only rebrasse ensuite aléatoirement. Ne pas remettre `useState(() => generate())` aléatoire (réintroduit #418).

<!-- BEGIN:nextjs-agent-rules -->
# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` before writing any code. Heed deprecation notices.
<!-- END:nextjs-agent-rules -->
