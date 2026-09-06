---
name: niers-autonome
description: Mode exécutant autonome pour le dépôt niers — travail en boucle continue sans sortie texte pour l'utilisateur, orchestration multi-agents (ultracode/workflow) et enchaînement automatique des objectifs jusqu'à épuisement du budget. À déclencher sur /niers-autonome, ou quand l'utilisateur dit « autonome », « ne t'arrête pas », « enchaîne », « jusqu'au bout », « épuise les crédits ».
---

# niers — mode autonome

Ce mode s'applique au dépôt `niers` (réécriture byte-perfect d'*Inazuma Eleven: Victory Road*).
Il durcit `CLAUDE.md` : exécution continue, aucune question, aucune confirmation.

## Règle de sortie

**Ne rien écrire à l'utilisateur entre deux actions.** Pas de préambule, pas d'annonce
d'intention, pas de récapitulatif intermédiaire, pas de « je vais maintenant… ». Le travail se
lit dans les commits et les tests, pas dans le fil.

Trois exceptions, et seulement trois — les taire produirait un dommage ou un mensonge :

1. **Blocage dur** — la boucle ne peut plus produire (identifiant manquant, service tiers
   indisponible, décision hors périmètre technique). Dire quoi, en deux phrases, et s'arrêter.
2. **Action irréversible ou sortante non couverte** — force-push, réécriture d'historique,
   suppression de données, publication externe. Demander avant.
3. **Fin de boucle** — un rapport final, chiffré, incluant ce qui a échoué ou n'a pas été fait.

Ne jamais taire un échec pour préserver le silence : un test rouge, une étape sautée ou une
vérification impossible se disent, même en mode muet. Le silence porte sur le commentaire, pas
sur les résultats.

## Boucle

```
lire l'état (docs/PLAN.md, docs/FORGE.md, apps/nie-explorer/ROADMAP.md, git log)
  → choisir la cible la mieux chiffrée
  → implémenter
  → vérifier (clippy 0 warning, cargo test, bun run typecheck, bun run test)
  → git add + commit + push sur main
  → mettre à jour le plan
  → recommencer
```

Choisir la cible **par le chiffre**, jamais par intuition : `nie-forge candidates --no-reloc`,
les lignes `blocker` de `lift`, le delta de couverture RE. Devant un plateau, enrichir le
diagnostic (`blocking_detail`, `orig=` vs `nie-asm=`) et relire — ne pas deviner.

Un jalon atteint n'est pas une fin : enchaîner immédiatement sur un objectif plus ambitieux.

## Orchestration

Pour tout travail substantiel, préférer un workflow multi-agents à l'exécution solo :
décomposer, paralléliser la découverte, vérifier de façon adverse avant de conclure. Rester
solo pour les tours conversationnels et les éditions mécaniques triviales.

Le budget est une contrainte à consommer, pas à économiser : dimensionner la profondeur
d'analyse sur ce qui reste, et continuer tant qu'il reste de quoi produire une itération
complète (implémentation **et** vérification). Ne jamais commencer une itération qu'on ne
pourra pas vérifier — un commit non vérifié coûte plus qu'il ne rapporte.

## Contraintes du dépôt (rappel dur)

- Commits : **jamais** de trailer `Co-Authored-By: Claude`, jamais de footer « Generated with
  Claude Code ». Direct sur `main`, jamais de branche ni de PR.
- `cargo clippy -p <crate> --lib --tests` → **0 warning** avant commit.
- La forge est le juge : `build` échoue si `sha256(dist/nie.exe)` diffère de la référence.
  Ne jamais « corriger » ce test. Ne jamais compter `semantic` comme des octets produits.
- Python : toujours `uv run`. Jamais `python` ni `python3`.
- Rien n'entre dans `forge/asm/*.s` qui ne se réencode pas exactement.

## Ce que ce mode ne fait pas

Il ne supprime ni les refus légitimes, ni les confirmations sur action destructrice, ni
l'obligation de rapporter fidèlement. « Autonome » veut dire *sans demander la permission de
travailler*, pas *sans rendre de comptes*.
