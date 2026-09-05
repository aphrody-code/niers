# Azalée — Wiki Inazuma Eleven: Victory Road — Référence complète

> Azalée est le wiki francophone de référence pour Inazuma Eleven: Victory Road (IEVR), le jeu de LEVEL-5. Maintenu par l'association Rose Griffon, il agrège personnages, techniques, objets, auras, tactiques, compétences passives et actualités du jeu, avec des outils communautaires.

Wiki : https://azalee.rosegriffon.fr
Association : Rose Griffon — https://rosegriffon.fr
Développeur & fondateur : yoyo — https://x.com/yoyo__goat
Langue : fr-FR
Généré le : 2026-09-05

## Présentation

Azalée recense les données de jeu d'Inazuma Eleven: Victory Road et propose des
fiches détaillées ainsi que des outils (comparaison, génération d'équipe,
traduction). Le wiki est produit et maintenu par Rose Griffon, association
française de la communauté Inazuma Eleven.

## Base de données

- [Personnages](https://azalee.rosegriffon.fr/chara) : fiches complètes des personnages jouables.
- [Techniques](https://azalee.rosegriffon.fr/skill) : tirs, dribbles, blocs et techniques de gardien.
- [Objets](https://azalee.rosegriffon.fr/item) : objets, équipements et consommables.
- [Auras](https://azalee.rosegriffon.fr/aura) : esprits guerriers, totems, miximax, éveils, changements de mode.
  - [Esprits guerriers](https://azalee.rosegriffon.fr/aura/esprits-guerriers)
  - [Totems](https://azalee.rosegriffon.fr/aura/totems)
  - [Miximax](https://azalee.rosegriffon.fr/aura/miximax)
  - [Éveil](https://azalee.rosegriffon.fr/aura/eveil)
  - [Changement de mode](https://azalee.rosegriffon.fr/aura/changement-mode)
- [Passifs](https://azalee.rosegriffon.fr/passive) : compétences passives.
- [Tactiques](https://azalee.rosegriffon.fr/tactic) : tactiques d'équipe.

## Actualités

- [News](https://azalee.rosegriffon.fr/news) : actualités du jeu et de la communauté.
- [Patch-notes](https://azalee.rosegriffon.fr/patch-notes) : notes de mise à jour.

## Outils

- [Outils](https://azalee.rosegriffon.fr/tools)
- [Comparateur](https://azalee.rosegriffon.fr/tools/compare)
- [Équipe aléatoire](https://azalee.rosegriffon.fr/tools/random-team)
- [Traducteur](https://azalee.rosegriffon.fr/tools/translator)
- [Recherche](https://azalee.rosegriffon.fr/search)

## API (pour les IA et outils)

Azalée expose une API GraphQL publique en lecture seule pour interroger la base de données du jeu.

- Endpoint GraphQL : POST https://azalee.rosegriffon.fr/api/graphql (Content-Type: application/json)
- Queries disponibles : characters, character, skills, skill, items, item, auras, ragSearch, tweets.
- Filtres skills(page, limit, q, category, element) ; characters(page, limit, q, element, position, rarity, team, series).
- Champs Skill : id, name { fr en ja }, description { fr en ja }, category, element, power, tension, image, sheetData. Le champ name est de type LocalizedString (sous-champs fr/en/ja).
- Recherche sémantique (RAG) : POST https://azalee.rosegriffon.fr/api/rag/search ; données formatées LLM : https://azalee.rosegriffon.fr/api/llm/<model>.

Exemple de requête (techniques de Tir) :
POST https://azalee.rosegriffon.fr/api/graphql
{"query":"{ skills(category: \"Tir\", limit: 5) { id name { fr en } category element power } }"}

Réponse (extrait) : { "data": { "skills": [ { "name": { "fr": "Feu tout-puissant" }, "category": "Tir", "element": "Feu", "power": "100-640" } ] } }

## Mentions légales

- [CGU](https://azalee.rosegriffon.fr/legal/cgu)
- [Confidentialité](https://azalee.rosegriffon.fr/legal/confidentialite)
- [Mentions légales](https://azalee.rosegriffon.fr/legal/mentions-legales)

## Crédits

Conception, développement et architecture : yoyo — https://x.com/yoyo__goat.
Stack : Next.js, React, Bun, TypeScript. Données : Inazuma Eleven: Victory Road (LEVEL-5).
