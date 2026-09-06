# Documentation de niers

Vingt documents, un rôle chacun. Ce qui n'y est pas mesurable ou vérifiable n'y a pas sa place :
pas de journal, pas d'historique daté — l'état vient des outils, l'histoire vient de `git log`.

## La direction — un seul cap, quatre plans subordonnés

Les plans se sont multipliés ; ils ne se contredisent pas, ils ne disaient simplement pas
lequel commande. Voici l'ordre, du cap au geste :

| Rang | Document | Ce qu'il décide |
|---|---|---|
| **Le cap** | [PLAN-SITE-ULTIME.md](PLAN-SITE-ULTIME.md) | **L'état d'arrivée** : un seul site qui expose TOUT ce que le dépôt sait faire. Instrument unique — une matrice de couverture où chaque capacité est `servi`, `interne` (avec sa raison) ou `manquant`. Gate maîtresse : `manquant = 0`. |
| L'échéance | [../PLAN.md](../PLAN.md) | La bascule Azalée → Vercel et Aphrody sur `aphrody.com` : dates, gates chiffrées, rollback par journée |
| L'exécution | [CODEX-JOUR-UNIQUE.md](CODEX-JOUR-UNIQUE.md) | Ce que l'agent fait aujourd'hui, dans quel ordre, et à quoi on reconnaît que c'est fait |
| Le long terme | [PLAN.md](PLAN.md) | Le moteur et la forge : les deux faces, l'état chiffré, les priorités |
| Le gel | [stack/](stack/README.md) | Les décisions techniques figées, leurs versions, les alternatives rejetées |

**La règle qui les relie :** un plan qui n'avance pas la matrice de couverture n'avance pas le
projet. Un compte, une commande, un hôte, une date — sinon ce n'est pas fait.

## Commencer ici

| Document | Contenu |
|---|---|
| [PLAN-SITE-ULTIME.md](PLAN-SITE-ULTIME.md) | **Le cap** : couverture de toute la surface du dépôt vers un seul site |
| [../PLAN.md](../PLAN.md) | **La semaine du 2026-09-05 au 2026-09-11** : Azalée sur Vercel, Aphrody sur `aphrody.com`, Inacord — jour par jour, trois agents, une gate qui compte par jour |
| [PLAN.md](PLAN.md) | **L'objectif et l'état chiffré** : les deux faces (moteur et forge), ce qui est mesuré, les priorités |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **La carte** : les quatre arbres, qui fait autorité sur quoi, les crates, les ponts, les fusions interdites |
| [FORGE.md](FORGE.md) | Produire `nie.exe` au byte près depuis le workspace — le juge du projet |
| [INSTALLATION.md](INSTALLATION.md) | Installer la CLI `niers` |
| [../PROVENANCE.md](../PROVENANCE.md) | D'où vient chaque arbre, ce qui a été écarté à la copie |

## Le moteur

| Document | Contenu |
|---|---|
| [STACK.md](STACK.md) | Les briques runtime, ce qui est écarté et pourquoi, les règles de la boucle et de Lua |
| [DESIGN.md](DESIGN.md) | Rendu pixel-perfect des écrans START et MENU, décomposition par couche |
| [AVATAR.md](AVATAR.md) | L'éditeur d'avatar (`chara_edit`) : composition, icônes, ce qui reste non prouvé |
| [BENCHMARKS.md](BENCHMARKS.md) | Banc d'essai inter-langages des hot paths |
| [PLAN-SESSION-3D.md](PLAN-SESSION-3D.md) | Le plan de travail en cours sur le moteur 3D, les avatars et la publication |
| [stack/README.md](stack/README.md) | **Stack gelée le 2026-09-05** : Azalée sur Vercel + Supabase Cloud, Aphrody (`aphrody.com`) servi par `nie-site` (Axum, 100 % Rust), Inacord et l'interface partagée `inacord-ui` ; mobile et Steam gelés hors semaine |

## Le binaire et ses données

| Document | Contenu |
|---|---|
| [RE.md](RE.md) | La cible `nie.exe`, la boucle de reverse, la couverture, ce que le RE a établi |
| [FORMATS.md](FORMATS.md) | Les formats Level-5 et Criware, et l'état du VFS |
| [modele-de-match.md](modele-de-match.md) | Le modèle tir/blocage/but : ce qui est résolu, ce qui reste opaque |
| [game-data/](game-data/) | Les familles `cfg.bin` décrites une par une |
| [nie-rtti-classes.txt](nie-rtti-classes.txt) | Les 1 234 classes RTTI extraites |
| [dll-exports/](dll-exports/) | Exports des DLL tierces (Steam, EOS, curl) |

## Le wiki, l'application et la production

| Document | Contenu |
|---|---|
| [FUSION.md](FUSION.md) | Pourquoi tout ce qui touche Inazuma Eleven vit dans ce dépôt, et comment les gisements s'y rejoignent |
| [AZALEE.md](AZALEE.md) | Le wiki : les trois choses que « Azalée » désigne, et leur rapport à niers |
| [MIGRATION-EXPLORATEUR.md](MIGRATION-EXPLORATEUR.md) | Le passage des outils et de la galerie du web vers l'explorateur de bureau |
| [EXPLOITATION.md](EXPLOITATION.md) | Ce qui tourne sur cette machine, d'où, et sous quel service |
| [ASTRO-LOR.md](ASTRO-LOR.md) | Astro Lor, personnage original : du wiki au jeu |

## Chantiers transverses

| Document | Contenu |
|---|---|
| [ORGANISATION.md](ORGANISATION.md) | **Où va quoi** : la structure du dépôt, prise sur `openai/codex`, et les écarts assumés |
| [ABSORPTION-IECODE.md](ABSORPTION-IECODE.md) | Rendre `csharp/` redondant en portant ce qu'il sait faire |
| [A2A-CODEX.md](A2A-CODEX.md) | Le protocole entre les agents qui codent ici en même temps |
| [EXPORT-APP.md](EXPORT-APP.md) | L'outil C++ d'export des icônes d'application (WebP + zstd) |
| [legal/](legal/) | L'accord commercial signé |

## Ailleurs dans le dépôt

`../CLAUDE.md` et `../AGENTS.md` (règles de travail) · `../apps/inacord/ROADMAP.md`
(app desktop) · `../plugins/niers-plugin/` (plugin et skills) · `../CHANGELOG.md`,
`../NOTICE`, `../SECURITY.md`.

Chaque arbre porte son propre README : [`../crates/`](../crates/README.md),
[`../packages/`](../packages/README.md), [`../apps/`](../apps/README.md),
[`../csharp/`](../csharp/README.md), [`../python/`](../python/README.md),
[`../scripts/`](../scripts/README.md), [`../deploy/`](../deploy/README.md),
[`../plugins/`](../plugins/README.md), [`../cmake/`](../cmake/README.md),
[`../third_party/`](../third_party/README.md), [`../supabase/`](../supabase/README.md).
