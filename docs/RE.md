# Reverse-engineering de `nie.exe`

Le RE est **le moyen** (résoudre `nie.exe` pour porter la logique en Rust), pas la fin. Ce
document décrit la cible, la boucle qui l'attaque, et ce qui en est établi.

## La cible

`nie.exe` est **à la racine du dépôt** (pas dans `data/`). Base image `0x140000000`.

> **La racine porte deux liens symboliques vers l'installation Steam**, `nie.exe` et
> `nie_eacpatched.exe` → `~/.local/share/Steam/iecode/inazuma/`. **Vérifié 2026-08-15 : les deux
> sont actuellement BYTE-IDENTIQUES** (même sha256 `b1fa04ea3658…`, `stat -L -c %s` = 33 918 464
> pour les deux) — le patch EAC n'a rien à modifier sur ce build, ou n'a pas été réappliqué ;
> à re-vérifier si un `steam-update-post.sh` réapplique le patch plus tard.
>
> **Correction du 2026-08-14 (commit `22b1177`) elle-même périmée** : entre le 2026-08-14 soir et
> le 2026-08-15, l'installation Steam locale portait transitoirement un AUTRE build
> (31 468 032 octets, sha `4c2b91fbae6f…`, `app_config_5.00.24.00`) que ce commit avait pris pour
> la nouvelle référence. La MAJ Steam du 2026-08-15 (cf. mémoire `maj-steam-ievr-etat`, bug du bit
> répertoire 64≠2 corrigé) a ramené l'installation au build `6.00.23.00` / 33 918 464 octets / sha
> `b1fa04ea3658…` — **exactement celui que ce tableau décrivait avant la « correction » du
> 2026-08-14**, restaurée ci-dessous. Leçon : un « c'est corrigé » qui ne cite pas le sha256 se
> périme au prochain download Steam sans que rien ne le signale — toujours mesurer
> (`stat -L -c %s`, `sha256sum -L`) avant de croire la doc.

| Propriété | Valeur |
|---|---|
| Format | PE32+ x86-64, Windows GUI, 9 sections, **non strippé** |
| Taille | 33 918 464 octets, sha256 `b1fa04ea365868e5c8933aca393366f82d0d446187e2187f2737dc4fa2acd40c` (vérifié 2026-08-15, `nie.exe` et `nie_eacpatched.exe` identiques) |
| Éditeur / produit | LEVEL5 Inc. — *INAZUMA ELEVEN: Victory Road* (nom interne `nie1v2.exe`) |
| Linker | MSVC 14.44 — le toolset à réutiliser pour la forge |
| PDB de build | `G:\nie1v2\program\main\program\SteamRelease\x64\nie.pdb` (symboles absents du dump) |
| RTTI | **1 745 classes** ingérées en base (`rtti_class`, mesuré 2026-08-29 sur `b1fa04ea3658…`). Les comptes de 1 234, 1 575 (ancien) et 3 336/3 150 (build transitoire du 2026-08-14) valaient pour d'autres builds — ne pas les citer |
| Exports | 2 seulement : `AmdPowerXpressRequestHighPerformance`, `NvOptimusEnablement` |
| Imports | **485 fonctions** sur 20 DLL (mesuré 2026-08-29, `pefile`). Les plus grosses : KERNEL32 (184), EOSSDK-Win64-Shipping (152), USER32 (57), WS2_32 (28). Le reste du moteur est **statiquement lié** : PhysX 3.4, CriWare, CryptoPP n'apparaissent pas dans l'IAT |
| Version de l'app | `6.00.23.00` (`common/system/app_config_*.cfg.bin`) |

### Sections PE

| Section | VSize | RawSize | Contenu |
|---|---|---|---|
| `.text` | 25 601 760 | 25 602 048 | Code exécutable |
| `.rdata` | 4 399 310 | 4 399 616 | Read-only : chaînes, vtables, imports |
| `.data` | 10 189 132 | 2 413 568 | Globales (BSS creux) |
| `.pdata` | 1 226 652 | 1 226 752 | Unwind SEH x64 — **la vérité terrain des bornes de fonction** |
| `.reloc` | 201 036 | 201 216 | Relocations |
| `.rsrc` | 61 552 | 61 952 | Icône, manifeste, version |
| `_RDATA` | 10 672 | 10 752 | Runtime data |
| `.rodata` | 992 | 1 024 | Constantes |
| `.fptable` | 256 | 512 | Table de pointeurs de fonctions |

### Technologies

| Composant | Détail |
|---|---|
| Moteur | Level-5 propriétaire, classes `gmdC*` — ni Unreal ni Unity |
| Rendu | DirectX 11 (`d3d11.dll`, `dxgi.dll`, `D3DCOMPILER_47.dll`) |
| Physique | NVIDIA PhysX (`PhysX3Gpu_x64.dll`) |
| Audio / vidéo | CRI Middleware — CriFs, CriAtomEx (ADX/HCA), Sofdec2, CriMana (VP9) |
| Scripting | Lua 5.2 (`LUA_PATH_5_2`, patterns `!\lua\?.lua`) |
| Réseau | Steamworks (`steam_api64.dll`), libcurl, Winsock2 |
| Anti-triche | EasyAntiCheat |

Constantes utiles : App ID Steam `2799860` · clé XOR CRI `0x1717E18E` · footer `cfg.bin`
`01 74 32 62 FE`.

## Outillage

**`r2` et `objdump` ne sont pas installés.** Désassembler passe par le crate `nie-re`
(iced-x86) ou `uv run --with capstone <script>`. Les bornes de fonction viennent de `.pdata`.

### `refs/` — dépendance de build, hors dépôt

`refs/iecode-re/` porte l'index Ghidra/iecode dérivé de `nie.exe` : 124 Mo, 1 628 fichiers.
Dix-huit fichiers **suivis** y font référence (`nie-core` pour la physique, `nie-seed` pour
les classes RTTI) et `just re-seed` lit `refs/iecode-re/research/nie-index.json`. Un clone
frais ne peut donc pas rejouer la boucle RE sans lui.

Il n'entre pas dans ce dépôt-ci — c'est un dérivé d'un binaire sous droits, son poids
dépasse celui du reste du dépôt, et il porte déjà son propre `.git`. Il est explicitement
ignoré (`.gitignore`, ligne `/refs/`) plutôt que laissé en `??`, où un `git add -A`
distrait l'aurait fait entrer. Le récupérer sur un clone frais :

```bash
git clone https://github.com/aphrody-code/iecode.git refs/iecode-re
```

Les deux fichiers dont dépendent les recettes — `research/nie-index.json` et
`research/nie-rtti-classes.txt` — y sont versionnés : le clone suffit, rien à reconstruire.

Les recettes qui en dépendent échouent tôt avec un message explicite (`justfile`, `test -f`
sur `{{seed_json}}`) plutôt que par une erreur de parsing à mi-chemin.

## La boucle

```
seed  →  rebuild (pdata → vtable → disasm → propagate)  →  coverage
```

| Étape | Recette | Ce qu'elle fait |
|---|---|---|
| Ingestion | `just re-seed` | Importe le savoir fusionné comme ancres : formats iecode, tables hash→nom inagle, classes RTTI |
| Refonte | `just re-rebuild` | Reconstruit la carte sur `.pdata`, ré-ancre, désassemble, propage |
| Couverture | `just re-coverage` | `cov <classifiées>/<total> (<pct>%) named=<n>` |
| Tout | `just re-all` | Les trois, fail-fast |

> L'ordre compte : `disasm` avant `rtti` produit un résultat incomplet **sans erreur**. Toujours
> passer par `just re-rebuild`, qui orchestre les sous-étapes, plutôt que les CLI brutes.

`niers seed`, `rtti`, `index`, `pdata`, `rebuild`, `disasm`, `propagate`, `coverage`, `queue`,
`textures`, `uniform-map`, `menu-predecode`, `save`, `wiki`, `mem` (runtime, `nie-trace`).

**Stores** : `var/niers.sqlite` (base de connaissance — tables `function`, `xref`, `coverage`,
`rtti_class`, `func_str_ref` ; schéma dans `crates/forge/nie-index/src/schema.sql`) et redis db0
(frontière BFS `nie-queue`) / db3 (index fichiers CPK et textures). Piège : `NIERS_REDIS`
surcharge **toutes** les commandes — ne pas l'exporter pour `textures`/`menu-predecode`, qui
visent db3.

## L'index Ghidra est désaligné — `.pdata` est la vérité terrain

C'est la découverte structurante de la boucle, et elle explique pourquoi deux couvertures
coexistent dans la base.

Vérification byte-à-byte contre `.pdata` (table d'unwind générée par le compilateur, donc
incontestable) :

- `.pdata` contient **94 748 entrées** `RUNTIME_FUNCTION` = 44 074 fragments chaînés
  (`UNW_FLAG_CHAININFO`) + **50 674 fonctions racines** réelles.
- Des 59 991 adresses `FUN_<hex>` de l'index Ghidra, **2 243 seulement (3,7 %)** coïncident avec
  un début de fonction réel ; **≥54,9 %** tombent *strictement à l'intérieur* d'un corps, et
  l'ensemble est artificiellement aligné sur 16 octets à 99,2 %.
- Spot-checks décodés : les adresses non alignées pointent sur des épilogues ou des milieux
  d'instruction.
- Le champ `ce` (callees) de l'index n'est pas le graphe d'appels directs réels — vérifié : une
  fonction dont le décodage montre trois `call` précis a un `ce` listant cinq fonctions toutes
  différentes.

**Conséquence** : l'index Ghidra reste exploitable comme graphe de métadonnées (chaînes,
namespaces, relations), mais ses adresses ne sont pas des débuts de fonction physiques. La vraie
couverture se mesure sur les racines `.pdata`.

Le pipeline `rebuild` travaille donc sur des adresses correctes :

1. **`pdata::rebuild_from_pdata`** — carte reconstruite sur les 50 674 racines ; métadonnées
   Ghidra ré-ancrées **par inclusion** (17 403 chaînes, 340 100 constantes, 55 142 arêtes repliées,
   1 575 classes RTTI).
2. **`vtable::vtable_edges_into`** — lecture des slots `.text` de chaque vtable localisée par
   RTTI : 6 681 méthodes, dont **2 109 fonctions feuilles** (sans unwind, donc absentes de
   `.pdata`) ajoutées comme nœuds, plus 13 927 arêtes de cohésion de classe.
3. **`disasm`** depuis les bons débuts — 169 828 arêtes d'appel directes réelles.
4. **Propagation** pondérée sur le graphe `call` + `vtable`, avec amortissement de degré
   (`1/ln(deg+2)`) pour qu'un utilitaire appelé par des milliers de fonctions ne domine pas le
   label de ses voisins.

### `.pdata` ne voit que 88,37 % de `.text` (mesuré 2026-08-29)

Sur le binaire cible `b1fa04ea3658…`, mesuré directement dans le fichier :

- `.text` = **25 601 760 octets** ; `.pdata` = **102 221 entrées** `RUNTIME_FUNCTION`, qui se
  replient en **53 668 plages fusionnées** couvrant **22 625 021 octets — 88,37 %**.
- Les **2 976 739 octets restants**, en **53 669 trous**, sont des fonctions *feuilles* : pas de
  prologue, pas d'unwind, donc aucune entrée `.pdata`. Codecs SIMD (un bloc de 94 736 octets d'un
  seul tenant en tête de `.text`), thunks d'ajustement d'héritage multiple, accesseurs générés par
  instanciation de patron. **Elles n'existaient dans aucune table** — ni comme nœuds, ni comme
  cibles d'appel.

`nie_re::recover` (commande `niers recover`) les récupère par point fixe — références directes
(`call`/`jmp rel32`, pointeurs de données) puis balayage linéaire des résidus recalé sur la
frontière de 16 octets — chaque début n'étant retenu que si son décodage atteint un terminateur
réel. La provenance est distinguée : `leaf-ref` (désignée par une référence) vs `leaf-scan`
(balayage seul). **98,27 % des trous sont expliqués** (code attribué + remplissage), résidu
**51 553 octets**, soit 0,20 % de `.text`.

`nie_re::vtable_anon` complète `vtable` : `.rdata`/`.data` portent **3 078** suites d'au moins
trois pointeurs `.text` consécutifs, dont **1 528 sans COL RTTI** (37 334 slots, 11 588 méthodes
distinctes) — classes compilées sans RTTI, tables de rappels, tables d'interface. Elles donnent
16 428 arêtes de cohésion.

### Les deux chiffres de la base, et lequel citer

`var/niers.sqlite` indexe deux espaces sous deux `binary_id`. Ne pas les confondre :

| Espace | Fonctions | Classées | Nommées | Statut |
|---|---|---|---|---|
| `#pdata` — fonctions réelles | 117 521 | 102 053 (86,84 %) | 49 431 (42,06 %) | **La mesure à citer** (2026-08-29) |
| Index Ghidra — nœuds désalignés | 60 183 | 53 083 (88,20 %) | 192 | Référentiel historique, figé |

Le dénominateur a **doublé** dans la session du 2026-08-29 (57 779 → 117 521) : ce n'est pas une
régression de couverture quand le pourcentage classé baisse, c'est l'apparition de 59 742
fonctions qui existaient dans le binaire et manquaient à la base. Comparer un pourcentage à
dénominateur mouvant n'a pas de sens — citer les deux nombres.

Évolution mesurée dans cette session (même binaire, mêmes outils) :

| Mesure | Avant | Après |
|---|---|---|
| Fonctions connues | 57 779 | 117 521 |
| Nommées | 7 539 (13,05 %) | 49 431 (42,06 %) |
| Classées (brut) | 52 308 | 102 053 |
| Classées avec confiance ≥ 0,3 | 5 248 (9,08 %) | 33 672 (28,65 %) |
| `.text` hors `.pdata` expliqué | — | 98,27 % |
| Chevauchements de fonctions | — | 272 (dont 131 entre racines `.pdata` chunkées) |

La ligne « confiance ≥ 0,3 » est la plus parlante : les ancres dures ajoutées (funcLua à 0,9,
héritage de thunk, contiguïté à 0,5) ne se contentent pas d'étiqueter plus de fonctions, elles
remplacent des étiquettes de propagation quasi nulles par des labels que la propagation peut
ensuite diffuser. C'est ce qui fait passer ce chiffre de 5 248 à 33 672.

**Nommage** : cinq sources, dont une seule est *sémantique*. Aucune ne prétend restituer le
symbole C++ d'origine — le PDB n'est pas dans le dump.

| `name_source` | Nombre | Forme | Fondement |
|---|---|---|---|
| `leaf-shape` | 23 825 | `thunk_to_<va>`, `get_const_<K>`, `get_ptr_<va>`, `stub_<va>` | forme syntaxique lue dans les octets |
| `vtable-anon-struct` | 10 149 | `vtbl_<va>::slot_N` | adresse de table + rang, aucune classe connue |
| `vtable-struct` | 7 282 | `Namespace::Classe::vmethod_N` | classe RTTI + rang de slot |
| `funclua` | 6 742 | `funcLuaCmd_<cmdId>` | table de répartition du script (identifiant exact, nom d'origine inconnu) |
| `strref` | 1 164 | `fn_<identifiant>` | **sémantique** : la seule chaîne identifiante que cette fonction, seule, manipule |

Un thunk hérite du sous-système de sa cible (`subsys_src='thunk-inherit'`) : identité
structurelle, elle prime donc sur l'étiquette statistique de la propagation (`ml`). Le résidu sans
arête entrante est classé par contiguïté d'adresse (`adjacency`, 20 998 fonctions, cohérence
mesurée 87,9 % — cf. le module, qui s'auto-évalue à chaque exécution).

### Le hachage des `cmdId` funcLua reste inconnu — piste close

Les `cmdId` sont des hachages de noms de commandes, calculés hors ligne par la chaîne de build de
LEVEL-5 : aucune table inverse dans le binaire. Testé le 2026-08-29, **16 726 identifiants du
binaire × 7 123 cmdId**, pour sept familles de hachage — CRC-32 (ISO-HDLC, JAMCRC, BZIP2, MPEG-2,
POSIX, CRC-32C/D/Q, AUTOSAR), FNV-1, FNV-1a, djb2, djb2-xor, sdbm, ELF : **5 correspondances en
CRC-32, 0 partout ailleurs**, soit moins que les ~27 collisions attendues par pur hasard à ce
volume. Ne pas rouvrir cette piste sans un élément nouveau (un script Lua *source*, ou l'outil de
build). À noter : les noms d'écran de menu, eux, **sont** du CRC-32 standard (vérifié 200/200 sur
`hash_name`) — c'est une fonction de hachage différente pour les cmdId.

**Plafond honnête** : 68 090 fonctions restent sans nom et 15 468 sans sous-système. Ce dernier
résidu est un plafond *structurel*, pas un oubli : vérifié le 2026-08-29, **les 15 468 sont
encadrées par deux sous-systèmes différents** — aucune n'est en bordure de section, aucune n'a un
encadrement concordant inexploité. Elles sont exactement à la frontière entre deux unités de
compilation, et les classer reviendrait à choisir arbitrairement l'un des deux voisins. La règle
de contiguïté a épuisé ce qu'elle peut affirmer. Le résidu de nommage, lui, est isolé autrement :
ni chaîne, ni RTTI, ni arête vers une fonction étiquetée. Le nommage sémantique
généralisé reste hors d'atteinte sans PDB : sauf pour les 1 164 `strref`, ce qui est produit ici
identifie sans ambiguïté, il n'interprète pas.

Les 51 553 octets de `.text` encore non attribués (0,20 %) se répartissent en 2 260 blocs qui sont
majoritairement de l'**intérieur** de fonctions : blocs froids de boucles SIMD atteints seulement
par une branche, ou fragments dont le décodage démarre au milieu d'une instruction. Les récupérer
demanderait une analyse de flot par fonction — un chantier distinct, au rendement faible à ce
niveau. Le point de rendement décroissant est ici.

### Ce que ça donne bout à bout

`niers`/MCP `re_function` sur `fn_SetNaviStopLayerVisible` (`0x14179fa70`, 6 940 octets,
sous-système `menu`) — un nom obtenu par la seule chaîne identifiante que cette fonction manipule :

- **appelée par** `game::CMapMenu::vmethod_0`, `game::CMenuMinimap::vmethod_11`,
  `game::MoveMainState::vmethod_2` — noms venus du RTTI, tous cohérents avec « couche de
  navigation » ;
- **et par trois handlers de commande Lua** (`funcLuaCmd_19bd7c0b`, `_0db0187f`, `_7d9fd76b`) :
  le script du jeu pilote donc directement cette fonction ;
- **appelle** `__security_check_cookie` et `__chkstk`, reconnus par ailleurs.

Quatre sources indépendantes — chaîne, RTTI, table de répartition du script, CRT — convergent sur
le même sens sans avoir été rapprochées à la main. C'est le signe que les couches se recoupent
plutôt qu'elles ne s'empilent.

### La boucle à rejouer

```bash
niers recover --db var/niers.sqlite --exe nie.exe   # feuilles, formes, vtables anonymes,
                                                    # chaînes, funcLua, contiguïté, snapshot
niers rebuild --db var/niers.sqlite --exe nie.exe   # propagation sur le graphe enrichi
```

Les deux passes sont **idempotentes** et se renforcent : `recover` pose des ancres dures que
`rebuild` diffuse, et `rebuild` produit des labels dont `recover` se sert pour la contiguïté.
Deux tours suffisent à converger sur ce binaire.

## Ce que le RE a établi sur le binaire

### Namespaces

**`lives::`** — moteur bas niveau : `CVector2`, `TVector3`, `TVector4`, `CVectorBase3/4`,
`hash32`, `SCREEN_STRETCH`, `CCT_SHAPE_TYPE`, `CRand`/`CPseudoRand`/`IRand`.

**`game::`** — gameplay IEVR : `CGameCameraParam`, `CModelIK`, `CCharaAlphaState`,
`CCharaWaterEffectComponent`, `CCharaEditCustomMdlComp`, `CCustomAnimePlayer`, `CRopeComponent`,
`CEffectLightData`, `WorldCharaCol`, `COL_PART_INFO`, `CHARA_EDGE_PARAM`.

Liste complète : [`nie-rtti-classes.txt`](nie-rtti-classes.txt).

### Classes moteur `gmdC*`

ECS custom avec `gmdCObject` en base : `gmdCObjModel`, `gmdCObjModelComponent`,
`gmdCObjModelIK`/`IkJob`, `gmdCObjModelLodInfo`, `gmdCObjBlendShapeManager`,
`gmdCObjDecalComponent`, `gmdCLookAtComponent`, `gmdCAnimation`/`Async`/`RefAnim`,
`gmdCObjPlayAnime`(`Manager`), `gmdCShareObjAnimeList`(`Manager`), `gmdCDrawObjModelPriority`.

### Classes soccer

`CSoccerCtrl`/`Base`, `CSoccerCtrlAI`(`StateMachine`), `CSoccerStateMachine`, `CSoccerCharaData`,
`SoccerCharaCtrl` (+`InPlay`/`SetPlay`/`Zone`), `SoccerCharaTacticsAI`, `SoccerTacticsAI`,
`SoccerPlayCmdManager`, `CharaPlayCmdManager`, `SoccerCommandEffect*`, `BallComponent`,
`IBallMoveController` (+`BallMoveDribble`, `BallMoveRealSkillShootBezier`),
`CRpgBattleShootTurnManager`, `DribbleTurnManager`, `GoalnetComponent`,
`SoccerCalcKeeperSaveComponent`, `CSceneSoccer`(`Training`).

Détail du modèle de match : [`modele-de-match.md`](modele-de-match.md).

### Système GDS — 268 classes de configuration

Toutes les configs sont des classes `GDS*Config` chargées depuis `cfg.bin` :

| Domaine | Classes clés |
|---|---|
| Personnages | `GDSCharaBase`, `GDSCharaParam`, `GDSCharaModel`, `GDSCharaMotion`, `GDSCharaExpTableConfig` |
| Skills | `GDSSkillConfig`, `GDSRealSkillConfig`, `GDSAuraSkillConfig`, `GDSOverrideSkillConfig` |
| Passifs | `GDSPassiveSkillConfig`, `GDSPassiveSkillEffectConfig`, `GDSPassiveSkillRarityTableConfig` |
| Soccer | `GDSSoccerGameConfig`, `GDSSoccerCameraConfig`, `GDSSoccerPhaseConfig`, `GDSSoccerRankConfig` |
| Équipes | `GDSTeamConfig`, `GDSTeamBuildConfig`, `GDSBelongTeamConfig`, `GDSOpponentTeamConfig` |
| Carte | `GDSMapConfig`, `GDSMapEnvDataConfig`, `GDSMapMinimapConfig`, `GDSMapDoorConfig` |
| Événements | `GDSEventPlayConfig`, `GDSEventCmndConfig`, `GDSEventCameraPresetConfig` |
| Combat RPG | `GDSRpgBattleCmdConfig`, `GDSRpgBattleAiConfig`, `GDSRpgBattleFormationConfig` |
| Audio | `GDSBgmConfig`, `GDSSoccerGameBgmConfig`, `GDSMotionSoundConfig` |
| UI | `GDSMenuCreateConfig`, `GDSMenuPresetConfig`, `GDSMenuIconManagerConfig` |

### Priorités de rendu

Le moteur nomme ses passes : `00_Zero`, `00_UI_Before`, `10_MapBefore`, `15_ProjEffect`,
`20_CharaBefore`, `25_CharaAfter`, `30_MapAfter`, `40_Effect`, `50_Post`, `51_PostAfter`,
`59_PreMenuEndDraw`, `60_UI`, `61_PostMenuEndDraw`.

### Arborescence virtuelle des assets

```
#/                     racine virtuelle
#/chr/_uniform/        uniformes
#/effect/ , /locus/    effets visuels
#/font/ , #/font/<LG>/ polices (gaiji par plateforme : nx, ps4, ps5, xbox, SteamDeck)
#/map/ar/ao*/          areas outdoor (ao001–ao403)
#/map/ar/gr*/          terrains (gr001–gr080)
#/map/ar/pl*/          places (pl001–pl339)
#/map/ar/tr*/          zones d'entraînement
#/menu/220_img/        images de menu (opponent_img, meet_img/<LG>, stadium, savedata_img)
```

`<LG>` = langue, `%s` = segment dynamique, `_l` = variante large.

## RE en direct — ce que le process vivant dit du fichier (2026-08-27)

Mesure faite sur l'installation Steam (`NIE_GAME_DIR` = `…/steamapps/common/INAZUMA ELEVEN Victory Road`),
`nie.exe` lancé **sans** `GameBootstrapper`/`EACLauncher` (service `EasyAntiCheat_EOS` à l'arrêt),
mémoire lue par `nie-mem` **élevé** (sans élévation, l'énumération des modules échoue et l'outil
dit « module introuvable » — le message accuse le module quand le privilège est en cause).

- Base runtime `0x7ff7ed6e0000`, soit un **slide ASLR de `0x7ff6ad6e0000`** sur `NIE_IMAGE_BASE`.
- Le `.text` mappé est **byte-identique au fichier**, à **5 plages près** sur 25 602 048 octets.
  Une seule est légitime (`IMAGE_REL_BASED_DIR64` à `rva 0xA4DEDD`) ; les **4 autres sont des
  patchs runtime** posés par un trainer tiers actif, et n'existent pas dans le fichier :

  | rva | fichier → mémoire | effet |
  |---|---|---|
  | `0x123B520` | `48` → `c3` (`ret` à l'offset +0) | neutralise la fonction qui appelle `EOS_Platform_GetAntiCheatClientInterface` + `EOS_Platform_GetP2PInterface` |
  | `0x123D340` | `48` → `c3` (`ret` à l'offset +0) | neutralise la routine `QueryPerformanceCounter` + `IsDebuggerPresent` appelée par la précédente |
  | `0x1563800` | `movzx r8d,[rax+rcx+0x2C26]` → `jmp` + 4 `nop` | trampoline en page RWX anonyme (`base+0x10000000`) qui force `[rax+0x2C26] = 0x63` avant de rejouer l'instruction volée |
  | `0x16831BF` | `addss xmm0,[rdi+0x2208]` → `movss xmm0,[rip+…]` | remplace l'accumulation du chrono par une constante lue en `base+0x20000000` — **gel du temps de match** |

  Méthode reproductible : dumper les régions du module, comparer chaque `rva` au fichier **section
  par section** (une région dumpée déborde sur la section suivante — comparer sans re-résoudre la
  section produit des milliers de fausses divergences), puis **croiser avec `.reloc`**. Ce qu'aucune
  relocation ne couvre est un patch, pas un artefact du loader.

- Conséquence directe sur le catalogue : `match-time` **ne se résout plus en live** non parce que sa
  signature est périmée, mais parce que le trainer a patché les octets que l'AOB attend. Un
  localisateur qui échoue en mémoire et réussit sur le fichier accuse l'environnement, pas la
  signature. Le même patch a livré l'offset du champ : le chrono vit à **`entity+0x2208`**.

### Ré-ancrage du catalogue `nie-trace`

Le catalogue était **0 ✓ / 22 drift / 4 introuvable** : ses `rva` de référence venaient d'un autre
build. Les AOB, eux, étaient bons — ils tombaient sur un site unique, à une `rva` simplement
différente. Ré-ancrage en scannant le **fichier** (pas la mémoire : pas d'élévation, pas d'ASLR,
reproductible), puis validation live → **20 ✓ / 0 drift / 4 introuvable**.

Les `rva` trouvées par scan statique coïncident **exactement** avec celles du scan live : les deux
voies se confirment. Discipline conservée — un AOB à hits multiples (`free-buy-shop`, 2 hits) ou
introuvable (`special-move-type`) repasse à `rva: None` : on ne devine pas une adresse.

### Portail `just preuves` — état mesuré le 2026-09-07

Le portail uemu est actuellement **hors service sur cet environnement** :
`just preuves` rend **0 ✓ / 47 ✗ / 0 ⧗**, et chacune des 47 validations termine avec
`exit=127` (`validate_affine_compose` jusqu'à `validate_variant_set_int`). Ce code indique que
l'exécutable ou le lanceur de preuve n'est pas disponible ; il ne s'agit pas d'un écart de valeur
du portage. La commande de reprise reste `just preuves` (ou `just preuves <motif>` pour une famille).
La gate n'est donc pas laissée ambiguë : les autres portails sont mesurés séparément, mais aucune
preuve uemu positive n'est revendiquée tant que la dépendance manquante n'est pas réinstallée.

## Qualité

```
just check        # fmt-check + clippy -D warnings + test
just test-real    # goldens adossés aux vrais fragments du jeu
just health       # couverture + intégrité KB + EXTERN + heartbeat
```

Invariants : **0 warning clippy** sur tout le workspace ; `todo!`/`unimplemented!`/`dbg!` en deny ;
crates de jeu en `#![forbid(unsafe_code)]`.
