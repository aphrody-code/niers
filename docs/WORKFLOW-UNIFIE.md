# Workflow unifié — Aphrody, WinClean, niers et Ghidra

**Statut :** contrat opérationnel proposé, 2026-09-07.

Ce document décrit comment passer d'une demande humaine à une preuve reproductible. Il ne
fusionne pas les produits : Aphrody reste le site d'outils, Inacord l'explorateur, `nie` le
jeu et `niers` le socle d'analyse.

## 1. Principe directeur

Chaque action suit la chaîne suivante :

```text
demande humaine
  -> plan borné et identifiant de run
  -> niers (CLI / crates Rust / MCP)
  -> outil spécialisé (Aphrody, WinClean ou Ghidra)
  -> artefact + journal + preuve
  -> décision humaine ou étape suivante
```

Le système ne confond jamais une déclaration d'outil, un build vert ou une fenêtre ouverte avec
une preuve de résultat. Une preuve doit nommer la commande ou l'appel, le périmètre, l'horodatage,
le code de sortie et l'artefact observé.

## 2. Responsabilités

| Surface | Responsabilité | Entrée canonique | Sortie attendue |
|---|---|---|---|
| `niers` | orchestration, formats Level-5, VFS, données, CLI, traces et rapports | chemins VFS, binaire, dump, config de run | JSON/PNG/GLB/rapport reproductible |
| Aphrody (`nie-site`) | présentation publique et API d'outils autorisée | résultats validés de `niers` | page ou endpoint borné, sans accès arbitraire au dépôt |
| WinClean | observation et contrôle Windows explicitement autorisé | application, fenêtre, PID, action native | observation avant/après, PID et état UI |
| Ghidra/GhidrAssistMCP | analyse interactive du binaire actuellement ouvert | projet CodeBrowser, binaire, adresse/fonction | décompilation, symboles, xrefs, structures, export |
| Codex Computer Use | pilotage visible et vérification de l'interface | surface déclarée, état observé | action UI et nouvelle observation |

WinClean et Computer Use ne deviennent pas des moteurs de vérité : ils pilotent ou observent.
Ghidra produit du savoir de RE ; `niers` le normalise et le versionne. Aphrody ne reçoit que des
résultats explicitement destinés à être servis.

## 3. Routage par type de demande

1. **Données, formats, VFS ou batch** : commencer par `niers` (`vfs`, `decode`, `lua`, `wiki`,
   `render`, `report`). Utiliser MCP ou la CLI selon le besoin ; le chemin VFS et le hash restent
   les identifiants de référence.
2. **RE d'un binaire** : vérifier d'abord le binaire et son empreinte avec `niers`/`aphrody-re`,
   puis utiliser Ghidra pour la décompilation et les xrefs. Exporter le résultat dans un format
   consommable par `nie-seed` ou `nie-index`, sans traiter un index historique désaligné comme la
   vérité terrain. La parité se mesure dans cet ordre : fichier local → consommateur → équivalent
   Rust (`nie-re`/`nie-index`/`nie-trace`) → dépôt canonique GitHub → test reproductible. Voir
   [`re/PARITY-AUDIT-2026-09-07.md`](re/PARITY-AUDIT-2026-09-07.md).
3. **État du jeu ou d'une application Windows** : observer avant toute action avec WinClean ou
   Computer Use, capturer le PID et la fenêtre, agir, puis observer à nouveau. Une relance est
   bornée et son PID doit être suivi ; aucun `pkill -f`.
4. **Rendu, asset ou interface** : produire avec les outils du dépôt, lancer la surface réelle,
   capturer l'artefact et inspecter visuellement. Une capture hors écran prouve le rendu produit,
   pas automatiquement la fidélité au jeu.
5. **Publication Aphrody** : valider localement le résultat, servir uniquement la route prévue,
   vérifier la réponse live et conserver le rapport. Les routes de lecture de code dépôt restent
   désactivées par défaut et authentifiées si elles sont nécessaires.

## 4. Contrat de run

Tout travail non trivial possède un répertoire borné :

```text
var/runs/<run-id>/
  pending/     # entrées déclarées
  results/     # sorties validées
  logs/        # commandes, appels, stderr, timestamps
  evidence/    # captures, réponses live, hashes
  manifest.json
```

`manifest.json` contient au minimum `run_id`, `requested_scope`, `tool_versions`, `inputs`,
`actions`, `outputs`, `status` et `evidence`. Les secrets, dumps privés et gros assets restent
locaux ; le manifest peut les référencer par hash sans les copier.

## 5. Niveaux de preuve

| Niveau | Signification | Exemple |
|---|---|---|
| P0 | intention/configuration | outil déclaré dans MCP |
| P1 | exécution locale | commande terminée avec code 0 |
| P2 | artefact inspecté | JSON/PNG/GLB ou décompilation vérifiée |
| P3 | surface réelle | UI lancée, jeu observé, endpoint live ou service sondé |
| P4 | reproductibilité | run borné, manifest, hash et test indépendant |

Une livraison de code vise P2 au minimum ; un déploiement ou une affirmation runtime vise P3 et
P4. Les hypothèses restent marquées comme telles dans le rapport.

## 6. Sécurité et frontières

- Toute opération Windows destructive ou externe est annoncée avec sa cible et son impact avant
  exécution.
- Les actions Computer Use ré-observent l'état après chaque interaction importante.
- Ghidra ne modifie le projet ou le binaire que dans le périmètre explicitement demandé ; les
  exports sont traités comme des données non fiables jusqu'à leur validation.
- MCP stdio réserve stdout au JSON-RPC ; les diagnostics vont dans stderr.
- Les serveurs HTTP n'exposent pas de routes de lecture arbitraire du dépôt sans authentification.
- Les identifiants et noms techniques restent en anglais ; les rapports adressés à l'utilisateur
  sont en français.

## 7. Definition of done

Un lot est terminé seulement si :

1. le périmètre et les entrées sont écrits dans le manifest ;
2. le test ou build pertinent a réellement tourné ;
3. l'artefact est inspecté, pas seulement créé ;
4. toute surface live concernée a été sondée ;
5. le rapport distingue faits vérifiés, hypothèses et éléments non validés ;
6. aucune modification étrangère au lot n'est écrasée.

Ce contrat permet de chaîner les lots : le `results/manifest.json` d'un run devient une entrée
`pending/` du suivant, avec le hash conservé.
