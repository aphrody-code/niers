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

## Audit de parité — 2026-09-07

| Fichier local | Consommateur | Équivalent Rust canonique | Parité vérifiée | Décision |
|---|---|---|---|---|
| `crates/forge/nie-re/src/*.rs` | `nie-cli re`, exemples RE, base `var/niers.sqlite`, Inacord en lecture | mêmes modules `nie-re` du dépôt [aphrody-code/nie](https://github.com/aphrody-code/nie) | `cargo test -p nie-re --lib --locked`: **72 réussis, 0 échec, 1 ignoré** | conserver |
| `crates/forge/nie-dump/src/lib.rs` | exemples `dump_scan`/`dump_census`, scans de dumps | crate `nie-dump`, réexporté par `nie-re::dump` | couvert par la compilation et les tests de `nie-re` ; smoke réel dépend d’un dump | conserver |
| `crates/forge/nie-trace/src/*.rs` | `nie mem`, `nie-edit`, `nie-mem`, consommateurs live | mêmes backends Windows/Wine du dépôt canonique | `cargo test -p nie-trace --tests --locked`: **43 réussis, 0 échec** ; le test `self_mem` est Linux-only | conserver, compléter la parité Windows |
| `crates/tools/nie-computer-use/src/lib.rs` | commande `computer-use`, orchestration agent | façade Rust locale `NiersComputerUse` + `ReSession` | `cargo test -p nie-computer-use --tests --locked`: **9 réussis** ; session SQLite/hash, lectures et scans bornés | conserver |
| scripts/exports Ghidra | Ghidra headless ou CodeBrowser | `nie-re::ghidra_import` (CSV exact) | import CSV disponible ; handshake MCP et identité binaire non prouvés | migrer vers un adaptateur typé |

### Contrat reproductible livré

Il n’existe pas de doublon local à supprimer : les sources locales sont déjà la copie de travail
du dépôt canonique GitHub (`origin/main`, aucune divergence sur ce périmètre). La migration à faire
porte sur l’interface, pas sur les algorithmes : `nie-computer-use::ReSession` valide le hash et la
taille du binaire, résout explicitement le `binary_id`, conserve la base image, borne les lectures,
convertit RVA/VA et retourne une `ReProvenance` sérialisable. Les écritures mémoire, recettes,
patchs EAC et lancement de processus restent hors de la façade read-only.

La reproduction est `scripts/verify-computer-use-re-trace.ps1`. Elle vérifie l’identité SHA-256 du
binaire canonique, la session `ReSession` réelle via `examples/verify_session`, les tests
`nie-re`/`nie-trace`, les deux invariants de session (build accepté et build rejeté), puis exécute le
probe CLI réel. La base est ouverte en lecture seule par la session.

Le test live Windows reste conditionnel à un `nie.exe` lancé et au même hôte Windows : l’absence du
processus n’est pas transformée en faux succès. De même, une réponse HTTP Ghidra ne vaut pas handshake
CodeBrowser ; une preuve Ghidra complète doit fournir l’identité du programme et un export importable.
