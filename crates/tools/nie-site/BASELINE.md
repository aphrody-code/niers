# Baseline de `nie-site`

Mesures **rejouables**, pas des objectifs. Elles servent à voir une régression, pas à se
féliciter d'un chiffre.

```
cargo bench -p nie-site
```

## Routage, 2026-09-05 — VPS, profil `bench`

Le bench appelle le routeur en mémoire : index VFS injecté, aucun disque, aucun réseau, aucun
amont. Il mesure donc ce que la crate contrôle — reconnaissance de route, extraction de chemin,
sérialisation — et rien de l'environnement.

| Route | Médiane | Intervalle |
|---|---|---|
| `/healthz` | 43,09 µs | 42,62 – 43,55 |
| `/api/v1/health` | 49,21 µs | 48,55 – 49,98 |
| `/api/v1/textures` | 62,93 µs | 62,04 – 63,88 |
| `/api/v1/textures?page=100` | 59,18 µs | 58,34 – 60,33 |
| `/b/<préfixe>` (parcours) | 68,96 µs | 67,32 – 70,88 |
| `/robots.txt` | 43,07 µs | 42,32 – 43,92 |
| `/sitemap.xml` | 41,54 µs | 41,05 – 42,10 |
| Construction d'un index de 20 000 chemins | 7,76 ms | 7,56 – 7,99 |

## Ce que ces nombres disent, et ce qu'ils ne disent pas

**La pagination ne coûte rien.** La page 100 n'est pas plus lente que la page 1 (59 contre
63 µs, intervalles qui se recouvrent) : le découpage se fait par index, pas en parcourant les
éléments précédents. Une régression ici se verrait comme un écart qui grandit avec le numéro de
page.

**Le routage n'est pas le facteur limitant.** À ~50 µs, il est deux à trois ordres de grandeur
sous le TTFB mesuré de bout en bout (`scripts/e2e-site.sh` : 0,5 à 0,6 ms sur les catalogues).
Optimiser le routeur n'améliorerait rien de perceptible ; c'est l'accès aux données et le réseau
qui décident.

**L'index se construit en 7,8 ms pour 20 000 chemins**, soit ~100 ms extrapolés pour les
255 308 entrées réelles. Le montage observé en conditions réelles prend 1,16 s : l'écart est
l'énumération du VFS, pas l'indexation. C'est pourquoi il se fait en tâche de fond et que
`/healthz` répond avant lui.

## Comparer plus tard

`criterion` conserve la mesure précédente dans `target/criterion/` et affiche l'écart au
passage suivant. Un `cargo bench` sur une machine chargée rend des chiffres plus élevés sans que
rien n'ait changé : lire l'écart relatif, jamais la valeur absolue d'un seul passage.
