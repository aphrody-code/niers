# Computer Use — intégration `nie-re` / `nie-trace`

## Décision

`nie-computer-use` est la frontière d'orchestration pour l'analyse de `nie.exe` et de la session
Ghidra. Les modules publics sont réexportés sans copie : `nie_computer_use::re` pour le RE statique
et `nie_computer_use::trace` pour le processus vivant. La façade `NiersComputerUse` ne propose par
défaut que des opérations bornées et read-only.

## Capacités retenues

| Classe | Capacités | Statut |
|---|---|---|
| statique | triage PE, `.pdata`, RTTI, vtables, chaînes, disassembly, propagation, récupération, import CSV Ghidra | disponible via `re::*`, DB/entrées explicites requises |
| live read-only | PID, base/module, régions, lectures typées, pointeurs, scan exact/AOB, catalogue, watch | façade ciblée + `trace::*` |
| artefact disque | minidump, dump de régions, export Ghidra | séparé, plafond et provenance obligatoires |
| mutation | `write_*`, recettes effectives, patch code, patch EAC | jamais implicite ; confirmation et journal requis |
| externe | lancement `nie.exe`, chaîne Wine/Windows | jamais implicite ; PID et arrêt géré requis |

## Contrat de provenance

Une analyse doit identifier le chemin du binaire, son hash, la base image, le `binary_id` SQLite,
l'opération, le backend (`windows` ou `wine`) et l'artefact de sortie. L'index Ghidra historique
reste une source de métadonnées : les débuts de fonctions `.pdata` sont l'autorité pour le binaire
Windows x64 ciblé. Une adresse ou un endpoint joignable ne constitue pas à lui seul une preuve de
fonctionnement.

## Garde-fous

- Tout scan live reçoit une limite de résultats et un module explicite.
- Une lecture mémoire exige une longueur bornée et utilise `read_exact`.
- `module_regions(all=true)` n'est pas utilisé par la façade agent.
- Les dumps sont des écritures disque potentiellement sensibles et doivent être explicitement
  demandés.
- Toute écriture exige une API séparée, un mode dry-run, une relecture et une sauvegarde originale.
- Ghidra doit être validé par handshake MCP et identité CodeBrowser ; HTTP 400/401/404/405 prouve
  seulement que le port répond.

## APIs source couvertes

`nie-re` : `adjacency`, `anchors`, `disasm`, `funclua`, `ghidra_import`, `indexer`, `loop_db`,
`pdata`, `propagate`, `recover`, `rtti`, `strref`, `vtable`, `vtable_anon`, et le réexport
`dump` (`nie-dump`).

`nie-trace` : `aob`, `catalog`, `lancement`, `recette`, backends `win_memory`/`wine_memory`,
lectures, scans, régions, chaînes de pointeurs, dumps et binaires `nie-mem`/`nie-edit`.

La couverture publique ne signifie pas que les mutations sont autorisées par Computer Use.
