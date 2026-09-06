# LOCAL.md — cloner `niers` sur Windows, branché au jeu

Ce fichier a **un seul sujet** : monter un poste de travail Windows à partir d'un clone frais.
Il ne redit rien de ce que possèdent déjà les autres :

| Ce que vous cherchez | Où c'est écrit |
|---|---|
| Installer la CLI `niers` seule (`cargo install`) | [`docs/INSTALLATION.md`](docs/INSTALLATION.md) |
| Les règles de travail du dépôt (outils, pièges, gates) | [`CLAUDE.md`](CLAUDE.md) |
| La chaîne C++ / MSVC / vcpkg | [`scripts/setup.ps1`](scripts/setup.ps1) |
| Faire circuler les données **VPS ↔ Windows** en régime établi | [`scripts/ops/sync-machines.sh`](scripts/ops/sync-machines.sh) |
| Ce qui tourne sur le VPS, et sous quel service | [`docs/EXPLOITATION.md`](docs/EXPLOITATION.md) |

---

## 1. Ce que le clone ne contient pas, et pourquoi

`git clone` rend le code et rien d'autre. Trois familles de fichiers manquent, chacune pour une
raison différente — et **aucune ne se signale** : un outil qui ne les trouve pas parle de VFS,
de base vide ou de 503, jamais du fichier absent.

| Famille | Poids | D'où elle vient | Pourquoi elle n'est pas dans Git |
|---|---|---|---|
| Les fichiers du jeu (`data/common`, `data/dx11`, les `.cpk`) | ~57 Go installés | **Steam**, pas le VPS | assets © LEVEL-5 : `.gitignore` les exclut, et les pousser serait une redistribution |
| Les gisements (`var/mirror.sqlite`, `data/anime/episodes.db`) | 69 Mo | le VPS | produits par les moissons nocturnes, ils changent tous les jours |
| Les index dérivés (`var/vfs/inventaire.txt`, `data/re/`) | 37 Mo | le VPS | régénérables ici, mais la copie prend une minute quand la reconstruction prend une heure |
| La base de reverse (`var/niers.sqlite`) | **17 Go** | le VPS | facultative, **et trompeuse** : voir § 6 |

Le jeu vient de Steam et **pas** du VPS. C'est le point qui surprend : le VPS porte bien un dump
de 111 Go, mais le poste Windows a l'installation Steam, qui est la même chose en mieux — c'est
elle qui porte `nie.exe`, et c'est sur elle que les 255 308 entrées se mesurent.

## 2. Prérequis

- **Windows 10/11 x64**, PowerShell **7+** (`pwsh`) ;
- **Rust** : la toolchain épinglée par [`rust-toolchain.toml`](rust-toolchain.toml)
  (`nightly-2026-05-17`). `rustup` la pose seul à la première commande `cargo` ;
- **Bun** ≥ 1.4 ([bun.com/install](https://bun.com/install)) ;
- **Git** et le **client OpenSSH** de Windows (Paramètres → Fonctionnalités facultatives) ;
- **Inazuma Eleven: Victory Road** installé par Steam (app **2799860**) ;
- un accès SSH au VPS. L'alias attendu est `ovh-vps-direct` —
  **ne jamais viser `ovh-vps`**, qui passe par le VPN (`10.8.0.1`) et expire.

Facultatif : `sqlite3` sur le `PATH` (le script s'en sert pour **compter** les tables importées ;
sans lui il ne rend qu'une taille de fichier, ce qui prouve moins).

MSVC et vcpkg ne sont nécessaires que pour la chaîne C++ (`iecode`) et le **chemin B de la
forge** : [`scripts/setup.ps1`](scripts/setup.ps1) s'en occupe, séparément.

## 3. En trois commandes

```powershell
git clone https://github.com/aphrody-code/nie.git niers
cd niers
pwsh -File scripts\ops\bootstrap-windows.ps1
```

Le script fait quatre choses, dans cet ordre, et **compte** à chaque étape :

1. **trouve l'installation Steam** sans deviner son chemin. Il lit
   `steamapps\libraryfolders.vdf` puis `appmanifest_2799860.acf` (clé `installdir`) — le jeu
   peut vivre sur un autre disque que Steam, et le dossier ne s'appelle pas forcément
   `INAZUMA ELEVEN Victory Road` ;
2. **pose `NIE_GAME_DIR`** en variable **utilisateur**. Une variable de session disparaît avec
   le terminal, et les outils lancés depuis un IDE ne la verraient jamais ;
3. **rapatrie du VPS** les gisements et les index — **102 Mo** par défaut, mesurés le
   2026-09-06 (miroir 67 Mo, inventaire VFS 25 Mo, `data/re/` 12 Mo, épisodes 2 Mo) ;
4. **vérifie en comptant** : un nombre de tables par gisement, un nombre de lignes pour
   l'inventaire. « Copié » n'a jamais prouvé qu'une base porte des lignes.

Options : `-VpsHost ubuntu@51.77.147.152` (si l'alias n'existe pas), `-GameDir <racine>` (si la
détection Steam échoue), `-SkipVps` (détection Steam seule), `-WithRe` (voir § 6).

## 4. Vérifier — dans un terminal NEUF

`NIE_GAME_DIR` vient d'être posée : un terminal déjà ouvert garde son ancien environnement, et
la vérification y échouerait pour une raison qui n'a rien à voir avec le dépôt.

```powershell
cargo build --release -p nie-cli
.\target\release\niers.exe info
```

`niers info` doit annoncer **255 308 entrées** et **936 packs**. Un autre nombre, ou une erreur
qui parle de `cpk_list.cfg.bin`, veut dire que `NIE_GAME_DIR` ne pointe pas la racine du jeu —
c'est le dossier qui **contient** `data\cpk_list.cfg.bin`, pas le dossier `data`.

Puis le côté TypeScript :

```powershell
bun install                 # à la racine, jamais dans un sous-paquet
bun run build:ffi           # cargo build -p nie-ffi — EXIGÉ avant tout autre `bun run`
bun run typecheck           # doit rendre 0 sur les 15 workspaces
```

`bun install` à la racine n'est pas un détail : sans lui, `import … from "nie"` résout le paquet
**`nie` du registre npm** au lieu de `packages/nie`, et l'erreur parle d'un export manquant, pas
d'une installation.

## 5. Ce que le poste Windows peut faire, et que le VPS ne peut pas

C'est la raison d'être de ce poste, et elle est mesurée dans
[`scripts/ops/sync-machines.sh`](scripts/ops/sync-machines.sh) :

- **la forge, chemin B** — `nie-forge cc` compile avec **MSVC 14.44**, le toolset qui a lié
  `nie.exe`. Absent du VPS, donc le chemin qui monte le plus haut n'y existe pas ;
- **tout ce qui se voit** — rendu 3D, `nie-game --menu`, l'application Inacord (Tauri), les
  captures de vérification : il faut un écran et un GPU ;
- **lire la mémoire du jeu** — `nie-mem.exe` et `nie-edit.exe` (`ReadProcessMemory`), avec
  élévation. `niers mem` est l'équivalent Linux et n'existe pas ici ;
- **le C#** — `dotnet` est **absent du VPS** : `csharp/` n'y compile ni ne s'y teste. Un lot C#
  ne peut y être que *relu*, jamais vérifié.

Inversement, le VPS garde la moisson réseau et les 19 services.

## 6. `var/niers.sqlite` — 17 Go, et un piège

La base de reverse n'est **pas** rapatriée par défaut, et la taille n'est pas la seule raison.

Elle est **ancrée sur un autre binaire** que le `nie.exe` installé : son `binary` id=2 porte le
sha `4c2b91fbae6f…` / 31 468 032 octets, quand la cible documentée est `b1fa04ea3658…` /
33 918 464 octets. Ses chiffres — 108 650 fonctions, 12,57 % nommées — décrivent donc un build
**transitoire**, pas la cible. Le hook d'ouverture de session imprime cette contradiction à
chaque fois.

Conséquence pratique : ne citez aucun de ses nombres comme une mesure de `nie.exe` avant d'avoir
rejoué `niers rebuild --db var\niers.sqlite --exe nie.exe`. Si votre travail n'est pas du
reverse, `-WithRe` ne vous apporte rien.

## 7. Quand quelque chose ne marche pas

| Symptôme | Cause réelle |
|---|---|
| `niers info` parle de `cpk_list.cfg.bin` | `NIE_GAME_DIR` vise `…\data` au lieu de la racine, ou le terminal est antérieur au script |
| Une commande `export_*` échoue hors du dépôt | même cause : ces binaires n'ont pas de `--help` et ne disent rien d'autre |
| `bunx tsc --noEmit` échoue sur `apps/nie-web` (`TS5101`) | le `tsc` global n'est pas celui du workspace. La gate est `bun run --filter '*nie-web*' typecheck` |
| Un `import … from "nie"` ne trouve pas ses exports | `bun install` n'a pas été lancé **à la racine** |
| Une suite affiche `0 passed` | elle n'a pas tourné : feature désactivée ou garde de test qui saute. Ce n'est jamais un succès |
| `ovh-vps` expire | il passe par le VPN. Utiliser `ovh-vps-direct` |
| ~55 goldens `nie-data` sautent | ils dépendent du dump ; ils ne tournent qu'avec le jeu monté |

Un dernier réflexe, qui vaut pour tout ce fichier : **un compte, une commande, une date — sinon
ce n'est pas fait.** Un `systemctl active`, un « copié », un build vert ne prouvent rien tant
qu'on n'a pas interrogé la chose et obtenu un nombre.
