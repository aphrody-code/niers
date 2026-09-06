# astro-lor — le dossier d'Astro Lor

Astro Lor est un **personnage original** (OC) inséré dans l'univers d'*Inazuma Eleven*.
Il n'existe dans aucun fichier du jeu : tout ce qui le concerne est produit ici.

Ce dossier tient les **originaux** et leur **provenance**. Il ne tient ni les
dérivés publiés, ni les données, ni le plan — chacun a sa place, dite plus bas.

---

## Structure

```
data/oc/astro-lor/
├── README.md                      ce fichier
├── manifest.json                  ce qu'il faut produire pour que le jeu le connaisse
├── game/                          contrat VFS/cfg.bin/Lua/nie.exe machine-lisible
│   ├── character-contract.json    préfixes, formats, chemins et gates d'intégration
│   └── README.md                  règles de séparation source / jeu
├── provenance/                    d'où vient chaque original — versionné
│   ├── SHA256SUMS                 les 12 originaux, vérifiables
│   └── discord-<id message>.json  journal de récupération : ids, empreintes, dimensions
└── source/                        les originaux — JAMAIS versionnés
    ├── sheets/                    9 planches de character design (JPEG 4161×3000)
    └── comic/                     3 pages de bande dessinée (WebP 914×1280)
```

Pourquoi un dossier versionné sous `data/`, qui est ignoré en entier :
[`data/oc/README.md`](../README.md).

## Droits

Character design et planches : **@Karumina_san**. Les originaux sont son œuvre.

`source/` est exclu du dépôt (`.gitignore`). Ce qui reste versionné à côté —
la provenance et les empreintes — dit d'où chaque fichier vient et ce qu'il pèse,
**sans le distribuer**.

Les dérivés publiés, eux, sont versionnés : voir plus bas, et la raison qui va avec.

## Provenance

Les **9 planches** viennent d'un message Discord, récupéré par le bot
`niers-wonderbot`. Le journal `provenance/discord-<id>.json` porte, pour chaque
image, son identifiant Discord, son empreinte sha256, ses dimensions et son poids.

Les **3 pages de bande dessinée** viennent d'URL fournies à la main. Elles n'ont pas
de journal de récupération : leur trace est dans `provenance/SHA256SUMS`.

Rien ne garantit que ce dossier soit **complet** : il contient ce qui a été
transmis, pas nécessairement tout ce qui existe.

### Vérifier que rien n'a bougé

```bash
cd data/oc/astro-lor/source && sha256sum -c ../provenance/SHA256SUMS
```

Douze lignes `OK` attendues. Les empreintes des neuf planches recoupent celles du
journal Discord — la chaîne se vérifie de bout en bout.

## Ce qui n'est pas ici, et où c'est

| Quoi | Où | Pourquoi là |
|---|---|---|
| Dérivés publiés (portraits 512×512, planches 1600 px, pages BD) | `apps/azalee/public/oc/astro-lor/` | le site les sert ; sans eux les pages cassent, donc ils sont versionnés |
| Données du wiki (personnage, techniques, esprit, Mixi Max) | `scripts/donnees/astro-lor-oc.py`, `astro-lor-auras.py` | scripts rejouables, en `ON CONFLICT DO UPDATE` |
| Plan d'intégration au jeu et verrous | `docs/ASTRO-LOR.md` | c'est de la documentation, pas de la donnée |
| Récupération Discord | `scripts/donnees/astro-lor-planches-discord.py` | le dépôt range ses scripts dans `scripts/` |

Les dérivés sont volontairement de basse résolution : ils suffisent à l'affichage
et ne remplacent pas les originaux. Leurs noms de fichiers sont restés ceux que
le site sert déjà (`planche-og-tenue-jaune.webp`, …) : les renommer casserait des
URL publiées.

## Commandes

```bash
# Régénérer le manifeste d'assets (interroge le VFS ; quelques minutes)
uv run scripts/donnees/astro-lor-manifest.py
jq '.resume' data/oc/astro-lor/manifest.json

# Vérifier l'intégrité des originaux
cd data/oc/astro-lor/source && sha256sum -c ../provenance/SHA256SUMS

# Récupérer à nouveau depuis Discord — dans un dossier de TRAVAIL, pas dans source/
uv run scripts/donnees/astro-lor-planches-discord.py /tmp/astro-brut
```

## Deux pièges déjà payés

**Ne pas rejouer la récupération Discord dans `source/`.** Le script nomme ce
qu'il rapporte `attachment-<id>-<empreinte>.jpg`, pas `01-og-outfit-yellow.jpg` :
seul un regard humain sait ce que montre une planche. Un rejeu dans `source/`
y déverse douze doublons sous leur nom brut — c'est arrivé, et il a fallu les
retrouver par empreinte pour les distinguer des vrais. Le script exige désormais
un dossier de sortie explicite, sans valeur par défaut.

**Se méfier de l'extension.** Deux des pages de bande dessinée sont arrivées en
`.jpg` alors que leur contenu est du WebP — Discord sert en WebP ce qu'on lui a
donné en JPEG. Vérifier avec `identify` ou `file`, pas avec le nom.

## État

Le wiki connaît Astro : fiche complète sur `/chara/astro-lor`, au même niveau
qu'un personnage du jeu.

Le jeu, lui, ne le connaît pas. `manifest.json` dit ce qui manque, et
`docs/ASTRO-LOR.md` pourquoi — les deux affirmations sont vraies en même temps.

Le contrat d'intégration séparé dans [`game/character-contract.json`](game/character-contract.json)
nomme les deux codes internes (`c99019010`, `c99019020`), leurs chemins VFS et les formats
attendus par le runtime. Les hashes encore inconnus restent `null` jusqu'à lecture des tables
réelles ; aucun zéro de remplissage n'est une identité de personnage.
