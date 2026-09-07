---
name: jouer-ievr
description: Faire tourner et « jouer » Inazuma Eleven Victory Road depuis le dépôt niers — lancer nie.exe, simuler un match déterministe avec le moteur Rust (nie-runtime, nie-headless, nie-play), rendre les vrais assets avec nie-game, observer le jeu en mémoire avec nie-trace, et piloter l'explorateur par le pont MCP. À charger pour lancer, jouer, simuler, rejouer, capturer ou observer le jeu.
---

# Jouer à IEVR depuis niers

Il y a **deux jeux** dans ce dépôt, et il faut choisir lequel on veut faire tourner.

1. **`nie.exe`** — le vrai jeu Level-5, à la racine du dépôt. On peut le **lancer** et
   l'**observer**, pas le piloter.
2. **Le moteur niers** — une simulation Rust déterministe et scriptable, jouable de bout en bout
   sans le jeu original. C'est là qu'on « joue » de façon reproductible.

## Ce qui est possible aujourd'hui, et ce qui ne l'est pas

| Action | Disponible | Comment |
|---|---|---|
| Lancer le vrai jeu | oui | outil MCP `game_launch` (process détaché, rend le PID) |
| Observer le jeu qui tourne | oui, **lecture seule** | `nie-trace` : régions du module, lecture d'octets, dump |
| Écrire dans la mémoire du jeu | **non — décision tranchée** | Jamais d'écriture mémoire dans un process tiers depuis le dépôt |
| Envoyer des entrées clavier/manette à `nie.exe` | **non — n'existe pas** | Aucune infrastructure d'input synthétique dans le dépôt |
| Simuler un match complet | oui | `nie-headless match`, `nie-runtime`, `nie-play` |
| Rendre les vrais assets | oui | `nie-game` (wgpu, fenêtre ou capture PNG) |
| Piloter l'explorateur | oui | outils MCP `explorer_navigate`, `explorer_open`, `explorer_tab` |

**Ne pas prétendre piloter `nie.exe`.** Une demande de « jouer comme un humain » au vrai binaire
supposerait capture d'écran plus entrées synthétiques : rien de tel n'existe ici, et il faudrait
le construire. Le dire, puis proposer la voie moteur, qui est reproductible et scriptable.

## Simuler un match

```bash
# Résumé JSON déterministe, sans rendu
./target/debug/nie-headless.exe match

# Match simulé + rendu vidéo (headless)
./target/debug/nie-runtime.exe --frames 600 --fps 60 --out match.mp4
./target/debug/nie-runtime.exe --no-video --out frame.png     # dernière frame seulement

# Match avec de vraies données de personnages et la police du jeu
./target/debug/nie-play.exe \
  --font-cfg <font.cfg.bin> --font-g4tx <font.g4tx> \
  --seed 12345 --chara-param <chara_param_*.cfg.bin.json>
```

Le moteur est **déterministe** : même graine, même résultat. C'est ce qui rend une régression
détectable — comparer deux exécutions, pas deux impressions.

## Rendre les vrais assets

```bash
./target/debug/nie-game.exe --window                    # fenêtre wgpu
./target/debug/nie-game.exe --capture out.png           # rendu hors-écran
./target/debug/nie-game.exe --g4tx <chemin VFS .g4tx>   # texture précise
```

`--game-dir` résout automatiquement via la variable d'environnement `NIE_GAME_DIR` ou pointer directement vers l'installation Steam du jeu (`<racine>/data`).

## Observer le vrai jeu

`nie-trace` (et les commandes Tauri `re_trace_*` de l'explorateur) lisent la mémoire d'un
`nie.exe` en cours : recherche du process, régions du module, lecture d'octets, dump complet.
**Lecture seule, sans exception** — c'est une décision du projet, pas une limite technique.

Enchaînement utile : `game_launch` pour démarrer, puis `re_trace_find_process` pour s'y
raccrocher, puis lecture des régions.

## Piloter l'explorateur pendant une session

Quand `nie-explorer` tourne et que son pont est activé, le serveur MCP le dirige :
`explorer_navigate` (dossier VFS), `explorer_open` (fichier), `explorer_tab`, `explorer_toast`.
`explorer_status` dit si un client est connecté — l'appeler avant de conclure qu'une commande a
échoué.

## Honnêteté sur les résultats

Un match simulé n'est pas une partie du vrai jeu : la physique PhysX exacte et la résolution de
but event-driven de IEVR sont des pistes séparées. Dire « le moteur niers donne X » et non « le
jeu donne X », sauf si l'écart a été mesuré contre le binaire.
