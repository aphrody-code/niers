# `@niers/bxc` — scrapping Inazuma, point d'entrée unique

Enchaîne en **une passe** le scrapping des épisodes, le balayage `iecrawl` des
sites officiels LEVEL-5, la mise à jour du catalogue et les annonces Discord ;
et fait tourner le bot **Wonderbot**.

```bash
bun --bun apps/bxc/src/cli.ts workflow --dry-run   # scrape, compare, n'écrit rien
bun --bun apps/bxc/src/cli.ts workflow             # la passe réelle
bun --bun apps/bxc/src/cli.ts wonderbot doctor     # vérifie l'installation
bun --bun apps/bxc/src/cli.ts wonderbot start      # le bot (ce que lance systemd)
```

## Les quatre étapes

| # | Étape | Qui la fait |
|---|-------|-------------|
| 1 | scrapping des épisodes | `@aphrody/ietv` — 4 chaînes YouTube, `inazuma-eleven.fr/tv`, Pluto TV `no`/`fr` |
| 2 | mise à jour du catalogue | `@aphrody/wonderbot` → base SQLite `IETV_CACHE_PATH` |
| 3 | `iecrawl` | `src/iecrawl.ts` — `inazuma.jp`, `www.inazuma.jp/victory-road/`, `zukan.inazuma.jp` |
| 4 | annonces | `@aphrody/wonderbot` — salon `WONDERBOT_ANNOUNCE_CHANNEL_ID` |

Les étapes 1 et 2 sont **un seul appel** (`Catalogue.rafraichir()`) : le paquet
amont scrape avant de remplacer la base, exprès — les découper reviendrait à
réécrire cette garantie.

## Ce qui n'est PAS dans le dépôt

Le moteur de navigation `@aphrody/bxc` reste une **dépendance du registre npm**
(0.9.6, dernière version publiée), celle que `@aphrody/ietv` et `@aphrody/zukan` réclament déjà. Le
binaire natif standalone BXC 0.9.7 est installé séparément dans
`%USERPROFILE%\\.bxc\\bin` ; voir [`docs/BXC-NATIVE.md`](../../docs/BXC-NATIVE.md).
Seuls les
paquets métier sont entrés dans niers : `packages/ietv`, `packages/ietv-client`,
`packages/wonderbot`, `packages/zukan`.

## Secrets

Jamais dans le dépôt. `~/.config/niers/wonderbot.env` (`chmod 600`), chargé par
`EnvironmentFile=` dans `niers-wonderbot.service`.
