# Points d’entrée vérifiés dans NIE

Les chemins ci-dessous sont relatifs à la racine NIE, pas au répertoire du plugin installé. Résoudre la racine depuis le contexte de travail ou `NIERS_REPO`, puis vérifier les fichiers. Cette carte décrit les sources inspectées ; elle ne garantit pas qu’un binaire installé a été reconstruit ni qu’un service tourne.

| Besoin | Source à consulter | Utilisation |
|---|---|---|
| Import Blender | `plugins/niers-blender/__init__.py`, `niers_bridge.py` | L’import réel passe par `import_scene.level5_g4`. Lire les propriétés de l’opérateur avant son invocation. |
| Préparation de portage Blender | `plugins/niers-blender/g4_port_addon.py` | Les opérateurs `level5_g4_port.prepare_atlas`, `build_expression_atlas`, `auto_map_joints` existent ; leur contexte et leurs propriétés se lisent dans leur classe. `load_original_model` prépare le wizard et ne constitue pas l’import d’une scène. |
| Assemblage Rust partagé | `crates/engine/nie-formats/src/assemble.rs` | `AssembledModel`, `Skeleton`, `MeshPrimitive`, textures et diagnostics ; lire les entrées de l’assembleur avant de composer des pièces. |
| Décodage navigateur | `crates/engine/nie-wasm/src/lib.rs` | `detect_format`, `g4tx_info_json`, `g4pk_parse_json`, `model_to_glb`. Vérifier signature et exports générés. `model_to_glb` reçoit géométrie et métadonnées ; ne pas le présenter comme un assemblage complet de personnage. |
| Rendu de contrôle | `crates/engine/nie-render3d/src/main.rs`, `glb.rs`, `render.rs` | GLB vers PNG/MP4 ; les textures PNG embarquées sont lues par `glb.rs`. La structure chargée ne démontre pas la prise en charge de tout glTF. |
| Édition de scène | `crates/tools/nie-editor/src/main.rs` et `crates/engine/nie-render3d/src/document.rs` | Éditeur natif utilisant `nie-render3d` ; options `--glb`, `--project`, `--backend`. Réutiliser le document et le moteur existants. |
| Explorateur | `apps/inacord` et `apps/nie-mcp` | Inspecter les commandes courantes de prévisualisation et le pont disponible avant pilotage. |

## Commandes de rendu

Depuis la racine NIE, vérifier d’abord l’interface du code qui sera exécuté :

```text
cargo run -p nie-render3d -- --help
cargo run -p nie-editor -- --help
```

Exemple à adapter aux fichiers réels et à un dossier de sortie existant :

```text
cargo run -p nie-render3d -- --glb CHEMIN_ABSOLU_MODELE.glb --frames 1 --angle 0 --width 720 --height 720 --out CHEMIN_ABSOLU_CAPTURE.png
```

`--angle` est en radians. Pour un contrôle face/profil/dos, conserver tous les autres paramètres et choisir respectivement 0, 1.5707963 et 3.1415927, puis vérifier visuellement l’orientation réelle de l’asset. Les noms de vues seuls ne la garantissent pas.

Les options GPU dépendent de la feature `gpu`. Le mode `--verify` compare des silhouettes CPU/GPU, pas une identité des couleurs ni une référence du jeu. Le chemin CPU ordinaire suffit à une première inspection et ne valide pas le GPU de l’éditeur. Un changement de backend exige une nouvelle capture explicitement identifiée.

Blender s’utilise via les outils effectivement exposés ou son exécutable local détecté. Lire l’aide locale et les scripts existants avant de composer une commande ; ne pas inventer une CLI `niers blender`. Pour un script Python hors Blender, suivre le dépôt avec `uv run`. Éviter les relances en boucle : expliquer un redémarrage nécessaire et vérifier le processus ciblé.

## Inspiration Game Development Studio, facultative

Les workflows reprennent trois idées : paquet d’asset traçable, vérification au moment de l’import et comparaison de captures contrôlées. Ils fonctionnent sans ce plugin.

Si Game Development Studio est disponible et utile à la demande, charger son skill correspondant depuis le catalogue actif :

- `game-asset-production/SKILL.md` et sa référence `references/commands.md` pour inspection, normalisation et paquet.
- `game-asset-vendoring/SKILL.md` et sa référence `references/commands.md` pour admission d’un paquet existant.
- `game-visual-debugging/SKILL.md` et sa référence `references/capture-workflow.md` pour scénarios reproductibles et comparaison.

Ne pas recopier leur manuel ni figer le chemin d’un cache versionné. Vérifier d’abord que `game-dev` existe, puis utiliser `game-dev capabilities --json` et `game-dev doctor --json`. Les schémas installés font foi. Ne pas inventer d’adaptateur NIE, de scénario ou de pièces jointes de profondeur/normales. Ces skills NIE n’autorisent ni installation, ni admission externe, ni génération payante ; une tâche locale déjà autorisée reste dans son périmètre.
