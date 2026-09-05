<p align="center">
  <img src="public/logo-og.png" alt="Azalée" width="100" />
</p>

<h1 align="center">Azalée</h1>

<p align="center">
  Base de données communautaire pour <strong>Inazuma Eleven: Victory Road</strong>
  <br />
  <a href="https://azalee.rosegriffon.fr"><strong>azalee.rosegriffon.fr</strong></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Next.js-16-000000?logo=next.js&logoColor=white" alt="Next.js" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?logo=tailwindcss&logoColor=white" alt="Tailwind" />
  <img src="https://img.shields.io/badge/Supabase-PostgreSQL-3FCF8E?logo=supabase&logoColor=white" alt="Supabase" />
  <img src="https://img.shields.io/badge/Better_Auth-1.4-FF6B35?logoColor=white" alt="Better Auth" />
</p>

---

## Contenu de la base

<table>
  <tr>
    <td align="center"><strong>5 930</strong><br />Personnages</td>
    <td align="center"><strong>691+</strong><br />Techniques</td>
    <td align="center"><strong>1 000+</strong><br />Objets</td>
    <td align="center"><strong>200+</strong><br />Auras</td>
    <td align="center"><strong>114</strong><br />Équipes</td>
    <td align="center"><strong>54</strong><br />Tactiques</td>
  </tr>
</table>

> 7 raretés : Normal → En progression → Expérimenté → Émérite → Légendaire → Héros → BASARA

## Stack

| Couche          | Technologie                                                                                                                   |
| :-------------- | :---------------------------------------------------------------------------------------------------------------------------- |
| Framework       | **[Next.js 16](https://nextjs.org/docs)** (App Router, Turbopack, standalone Docker)                                          |
| UI              | **[React 19](https://react.dev)**, [Tailwind CSS v4](https://tailwindcss.com/docs), **Material Design 3** (Shadcn/ui + Radix) |
| Auth            | **[Better Auth 1.4](https://www.better-auth.com/docs)** (OAuth Google + Discord, pont JWT Supabase)                           |
| Base de données | **[Supabase](https://supabase.com/docs)** (PostgreSQL, 23 tables, RLS 23/23)                                                  |
| Données jeu     | `@rosegriffon/inagle` (parser binaire Victory Road)                                                                          |
| Éditeur         | **[Lexical](https://lexical.dev/docs/intro)** (articles, patch notes)                                                         |
| Canvas          | **[Konva](https://konvajs.org/docs/)** + React-Konva (outils visuels)                                                         |
| Recherche       | **[uFuzzy](https://github.com/leeoniya/uFuzzy)** + **[cmdk](https://cmdk.paco.me/)** (palette de commandes)                   |
| Notifications   | **Web Push** (VAPID)                                                                                                          |
| Images          | **[Sharp](https://sharp.pixelplumbing.com/)** + CloudFront (zukan)                                                            |
| Japonais        | **[Wanakana](https://wanakana.com/docs/global.html)** (romaji ↔ kana)                                                         |

## Pages principales

| Route                   | Description                                         |
| :---------------------- | :-------------------------------------------------- |
| `/`                     | Accueil (hero, carousel wiki, actualités)           |
| `/chara/[id]`           | Fiche personnage (stats, techniques, rareté, zukan) |
| `/skill/[id]`           | Fiche technique (vidéo, description FR/EN/JA)       |
| `/item/[id]`            | Fiche objet (stats, recettes d'échange)             |
| `/aura/[category]/[id]` | Fiche aura (Esprits Guerriers, Souls, MixiMax)      |
| `/tactic/[id]`          | Fiche tactique                                      |
| `/teams`                | Liste des équipes                                   |
| `/search`               | Recherche globale avec filtres                      |
| `/tools/*`              | Comparateur, équipe aléatoire, traducteur           |
| `/news`                 | Actualités, patch notes traduits                    |
| `/dashboard/*`          | Admin (articles, import, zukan review)              |

## Pipeline de données

Les données sont extraites des fichiers binaires du jeu et enrichies en 13 étapes :

```
generate-entries.ts          # Extraction des entries brutes
    ↓
deploy-core.ts               # Import vers Supabase
    ↓
match-characters.ts          # 5 930/5 930 matchés (Google Sheet + growth table)
match-basara.ts              # 64 BASARA
match-items.ts               # 337 objets
match-skills.ts              # 691 techniques
match-teams.ts               # 114 équipes
match-heroes.ts              # 249 Héros
match-hyper-moves.ts         # 98 hyper moves
    ↓
update-sheet-data.ts         # Push stats + metadata
extract-constellations.ts    # 4 665 constellations
map-zukan-images.ts          # 5 475 images CloudFront (92%)
update-item-prices.ts        # 2 587 prix
fix-slugs.ts                 # Slugs uniques (name-0xHEXID)
update-exchange-costs.ts     # 1 652 items + 685 skills
```

## Développement

```bash
bun install            # à la racine du monorepo
bun run azalee:dev     # http://localhost:3000
```

<details>
<summary><strong>Variables d'environnement</strong></summary>

```env
NEXT_PUBLIC_SUPABASE_URL=
NEXT_PUBLIC_SUPABASE_ANON_KEY=
SUPABASE_SERVICE_ROLE_KEY=
SUPABASE_JWT_SECRET=
# chaine de connexion Postgres (Better Auth) :
<POSTGRES_URL>=
BETTER_AUTH_SECRET=
BETTER_AUTH_URL=
DISCORD_CLIENT_ID=
DISCORD_CLIENT_SECRET=
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
NEXT_PUBLIC_VAPID_KEY=
VAPID_PRIVATE_KEY=
```

</details>

## Commandes

Toutes en **Bun** — jamais `npm`, `pnpm` ni `yarn`.

| Commande | Description |
| --- | --- |
| `bun run dev` | serveur de développement (Turbopack) |
| `bun run build` | build de production (standalone) |
| `bun run lint` | oxlint puis eslint |
| `bun run type-check` | `tsc --noEmit` |
| `bun test src tests` | tests de l'app |
| `bun run sync:inagle` | synchronise les données inagle vers Supabase |
| `bun run backup:supabase` | dump Supabase → SQLite |

Les données du jeu s'interrogent par le CLI de la bibliothèque :
`bun packages/azalee/src/cli.ts --help`.

## Déploiement

Auto-hébergé sur le VPS, derrière nginx :

```bash
bash scripts/ship-azalee.sh     # build → standalone → azalee-web.service (:3003)
```

Le script ignore le code de sortie 132 de Next 16 sous Bun (crash à la sortie
du processus **après** un build réussi). Procédures complètes, diagnostic et
interdits : [`docs/deploy.md`](../../docs/deploy.md).

## Licence

Repository privé — Les assets Inazuma Eleven sont la propriété de Level-5.
