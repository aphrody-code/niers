# Couverture Lua runtime — Kizuna

Date de clôture : 2026-09-06.

## Résultat

Le chemin Lua brut VFS → décodage → VM Lua 5.2 unsafe → hôte de menu → état de
runtime est opérationnel pour le menu `kizuna_town_mainmenu`.

- Scripts réels décodés : **1 143/1 143**, soit **985 971 instructions**.
- Audit ciblé Kizuna : **25/25 scripts exécutés**, 0 erreur, 0 include manquant,
  0 appel hôte manquant.
- Décodage ciblé Kizuna : **25/25 chunks décodés**, 0 erreur, **31 957
  instructions** parcourues par le décodeur Rust.
- Audit VFS complet : **1 197/1 197 scripts exécutés**, 0 erreur, 0 include
  manquant.
- Runtime Kizuna : **102 commandes connues**, 0 commande menu inconnue,
  0 commande générale inconnue.
- Trace `INCLUDE` Kizuna : **25 modules distincts résolus**, 0 include manquant
  (les 25 scripts audités partagent bien les modules dans la résolution VFS).
- `lua-run` sur le chunk primaire Kizuna : `decoded=true`, **1 933
  instructions**, erreur de décodage nulle et aucune invocation hôte inconnue.
- Export runtime : 21 objets de layout, 8 objets Lua, 12 objets mutés,
  10 masqués, 9 sprites et 9 textes mis à jour.

## Travaux réalisés

`LuaSession` conserve désormais le même état de menu lors de l’exécution et du
rechargement de VM. `drive_menu_for_frames` permet d’exécuter plusieurs frames
avec le même résolveur d’include et le même hôte.

La sélection d’un include versionné compare ses composants de version
numériquement, afin qu’un fichier `..._10...` ne soit pas devancé par
`..._9...` selon l’ordre ASCII.

`ExecOutput` expose désormais `loaded_includes`, dans l’ordre réel de chargement.
`lua-run` le rend dans `loadedIncludes` et `lua-audit` l’agrège par nom de module,
ce qui rend la résolution VFS observable et vérifiable.

`lua-audit` compte aussi séparément les chunks décodés et le nombre total
d’instructions, afin qu’un succès VM ne masque pas une divergence du décodeur.

La même instrumentation est maintenant disponible sur `LuaSession` via
`take_loaded_includes()`. Elle survit à `reload()` et se prélève séparément,
ce qui permet à une console ou à un pilotage live de distinguer un module
réellement chargé d’un simple état global déjà présent.

`LuaSession::with_script_paths` fournit désormais le branchement VFS standard :
il construit l’index physique/logique, sélectionne la version numérique correcte
et délègue la lecture au reader brut de l’appelant. Un test vérifie la sélection
de `module_10` devant `module_9`.

`RuntimeContext` fournit l’injection typée des globals primitifs que le manager
natif pose avant un callback (nombres, booléens, chaînes). Le contexte est
appliqué après les stubs, conservé par la session et réappliqué au `reload()` ;
il permet donc de fournir les valeurs de scène/save réellement connues sans
transformer un manque de contexte en table Lua truthy.
Lorsqu’un écran remplace le contexte, les anciennes clés absentes du nouveau
contexte sont supprimées de la VM avant injection ; aucun slot de scène ne fuit
ainsi dans le menu suivant.
Le même type est désormais porté par `ExecOptions` : `execute` et
`execute_with_include` utilisent ainsi ce contexte sur le chunk principal comme
sur les modules VFS inclus.
`LuaSession::call_menu_callback` expose aussi les événements host→Lua de
navigation (`OnSetupLayer`, `OnOpenLayer`, `OnCloseLayer`, `OnCloseEndLayer`,
`OnChangeLayerGroup`) sur la même VM, avec injection de contexte par événement.

`execute_with_script_paths` centralise désormais le chemin brut VFS → index des
scripts → résolution logique/versionnée → VM. `lua-run` et `lua-audit` l’utilisent
directement, ce qui supprime leurs résolveurs divergents et vérifie la sélection
numérique des versions au même endroit.
`ExecOutput.decoded_instructions` expose en plus la mesure du chunk principal
décodé dans le même appel que son exécution ; le CLI la publie sous
`liveDecodedInstructions`.

Le driver `nie-game --runtime` utilise maintenant cette session persistante
plutôt qu’une VM et des index d’include reconstruits à la main. La vérification
Kizuna réelle termine avec `on_init=true`, `on_open=true`, 102 commandes
connues et 0 inconnue.
L’export `runtimeSummary.loadedIncludes` conserve aussi les modules chargés et
leur fréquence (notamment `LUA_KIZUNA_TOWN_MENU_INC`, `LUA_MENU_DEF` et
`LUA_PROG_BASE`).
Le même export contient désormais `decodedScripts`, `decodeErrors` et
`decodedInstructions` ; Kizuna vérifie `1/1`, `0` et `1 933` respectivement.

`menu_host` couvre les commandes Kizuna de visibilité, couleur RGBA, paramètres,
texture et application de flags. Les commandes générales identifiées par le RE
sont décodées avec leur protocole de retour ; les requêtes d’état sans donnée
native disponible renvoient un neutre explicite et déterministe.

Les espaces de noms et constantes observés dans les scripts sont injectés avec
leurs valeurs CRC32 connues, notamment les recettes de Chara Edit, les os,
les types de tutoriel, les types d’onglet et les constantes de texture/texte.
L’état `partVisible` et `partColorRgba` est propagé jusqu’à l’export du layout.
Les mutations Kizuna `SetPartTexture`, `SetPartParam` et `ApplyPartFlags` conservent
également leurs arguments numériques bruts par partie, exportés sous
`partTextureArgs`, `partParamArgs` et `partFlagArgs` sans leur attribuer un sens
non prouvé.

## Vérifications

```text
cargo test -p nie-lua --lib
97 passed, 0 failed, 1 ignored

cargo clippy -p nie-lua --lib --tests -- -D warnings
cargo clippy -p nie-game --bins --tests -- -D warnings
success, 0 warning sur les deux cibles
```

Les audits ont été lancés avec `NIE_GAME_DIR`/`--game-dir` vers l’installation
locale du jeu, sans chemin machine écrit dans le code ou dans ce rapport.

Le désassembleur résout aussi les cibles de saut avec la règle Lua
`pc + 1 + sBx` et affiche les arités réelles de `CALL`, `TAILCALL` et `RETURN`
(`B-1`/`C-1`, `vararg` et `multret` inclus). Un test VM généré vérifie ce listing.
Il rejette désormais explicitement les formats, tailles C et endianess d’en-tête
incohérents, au lieu de tenter un décodage ambigu.
`LOADKX` est également résolu avec l’`EXTRAARG` suivant, comme dans le
bytecode Lua 5.2, au lieu d’être présenté à tort comme `K0`.
Les lectures bornées du décodeur vérifient désormais les additions de curseur
et les conversions `size_t` avant tout accès mémoire ; le corpus réel confirme
toujours **1 143/1 143 scripts** et **985 971 instructions** décodés.
Le chargement live appelle maintenant ce même décodeur avant `mlua` pour le
chunk principal de `execute`, `LuaSession::exec/attach`, le pilotage de menu,
**et chaque `INCLUDE` binaire**. Un test de régression vérifie
qu’un conteneur malformé est refusé avant son exécution et qu’un include invalide
remonte son nom logique dans l’erreur.

## Limites connues

L’audit complet signale 13 paramètres non résolus dans un script d’effets
générique (`x`, `y`, `layerIdx`, `pieceIdx`, `pieceType`, `effectIdx` et
métadonnées associées). Ils ne provoquent aucune erreur d’exécution et ne
concernent pas le chemin Kizuna ciblé. Les effets natifs dont le binaire ne
fournit pas encore de sortie observable restent modélisés par un état neutre
documenté ; cela ne constitue pas une preuve d’identité pixel-perfect du jeu
complet.

L’audit conserve maintenant la provenance de chaque manque. Les paramètres de
pièce viennent de `ability_learning_board_menu_7.00.00.00.lua.bin`, les
coordonnées `x/y` de quatre menus de recherche/summon, et `MENU_LINIT_NONE` de
`soccer_top_menu_1.03.98.00.lua.bin` ; cette liste est donc actionnable pour le
prochain RE.

La classification runtime confirme que ces 13 résidus sont des **lectures
uniquement** : l’audit global rend `missingHostInvocations={}`. Aucun appel de
fonction hôte inconnue ne reste donc dans le corpus actuel ; les valeurs à
injecter concernent le contexte de données fourni aux scripts.

Un build workspace complet n’a pas été lancé, conformément à la règle du dépôt
qui le déconseille lorsque l’espace disque est contraint.

## Clôture de session — 2026-09-06

Le lot de reproduction Lua/Kizuna est versionné et poussé sur `main` jusqu’à
`cbe5fde` (`feat: validate live Lua bytecode before execution`). Il couvre le
décodeur Lua 5.2, l’index VFS à versions numériques, les includes persistants,
le pilotage live du menu, les hôtes Kizuna, les métriques d’audit et leur
provenance.

Les modifications concurrentes de formatage présentes dans l’arbre de travail
ne sont pas incluses dans ce lot et restent à arbitrer par leur auteur. Aucun
chemin machine, secret ou dump hors périmètre n’a été ajouté. La prochaine
étape RE clairement identifiée est l’injection documentée du contexte natif
des 13 lectures résiduelles ; elle est distincte de la couverture Kizuna déjà
validée et ne doit pas être remplacée par des valeurs inventées.
