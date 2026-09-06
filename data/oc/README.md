# data/oc/ — les personnages originaux

Un **OC** (*original character*) est un personnage qui n'existe dans aucun fichier
d'*Inazuma Eleven* : tout ce qui le concerne est produit ici.

Un dossier par personnage, nommé par son *slug* :

```
data/oc/
└── <slug>/
    ├── README.md        ce que le personnage est, d'où il vient, ce qui manque
    ├── manifest.json    ce qu'il faut produire pour que le JEU le connaisse — généré, jamais écrit à la main
    ├── game/            contrat machine : préfixes, formats, tables et runtime
    │   └── evidence/    mesures issues des dumps locaux, jamais des valeurs devinées
    ├── provenance/      d'où vient chaque original — versionné
    │   ├── SHA256SUMS   les empreintes, vérifiables par `sha256sum -c`
    │   └── *.json       journaux de récupération (ids, empreintes, dimensions)
    └── source/          les originaux — JAMAIS versionnés (œuvre de leur auteur)
```

Personnages présents : [`astro-lor/`](astro-lor/README.md).

Chaque personnage peut porter `game/character-contract.json`. Ce contrat ne remplace pas les
fichiers du jeu : il décrit les chemins VFS versionnés, les formats/magics vérifiés, les nœuds de
tables attendus et les valeurs encore à mesurer. `catalog.json`, produit par
`uv run scripts/donnees/oc-catalog.py --write`, inventorie récursivement les fichiers réellement
présents sous `data/oc`.

## Pourquoi ici, alors que `data/` est ignoré

`data/` porte les 111 Go de contenu de jeu © LEVEL-5 et est ignoré en entier.
`data/oc/` en est la **seule exception** : la doc, la provenance et les manifestes
sont du travail *du dépôt*, pas du contenu de jeu, donc ils se versionnent.

Git ne descend jamais dans un répertoire exclu : `!/data/oc/` seul ne suffirait pas.
`.gitignore` ré-inclut `/data/` lui-même, ré-exclut tout son contenu direct, puis
ré-inclut `/data/oc/` — et, plus bas, les `*.md` que la règle Markdown globale
reprendrait sinon en silence.

Seuls les **originaux** restent dehors (`/data/oc/*/source/`) : ils appartiennent à
leur auteur. Ce qui est versionné à côté dit d'où chaque fichier vient et ce qu'il
pèse, sans le distribuer.

## Ce qui n'est pas ici

| Quoi | Où |
|---|---|
| Dérivés publiés (portraits, planches réduites, pages) | `apps/azalee/public/oc/<slug>/` |
| Données du wiki (personnage, techniques, esprit, Mixi Max) | `scripts/donnees/<slug>-*.py` |
| Plan d'intégration au jeu et verrous | `docs/` |
