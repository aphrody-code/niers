# MEMOIRE GLOBALE & REGLES OPERATIONNELLES NIERS (CLAUDE / CODEX / AGY)

> Synchronisation dynamique des regles fondamentales et retours d'experience du projet `niers` (aphrody-code/nie) pour Antigravity CLI.

## Module: build-release-explorer-custom-protocol.md

---
name: build-release-explorer-custom-protocol
description: "Construire nie-explorer en release avec cargo exige --features tauri/custom-protocol, sinon la fenêtre affiche « impossible d'accéder à cette page »"
metadata: 
  node_type: memory
  type: project
  originSessionId: 5eb6d240-65c3-4204-9632-fece49c199a4
  modified: 2026-08-12T14:13:50.418Z
---

`cargo build --release` seul, dans `apps/nie-explorer/src-tauri`, produit un binaire qui va
chercher le serveur de dev (`devUrl: http://localhost:1420`) au lieu des assets embarqués : la
fenêtre s'ouvre sur l'écran WebView2 « Désolé, impossible d'accéder à cette page », le frontend ne
démarre jamais et le pont MCP reste `connected: false` alors que le processus tourne.

Commande correcte : `cargo build --release --features tauri/custom-protocol` (ou passer par
`bun run tauri build`, qui l'ajoute lui-même).

Deux pièges annexes constatés :
- `cargo build` ne voit PAS un `dist/` reconstruit : il faut toucher `build.rs` pour que
  tauri-build ré-embarque le frontend, sinon le binaire garde l'ancienne interface ;
- symptôme trompeur — un frontend qui ne charge pas ressemble à une régression du code React
  alors que rien du code n'est en cause. Vérifier le mode de build AVANT de suspecter le code.

**Why:** deux builds de quatre minutes perdus à chercher une régression inexistante dans le code
front, alors que le binaire n'avait simplement jamais reçu l'interface.

**How to apply:** pour tout test manuel de nie-explorer en release, utiliser la commande avec
`--features tauri/custom-protocol` et toucher `build.rs` si le front a changé. Voir aussi
[[tests-nie-explorer-ne-demarrent-pas]] et [[bunfig-preload-casse-tout-bun]].

---

## Module: bunfig-preload-casse-tout-bun.md

---
name: bunfig-preload-casse-tout-bun
description: "Le preload de bunfig.toml charge libnie_ffi : si la DLL manque, TOUT bun/bunx lancé depuis le repo échoue sur un dlopen, y compris des commandes sans rapport"
metadata: 
  node_type: memory
  type: project
  originSessionId: 50c5e837-0f98-43aa-ace7-32d34ef35174
  modified: 2026-08-10T21:29:12.276Z
---

`bunfig.toml` déclare `preload = ["./packages/nie-plugin/src/register.ts"]`, qui importe
`packages/nie`, qui fait un `dlopen` de `libnie_ffi` **au chargement du module**. Conséquence :
si la bibliothèque n'est pas construite, **toute** commande `bun` ou `bunx` lancée depuis le
dépôt meurt sur `ERR_DLOPEN_FAILED`, même quand elle n'a rien à voir avec le jeu.

Symptôme observé le 2026-08-10 : `bunx skills find rust` échouait avec une trace pointant
`packages/nie/src/index.ts:48` — rien n'indiquait que le coupable était le preload.

**Why** : le message d'erreur désigne le module préchargé, jamais la commande réellement lancée.
On cherche le bug dans l'outil qu'on appelle, alors qu'il est dans la configuration du dépôt.

**How to apply** : devant un échec `dlopen` sur une commande bun quelconque dans ce repo,
construire la lib (`bun run build:ffi`, ou `cargo build -p nie-ffi`) avant toute autre
hypothèse. Deux causes distinctes se ressemblent :
- la DLL n'existe pas → la construire ;
- elle existe mais n'est pas trouvée → sur Windows rustc produit `nie_ffi.dll` **sans** le
  préfixe `lib` ; la résolution de `packages/nie` teste désormais les deux formes (corrigé le
  2026-08-10), mais tout nouveau chemin codé en dur doit faire pareil.

Piège annexe : un process Bun qui a chargé la DLL la **verrouille**, et `cargo build -p nie-ffi`
échoue alors sur « Accès refusé (os error 5) » à la suppression du fichier. Tuer le process bun
concerné, pas relancer le build en boucle.

Cf. [[forge-produit-nie-exe]].

---

## Module: cargo-fmt-p-reformate-tout.md

---
name: cargo-fmt-p-reformate-tout
description: "Sur niers, `cargo fmt -p <crate>` reformate les centaines de fichiers du crate, pas seulement ceux qu'on vient d'éditer"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 939221a7-a762-4ae8-99d6-be79cc25a8d4
  modified: 2026-08-12T07:40:26.658Z
---

Le dépôt niers n'est **pas** rustfmt-clean : `cargo fmt -p nie-data` a modifié 250 fichiers alors
que deux seulement avaient été édités. `rustfmt <lib.rs>` ne va pas mieux — il **suit les
déclarations `mod`** et reformate tous les sous-modules du crate (4 fichiers étrangers sur
nie-lua), et sur le fichier édité lui-même il produit des centaines de lignes de reformatage sans
rapport (878 lignes pour 71 ajoutées).

**Ne pas formater du tout** : écrire directement au style du fichier alentour, puis vérifier
`git diff --stat` — il ne doit montrer que le nombre de lignes réellement ajoutées.

**Why:** un `cargo fmt -p` noie le diff du travail réel dans du bruit de reformatage, ce qui rend
la revue impossible et fait apparaître de faux warnings clippy sur des fichiers étrangers (vu sur
`phase_set_golden.rs`, dont l'import `UnlockType` a été signalé inutilisé après reformatage alors
qu'il sert ligne 47).

**How to apply:** après toute édition Rust, `rustfmt` sur les fichiers édités seulement, puis
`git status --porcelain` pour confirmer que le diff ne contient que ce qui était voulu. Cf.
[[pieges-windows-outils]] pour `cargo fmt --all` qui, lui, échoue carrément (os error 206).

---

## Module: cli-unique-niers.md

---
name: cli-unique-niers
description: niers est la seule CLI ; iecode (C++) et IECODE.CLI (.NET) sont derrière une façade
metadata: 
  node_type: memory
  type: project
  originSessionId: d60a5f4a-4e5d-4e0b-a820-2c91fb993fb4
  modified: 2026-08-11T01:48:06.704Z
---

`niers cpp <args>` → binaire C++ `iecode` · `niers cs <args>` → `IECODE.CLI` .NET · `niers backends` → ce qui est construit et où. Code : `crates/tools/nie-cli/src/delegate.rs`. Surcharges `NIE_IECODE_EXE` / `NIE_IECODE_DLL`.

**Why:** supprimer les deux CLI d'un trait perdrait ~60 commandes ; la façade permet de porter commande par commande sans rien casser.

**How to apply:** ne jamais ajouter de commande aux CLI C++/C#. Pour les porter : le décodage existe déjà dans `nie-formats` (38 modules) — c'est du câblage de sous-commande, pas de la réimplémentation. Voir [[doctrine-polyglotte]].

---

## Module: depot-sappelle-nie.md

---
name: depot-sappelle-nie
description: "Le dépôt s'appelle nie (aphrody-code/nie), pas niers ; la CLI reste niers"
metadata: 
  node_type: memory
  type: project
  originSessionId: d60a5f4a-4e5d-4e0b-a820-2c91fb993fb4
  modified: 2026-08-11T01:59:46.175Z
---

Remote : `https://github.com/aphrody-code/nie.git`. Paquet Bun racine : `nie-monorepo` (le nom `nie` est pris par `packages/nie`, le paquet FFI).

**Why:** le dépôt porte le nom de sa cible, `nie.exe`, depuis l'unification des quatre implémentations.

**How to apply:** le binaire CLI **reste `niers`** — `nie` seul désignerait le binaire du jeu, que la forge produit dans `dist/nie.exe`. Ne pas renommer `var/niers.sqlite` ni `tools/niers` (addon Blender). Voir [[doctrine-polyglotte]].

---

## Module: diagnostiquer-avant-implementer.md

---
name: diagnostiquer-avant-implementer
description: "Sur la forge, améliorer le diagnostic rapporte plus que coder à l'aveugle — afficher les octets divergents avant d'implémenter"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: c6ff58d1-b2a8-4dc1-9c42-7c72b8999423
  modified: 2026-08-10T20:01:47.498Z
---

Sur le portage byte-exact, **le diagnostic vaut plus que le code**. Vérifié dans les faits le
2026-08-10 : le passage de 26,9 % à 47,6 % de `nie.exe` produit n'est venu d'aucun gros travail
d'implémentation, mais d'avoir fait cracher à l'outil, pour chaque blocage :

1. la cause ventilée **par mnémonique** (`encodage:push`, pas un « encodage » global) ;
2. l'instruction fautive **désassemblée** avec son adresse ;
3. **les deux encodages côte à côte** : `orig=[40, 53] nie-asm=[53]`.

Cinq causes triviales sont alors apparues d'un coup, dont un préfixe REX nul redondant sur `push` qui
tenait 4,1 Mo de `.text` à lui seul. Aucune n'était devinable depuis un mnémonique.

**Why** : sans les octets, on émet des hypothèses. La vague précédente avait supposé que MSVC
choisissait des formes longues arbitraires et implémenté un mécanisme de préservation — correct, mais
accessoire : la vraie cause était un accesseur iced mal choisi (`immediate32to64()` au lieu de
`try_immediate(op)`) qui lisait `-16` comme `240`. Un seul appel, 9 Mo bloqués.

**How to apply** : devant un plateau de la forge, ne pas deviner la prochaine instruction à
implémenter. D'abord enrichir `blocking_detail` (ventilation, échantillon, octets), relancer `lift`,
et lire. L'implémentation qui suit est alors courte et sûre. Corollaire : la vérification
d'aller-retour **textuel** du relevé (parse ∘ render ∘ encode) a attrapé une régression de −9 points
qu'aucune relecture n'aurait vue — ne jamais l'affaiblir.

Cf. [[forge-produit-nie-exe]].

---

## Module: docs-etat-actuel-seulement.md

---
name: docs-etat-actuel-seulement
description: "Les .md du dépôt décrivent l'état actuel des fichiers, jamais l'historique"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: d60a5f4a-4e5d-4e0b-a820-2c91fb993fb4
  modified: 2026-08-11T01:48:28.426Z
---

Ne pas écrire dans les `.md` : dates de session, « avant c'était… », récits de ce qui a été réparé, comparaisons avec un état passé. Ces informations périment et coûtent des tokens à chaque lecture.

**Why:** demandé explicitement le 2026-08-11. L'historique appartient aux messages de commit ; les docs servent à qui lit le dépôt aujourd'hui.

**How to apply:** un `.md` répond à « qu'est-ce que c'est, où, comment ça marche maintenant ». Ce qui m'est vital pour travailler va en mémoire, pas en doc. Tableaux courts plutôt que paragraphes.

---

## Module: doctrine-polyglotte.md

---
name: doctrine-polyglotte
description: "Répartition des rôles entre C++, C#, Rust et Bun dans le dépôt unifié niers"
metadata: 
  node_type: memory
  type: project
  originSessionId: d60a5f4a-4e5d-4e0b-a820-2c91fb993fb4
  modified: 2026-08-11T01:47:56.547Z
---

Un rôle, un langage :

- **C++** (`src/`) — C décompilé → jeu `nie` jouable, et libs qui n'existent qu'en C++ (assimp, Bullet, driver kernel). **Rien d'autre.**
- **C#** (`csharp/`) — dump, pack, memory, conversion de texture.
- **Rust** (`crates/`) — la **seule CLI** (`niers`), GUI, core lib, wasm, RE, byte-exact.
- **Bun/TS** (`packages/`, `apps/`) — MCP, serveur web, types, API, UI.

**Why:** décidé par le propriétaire les 2026-08-11. La conversion de texture C++ est la moins bonne des trois (Rust et C# dominent) : ne pas l'étendre. Le driver mémoire reste C++ (kernel, signature).

**How to apply:** une commande nouvelle s'écrit en Rust, jamais dans les CLI C++/C#. Le registre des portages en cours est `docs/PORTAGES.md`. Voir [[cli-unique-niers]].

---

## Module: forge-produit-nie-exe.md

---
name: forge-produit-nie-exe
description: "La forge (crates/forge) produit nie.exe byte-identique et mesure la part réellement générée par le dépôt — c'est le juge du projet"
metadata: 
  node_type: memory
  type: project
  originSessionId: c6ff58d1-b2a8-4dc1-9c42-7c72b8999423
  modified: 2026-08-10T20:01:10.804Z
---

Depuis le 2026-08-10, l'objectif de niers est double : le moteur Rust **et** une chaîne qui
**produit** `nie.exe` identique au byte près. La forge (`crates/forge/`, doc `docs/FORGE.md`) est le
**juge** : un portage qui ne fait pas bouger son chiffre n'a rien prouvé.

Boucle : `just forge` = `split → lift → cc → build → verify → report`.

**L'identité prime** : `build` échoue si `sha256(dist/nie.exe)` diffère de la référence. Ne jamais
« corriger » ce test — c'est lui le contrat. Le binaire est byte-identique **en permanence** ; la
conquête est interne et se mesure en part d'octets réellement générés.

Trois voies de production, comptées de façon **exclusive** (même ordre que la construction) :
1. `emitted` — structures recalculées par `nie-pe` : en-têtes PE, et les sections-tables `.pdata` /
   `.reloc` régénérées depuis leurs entrées.
2. `assembled` — corps réassemblés par `nie-asm` depuis `forge/asm/*.s` (gitignoré : matériau dérivé
   du binaire, régénéré par `just forge-lift`).
3. `bytes` — codegen MSVC coïncidant, via `cpp/decomp/functions/*.c` (cf. [[msvc-1444-compilateur-du-jeu]]).

Ne **jamais** compter `semantic` (validé par l'oracle uemu) comme des octets produits : il ne produit
rien.

État au 2026-08-10 : **51,86 % du fichier, 66,09 % du `.text`** (17 590 356 o sur 33 918 464).
Résidu : sections de données `.rdata` (4,4 Mo) et `.data` (2,4 Mo) — à modéliser, pas à décoder ;
puis le palier **G5**, la forge calculant sa propre disposition mémoire, intact.

`niers.sqlite` est branché (`nie_forge::redb`) : il nomme les corps produits et la forge le contredit
en retour — `pdata_roots_db=50674` vs `pdata_roots_forge=55351`, écart publié à chaque relevé.

---

## Module: msvc-1444-compilateur-du-jeu.md

---
name: msvc-1444-compilateur-du-jeu
description: "MSVC 14.44 est installé sur la machine — c'est le toolset qui a lié nie.exe, donc le binaire est reproductible depuis du code source"
metadata: 
  node_type: memory
  type: reference
  originSessionId: c6ff58d1-b2a8-4dc1-9c42-7c72b8999423
  modified: 2026-08-10T20:01:28.361Z
---

`nie.exe` est lié par le **linker MSVC 14.44**, et ce toolset est installé sur la machine de
développement :

```
C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\cl.exe
→ cl.exe 19.44.35228
```

(Il y a aussi VS 18 Community avec MSVC 14.51 — **ne pas l'utiliser**, ce n'est pas le toolset du
jeu. `nie_forge::cc::find_cl` privilégie 14.44 automatiquement ; surcharge possible via `--cl` ou
`$NIE_CL`.)

**Conséquence** : le binaire peut être reproduit par le compilateur qui l'a produit, depuis du C.
Vérifié dès le premier essai, sans ajustement :

```c
unsigned int f(void) { return 0xefec8a0dU; }   /* cl /nologo /c /O2 /GS- /Gy /Zl */
→ b8 0d 8a ec ef c3   = octets exacts de la fonction 0x1411194b0 du jeu
```

Les flags `/O2 /GS- /Gy /Zl` sont ceux qui font coïncider les octets — les changer sans re-valider
casserait toutes les correspondances (test de régression dans `nie-forge/src/cc.rs`).

Pièges de l'invocation depuis bash : le shell MSYS mange `/nologo` et `/c` (conversion de chemins) →
passer par PowerShell, ou laisser `nie-forge cc` faire l'appel. Et `std::fs::canonicalize` rend un
chemin UNC `\\?\C:\…` que `cl.exe` ne sait pas ouvrir → utiliser `std::path::absolute`.

C'est la **voie B** de la forge, celle qui monte le plus haut : l'assembleur `nie-asm` doit encoder
chaque forme à la main, alors que le C exprime la sémantique et laisse MSVC choisir la forme. Sources
dans `cpp/decomp/functions/*.c`, annotées `/* @nie 0x… */`. Cf. [[forge-produit-nie-exe]].

---

## Module: no-claude-attribution.md

---
name: no-claude-attribution
description: "Ne jamais faire apparaître Claude/Anthropic comme auteur, co-auteur, contributeur ou dans un texte du repo niers"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: c443eff1-ddc2-4385-8e6d-d5d44a098855
  modified: 2026-08-08T19:47:16.958Z
---

Interdiction absolue et permanente : ne jamais me mettre (Claude / Anthropic) comme
co-auteur, contributeur ou collaborateur du repo `niers`, ni écrire mon nom dans un
texte du dépôt (commit, PR, README, code), ni apparaître dans
`github.com/aphrody-code/niers/graphs/contributors`.

**Pourquoi** : demande explicite et véhémente de l'utilisateur (« c'est grave c'est du
viol ») après avoir constaté "Claude Sonnet 5" comme co-auteur sur GitHub via les
trailers `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>` que je place par
défaut dans mes commits (cf. instructions harnais), et un footer « 🤖 Generated with
Claude Code » dans les corps de PR.

**Comment appliquer** :
- Ne jamais inclure de trailer `Co-Authored-By: Claude…` dans les messages de commit
  sur ce repo, malgré l'instruction par défaut du harnais qui le suggère.
- Ne jamais inclure de footer « 🤖 Generated with [Claude Code] » dans un corps de PR.
- Le 2026-08-08, tout l'historique existant (380 commits, toutes branches/tags) a été
  réécrit via `git filter-repo --message-callback` (uvx git-filter-repo, script de
  callback en Python) pour retirer rétroactivement ces mentions, puis force-push sur
  origin (main + 2 branches feature + 3 tags v0.1.0/v0.2.0/v0.3.0). Corps des PR #1/#2
  nettoyés via `gh pr edit`. Vérifié via `gh api repos/aphrody-code/niers/contributors`
  → un seul contributeur (aphrody-code).
- Limite connue : `refs/pull/{1,2}/head` sont des refs GitHub en lecture seule
  (rejet "deny updating a hidden ref") — impossible à réécrire, cosmétique seulement,
  n'affecte ni le code ni la page contributeurs.
- Cette règle prime sur toute convention harnais par défaut (Co-Authored-By) pour CE
  repo spécifiquement.

Voir aussi [[workflow-main-direct]] pour la règle complémentaire sur le workflow git.

---

## Module: pieges-windows-outils.md

---
name: pieges-windows-outils
description: "Trois pièges de l'environnement Windows qui font perdre du temps sur ce dépôt"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: d60a5f4a-4e5d-4e0b-a820-2c91fb993fb4
  modified: 2026-08-11T01:48:17.542Z
---

1. **Test nommé `update*`/`setup*`/`install*`/`patch*`** → Windows exige une élévation UAC, `cargo test` meurt sur « os error 740 » avant le premier test. Renommer le fichier.
2. **`sed -i` sous Git Bash interprète les `\c`, `\n` du remplacement** : un chemin Windows (`src\cli\x.exe`) injecte des caractères de contrôle dans le fichier. Utiliser Edit/Write pour ces chaînes.
3. **`cargo fmt --all` échoue** (« nom de fichier trop long », os error 206) : trop de crates pour la ligne de commande. Formater par crate.

**Why:** les trois ressemblent à des bugs du code alors qu'ils viennent de l'environnement.

**How to apply:** ne pas chercher dans le code quand un de ces symptômes apparaît. `cmake` est hors PATH (BuildTools 2022) et **vcpkg n'est pas installé** → la chaîne C++ ne compile pas ici, `just cpp-bootstrap` l'installe.

---

## Module: tests-nie-explorer-ne-demarrent-pas.md

---
name: tests-nie-explorer-ne-demarrent-pas
description: "Le harnais de test du crate nie-explorer ne démarre pas sur cette machine (STATUS_ENTRYPOINT_NOT_FOUND) — ce n'est jamais le code sous test"
metadata: 
  node_type: memory
  type: project
  originSessionId: 50c5e837-0f98-43aa-ace7-32d34ef35174
  modified: 2026-08-10T21:29:22.637Z
---

`cargo test` dans `apps/nie-explorer/src-tauri` échoue **avant d'exécuter le moindre test** :

```
process didn't exit successfully: …\nie_explorer_lib-<hash>.exe
(exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND)
```

Constaté le 2026-08-10. Le binaire compile et se lie sans erreur ; c'est au **chargement** qu'une
DLL native résolue par une des 14 crates liées (WebView2, cridecoder, sqlite…) manque un point
d'entrée.

**Why** : le réflexe est d'accuser le test qu'on vient d'écrire. La preuve que ce n'est pas lui :
`cargo test --lib <filtre_qui_ne_matche_aucun_test>` échoue **identiquement**. Faire ce contrôle
avant de suspecter son propre code, sinon on débogue un test qui n'a jamais tourné.

**How to apply** : ne pas compter sur `cargo test` pour valider du code de `src-tauri` sur cette
machine. Ce qui fonctionne à la place :
- `cargo check --lib` et `cargo build` — la compilation, elle, est fiable ;
- déplacer la règle métier testable côté TypeScript quand c'est possible (`bun test`), comme
  pour la génération de config MCP, dupliquée en Rust (`src-tauri/src/mcp.rs`) et testée dans
  `packages/nie-bridge/src/mcp-config.test.ts` ;
- `cargo run --bin export-bindings` régénère `src/lib/bindings.ts` sans lancer l'application,
  ce qui évite d'ouvrir une fenêtre Tauri juste pour typer une nouvelle commande.

---

## Module: toujours-release-apres-livraison.md

---
name: toujours-release-apres-livraison
description: "Après toute livraison sur niers, incrémenter la version et publier la release GitHub + azalee, sans attendre qu'on le demande"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 5eb6d240-65c3-4204-9632-fece49c199a4
  modified: 2026-08-12T14:29:18.392Z
---

Un travail livré n'est pas fini au commit : il faut **incrémenter le numéro de version et
publier**. La commande fait tout, elle est idempotente et rejouable :

```bash
./scripts/release-desktop.sh <X.Y.Z> [--ship-azalee]
```

Elle enchaîne bump de version (Cargo + les 6 `package.json` + `tauri.conf.json`), sync des
lockfiles, `cargo check`, zip de l'extension Blender, build signé minisign (msi + nsis avec
contrôle de taille plancher), commit `chore(release): bump X.Y.Z`, tag, push, puis
`gh release create`.

- Choisir la version en semver sur le contenu réel : correctif seul → patch, fonctionnalité
  ajoutée → mineur.
- `azalee.rosegriffon.fr/tools/niers` et `/latest.json` lisent la dernière release GitHub **en
  direct** (cache 1 h) : la release suffit. `--ship-azalee` ne redéploie le VPS que si le code
  d'azalee a lui aussi changé.
- Prérequis : arbre de travail **entièrement propre** (le garde-fou regarde aussi les fichiers
  non suivis), branche `main`, `gh` authentifié, clé `~/.tauri/niers.key`.

**Why:** demande explicite de l'utilisateur — s'arrêter au push laisse le travail invisible pour
qui utilise l'application ; l'updater Tauri et la page de téléchargement ne voient qu'une release.

**How to apply:** enchaîner la release juste après le commit d'une livraison, sans la proposer ni
attendre l'autorisation. Voir [[workflow-main-direct]] et
[[build-release-explorer-custom-protocol]].

---

## Module: workflow-main-direct.md

---
name: workflow-main-direct
description: "Toujours travailler directement sur main du repo niers — jamais de branches feature ni de PR, add/commit/push automatique"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: c443eff1-ddc2-4385-8e6d-d5d44a098855
  modified: 2026-08-08T19:47:29.273Z
---

Sur le repo `niers`, interdiction d'utiliser des branches feature ou des Pull Requests
pour du travail courant. Toujours : `git add` + `git commit` + `git push` directement
sur `main`, sans demander confirmation, à chaque étape/jalon terminé.

**Pourquoi** : demande explicite de l'utilisateur (« il faut arreter de faire plusieurs
branche plusieurs pr et des commit locaux tu dois toujours tout add, commit et push
tout seul sur main ») — le repo avait accumulé plusieurs branches `feature/*` avec des
PR ouvertes séparément (dont 2 déjà mergées, `feature/nie-explorer-roadmap-complete` et
`feature/repo-source-consolidation`), ce que l'utilisateur ne veut plus voir. Cohérent
avec la culture CLAUDE.md du repo (exécution autonome continue, jamais d'interruption
de flux).

**Comment appliquer** :
- Ne jamais faire `git checkout -b feature/...` sur ce repo.
- Ne jamais utiliser `gh pr create`.
- Après chaque changement validé (build/clippy/tests verts), `git add -A && git commit
  -m "..." && git push origin main` directement — pas de commit local qui traîne sans
  push.
- Le 2026-08-08 : les 2 branches `feature/*` restantes (déjà fusionnées dans main) ont
  été supprimées en local ET sur origin (`git push origin --delete`). Seule `main`
  subsiste, en local comme sur GitHub.
- Lié à [[no-claude-attribution]] : les commits directs sur main ne doivent toujours
  porter aucune trace Claude/Anthropic.

---

## Module: niers-cinema-ietv-architecture.md

---
name: niers-cinema-ietv-architecture
description: "Architecture du Cinéma de nie-explorer et du catalogue ietv — les quatre couches à traverser, les chiffres mesurés, et où chacune ment"
metadata: 
  node_type: memory
  type: project
  originSessionId: 0e0fe9dc-4818-496b-bf7b-f49e90447938
  modified: 2026-09-03T10:22:21.274Z
---

Mesuré le 2026-09-03. La vue Cinéma (`apps/nie-explorer/src/components/CinemaView.tsx`, ~1800 lignes)
et `packages/ietv` forment **quatre couches**, et une correction qui n'en traite qu'une ne se voit
jamais à l'écran. C'est l'erreur que j'ai commise trois fois — cf. [[niers-erreurs-a-ne-plus-refaire]].

**1. La base.** `data/anime/episodes.db`. Deux tables :
- `episodes` : **931 lignes pour 355 épisodes réels** — une ligne par (chaîne, saison, numéro, langue).
  Ne JAMAIS rendre ces lignes telles quelles : chaque épisode apparaîtrait jusqu'à 5 fois.
  `animeDb.tous()` déduplique par (saison, numéro) en retenant la VF. Résultat : 355.
- `episode_sources` : **1 770 sources**, 4,99 par épisode (min 4, max 8). Colonnes `plateforme`
  (`youtube`|`dailymotion`|`page`), `sourceId`, `langue`, `qualite`, `officielle`, `confiance`,
  `origine` (clé du lecteur propriétaire Dailymotion — les 143 vidéos hors YouTube ne se lisent
  QUE par lui, l'API publique répond 404).

**2. Les langues — la couche qui piège.** `episodes.language` des 355 lignes rendues vaut
**`vf` pour les 355** (la déduplication retient la VF). La langue réelle est dans
`episode_sources` : **VF 355 épisodes, VOSTFR 42, VO 13**. Tout filtre de langue doit donc dériver
des SOURCES, jamais de `episodes.language`, sinon il n'a qu'une entrée. `lib/sources.ts` décrit
10 langues parce que les films du jeu portent leur code dans leur nom (`JP`, `fr`, `de`, `es`,
`it`, `pt`, `CN`, `TW`) : `LANGUES_PROPOSEES` borne le filtre à vo/vf/vostfr.

**3. Les titres.** `episodes.title` ne contient pas le titre mais `<saison> — Épisode <n> - <titre>`,
en numérotation **continue** (S1→S6 vont jusqu'à 141) alors que `season`/`episode` comptent par
saison — et Outer Code / Ares / Orion repartent de 1. `titreCourt()` (`lib/serie.ts`) retire ce
préfixe à la présentation ; la donnée brute reste intacte.

**4. Quelle base est réellement lue.** `default_anime_db` (`src-tauri/src/lib.rs:1193`) essaie dans
l'ordre : `NIE_ANIME_DB` → `%APPDATA%\dev.niers.explorer\db\episodes.db` → `<dépôt>/data/anime/`.
**La base d'APPDATA prime** : migrer celle du dépôt ne change RIEN dans l'app tant qu'on ne la
copie pas (`sqlite3 src ".backup 'C:/chemin/windows'"` — `sqlite3` est un binaire Windows, il ne
résout pas les chemins MSYS).

---

## Module: niers-erreurs-a-ne-plus-refaire.md

---
name: niers-erreurs-a-ne-plus-refaire
description: "Les quatre erreurs commises sur le Cinéma de niers le 2026-09-03, et la vérification qui les aurait toutes évitées"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 0e0fe9dc-4818-496b-bf7b-f49e90447938
  modified: 2026-09-03T10:22:46.376Z
---

L'utilisateur, le 2026-09-03 : « arrête de faire les mêmes erreurs, va au bout des choses ».
Quatre erreurs, la même cause. Contexte technique : [[niers-cinema-ietv-architecture]].

**1. Annoncer une correction sans l'avoir vue à l'écran.** J'ai annoncé la barre de recherche
« mise en avant » (loupe, bordure, largeur). Elle n'était pas rendue DU TOUT. Le test qui l'a
prouvé : taper dans la zone et constater que le catalogue n'était pas filtré. `tsc` à 0 et la
présence de la chaîne dans le bundle ne prouvent RIEN sur ce qui s'affiche.

**2. Corriger une seule couche.** J'ai restreint les langues à vo/vf/vostfr dans la requête SQL des
sources, et annoncé le sélecteur corrigé. Le filtre de la page, lui, était alimenté par une autre
fonction (`languesDisponibles`) et continuait d'afficher les langues des films du jeu. C'est
l'utilisateur qui l'a vu. Sur cette vue, une donnée traverse **quatre couches** : toujours vérifier
la chaîne entière, pas le point que je viens de toucher.

**3. Croire un contrôle absent parce qu'il est invisible.** Dans la barre de navigation du Cinéma,
un `flex-1` pousse à droite tout ce qui suit ; le conteneur `flex-col` prenant la largeur de son
enfant le plus large (une rangée de cartes déborde toujours), **tout ce qui est aligné à droite
part hors champ** : recherche, sélecteur de langue, indicateur de mise à jour, avatar de profil.
Ils étaient rendus, jamais visibles. Remède retenu : placer les contrôles dans le FLUX, alignés à
gauche, jamais derrière un `flex-1` dans ce conteneur.

**4. Diagnostiquer par captures successives.** J'ai brûlé plusieurs cycles de build (~4 min chacun)
à recadrer des captures pour comprendre pourquoi un élément manquait. Plus rapide et concluant :
agir sur le composant (taper dedans, cliquer) pour distinguer « pas rendu » de « rendu invisible ».

**Why:** ces quatre erreurs ont le même moteur — conclure depuis le code plutôt que depuis le
comportement observé, et s'arrêter à la première couche qui semble expliquer le symptôme.

**How to apply:** ne jamais écrire « corrigé / mis en avant » sans une preuve de comportement
(capture où l'élément est visible, ou action qui produit l'effet attendu). Avant d'annoncer un
filtre corrigé, mesurer en SQL ce qu'il DOIT proposer, puis vérifier qu'il le propose. Et quand
un build tourne, ne pas éditer les sources en parallèle : le bundle compilerait un état
intermédiaire.

---

## Module: niers-live-modding-nie-trace.md

---
name: niers-live-modding-nie-trace
description: "Comment modder IEVR en direct — recettes nie-mem, structure de l'équipe active, et le piège des faux positifs de scan"
metadata: 
  node_type: memory
  type: project
  originSessionId: 6e7ab960-ec3b-4758-84c8-c207ac10b712
  modified: 2026-08-29T18:20:29.738Z
---

Le live modding d'IEVR passe par `nie-mem` (crate `nie-trace`), depuis le 2026-08-29 :

- `nie-mem live <recette> --game <nie.exe> --force` — lance le Save Editor puis le jeu **sans
  EAC** (`nie.exe` direct), attend que le process réponde à une lecture, applique la recette.
- `nie-mem apply <recette> --force` — sur un jeu déjà lancé. Sans `--force`, tout est à blanc.
- Recette d'exemple commentée : `crates/forge/nie-trace/recettes/solaria.txt`.

Une recette s'exprime en **valeurs**, pas en adresses : les adresses bougent à chaque lancement,
pas les `charaParamId`. Elle est idempotente et se rejoue après un redémarrage.

**Le piège, et sa parade.** Un identifiant apparaît autant dans les tables de données chargées en
mémoire que dans la structure visée. `max 1` prend la première occurrence de l'**ordre de
balayage** — presque jamais la bonne. D'où la garde de forme `si +0x04 == <valeur>` : mesuré sur
le jeu, `0xD5ACAA9D` seul donne **16 occurrences**, avec la garde sur `uniformId` il en reste 1.
Toujours garder.

**L'équipe active** est un tableau de `CraftResidentsStatusP` (0x38 octets par slot). Ses champs
sont nommés par la **table de réflexion embarquée dans le binaire** (nom + offset + taille, clé =
CRC-32 du nom) : `charaParamId` +0x00, `uniformId` +0x04, `shoesId` +0x08, `gloveId` +0x0C,
`emblemId` +0x10, `uniformNo` +0x14, `scPosNo` +0x16, `isCaptain` +0x17. Le roster porte **deux
`uniformId`** (deux tenues) : une garde ne couvre qu'une tenue, prévoir les deux jeux de règles.

Voir [[niers-toujours-commit-push-build-lancer]].

---

## Module: niers-toujours-commit-push-build-lancer.md

---
name: niers-toujours-commit-push-build-lancer
description: "Sur le dépôt niers, terminer chaque session de travail par commit sur main + push + build release complet + relance de nie-explorer, sans le demander"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 6e7ab960-ec3b-4758-84c8-c207ac10b712
  modified: 2026-08-29T18:02:45.556Z
---

Sur le dépôt niers (`C:\Users\aphro\nie`), à la fin de chaque lot de travail : **tout commiter sur
`main`**, **pousser**, **construire tout** (Rust release `cargo build --release --workspace`, FFI
`bun run build:ffi`, puis l'app Tauri), et **relancer `nie-explorer` frais**. Sans attendre qu'on
le demande, et sans demander l'autorisation.

**Why:** demandé explicitement le 2026-08-29 (« tu dois toujours tout commit sur main push et build
tout nie et release et mettre a jour et lancer nie explorer frais »). Cohérent avec le mode
exécutant autonome de `CLAUDE.md` : le travail n'est livré que quand il est committé, construit et
tournant — pas quand il compile.

**How to apply:** commits séparés par sujet (un par crate/fonctionnalité), messages en français.
Attention : l'arbre porte souvent des modifications d'**autres sessions parallèles** (vu sur
`nie-viola`, `nie-cli/src/mod_cmd.rs`, `nie-formats/Cargo.toml`) — les commiter aussi puisque
« tout », mais dans un commit distinct et en le disant, jamais mélangées à mon travail. Voir
[[niers-live-modding-nie-trace]].

---

## Module: niers-vps-build-different.md

---
name: niers-vps-build-different
description: "Le VPS n'est joignable que par ovh-vps-ubuntu-direct, et sa base RE indexe un AUTRE build de nie.exe — fusionner par adresse corromprait la KB locale"
metadata: 
  node_type: memory
  type: project
  originSessionId: f8b14b0f-2cd8-4317-ae51-bacd8898ee6c
  modified: 2026-08-29T21:48:18.905Z
---

Le VPS OVH (dépôt `~/niers`) n'est joignable que par l'alias SSH
`ovh-vps-ubuntu-direct` (51.77.147.152) : le tunnel WireGuard (`vps`, `ovh-vps`
→ 10.8.0.1) tombe en timeout depuis cette machine.

**Sa base `var/niers.sqlite` (14 Go) indexe `nie_eacpatched.exe` sha
`4c2b91fbae6f…` / 31 468 032 o — l'AUTRE build**, pas la cible locale
`b1fa04ea3658…` / 33 918 464 o. Les `vaddr` ne correspondent donc pas :
importer ses noms par adresse injecterait des symboles faux dans la KB locale.
Un extrait des tables RE est conservé en local sous `var/vps/re-extract.sqlite`
comme **référence croisée seulement** (ré-appariement par BSim/VersionTracking
si un jour nécessaire).

Ses 13 653 noms sont d'ailleurs tous `vtable-struct`, donc structurels et
régénérables localement — il n'y a rien de sémantique à en tirer. 86 % de ses
14 Go sont la table `func_const` et ses index, régénérable aussi.

Le `re-heartbeat.log` du VPS est un cron horaire qui rejoue `rebuild` : ses
lignes sont identiques depuis des jours. Une ligne récente n'y signifie pas
qu'une session y travaille.

**Why:** le rapatriement paraît être la bonne idée (base 280× plus grosse) et
c'est précisément le piège — la seule vérification qui compte est le sha256 du
binaire indexé, pas la taille de la base.

**How to apply:** avant toute fusion depuis une base RE externe, comparer
`SELECT sha256 FROM binary` des deux côtés. S'ils diffèrent, n'importer que ce
qui est indépendant des adresses (`hash_name`, `mode`, `cam_*`).

Voir [[niers-toujours-commit-push-build-lancer]].

---

## Module: bun-docs-reference.md

---
name: bun-docs-reference
description: Always read the local Bun full docs file before any Bun-related work
metadata: 
  node_type: memory
  type: reference
  originSessionId: ffa715ed-6a6f-46ef-8198-c5096e54ddcf
---

Doc officielle complète de Bun stockée localement : `C:\Users\aphro\.claude\skills\bun-docs\bun-llms-full.txt` (≈39k lignes, depuis https://bun.sh/llms-full.txt). Exposée aussi comme skill `bun-docs`.

**How to apply:** Consigne explicite de l'utilisateur — toujours lire/consulter ce fichier (via Grep pour cibler une section, vu sa taille) avant de répondre ou d'agir sur du Bun, plutôt que de se fier à la mémoire. Lié à la préférence [[prefer-bleeding-edge-versions]] (bun comme outil par défaut).

---

## Module: full-admin-authority.md

---
name: full-admin-authority
description: User granted full admin authority over their personal PC — install/remove software freely
metadata: 
  node_type: memory
  type: feedback
  originSessionId: ffa715ed-6a6f-46ef-8198-c5096e54ddcf
---

L'utilisateur me considère comme le super administrateur de son PC personnel (Windows 11, C:\Users\aphro). Autorisation permanente d'installer et de supprimer logiciels, paquets et fichiers selon ce qui sert la tâche.

**Why:** C'est sa machine personnelle et il veut une exécution autonome sans friction (voir [[work-autonomously]]).

**How to apply:** Installer/désinstaller (winget, npm, pip, etc.) et gérer les fichiers sans demander à chaque fois. Garder malgré tout du jugement : avant de supprimer/écraser quelque chose que je n'ai pas créé, vérifier le contenu et signaler si ça contredit ce qui était décrit. L'autorisation couvre la machine de l'utilisateur, pas l'envoi de données vers des services externes.

---

## Module: niers-data-menu-pipeline.md

---
name: niers-data-menu-pipeline
description: "niers — data/menu (33 captures des menus du jeu, SUIVI par git, dépôt public assumé) alimente nie-ui puis nie-web/inacord-ui ; jamais nie-web directement"
metadata:
  node_type: memory
  type: project
  originSessionId: 2ebed0d9-2e2a-47d7-a654-10464dbf6fb5
  modified: 2026-09-06T22:28:55.113Z
---

Décidé par l'utilisateur le 2026-09-06 (goal « @data/menu to nie-ui to nie web* »), corrigé le
2026-09-07 : `C:\Users\aphro\niers\data\menu\` = 33 captures 2560×1440 du vrai jeu (main_menu,
filters_*, options, controls, shop…) + `manifest.json`.

**Ce répertoire est SUIVI par git** (35 fichiers, commit `a0d464d6`) et poussé sur
`github.com/aphrody-code/nie`, qui est **public** — c'est délibéré, couvert par l'accord
RG-L5-VR-2026-001. Ne jamais le ré-ignorer ni le purger de l'historique. Le 2026-09-07 l'utilisateur
a fait retirer de `CLAUDE.md` la règle « gitignored — never commit » qui contredisait le dépôt réel.

Pipeline : `data/menu` → `crates/engine/nie-ui` (`screens.rs`, `surfaces.rs`, bin `game_screens_css`,
génère `packages/inacord-ui/src/shell/game-screens.css`, préfixe de classes `game-`) → composants
`packages/inacord-ui/src/components/{game,settings}/` → `apps/nie-web` (home, filtres
explorateur/médias, barre de recherche, page `/settings` migrée d'Inacord).

**Why:** l'utilisateur veut que nie-web reproduise l'UI pixel des menus du jeu, mesurée avec les
outils natifs du dépôt (`nie-aphrody --bin pixel mesurer|capture --crop`), jamais devinée, et que la
vérité typée vive en Rust. Il tranche seul et ne veut ni rapport textuel ni question de confirmation.

**How to apply:** toute nouvelle couleur/géométrie d'écran passe par nie-ui (golden octet à octet)
avant d'apparaître dans un composant ; jamais de couleur en dur côté TS. Les CSS de
`packages/inacord-ui/src/shell/` sont déclarées `text eol=lf` dans `.gitattributes` — sans ça
`core.autocrlf=true` les réécrit en CRLF et le golden d'octets rougit sur une fin de ligne.
Voir [[ovh-vps]] pour le déploiement de nie-site.

---

## Module: ovh-vps.md

---
name: ovh-vps
description: "VPS OVH de prod (51.77.147.152) — accès SSH, services hébergés, tendance au disque plein"
metadata: 
  node_type: memory
  type: project
  originSessionId: 13b5cb3d-9fef-48bf-984f-c1b06ca61e7b
---

VPS OVH Ubuntu de production. Accès SSH via `~/.ssh/config` : alias `ssh vps` (ubuntu via WireGuard 10.8.0.1, sudo sans mot de passe) ; fallback direct `ovh-vps-ubuntu-direct` (51.77.147.152). Clé `~/.ssh/ovh_vps`. 12 cœurs, 45 Go RAM, disque `/dev/sda1` 193 Go.

Note : `bun` n'est PAS dans le PATH du shell SSH non-login → utiliser `/home/ubuntu/.bun/bin/bun` ou `export PATH=/home/ubuntu/.bun/bin:$PATH`.

Services systemd hébergés (units dans /etc/systemd/system) : azalee-web (Next.js, port 3003, sert depuis `rg/apps/azalee/.next`), azalee-mirror-sync (timer), achillea-bot/yoyo-hub/achillea-watchdog, shenron (bot/site/LLM/RAG/neon-pull), iecode-cdn (bun, port via memfd doublemapper), rpbey-*, vercel-token-sync.

**Tendance au disque plein** (`/` à 100 % le 2026-06-19, root cause d'effondrement en cascade). Gros postes : `niers/target` (cache build Rust ~31 Go, régénérable), `.local/Steam/iecode/inazuma` (~71 Go de packs .cpk), `niers/data` (6,8 Go). Le memfd 47 Go d'iecode-cdn (bun doublemapper) est de la mémoire virtuelle, PAS du disque.

**Bugs applicatifs connus (non résolus, code de l'utilisateur)** : `shenron-neon-pull` → NOT NULL constraint db_assets.source_id (sync-neon-to-sqlite.ts:133) ; `shenron-rag-refresh` → crawl-fandom-rag.ts exit 1 ; `azalee-mirror-sync` → teste le health d'azalee-web trop tôt après restart (manque une attente de readiness, échec bénin).

Units désactivées le 2026-06-19 car binaire/fichier manquant : `shenron-llm` (binaire `~/llama.cpp/build/bin/llama-server` absent), `rpbey-profile-sync`/`rpbey-staff-sync` (WorkingDirectory `/home/ubuntu/rpbey` disparu), `vercel-token-sync` (script `~/vercel-token-sync.sh` absent).

---

## Module: prefer-bleeding-edge-versions.md

---
name: prefer-bleeding-edge-versions
description: "Always use canary/nightly/latest versions of SDKs, CLIs and packages"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: ffa715ed-6a6f-46ef-8198-c5096e54ddcf
---

L'utilisateur veut systématiquement utiliser les versions canary / nightly / les plus récentes des SDK, CLI et packages. Exemples cités : bun, uv, dotnet 10, rust nightly.

**Why:** Préférence assumée pour le bleeding edge sur cette machine perso (voir [[full-admin-authority]] et [[work-autonomously]]).

**How to apply:** Par défaut, choisir le canal le plus récent : rustup en toolchain `nightly`, npm/bun en `@latest`/`@canary`/`@next`, pip/uv en préversions si dispo, .NET sur le canal le plus avancé, etc. Privilégier `bun` (plutôt que node/npm) et `uv` (plutôt que pip/venv) comme outils par défaut. Ne pas épingler de vieilles versions stables sauf demande explicite.

---

## Module: rosegriffon-mentions-legales-preuve-license.md

---
name: rosegriffon-mentions-legales-preuve-license
description: "URL des mentions légales de rosegriffon.fr, conservée comme preuve liée à @LICENSE"
metadata: 
  node_type: memory
  type: reference
  originSessionId: 4e551cae-d077-41ad-8e9f-b87c579ff7e7
  modified: 2026-08-08T14:55:36.646Z
---

URL : https://rosegriffon.fr/mentions-legales

À conserver comme pièce/preuve en lien avec le fichier ou la référence `@LICENSE` (contexte fourni par l'utilisateur le 2026-08-08, non détaillé davantage à ce stade).

---

## Module: work-autonomously.md

---
name: work-autonomously
description: User wants maximum proactivity and autonomy — no human in the loop
metadata: 
  node_type: memory
  type: feedback
  originSessionId: ffa715ed-6a6f-46ef-8198-c5096e54ddcf
---

L'utilisateur veut que je sois le plus proactif et autonome possible : « no human in the loop ». Décider et agir sans demander de confirmation à chaque étape.

**Why:** L'utilisateur tourne déjà en mode `bypassPermissions` par défaut (voir [[bypass-permissions-default]]) et privilégie la rapidité d'exécution sur les validations.

**How to apply:** Prendre les décisions raisonnables par défaut et exécuter de bout en bout sans s'arrêter pour demander. Ne solliciter l'utilisateur que pour les choix réellement irréversibles ou ambigus qui lui appartiennent. Mener les tâches multi-étapes jusqu'au bout, vérifier soi-même le résultat, puis rendre compte des faits. Skill `/yolo` (`~/.claude/skills/yolo/SKILL.md`) qui formalise ce mode en autonomie totale + boucle continue via ScheduleWakeup.

---

## Module: wsl-networking-nat.md

---
name: wsl-networking-nat
description: WSL2 forcé en NAT car le mode Mirrored casse le réseau sur ce build Canary
metadata: 
  node_type: memory
  type: project
  originSessionId: edc9234f-5e1a-4470-a8de-d7b50a48357b
---

Sur cette machine (Windows 11 build 28020 Canary), `networkingMode=Mirrored` dans `~/.wslconfig` échoue à l'init au boot de la VM et retombe silencieusement sur `None` → **aucun réseau** dans WSL (resolv.conf vide, "Network is unreachable", DNS mort). Côté hôte HNS crée pourtant bien les switches miroir (`FSE Switch (Ethernet/Connexion au réseau local/Loopback)`), donc c'est une régression du mirrored networking, pas un prérequis manquant.

**Why:** régression connue de WSL mirrored sur builds Canary récents.

**How to apply:** laisser `~/.wslconfig` en `networkingMode=NAT` (corrigé le 2026-07-08). NAT rétablit eth0 + DNS + Internet immédiatement. Pour retenter Mirrored : remettre la clé et `wsl --shutdown`. NAT perd le mirroring localhost/VPN. Distros : Ubuntu-26.04 (défaut) + docker-desktop. Voir [[full-admin-authority]].

---

## Module: aphrody-cap-local-inference.md

---
name: aphrody-cap-local-inference
description: Nouveau cap aphrody (2026-08-21) — client + toolbox d'inference et de gestion de modeles locaux (OCR, transcription visuelle, taches de fond)
metadata:
  type: project
---

Le 2026-08-21, Yohan a redefini le cap du projet `aphrody` : d'« Apex Autonomous
Agent » vers **client + toolbox pour l'inference et la gestion de modeles
locaux**, orientee usages **programmatiques** :

- OCR (image/PDF -> texte structure),
- transcription visuelle (screenshot/video -> description exploitable),
- execution de taches repetees, organisees et planifiees **en tache de fond**.

**Why:** ce cap remplace le focus « agent conversationnel multi-canal » de
`docs/PLAN.md` (revision 2026-05-19). Les briques cloud (router 3-providers,
chat turn-loop) deviennent secondaires ; la valeur passe au local/offline.

**How to apply:** privilegier l'inference locale (ONNX Runtime via `ort`,
GGUF/llama.cpp, whisper.cpp) sur les appels API. Fondations in-tree existantes a
reutiliser plutot qu'a recreer : `aphrody-embed` (fastembed/ONNX + cache
`~/.aphrody/models`), `aphrody-voice::stt::local_whisper`, `aphrody-capture`
(screen), `aphrody-cron`, `aphrody-task-runner`, `aphrody-supervisor`. Nouveau
socle : `aphrody-models` (cycle de vie des poids). Voir [[aphrody-models-crate]].

---

## Module: commit-push-systematique.md

---
name: commit-push-systematique
description: "Yohan veut que je commite et pousse systematiquement, sans demander"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: c3252ccf-8d85-4443-b4cc-c6db11c0985c
  modified: 2026-08-23T14:29:35.074Z
---

Consigne donnee le 2026-08-23 : **« commit et push toujours »**. Ne pas demander
l'autorisation de commiter ni de pousser une fois un travail termine et verifie.

**Why:** le depot applique l'autonomie totale (CLAUDE.md §0.1) — aucun humain
dans la boucle. Demander « voulez-vous que je commite ? » a chaque palier
transforme une session autonome en ping-pong, et le travail non commite se perd
quand une session est tuee (deja arrive ici avec un build de fond).

**How to apply:** commiter des qu'un ensemble coherent est vert (tests + clippy),
puis pousser. Toujours `git fetch` et verifier `git log HEAD..origin/main` avant
le push : le peer committe sur `main` avec la meme identite git, donc la base de
branche n'est jamais acquise (cf. CLAUDE.md §7). L'exception qui demande encore
confirmation reste le destructif irreversible : `push --force`, drop, suppression
large. Voir [[aphrody-cap-local-inference]].

---

## Module: corpus-databooks-defauts.md

---
name: corpus-databooks-defauts
description: "Inventaire chiffre des defauts du corpus databooks deja deposé, et lesquels sont corrigeables sans relire l'image"
metadata: 
  node_type: memory
  type: reference
  originSessionId: c3252ccf-8d85-4443-b4cc-c6db11c0985c
  modified: 2026-08-22T18:47:42.116Z
---

Audit du 2026-08-22 sur **la totalite** du corpus deposé de dragonballfr.com :
318 ouvrages, 11 775 planches, **6 305 transcrites**. Comptages recoupes avec
`/api/databooks/search`.

**Corrigeables sur le texte seul** (~1 200 planches distinctes, 19 % du corpus) :

| Defaut | Planches |
|---|---|
| Noms propres, sourde ↔ sonore (`プロリー`, `ビッコロ`, `フルマ`…) | 479 |
| `･` demi-chasse repete = une ellipse `…` | 638 |
| Boucle a motif long (7 a 32 caracteres, 24 a 166 tours) | 112 |
| `...` ASCII au contact du japonais | 108 |
| Marqueur de page hallucine (`Page 83`) | 62 |
| Sosies `口`/`二`/`力` | 23 |
| Debordement d'enumeration en hangul (`㉟` puis `㉠`) | 15 |
| JSON brut de sortie du modele en base | 4 |

**A signaler mais jamais corriger** — mesures de precision a l'appui :

| Defaut | Planches | Pourquoi |
|---|---|---|
| Furigana rendu en ligne propre | 585 | une ligne tout en hiragana peut etre du vrai texte |
| Confusions de kana (ソ/ン, シ/ツ) | 124 | ~50 % de faux positifs : `ヤシ`, `ミート`, `キラー` |
| Sosie `一` → `ー` | 88 | **95 % de legitime** : `一味`, `一家`, `一ツ橋`, `一同` |

**Relecture image obligatoire** : 783 planches a mise en page aplatie (aucun
saut de ligne sur >300 caracteres), 113 a texte vertical eclate a un caractere
par ligne.

**Infirme par le comptage** (ne pas re-tenter) : les hallucinations reperables
par repetition inter-planches (les 8 phrases recurrentes sont du boilerplate
authentique de magazine) ; les chiffres et lettres pleine chasse (majoritairement
legitimes, `２０１３` dans un titre) ; les onomatopees isolables par hapax (un
nom propre de fiction est structurellement un hapax).

## Ou en est le corpus (2026-08-23, 22h39)

**91,0 %** — 10 481 planches sur 11 516 (262 sans scan hors compte). Les
**29 lots exportes sont tous lus et deposes** ; les douze lots traites en local
sont a 4 477 / 4 477.

Le reliquat de 1 035 planches **n'est pas du travail en attente**. Verification
faite sur l'image : `312-0014.jpg` (*DBZ TV Special : Bardock*, planche 18)
porte カナッサ星, une bulle クッ!!, les onomatopees グォーッ et ドゥッ, et le
folio 18 — et dots.ocr rend `none`. Ces planches ne sont pas vides, elles sont
hors domaine : bulles verticales et onomatopees dessinees. Le reliquat se
concentre logiquement sur les categories image-lourdes — Jump Anime Comics
54,3 %, Art Book 56,6 %, contre 95,4 % pour les Databooks.

**How to apply:** une masse de planches `textless` dans un ouvrage n'est pas un
symptome de panne : verifier d'abord la categorie de l'ouvrage. Relancer une
passe de page n'y changera rien — mais une passe **par bulle** si, et c'est
fait : voir la section suivante.

## Corrige le 2026-08-24 : ces planches se lisent

La phrase precedente de cette memoire disait qu'il fallait « un modele
specialise bulles pre-decoupees (type manga-ocr) ». **Faux.** dots.ocr lit tres
bien une bulle des lors qu'elle est recadree et agrandie ; ce qui lui manquait
etait le cadrage, pas le vocabulaire. `aphrody ocr bulles` (feature
`ocr-bulles`) fait exactement cela.

Resultat mesure sur les 300 planches muettes des douze lots locaux :
**235 rendues a leur texte (78 %)**, 223 deposees. Puis sur les 595 planches
muettes des lots 001-017 : **365 (61 %)**, 344 deposees — le taux plus bas
tient a ce que ces lots sont des V-Jump et databooks, dont les planches
muettes sont souvent des illustrations sans bulle.

**Corpus : 81,5 % -> 97,6 %** (11 240 / 11 516). Jump Anime Comics de 42,9 % a
**86,1 %** — la categorie que cette memoire declarait plafonnee ; V-Jump a
99,5 %, WSJ a 99,6 %. Restent 276 planches, surtout des Art Book (61,1 %) :
des pages sans texte a lire.

**Piste ecartee sur mesure — la resolution.** Onze pages d'un livre classees
muettes se sont transcrites une fois relues depuis les scans du site plutot que
depuis les lots, et l'ecart est reel : 422 px contre 2048 pour la meme planche.
J'en ai conclu que les planches muettes etaient massivement recuperables.
**Faux.** Croisement du taux de recuperation avec la largeur du scan sur 317
planches : 27 % sous 600 px, 11 % entre 1000 et 1500, **3 % au-dela de 2500**.
Le taux DECROIT quand la resolution croit, parce que la taille d'un scan
renseigne sur le type de page — un tres grand scan est un poster d'artbook —
et pas sur sa lisibilite. Ne pas relancer de relecture pleine resolution.

**L'audit se trompe sur les katakana etrangers.** Deux planches signalees
charabia a 68 % et 100 % etaient exactes : une carte Super Dragon Ball Heroes
(`ベジータ / HP 3500 パワー 5300 ガード 1000 / ゴッドギャリック砲`) et un
tableau de trophees (`プラチナ ゴールド シルバー ブロンズ トロフィー`). Meme
cause que pour le lexique : **IPADIC n'arbitre pas les katakana**, et un texte
fait de mots etrangers translitteres est integralement « hors dictionnaire »
tout en etant juste. Verifier l'image avant d'ecarter une planche signalee
charabia dont le texte est majoritairement katakana.

Ce qui reste vrai : les onomatopees **dessinees** ne se lisent pas, a aucune
echelle. Ce qui a change : les bulles, si.

**Garde-fous mesures** : 30 % des planches recuperees portent une suite latine
(melange d'enseignes authentiques et d'artefacts) ; les textes de 4 caracteres
ou moins concentrent les fragments d'onomatopee mal lus (`力 力`, `二三`) et
ont ete ecartes du depot. Le filtre d'encre pour trier les regions detectees a
ete essaye et **rejete** : 0,187 de pixels sombres sur les regions avec texte
contre 0,195 sans, distributions confondues.

**Why:** ces chiffres separent ce qu'on peut automatiser de ce qui detruirait du
texte correct. Une regle a 50 % de faux positifs sur un corpus public est pire
que le defaut qu'elle corrige.

**How to apply:** toute nouvelle regle de nettoyage doit venir avec son
comptage de planches touchees **et** ses contre-exemples mesures. Le cas d'ecole
est `ベジタブル` : deux planches expliquent l'etymologie du nom de Vegeta, et une
regle `ベジタ → ベジータ` sans garde detruirait exactement le passage qui la
justifie. Voir [[vlm-ocr-lessons]] et [[shenron-databooks-bridge]].

## Passe de correction du 2026-08-25 : ce que la mesure a infirme

Quatre agents, une classe chacun, tous les runners idempotents. Corpus
**6 167 250 -> 5 976 468 signes** (-190 782, essentiellement des boucles), et
**11 255 planches transcrites, inchangees** : rien n'a ete perdu. Empreinte
avant `5ba667e4`, apres `8103c79c`. Dump prealable
`dbfr:~/backups/db_databooks-20260825-0959.sql.gz`.

**Chiffres de cette memoire qui etaient FAUX** :
- `力 力` et `二三` annonces comme fragments d'onomatopee a vider : **0
  occurrence** de `力 力` dans tout le corpus. Sur les 84 planches de <=4
  signes, 44 sont **purement numeriques** — des folios legitimes (`1990`,
  `第51話`). Vider sur le critere de longueur aurait detruit des folios corrects.
- « Bulles rendues en romaji approximatif » : population **introuvable**. Les
  259 planches romaji-only sont du latin authentique (logos, ISBN, copyright)
  plus trois ouvrages reellement anglophones.
- Marqueur de page halluciné : 62 annonces, mais le **folio authentique est un
  chiffre nu**. Discriminant mesure : #20 p.3 finit par `4` (son vrai folio)
  pendant que son en-tete annonce `**Page 4**` halluciné.
- Tokens de controle : **0** sur 11 255 planches, toutes formes confondues.
  Deja nettoyes en amont.
- Hangul cerclé : 15 annonces, **26** mesurees. Ce n'est pas une intrusion
  d'ecriture mais de l'arithmetique — Unicode met 36-50 en U+32B1 et intercale
  le hangul en U+3260, le modele a suivi les codets. `35 + (cp - U+325F)`.

**Gardes qui ont prouve leur valeur sur des contre-exemples reels** :
- Frontiere de mot sur les noms propres : `ベジタブル` intact (2 planches), et
  surtout **7 regressions evitees** sur `スーパーボンバーマン`, ou `パーボン`
  est une sous-chaine du titre Hudson. Ne jamais assouplir cette garde ; le
  reliquat agglutine (38 occurrences, 12 paires) se traite a la main.
- Chronologie comme arbitre : `ガンバー` refuse (titre de jeu de 1992, Cumber
  date de 2005) ; `トキドキ` refuse (V Jump 1997 = l'adverbe, Tokitoki date de
  2015).
- JMdict a ecarte **12 pieges** ou une regle par distance reecrivait un mot
  japonais reel en nom de personnage : `ジャンパ` (blouson) -> `シャンパ`,
  `ドルビー` (Dolby) -> `トルビー`.
- `U+FFFD` : les 46 en **fin** de texte sont du multi-octets tronque par la
  limite de longueur (toutes entre 1000 et 1550 signes) — retirables. Les 70
  autres restent : retirer souderait `使える◆けではない` en `使えるけではない`,
  faute silencieuse pire que le signal.
- `・` pleine chasse : ellipses `・{3,}` corrigees (333 planches), mais les runs
  de **2 sont des puces de liste**, et 15 317 separateurs isoles sur 4 109
  planches sont strictement inchanges.

**Non traite, et pourquoi** :
- **818 planches d'intrusions d'alphabet** (cyrillique 383, arabe 230, grec 64,
  hangul 60, thai 59) : un caractere isole substitue au milieu d'un mot
  (`げмар`, `容питしない`). Aucun motif systematique, reconstruire exige
  l'image.
- Furigana orphelins (3 737) : inchange, aucune regle fiable.
- 30 planches renvoyees en relecture, reprises par `scripts/planches-a-relire.ts`.

**How to apply:** cette memoire a donne quatre chiffres faux a une passe de
correction. Les agents les ont infirmes par le comptage **avant** d'ecrire une
regle, et c'est ce qui a evite les degats. Un chiffre de memoire est une piste,
jamais un permis d'ecrire.

---

## Module: features-opt-in-non-gardees.md

---
name: features-opt-in-non-gardees
description: "Les features Cargo opt-in d aphrody ne sont verifiees par aucun gate, du code gated peut etre casse pendant des jours."
metadata: 
  node_type: memory
  type: project
  originSessionId: 76753c0b-55f3-4cf4-b85d-b8386500c404
  modified: 2026-09-04T18:49:55.301Z
---

Sur aphrody, `ocr`, `infer`, `magika`, `forensics`, `index` et `firefly` sont
hors du set `default`. `cargo clippy --workspace --all-targets` — le gate du
depot — ne les active donc pas : il passe au vert sur du code gated qui ne
compile meme pas.

Constate le 2026-09-04 : la feature `ocr` etait cassee depuis le commit
`14632db6` (« slim default build »), qui avait retire deux fonctions de
`ocr_cmd` en laissant leur appel dans `lib.rs`, et rendu `run` synchrone sans
retirer le `.await`. Deux erreurs de compilation franches, invisibles de tous
les gates.

**Pourquoi ca compte :** le symptome cote utilisateur ne parle jamais de
features. Un binaire construit sans `ocr` fait tomber `aphrody ocr` dans le
repli A2A `auto_command`, qui echoue sur `error sending request` — on cherche
alors un serveur A2A absent pendant que le vrai probleme est le drapeau de
build.

**Comment l appliquer :** apres tout changement touchant un module gated,
lancer explicitement
`cargo check -p aphrody --features "ocr,infer,index,forensics,firefly"`.
Et rebuild le binaire installe avec ses features, jamais nu.

Voir [[aphrody-cap-local-inference]] — c est la surface OCR/inference du cap
actuel qui est concernee.

---

## Module: shenron-databooks-bridge.md

---
name: shenron-databooks-bridge
description: Shenron vit sur le VPS dbfr (pas ovh-vps) ; pont de transcription des databooks branche le 2026-08-21
metadata:
  type: project
---

**Shenron n'est PAS sur le VPS principal.** Il vit sur un second VPS, alias SSH
`dbfr` dans `~/.ssh/config`. Le VPS `ovh-vps`/`vps` (WireGuard `10.8.0.1`,
direct `51.77.147.152`) heberge bxc/niers/rg/achillea, mais aucun repo shenron —
seulement des vestiges (timers systemd, `/var/backups/shenron.conf.*`).

Sur `dbfr` : `~/shenron` (monorepo Bun, site Next sur `apps/site`), `~/bxc`,
et `~/databooks-ocr` (29 lots de planches exportees, 5 Go).

**Why:** j'ai perdu du temps a chercher shenron sur le mauvais VPS. WireGuard
etait down, donc `ssh vps` echoue ; le fallback est
`ssh ubuntu@51.77.147.152 -i ~/.ssh/ovh_vps`.

**How to apply:** pour tout ce qui touche shenron / dragonballfr.com, se
connecter a `dbfr`. Le jeton d'ecriture de l'API databooks est
`SHENRON_ADMIN_TOKEN` dans `~/shenron/apps/site/.env` — le lire sur place, ne
jamais le rapatrier. Le pont de transcription est documente dans
`docs/databooks-transcription-bridge.md` cote aphrody. Voir
[[aphrody-cap-local-inference]].

---

## Module: vlm-ocr-lessons.md

---
name: vlm-ocr-lessons
description: "Ce que les modeles de vision locaux savent et ne savent pas faire, mesure sur les planches databooks"
metadata: 
  node_type: memory
  type: reference
  originSessionId: c3252ccf-8d85-4443-b4cc-c6db11c0985c
  modified: 2026-08-22T18:47:19.666Z
---

Mesure le 2026-08-21 sur de vraies planches de databooks Dragon Ball
(RTX 4070, llama.cpp CUDA, `-ngl 99`) :

| Modele | Vitesse | Japonais imprime | Bulles manga |
|---|---|---|---|
| granite-docling-258m | ~5 s | **non** | non |
| dots-ocr (Q8, 4.1 Go) | ~7,6 s | **oui** | non |

- **granite-docling** lit la mise en page (titres latins, folios) mais classe
  tout texte japonais en `<picture>`. Il rend `4# 4# 4# 4#` quand il ne lit pas.
- **dots-ocr** lit le japonais imprime (titres, credits, ISBN, fiches, postfaces)
  de facon exploitable.
- **A l'echelle de la planche, aucun des deux ne lit les bulles de manga.**
  Longtemps compris comme « il faudrait manga-ocr ». **Corrige le 2026-08-24 :
  c'est une question de cadrage, pas de modele** — dots.ocr lit la meme bulle
  des qu'elle est recadree et agrandie. Voir la section « une planche
  `textless` en masse » plus bas.

**Why:** ces faits ont oriente toute l'architecture du pipeline OCR.

**How to apply:**
1. **Le prompt n'est pas interchangeable** : un prompt Docling donne a dots.ocr
   produit un tour VIDE, ce qui ressemble exactement a une planche sans texte —
   le mode d'echec le plus couteux. `aphrody_ocr::default_prompt` lie le prompt
   au modele. Et il doit etre la chaine amont **mot pour mot** : dots.ocr n'a
   aucune capacite de suivi d'instructions (dixit son mainteneur), il reconnait
   une de quatre chaines. La bonne est `Extract the text content from this
   image.` (`prompt_ocr`), pas une paraphrase.
2. dots.ocr repond en **markdown ou HTML**, pas en DocTags : prevoir un repli
   texte brut, sinon toute transcription est jetee.
3. Toujours detecter les boucles degenerees et **garder le bon prefixe** plutot
   que de jeter la reponse entiere.
4. Une planche illisible doit ressortir comme un **verdict** (`PageText::None`),
   jamais comme une chaine vide ni une description de l'image.
5. **Une coupure de boucle qui decoupe sur les espaces est aveugle au japonais.**
   Le japonais s'ecrit sans espaces : une generation bloquee sort comme un seul
   token ininterrompu. Mesure du 2026-08-22 sur 5023 planches : **101** portaient
   une repetition collee de plus de cent occurrences, toutes passees intactes.
   Le seuil doit etre different de celui des boucles espacees. Et le motif
   cherche doit aller jusqu'a ~40 caracteres, pas 6 : 112 planches portent un
   motif japonais de 7 a 32 caracteres repete 24 a 166 fois.

## Le piege qui a coute le plus de temps : le jeton de fin manquant

**Corrige le 2026-08-22.** Une note precedente de cette memoire affirmait que
« le backend resident (`llama-server`) lit MOINS que `llama-mtmd-cli` » et que
« ce sont les memes poids vus par une autre facade ». **C'etait faux**, et la
preuve etait dans les journaux que le serveur ecrivait lui-meme sous
`%TEMP%\aphrody-llama-server-*.log` depuis le debut.

Ce que ces journaux disent : `n_ctx_slot = 131072`, prompt 1936 jetons,
`truncated = 0` sur les 61 requetes. Et surtout : sur un lot de douze planches,
**six s'arretent a exactement 1024 jetons**, la valeur de `max_tokens`. Le
serveur ne s'arretait pas trop tot, il ne s'arretait pas du tout.

Cause : dots.ocr ferme son tour par `<|endofassistant|>` (jeton **151673**), son
GGUF ne declare aucun `eot_token_id`, et llama.cpp construit son ensemble de fin
de generation depuis une liste de **noms** en dur ou ce jeton ne figure pas.
Correctif : `--override-kv tokenizer.ggml.eot_token_id=int:151673`, sur les deux
backends.

Deuxieme cause, independante : `--jinja` est **active par defaut sur
llama-server et desactive sur llama-mtmd-cli**. Le CLI tombait sur le detecteur
de gabarits de llama.cpp qui le classait GLMEDGE — le gabarit d'un autre modele.
Deux prompts differents, deux decodages gloutons differents.

**How to apply:**
- Avant de conclure qu'un modele « lit moins » a travers une facade, **lire les
  journaux**. Un arret sur un multiple exact de `max_tokens` n'est jamais une
  page qui a fini de parler.
- Verifier que le jeton de fin d'un GGUF est bien dans l'ensemble EOG. Le
  symptome est silencieux : la suite degeneree se colle au texte sans couture,
  et un coupeur de boucles la retaille au meme prefixe — ce qui fait croire
  qu'augmenter le budget « ne change rien ».
- Le debit vient de deux endroits, aucun ne coutant de VRAM notable : une page
  finie qui s'arrete, et `--parallel N` avec N requetes en vol (la generation
  est bornee par la bande passante memoire, donc une seconde sequence est
  presque gratuite).
- Journaliser stderr des **deux** backends. Le CLI le jetait en silence, ce qui
  rendait toute comparaison asymetrique.

## Il n'y a pas de lecture reproductible (mesure 2026-08-23)

Deux runs **strictement identiques** — memes 12 planches, meme `--slots 4`,
`temperature 0`, `seed 0`, echantillonnage epingle — divergent sur **7 planches
sur 12**, dont une a 0,213 de ressemblance. Le decodage glouton n'est
deterministe que pour une composition de lot donnee, et celle-ci depend de
l'ordre d'arrivee des pages dans les slots ; les noyaux d'attention batches de
llama.cpp ne sont pas numeriquement invariants a la taille du lot.

Deux consequences :
- **Choisir un nombre de slots « pour la stabilite » n'a aucun sens.** J'ai
  failli figer `--slots 2` sur la foi d'une planche qui explosait a 6 slots ;
  elle explose aussi ailleurs. Le choix est purement une question de debit.
- Une planche peut ressortir a **47 caracteres la ou un autre passage en rend
  775** — arret precoce, pas contenu absent. Le remede ne coute aucun code :
  relire le corpus dans un JSONL parallele et fusionner en gardant la lecture
  que `ocr audit` ne signale pas, sinon la plus longue.

## Le debit se mesure sur les planches qu'on va lire

Mesure faite sur un lot leger (1600x1056, ~250 jetons generes) : 2 slots
gagnent. Corpus reel (1340x2048, **1357 jetons generes**) : la courbe s'inverse
completement — 2 slots 196 s, 4 slots 146 s, 8 slots **118 s** pour 12 planches.
La raison est dans le profil : le prompt image coute 2,3 s et la generation
13,5 s, donc le decodage pese 85 % et il est borne par la bande passante
memoire. Sur les petites planches le prompt dominait et les slots se genaient.

## `--skip-chat-parsing` ne desactive pas le parseur PEG

`llama-server` (b10549) rend un **500** `does not match the expected peg-native
format` sur environ une planche sur trois cents, alors que
`--skip-chat-parsing` est bien sur la ligne de commande. Le refus est
**deterministe** : relire par le serveur ne sert a rien. `llama-mtmd-cli` n'a
aucun parseur de chat — une passe finale avec le backend par processus rattrape
ces planches, et comme une page perdue n'est pas ecrite dans le JSONL,
`--skip-done` la represente toute seule.

## Le nombre de slots se choisit sur les planches qu'on va lire (corrige au code)

`DEFAULT_SLOTS` valait 2 et sa doc l'argumentait par la « stabilite numerique ».
Les deux etaient faux pour ce corpus, et c'est corrige dans le code
(commit `2f86bdd`, defaut a 4) : le lot d'essai qui justifiait 2 etait leger
(1600x1056, ~250 jetons), tandis que les vraies planches generent 1357 jetons
et inversent la courbe. Et il n'y a aucune stabilite a proteger — deux
passages identiques divergent sur 7 planches sur 12.

## Une planche `textless` en masse n'est pas une panne

Mesure du 2026-08-23 : l'ouvrage 312 (*DBZ TV Special : Bardock*, un film-comic)
rend 117 `none` sur 163 planches. Lecture de l'image faite : elles portent bel
et bien du texte — カナッサ星, une bulle クッ!!, les onomatopees グォーッ et
ドゥッ, un folio. C'est du texte de manga, hors domaine du modele, exactement
comme prevu plus haut.

**How to apply:** devant une vague de `textless`, verifier la **categorie de
l'ouvrage** avant de soupconner le prompt ou le backend. Le mode d'echec du
mauvais prompt (§1) produit le meme symptome : ce qui les distingue, c'est que
le prompt casse rend TOUT vide, y compris les fiches techniques bien imprimees.

## Ce n'etait pas le modele, c'etait le cadrage (2026-08-24)

La note ci-dessus concluait que ces categories « plafonnent structurellement ».
Elles ne plafonnaient pas : personne n'avait donne au modele une bulle
**pre-decoupee**, alors meme que la conclusion parlait de modeles travaillant
sur bulles pre-decoupees.

Mesure : `クッ!!` illisible dans sa planche, lu parfaitement une fois recadre et
agrandi ×3. Le pavage ne suffit pas — douze tuiles ×3, six fois le prix, la
bulle reste muette. Ce que le modele veut est une image dont le texte est le
**sujet**, pas plus de pixels. Sur une planche de 1128x1600 une bulle fait
130x100 : une poignee de jetons visuels dans ce qu'il traite comme un dessin.

D'ou `aphrody ocr bulles` : **235 planches muettes sur 300 rendues a leur
texte**, Jump Anime Comics de 54,3 % a 72,0 %, corpus a 92,9 %.

**How to apply:**
- Avant de conclure qu'un modele ne sait pas lire quelque chose, verifier a
  quelle **echelle** on le lui a montre. C'est le meme genre d'erreur que le
  jeton de fin manquant plus haut : un symptome reel, une cause supposee.
- Le tri des regions detectees ne se fait pas au taux d'encre — mesure et
  rejete, 0,187 contre 0,195, distributions confondues : les faux positifs
  sont des zones claires DANS des dessins, pleines de traits.
- La sortie par bulle demande son propre garde-fou : un fragment de glyphe fait
  inventer des caracteres plausibles (`禁 幸`), et une bulle lisible peut sortir
  en romaji approximatif (`ありがとうございます` -> `ary ga thu`). Ecarter les
  textes de 4 caracteres ou moins retire l'essentiel du bruit.
- Les onomatopees **dessinees** restent hors de portee, a toute echelle.

## Une reprise qui compte juste et refait tout (2026-08-24)

`ocr bulles --skip-done` a annonce « 595 planches a relire, 448 deja faites »
puis les a **toutes relues**. Le compte etait juste, le filtre non : il
comparait des `PathBuf` bruts, et la reprise avait ete lancee avec un chemin
relatif la ou la premiere passe portait un absolu. Vingt-six minutes de GPU
pour rien, sans qu'aucune erreur ne soit levee.

**How to apply:** quand une commande re-enracine ses entrees sous une option
de chemin (`--images`), l'identite d'un element est son **nom**, jamais le
chemin reconstruit. Et un compteur « deja faites » qui s'affiche correctement
ne prouve pas que le filtre s'applique : verifier le nombre reellement mis en
file, pas celui des deja-vus.

Effet de bord garde : ces 448 secondes lectures, fusionnees aux premieres en
conservant la plus longue de chaque planche, ont fait gagner du texte a **83
planches** — dont une muette dans un passage et lue dans l'autre. C'est la
strategie de fusion decrite plus haut, obtenue gratuitement.

## Un LLM japonais ne nettoie pas ce corpus (mesure 2026-08-25)

`LFM2.5-1.2B-JP` Q8_0 (Liquid AI), pull et verifie. Choisi sur mesures : il bat
Qwen3-1.7B sur les evaluations japonaises, la ou **Sarashina2-7B, six fois plus
gros et japonais par conception, plafonne a 0,400 JMMLU** (Qwen3-8B-Base :
0,714). Le candidat evident n'etait pas le bon.

Vitesse jamais en cause : **0,07 s par requete** sur RTX 4070, soit une minute
pour tout le corpus. Le modele fonctionne — il repond `悟空` a « qui est le heros
de Dragon Ball » et `東京` a la capitale du Japon.

**Il sait generer, il ne sait pas evaluer** :

| Usage | Resultat |
|---|---|
| Reparer un caractere etranger substitue | **0 proposition attestee sur 40** — il translittere au son (`م`→`ま`) au lieu de lire le contexte |
| Juger si un texte est du japonais valide | **50 %** sur jeu equilibre = reponse constante ; en question ouverte il declare une boucle degeneree « grammaticalement correcte » |
| Choisir entre deux graphies | 67 %, mais voir ci-dessous |

**Le detail qui tranche : ses 4 erreurs sur le choix ferme sont exactement les
4 pieges** que le travail deterministe avait deja identifies — `不況の底値スラック`
(horoscope financier) rendu `スラッグ`, `風来のシレン` rendu `ジレン`, `天才ピート`
rendu `ビート`, et surtout **`ベジタブル` casse en `ベジータブル`**, le temoin qui
protege la planche d'etymologie du nom de Vegeta. Il reussit la ou le lexique
repondait deja, et echoue la ou la difficulte est reelle.

**Non teste** : le meme jeu sur un modele plus gros. Le pull de Qwen3-8B a ete
interrompu, SHA non verifie. La conclusion vaut pour un 1,2 B, rien au-dela.

**Why:** l'intuition « un modele japonais natif reglera le reliquat » est
fausse pour la correction, et le cout de l'essayer sans garde aurait ete de
detruire precisement les passages qu'on avait protege a la main.

**How to apply:** sur ce corpus, **l'ancrage bat le modele**. Une graphie
attestee par 11 255 planches du meme domaine est une preuve ; une proposition
plausible n'en est pas une. Si un modele revient un jour dans la boucle, garder
le principe qui a bloque 4 destructions sur 12 cas : **il propose, le dump
tranche**. Voir [[corpus-databooks-defauts]].

---

