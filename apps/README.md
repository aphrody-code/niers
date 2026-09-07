# `apps/` — les applications

Une application a un point d'entrée qu'on lance (`bin`, `server.ts`, une fenêtre) ; une
bibliothèque va dans [`packages/`](../packages). Toutes partagent le lockfile de la racine.

| Application | Ce que c'est | Comment ça tourne |
|---|---|---|
| `azalee` | le site du wiki (Next.js 16.3.0-canary.37, App Router) | service `azalee-web`, déploiement bleu/vert |
| `nie-explorer` | explorateur / éditeur de bureau (Tauri : React + Rust) | `bun run tauri dev`, publié par `scripts/release-desktop.sh` |
| `nie-mcp` | serveur MCP `niers-game` — VFS, assets, KB RE, pilotage de l'explorateur | déclaré dans `.mcp.json` |
| `bxc` | passerelle vers `@aphrody/bxc` et workflow de scrapping unifié | `bun --bun apps/bxc/src/…` |
| `nie-bot` | le bot Discord du wiki | service `azalee-bot` |
| `cdn`, `cdn-variants` | service d'assets et ses variantes d'image | services `rg-cdn`, `cdn-variants` |
| `storage`, `realtime`, `rag-api` | le socle du wiki en Bun natif (stockage, temps réel, recherche vectorielle) | services `rg-storage`, `rg-realtime`, `rg-rag-embed` |

Les 18 services de production tournent sur le VPS Linux ; `systemctl` fait foi sur ce qui
est actif, pas ce tableau.

## `nie-explorer` — les pièges qui abattent l'application

`src-tauri` est **hors** du workspace Cargo et en **édition 2021** (le workspace est en
2024) : pas de let-chains, écrire des `if let` imbriqués.

- Une commande `#[tauri::command]` **synchrone** tourne sur le thread principal : tout
  `tokio::spawn` dedans panique (« there is no reactor running ») et, en contexte
  non-unwinding, **abat l'application** sans trace utile. Toute commande qui touche au
  VFS, à une tâche ou au disque doit être `async`.
- Une commande nouvelle demande **trois** gestes : `#[tauri::command] #[specta::specta]`,
  l'ajout à `invoke_handler`, puis
  `cargo run --bin export-bindings --features dev-bindings`. Sans le 2ᵉ ou le 3ᵉ, le front
  ne la voit pas.
- `bundle.resources` **conserve le chemin relatif déclaré** : `"resources/db/*.gz"` atterrit
  en `<resource_dir>/resources/db/`. Viser le mauvais chemin ne casse rien de visible — le
  paquet pèse son poids, la signature est valide, et la ressource n'est jamais lue.
- **Seul le lancement trouve ces bugs-là.** Ni `tsc`, ni clippy, ni le contrôle de taille
  du bundle ne voient une ressource jamais lue ou une table vide. Après un build, lancer
  l'exécutable et regarder ce qu'il a écrit dans son répertoire de données.

## Publier l'application de bureau

`scripts/release-desktop.sh <X.Y.Z>` fait tout et est idempotent. **Ne jamais rejouer ses
étapes à la main** : `bun run tauri build` seul produit des installeurs *non signés* à côté
de `.sig` périmés, que rien ne distingue et que l'updater refusera.
