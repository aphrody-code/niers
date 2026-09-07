# Absorber `iecode` C# dans niers

But : que `csharp/` (230 fichiers, 46 922 lignes) devienne redondant, puis supprimable, en portant
ses capacités en Rust **avec un gain mesurable**, pas à l'identique.

Document de reprise : il porte l'inventaire déjà fait, ce qui est acquis, et l'ordre à suivre.
Rien ici ne demande de refaire l'analyse.

---

## Ce qui est déjà acquis

`crates/engine/nie-viola` porte les quatre opérations de modding en Rust natif, validées sur le
jeu réel (`cargo run -p nie-viola --example valider_reel --release`) :

| | Preuve sur le jeu réel |
|---|---|
| `cpk_list.cfg.bin` | 255 308 entrées, enveloppe AES, décodé → réencodé → relu à l'identique |
| dump | 9 788 fichiers octet pour octet contre la lecture VFS, 0 échec |
| reprise de dump | 2ᵉ passage : 0 réécriture |
| pack | entrée basculée hors paquet, taille inscrite |
| merge au champ | deux mods éditant des champs différents d'un même `.cfg.bin` survivent tous les deux |
| crypto Criware | aller-retour exact sur un `.cpk`, par tranches |

Gains réels sur les trois implémentations amont (Viola C#, port C++ du dépôt, `ievr_toolbox` Rust) :
ordonnancement des paquets par volume décroissant, mappage mémoire au lieu d'un déchiffrement vers
un dossier temporaire, sommaire indexé une fois par paquet, reprise au paquet près, saut des
fichiers déjà à la bonne taille, et surtout **fusion au champ** — impossible sans comprendre les
formats.

L'onglet **Viola** de `nie-explorer` appelle ce crate en process (aucun binaire externe).

---

## Le constat qui change la stratégie

**La suite de tests C# protège beaucoup moins qu'elle n'en a l'air.** Sur 220 cas, **114 gardes de
saut silencieux** (`if (!Has…) return;`) dépendent de 9 racines codées en dur hors dépôt
(`/home/ubuntu/niers/data`, `/tmp/s`, `/tmp/g4pkm-extract`) et de fichiers `re/lua/raw`,
`re/lua/unluac.jar`, `re/menu/hash-dictionary.json` — or `git ls-files re` est **vide**. Sur une
machine Windows, `just cs-test` passe au vert **sans exécuter** la majorité des cas fichiers-réels.

Douze des 22 fichiers de test ont déjà un équivalent Rust au moins aussi fort (objbin, mevbin,
depot_resolver, token_store, cfgbin, bytecode Lua, menu_host, g4mg/g4md, g4pkm_motion, dxbc).

---

## À sauver avant tout retrait — irréversible : **fait**

Les quatre corpus sont portés en Rust, et ils **s'exécutent** sur le jeu réel — ce que les tests
C# ne faisaient plus ici : leurs racines sont codées en dur hors dépôt (`/tmp/g4pkm-extract`,
`/home/ubuntu/…`) et `re/lua/raw` est vide **y compris sur le VPS**. Leur vert était creux.

1. **20 paires (nom, hash) réelles** — `crates/engine/nie-formats/tests/level5_hash.rs`, plus un
   golden VFS qui retrouve les 20 noms dans les trois fichiers d'origine du jeu.
2. **Identifiants de scénario Lua** — `nie-lua/src/menu_host.rs` + golden qui rejoue le vrai
   script. Distinction établie : `general_win` (292844459) et
   `savedata_management_menu_save_and_upload` (1654568798) **sont** le CRC32 de leur nom ;
   `2492438505`, `536044352`, `711242136`, `532421851` sont des constantes observées —
   `CRC32("battle_menu_multi")` vaut `0xFEB5F0B8`, pas `2492438505`.
3. **Fixtures G4PK** — `s28g001b.g4sk` (3 344 o, `0x940E596D`) tranché du G4PK depuis le VFS :
   quatre tests `real-fixtures` morts redeviennent vivants. Les 192 octets de
   `mainmenu90_02_2.g4mg` sont vérifiés identiques au fichier du jeu.
4. **Layout G4PKM** — `g4pkm.rs` : `_cursor01`, `_pos_scl_base01`, échelles, hiérarchie, et les
   **20** noms d'os de `title00_09` (le C# n'en assertait que 9).

Corrigé au passage : `nie-lua/src/lib.rs` écrivait `0x1176F7AB` pour `general_win`, la valeur est
`0x117473AB`.

**`Resources/EACLauncher.zip`** (~2 Mio) est embarqué dans l'assembly, **gitignoré**, non
reconstructible depuis le dépôt, sans hôte Rust. Le sortir vers un artefact adressable est la
seule action qui rende `csharp/` retirable sans perte. Ce n'est **pas** le même sujet que
`niers mem patch-eac`.

**Un `nie.exe` byte-identique ne suffit pas à recréer le jeu.** Le binaire est lancé par Easy
Anti-Cheat, qui exige EOS, et le jeu appelle Steamworks. La forge sait produire `nie.exe` ; elle ne
produit **rien** des quatre autres maillons, et ne le pourra pas — ce sont des binaires tiers
signés. Leur seule source est Steam, d'où `niers steam`. `niers info` les inventorie et rend un
verdict `lancable` :

| Composant | Origine |
|---|---|
| `nie.exe` (33 918 464 o) | **produit par la forge** |
| `EACLauncher.exe` (3 975 920 o) | tiers, non reproductible |
| `EasyAntiCheat/Settings.json` | tiers |
| `EOSSDK-Win64-Shipping.dll` (19 035 600 o) | tiers, exigé par EAC |
| `steam_api64.dll` (319 584 o) | tiers |

---

## Capacités réellement absentes en Rust

À porter (aucun substitut aujourd'hui) :

- **`CfgBinTypesGenerator`** — génère `.d.ts` + `index/verify.json` depuis les `cfg.bin`. Seul pont
  automatique cfg.bin → TypeScript du dépôt. 0 hit sur `export interface` dans `crates/`.
- **`DiskBudget`** — garde-fou d'espace disque (`--max-disk 30G`). Le seul analogue est un exemple
  (`dump_packs.rs`) à réserve fixe, avec `statvfs` Unix-only donc inopérant ici. Le portable est
  `fs4` (MIT/Apache, `statvfs` Windows **et** Unix).
- ~~**`DumpPresets`**~~ → **fait** : `nie_viola::presets` porte les trois catalogues et leurs 21
  catégories, et `nie_viola::filtre::Filtre` la syntaxe complète (listes, `!`, `**`) avec la
  sémantique de `GlobToRegex`. `niers viola dump --preset inagle` extrait 45 438 fichiers / 98 Mio
  sur le jeu réel, 0 échec, zéro fichier des catégories exclues.
- **`HostProfile`**, **`FxbinParser`** sémantique (techniques/passes), **`G4maParser::ParseMotionNames`**,
  **`CdnMediaTypes` + ETag**, **`G4pk::DetectSubFormat`/`ExtractFiles`**.
- **Magics manquants de `nie_formats::detect`** : G4PKM, G4MT, G4MA, G4RA, ADX, `\x1bLua`, objb,
  **XFSA**, **XPCK** (ces deux-là : 0 hit dans tout `crates/`), plus le footer T2B.
  Correction : **PXCL n'en fait pas partie**, `col::is_pxcl` (`col.rs:39`) existe déjà. Les
  prédicats `is_g4mt`, `is_g4ma`, `is_g4ra`, `is_objb`, `is_lua52_bytecode` existent aussi — il ne
  manque que leur branchement dans `detect`. Source de vérité pour la table complète :
  `csharp/IECODE.Core/Formats/GameFileType.cs` (23 types, avec les subtilités `G4PK@`/`G4SK@`
  cinquième octet `@`, `G4PKM` cinquième octet `M`).

**Divergence numérique à trancher** — `AdxInfo/ComputeCoefficients` : `cri_audio.rs:214-217` rend
`(7298, −3535)` à `highpass = 0` là où le C# calcule `(8192, −4096)`. L'un des deux est faux ;
ce n'est pas une question de portage mais de correction.

---

## Les six appelants réels du C#

Tout le reste est documentaire. **Aucun couplage CI** (`.github/workflows/ci.yml` n'appelle que
cargo), **aucun couplage Bun**.

1. `crates/tools/nie-cli/src/delegate.rs:41-52,109-124,127` + `main.rs` (`Cmd::Cs`, dispatch)
2. `justfile:158,197-201,225,231,237` (`cs-build`, `cs-test`, remontés en `-` non bloquant)
3. `scripts/sync-gamedata.ts:29,34,48,54` — **seul appelant fonctionnel hors CLI**, et non câblé
   dans `package.json`
4. `bench/cs/` — mesure Rust **contre** C# ; sa valeur vient de la présence du C#
5. `IECODE.sln`, `global.json`, `NuGet.config` — dont `bench/cs` hérite
6. `.gitignore:83-85` (EACLauncher.zip)

---

## Ordre de travail

1. ~~**Réimplanter les quatre vérités terrain**~~ → **fait** (cf. section ci-dessus). Le seul point
   irréversible est franchi : `csharp/` ne porte plus rien d'unique côté tests.
2. **Statuer sur `EACLauncher.zip`** : le sortir de l'assembly vers un artefact adressable.
3. **Exposer sous `niers` ce que Rust sait déjà faire mais que la CLI cache** — c'est la doctrine
   « niers est la seule CLI », violée aujourd'hui :
   - ~~`niers viola dump|pack|merge|crypto`~~ → **fait** : les quatre sous-commandes appellent
     `nie-viola` en process, vérifiées sur le jeu réel (dump filtré + reprise, aller-retour
     crypto involutif, merge à deux mods, pack sur les 255 308 entrées en enveloppe AES).
   - ~~`niers steam list|download|sync`~~ → **fait**. Les deux préalables sont levés :
     `nie-steam` est déclaré dans `[workspace.dependencies]` (il était le seul crate à y manquer),
     et son API `async` tourne sur un runtime monté le temps de l'appel. Mêmes options et mêmes
     variables d'environnement que le binaire d'origine, pour que `scripts/sync-gamedata.ts`
     puisse basculer sans changer de contrat.
   - ~~`niers info --json`~~ → **fait** : racine, taille et sha256 du binaire, `cpk_list`, volume
     du VFS, et présence du corpus de dumps. Reste à y agréger la part produite par la forge
     (`nie-forge report` n'écrit pas encore de fichier : il faudrait un `--out`) et la couverture
     RE (`coverage()` doit d'abord rendre un struct plutôt qu'imprimer).
4. **Porter les capacités sans substitut** (§ précédent). `DumpPresets` est fait ; reste
   `DiskBudget` — c'est le dernier verrou de `scripts/sync-gamedata.ts`.
5. **Réécrire `scripts/sync-gamedata.ts` sur `niers`** — supprime l'unique dépendance d'exécution
   au .NET, et le `dotnet build` implicite au premier lancement.
6. **Retirer `Cmd::Cs`**, `delegate::cs`, `iecode_dll`, `iecode_cli_candidates`, la ligne `cs=` de
   `status()` et son test.
7. **Retirer** `justfile:158,197-201` et les trois `-just cs-*` des agrégats.
8. **Trancher `bench/cs`** : soit figer les colonnes `csharp`/`csharp-aot` dans
   `docs/BENCHMARKS.md` avant retrait, soit le garder comme dernier consommateur de `IECODE.Core`
   (auquel cas `global.json` et `NuGet.config` doivent survivre). Ne pas trancher, c'est perdre la
   preuve chiffrée qui justifie le portage.
9. **Re-viser les ancres documentaires** : `docs/DESIGN.md:561` cite `G4pkmLayoutTests.cs` comme
   preuve → viser `g4pkm.rs` ; `plugins/niers-plugin/agents/port-scout.md:43` fait grepper
   `csharp/` → retirer ; `ARCHITECTURE.md`, `SKILL.md`, `CLAUDE.md` passent de quatre
   implémentations à trois.

> **Statut re-mesuré le 2026-09-07 — (9) est à moitié fausse.** `docs/DESIGN.md:561` est
> **fait** : il cite le golden Rust `g4pkm.rs`, plus `G4pkmLayoutTests.cs`. Les deux autres points
> sont **prématurés**, pas juste en retard : `csharp/` reste vivant et entretenu (correctifs de
> résolution de chemin du 2026-09-05), toujours cité comme source d'origine par les en-têtes Rust
> (`//! Port Rust de IECODE.Core/…`) — retirer la cible `csharp/` du grep de `port-scout.md`
> **avant** que (5)-(8) soient faits couperait la seule méthode de recherche pour ce qui n'est pas
> encore porté. Et passer de « quatre » à « trois » implémentations tant que `csharp/` compile et
> sert encore serait une désinformation documentaire, pas une simplification. Les deux ne doivent
> être faits qu'**après** (5)-(8), jamais en anticipation.

`IECODE.sln` et les `.csproj` ne bloquent qu'après (6) et (8).

---

## Mesurer l'avancement

La doctrine est écrite en tête de `delegate.rs` : « chaque portage retire une délégation, l'écart
se mesure ». La mesure existe maintenant — `niers backends` la rend :

```
niers=24 commandes natives
iecode-cli=27 commandes deleguees
ecart=3
```

Le numérateur vient de clap (il suit le binaire, pas une note) ; le dénominateur est la constante
`delegate::COMMANDES_IECODE_CLI`, ancrée par un test et accompagnée de sa méthode de comptage —
sans quoi le chiffre dérive à la première relecture.

> **Correction du 2026-09-07 — le dénominateur de 27 était lui-même faux, et le compte de « 38 »
> qu'il réfutait ci-dessus était le bon, pour une méthode différente.** Le comptage par
> `grep -oE 'new Command\("[a-z][a-z0-9-]*"' csharp/IECODE.CLI/Program.cs` (utilisé pour arriver à
> 27) ne voit que les commandes **littérales** dans `Program.cs` ; il rate toutes celles
> enregistrées via une factory `XxxCommand.Create()` — 25 fichiers sous
> `csharp/IECODE.CLI/Commands/*.cs` (`BenchmarkCommand`, `CdnCommand`, `MemCommand`, `G4txCommand`,
> `LuaCommand`, `ShaderCommand`, etc.), invisibles à ce grep. Le compte robuste, vérifié ligne par
> ligne : `grep -c 'rootCommand.AddCommand' csharp/IECODE.CLI/Program.cs` = **38**, chaque appel
> distinct (aucune commande ajoutée deux fois). `delegate::COMMANDES_IECODE_CLI` vaut maintenant
> **38**, avec la méthode corrigée dans sa doc.
>
> Le numérateur a bougé aussi : `enum Cmd` (`crates/tools/nie-cli/src/main.rs:56-582`) compte
> aujourd'hui **42** variantes de premier niveau (41 à champs + la variante unité `Backends,`
> ligne 78 — un `grep` sur `{` seul ne la voit pas), pas 24.
>
> ```
> niers=42 commandes natives
> iecode-cli=38 commandes deleguees
> ecart=0 (saturating_sub)
> ```
>
> **Le `ecart=0` ne veut plus dire « absorption terminée ».** La formule
> `COMMANDES_IECODE_CLI.saturating_sub(niers_commandes)` suppose qu'un port `niers` retire
> toujours une délégation `iecode-cli` à due proportion. Ce n'est plus vrai : `mem`, `wiki`, `save`,
> `video`, `vn`, `avatar`, `icons` sont des commandes **nouvelles**, sans équivalent dans
> `IECODE.CLI` — elles gonflent le numérateur sans rien absorber. La liste réelle des 8 capacités
> encore sans substitut Rust (§ *Capacités réellement absentes*, plus haut) reste le bon inventaire
> du travail restant ; la soustraction brute ne l'est plus.

---

## Instrument vivant des capacités CLI/UI

Ne pas dupliquer ici les chiffres de `niers`, d'Inacord, d'Azalée ou du VFS en prose : ils périment
en silence (cf. `var/couverture-site.json` retrouvé daté d'un jour lors de cette même passe, avec
`niers=40`/`routes_montees=80` au lieu de 42/84 mesurés en source). La source vivante est
`nie-site --regenerer-couverture var/couverture-site.json` (fichier suivi par git, jamais tenu à
la main) et la page `/couverture` / `/api/v1/couverture` qu'il alimente — neuf sources mesurées
(`niers`, `inacord`, pages et routes d'`azalee`, modules de `nie-data`/`nie-formats`, fonctions
publiques de `nie-lua`, commandes `iecode` C++, extensions du VFS), classées `servi`/`partiel`/
`manquant`/`bloque`/`interne` par des règles écrites (`crates/tools/nie-site/src/couverture/
regles.rs`), pas par un tableau maintenu ici.

## Capacités C# non portées — reconduites, non re-vérifiées le 2026-09-07

Les 8 items de « Capacités réellement absentes » (plus haut) datent de la mesure du 2026-08-12 et
n'ont pas été repassés au crible dans cette passe (`CfgBinTypesGenerator`, `DiskBudget`,
`HostProfile`, `FxbinParser` sémantique, `G4maParser::ParseMotionNames`, `CdnMediaTypes`/ETag,
`G4pk::DetectSubFormat`/`ExtractFiles`, les magics de format manquants, la divergence
`AdxInfo/ComputeCoefficients`). Rien n'indique qu'ils aient bougé, mais rien ne le confirme non
plus — à recompter à la source avant de les cocher, pas à recopier.
