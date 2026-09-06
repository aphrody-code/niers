# Baseline de `nie-site`

Mesures **rejouables**, pas des objectifs. Elles servent à voir une régression, pas à se
féliciter d'un chiffre.

```
cargo bench -p nie-site
```

## Le bench mesurait la mauvaise chose — corrigé le 2026-09-05

Les chiffres publiés jusqu'ici (43 à 69 µs par route) étaient **faux par construction** :
`requete()` appelait `routeur(etat.clone())` *dans* la boucle mesurée, donc chaque itération
reconstruisait les 19 routes et leurs couches. Ils additionnaient une opération de démarrage
— une fois dans la vie du processus — au coût d'une requête, payé des millions de fois.

Le défaut est resté invisible tant que rien ne changeait du côté des couches. L'ajout de la
couche ETag et de la borne de débit l'a révélé d'un coup : le bench a annoncé **« +340 % sur
`/healthz`, la performance a régressé »**, alors que le coût par requête n'avait bougé que de
1,6 µs. C'est exactement le genre de chiffre qui fait annuler un bon changement.

Le routeur est désormais construit **une fois** et cloné par itération, et sa construction a son
propre bench. Les deux tableaux ci-dessous ne sont donc **pas comparables** aux valeurs
antérieures : elles mesuraient autre chose.

## Routage, 2026-09-05 — VPS, profil `bench`

Index VFS injecté, aucun disque, aucun réseau, aucun amont, routeur préconstruit. On mesure ce
que la crate contrôle sur le chemin d'une requête : reconnaissance de route, extraction,
traversée des couches, sérialisation.

| Route | Médiane | Intervalle |
|---|---|---|
| `/healthz` | 17,77 µs | 16,13 – 19,78 |
| `/api/v1/health` | 16,52 µs | 15,96 – 17,13 |
| `/api/v1/textures` | 34,55 µs | 32,99 – 36,57 |
| `/api/v1/textures?page=100` | 35,91 µs | 32,87 – 39,95 |
| `/b/<préfixe>` (parcours) | 43,45 µs | 40,37 – 47,47 |
| `/robots.txt` | 18,22 µs | 16,01 – 20,99 |
| `/sitemap.xml` | 41,00 µs | 35,54 – 47,75 |
| **Construction du routeur** (une fois au démarrage) | 171,70 µs | 162,43 – 183,23 |
| Construction d'un index de 20 000 chemins | 9,86 ms | 9,23 – 10,63 |

## Ce que coûte chaque couche, isolé

Mesuré sur un routeur d'une seule route JSON, pour que le coût de la couche ne se noie pas dans
celui de la route. C'est la seule forme qui répond à « combien coûte l'ETag ? ».

| Variante | Médiane | Surcoût |
|---|---|---|
| aucune couche | 1,59 µs | — |
| ETag conditionnel | 3,16 µs | **+1,57 µs** |
| borne de débit | 4,78 µs | **+3,19 µs** |
| les deux | 6,50 µs | **+4,91 µs** |

La borne coûte deux fois l'ETag : elle clone l'état à chaque requête (`from_fn_with_state`) et
consulte un cache `moka`, là où l'ETag ne fait qu'un `blake3` sur un corps déjà en mémoire.
Rapporté au TTFB mesuré de bout en bout (0,5 à 0,6 ms), les deux couches réunies pèsent **0,8 %**
— et l'ETag rend en échange les 304 mesurés plus bas.

## Ce que ces nombres disent, et ce qu'ils ne disent pas

**La pagination ne coûte rien.** La page 100 n'est pas plus lente que la page 1 (35,9 contre
34,5 µs, intervalles qui se recouvrent) : le découpage se fait par index, pas en parcourant les
éléments précédents. Une régression ici se verrait comme un écart qui grandit avec le numéro de
page.

**Le routage n'est pas le facteur limitant.** À ~20 µs, il est un à deux ordres de grandeur sous
le TTFB mesuré de bout en bout (`scripts/e2e-site.sh` : 0,5 à 0,6 ms sur les catalogues).
Optimiser le routeur n'améliorerait rien de perceptible ; c'est l'accès aux données et le réseau
qui décident.

**Les intervalles sont larges** (jusqu'à ±20 % sur `/sitemap.xml`), là où la mesure précédente
tenait dans ±2 %. Le VPS porte 18 services : lire l'écart relatif entre deux passages du même
jour, jamais la valeur absolue d'un passage isolé.

## Bande passante épargnée par les ETag — production, 2026-09-05

Relevé par `curl -w '%{size_download}'` contre l'instance du dépôt, puis rejoué avec
`If-None-Match`. Avant, **aucune** de ces douze réponses ne portait de validateur : chaque
revalidation renvoyait le corps entier.

| Réponse | 200 | 304 |
|---|---|---|
| `/api/v1/episodes` | 470 619 o | 0 o |
| `/api/v1/chara?per_page=200` | 54 211 o | 0 o |
| `/feed.atom` | 21 684 o | 0 o |
| `/api/v1/textures?per_page=200` | 20 659 o | 0 o |
| `/sitemap.xml` | 7 349 o | 0 o |
| `/llms-full.txt` | 4 019 o | 0 o |
| `/` (coquille) | 3 815 o | 0 o |
| `/robots.txt` | 1 372 o | 0 o |
| `/manifest.webmanifest` | 429 o | 0 o |
| `/api/v1/health` | 481 o | 0 o |
| `/healthz` | 179 o | 0 o |
| `/b` | 67 o | 0 o |

`/api/v1/episodes` porte l'essentiel de l'enjeu : c'est la porte par laquelle chaque Inacord
installé se met à jour, et un client déjà à jour qui interroge sans `?since=` téléchargeait
470 Kio pour n'apprendre aucune nouveauté.

**L'index se construit en 7,8 ms pour 20 000 chemins**, soit ~100 ms extrapolés pour les
255 308 entrées réelles. Le montage observé en conditions réelles prend 1,16 s : l'écart est
l'énumération du VFS, pas l'indexation. C'est pourquoi il se fait en tâche de fond et que
`/healthz` répond avant lui.

## Comparer plus tard

`criterion` conserve la mesure précédente dans `target/criterion/` et affiche l'écart au
passage suivant. Un `cargo bench` sur une machine chargée rend des chiffres plus élevés sans que
rien n'ait changé : lire l'écart relatif, jamais la valeur absolue d'un seul passage.
