---
name: creer-assets-3d
description: Créer ou retoucher localement un asset 3D pour NIE avec Blender, puis préparer son import dans l’éditeur existant. À utiliser pour modélisation, UV, matériaux et export GLB ; pour assembler des pièces du jeu, utiliser assembler-modeles-textures.
---

# Créer un asset 3D local

Produire l’asset demandé, sa source éditable et un aperçu vérifié. Respecter la direction artistique, la destination et le périmètre de la demande ; aucune génération payante ni clé API n’est nécessaire à ce workflow.

Lire [la carte des outils](references/outils-locaux.md) pour choisir les points d’entrée réellement présents. Lire aussi `../niers-monorepo/SKILL.md` avant une modification dans le dépôt. Pour un format du jeu, consulter `../ievr-terminologie/SKILL.md` et la fiche pertinente de `../formats-level5/SKILL.md`.

## Préparer et fabriquer

1. Identifier la source, l’usage final et le résultat attendu : objet isolé, accessoire, personnage ou scène. Réutiliser les assets disponibles. Fixer les unités, axes, silhouette, budget de géométrie et taille des textures selon cet usage ; distinguer les contraintes demandées des choix de travail.
2. Vérifier Blender et les opérations disponibles dans la session. Un manifeste d’extension ou une configuration MCP ne prouve pas qu’une instance répond. Si Blender est indisponible, poursuivre l’inspection locale possible et signaler précisément la partie non exécutée ; ne pas remplacer le workflow par un fournisseur distant.
3. Travailler dans un nouveau dossier de sortie convenu, conserver la source et enregistrer une copie `.blend`. Dans Blender, progresser par silhouette, proportions, topologie utile, UV, matériaux puis détails. Garder les pièces séparées tant que leurs transformations ou attributions sont encore incertaines.
4. Pour les retouches, conserver une capture initiale et faire une modification observable à la fois. Examiner coutures UV, normales, transparence, raccords et échelle. Préférer les matériaux et textures compatibles avec le lecteur cible ; ne pas supposer qu’un graphe de nœuds Blender s’exporte intégralement en GLB.
5. Exporter vers un nouveau GLB, le réouvrir et vérifier géométrie, textures et transformations dans le consommateur NIE choisi. Utiliser `../peaufiner-rendu-3d/SKILL.md` pour comparer le rendu, ou `../assembler-modeles-textures/SKILL.md` si le résultat doit rejoindre un assemblage.

## Livrer

Associer source éditable, export, textures nécessaires et captures à un relevé bref : chemins d’origine, empreintes SHA-256, transformations effectuées, attribution/licence connue et limites restantes. Une licence inconnue reste inconnue ; ne pas la transformer en SPDX supposé. Ne pas incorporer les assets du jeu à une publication de code.

Séparer ce qui a été créé, exporté, réimporté et réellement observé. Un GLB lisible ne prouve ni une animation correcte ni une fidélité au jeu. Arrêter lorsque les critères demandés sont satisfaits ou qu’une dépendance manque ; ne pas étendre automatiquement le projet.
