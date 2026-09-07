# Plan de session — moteur, avatars et publication

> Plan actif : [`../PLAN.md`](../PLAN.md). Cette annexe décrit uniquement le lot 3D/avatar et
> doit y reporter toute nouvelle mesure ou décision.

Dernière consolidation : 5 septembre 2026. Les cases ne sont cochées qu'après vérification.

## Règles et arbitrages

- Réutiliser Explorer, son éditeur, nie-render3d, les parseurs WASM et l'atelier avatar existants. Ne pas créer une nouvelle application parallèle.
- L'objectif récent d'un vrai logiciel d'édition remplace la copie de l'écran du jeu ; la fidélité des personnages reste un critère distinct, sans prétendre à une identité pixel-perfect non mesurée.
- DirectX 12 sous Windows, WebGPU sur le web/WASM, Vulkan ou OpenGL sous Linux. Compiler pour WASM ne prouve pas le fonctionnement WebGPU dans un navigateur.
- Préserver les travaux présents ; commit/push et synchronisation VPS aux jalons comme explicitement demandé. Ne pas publier les références privées ni les secrets.
- Demande « 200 img » : aucune occurrence exacte identifiée dans le code/VFS pour l'instant ; ne pas inventer de convention. Atlas 2D existants et sélection jusqu'à 200 images traités séparément.

## Ordre d'exécution

1. Références
   - [x] Récupérer et vérifier toutes les images du message Discord serveur 1544475258591907961 / salon 1544482971934007336 / message 1545590117895250101 vers astro : neuf JPEG, HTTP 200, SHA-256 concordants poste/VPS. Références privées hors Git public.
   - [ ] Vérifier le lot Zukan déjà téléchargé (16 images : huit portraits et huit corps entiers), manifeste et correspondance des angles.
2. Imports dans l'avatar existant
   - [ ] Images PNG/JPEG/WebP, planches Chara à grille explicite, atlas G4TX via WASM et rectangles JSON NIE ; aperçu, animation et export.
   - [ ] GLB, glTF et ressources locales, gzip, compression de maillage ; erreurs explicites pour formats non pris en charge.
   - [ ] Vérification fichiers invalides, tailles, ressources manquantes, annulation, nettoyage mémoire et tests navigateur.
   - [x] Script de test des liaisons texture/pièce/icône : 502 pièces, 491 icônes uniques, CRC source et comparaison pixels entre URL UI et icône canonique ; 0 erreur mesurée le 5 septembre 2026.
   - [x] Réutiliser les vrais composants @rosegriffon/ui pour boutons, listes, champs et inspecteur ; disposition responsive, bibliothèque de vignettes, scène centrale et inspecteur.
   - [ ] Vérifier chaque sélection UI vers la bonne recette et le bon assemblage, pas seulement la présence des images.
3. Astro
   - [ ] Examiner les références réellement reçues, produire un modèle 3D fidèle avec les outils locaux existants.
   - [ ] Textures, pose/squelette selon références, export GLB et import réel dans le créateur ; preuves visuelles.
4. Pipeline personnages et interfaces
   - [ ] Revalider les corrections héritées de Claude : matériaux par primitive, yeux/visage sans contamination des cheveux, teinte peau, pose, face/dos, numéro et accessoires.
   - [ ] Consolider NieModelServer et NieRender3D avec tests de régression.
   - [ ] Revalider le viewer réutilisé de l'éditeur dans l'aperçu Explorer, pas une simple vidéo.
   - [ ] Galerie /modeles/chara et détail sans erreurs sur le corpus validé ; annoncer la portée mesurée, pas « zéro erreur » universel sans audit.
   - [ ] Intégrer ou écarter explicitement le rendu cel expérimental après comparaison aux références.
5. Moteur partagé
   - [ ] Raccorder la surface native DX12 aux composants existants ; ne pas poursuivre un studio parallèle.
   - [ ] Présentation WebGPU réelle depuis WASM, interactions et ressources partagées avec le créateur/galerie.
   - [ ] Vérifier compilation Linux et backends Vulkan/OpenGL sur environnement disponible ; séparer compilation et lancement GPU.
6. Compétences
   - [x] Agent skill-creator : trois compétences NIE de création 3D, textures/assemblage et peaufinage, inspirées des outils Blender/Game Development Studio existants ; quick_validate et références vérifiés.
   - [x] Valider les compétences et leur disponibilité dans le plugin NIE, préserver les intégrations existantes : cache installé et activé 0.1.0+codex.20260905003718, trois skills présents.
   - [x] Workflow de publication enregistré via la note demandée (commit/push, synchronisation VPS, rebuild/déploiement, services et CDN ; respecter les demandes plus récentes).
7. Livraison
   - [ ] Tests ciblés, compilation, lancement réel et captures ; corriger les erreurs rencontrées.
   - [ ] Commit/push du code vérifié, pull de niers et rg sur VPS sans écraser de modifications.
   - [ ] Rebuild des composants concernés, déploiement Azalée, redémarrages des services concernés et invalidation/versionnement des caches CDN.
   - [ ] Contrôles publics, versions locales/distantes concordantes et bilan des limites restantes.

## Incrément en cours

Livraison urgente demandée le 5 septembre : commit local, push main, synchronisation VPS puis déploiement. Les imports PNG et GLB/gzip ont été exercés en navigateur ; 35 tests ciblés passent. L'audit réel couvre 502 sélections et 491 icônes, sans erreur de hash, pixels ou ressource de requête. TSC ciblé et Clippy nie-render3d/nie-wasm sans diagnostic. Le pont WebGPU facultatif est compilé en WASM avec quatre tests ; son intégration aux interfaces et sa validation GPU navigateur ne sont pas terminées. Aucun modèle Astro fidèle n'est encore produit. Ne pas confondre la livraison de cet incrément avec l'achèvement de ces objectifs restants.
