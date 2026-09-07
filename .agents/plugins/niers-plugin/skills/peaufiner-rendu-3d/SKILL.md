---
name: peaufiner-rendu-3d
description: Diagnostiquer et peaufiner un rendu 3D NIE par captures comparables et corrections ciblées de géométrie, UV, matériaux ou cadrage. À utiliser pour artefacts visuels, raccords, textures incorrectes et vérification avant/après ; pas pour promettre une fidélité sans référence.
---

# Peaufiner avec des preuves visuelles

Lire [les outils de capture locaux](../creer-assets-3d/references/outils-locaux.md). Choisir le consommateur réellement visé par la demande. Un aperçu Blender, un PNG CPU et le viewport GPU de l’éditeur constituent trois observations différentes.

## Boucle de travail bornée

1. Décrire le défaut observable et conserver une capture initiale avec le modèle exact, son empreinte, la pose, la caméra, la résolution, l’éclairage, le backend et la version du code. Choisir un critère de fin concret : raccord fermé, atlas correctement attribué, silhouette attendue ou absence de texture manquante.
2. Reproduire le défaut avec la plus petite vue utile. Pour une comparaison avant/après, conserver les mêmes paramètres. Si le cadrage ou la normalisation automatique dépend de la boîte englobante modifiée, signaler ce changement : une caméra apparemment identique ne garantit plus une projection comparable.
3. Formuler une hypothèse et un test qui la distingue d’une autre cause : mauvaise texture ou UV incorrects, transformation ou pose de repos, normale ou éclairage, alpha ou ordre de dessin. Utiliser les pièces jointes de profondeur/normales/identifiants seulement si le moteur les fournit réellement.
4. Si la demande autorise la correction, modifier la plus petite cause identifiée et produire un candidat séparé. Si elle demande seulement un diagnostic, conserver la proposition de correction sans l’appliquer. Revenir à `../assembler-modeles-textures/SKILL.md` lorsque la cause vient des associations de pièces.
5. Recapturer avec les contrôles initiaux, examiner les images directement et vérifier une seconde vue exposant une régression possible. Si une métrique est utile, préciser outil, paramètres, région comparée et seuil ; une amélioration globale peut masquer un visage dégradé.
6. Terminer dès que le critère convenu est satisfait. Si deux essais successifs n’apportent pas de preuve nouvelle, réexaminer l’hypothèse et signaler la limite avant d’engager une série plus large. Ne pas multiplier les relances de Blender ou les galeries sans gain diagnostique.

## Interpréter et livrer

Une capture exécutée doit être ouverte et examinée avant d’être décrite comme vérifiée visuellement. Un contrôle de dimensions, un code retour nul ou une statistique de pixels ne remplace pas cette observation.

Séparer les constats, les causes confirmées et les hypothèses restantes. Avec une référence du jeu, préciser sa provenance et les différences de scène/caméra qui empêchent une comparaison stricte. Sans référence comparable, parler de cohérence ou de correction du défaut observé, jamais de résultat pixel-perfect. Le contrôle CPU/GPU `--verify` de `nie-render3d` mesure la silhouette ; il n’atteste pas une fidélité globale au jeu.

Rendre le candidat, les captures avant/après, les paramètres de reproduction et le bilan des critères. Signaler explicitement si l’éditeur cible n’a pas été ouvert ou si une partie du modèle n’a pas pu être observée. Ne pas élargir ce travail à une refonte du moteur ou à un déploiement non demandé.
