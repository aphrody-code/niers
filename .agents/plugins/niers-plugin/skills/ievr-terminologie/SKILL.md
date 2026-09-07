---
name: ievr-terminologie
description: Vérifie tout terme technique d'Inazuma Eleven Victory Road avant de l'affirmer — noms de formats (G4TX, G4MD, cfg.bin, RDBN, T2B, CPK), extensions, codes internes de personnages et techniques (c01000010, waza), symboles reversés, adresses virtuelles, chemins VFS et noms de tables. À charger dès qu'une réponse doit nommer un format, un symbole, un chemin du jeu ou un identifiant IEVR, et avant d'écrire un tel nom dans du code, un commit ou une doc.
---

# Terminologie IEVR — ne jamais deviner un nom

Les identifiants d'Inazuma Eleven: Victory Road sont opaques et se ressemblent : `G4MD` et
`G4MG` désignent deux choses différentes, `.g4nv` porte le magic `NAVM`, un code personnage
comme `c01000010` n'a aucune structure devinable. Un nom inventé traverse ensuite le code, les
commits et la base de connaissance sans que rien ne le signale.

**Règle : aucun identifiant IEVR ne s'écrit de mémoire. Chacun se vérifie contre une source du
dépôt, et la vérification est bon marché.**

## Ce qui doit être vérifié

Extensions et magics · noms de formats · codes de personnage et de technique · symboles reversés
et adresses virtuelles · chemins VFS · noms de tables `.cfg.bin` · noms de crates et de familles
`nie-data`.

## Où vérifier, par nature de terme

| Terme | Source de vérité | Comment |
|---|---|---|
| Format, magic, extension | `crates/engine/nie-formats/src/lib.rs` (`enum FileFormat`) et un module par format | `Grep` sur l'enum ; le module `<fmt>.rs` porte le parseur réel |
| Chemin VFS, existence d'un fichier | Le VFS lui-même (255 308 fichiers) | outil MCP `vfs_search` / `vfs_stat`, ou `niers vfs find <sous-chaîne> -j` |
| Personnage, technique | Miroir wiki | `niers vfs chara <nom\|id\|code>` · `niers vfs waza <nom\|id\|code>` |
| Symbole reversé, adresse | `var/niers.sqlite` | outil MCP `re_function` (par nom ou vaddr) ou `re_query` |
| Table / clé d'un `.cfg.bin` | Le fichier décodé | outil MCP `asset_get` avec `decode: "cfg"`, ou `probe_rdbn` / `probe_t2b` |
| Famille de données portée | `crates/engine/nie-data/src/` | `grep -rl "<MARKER_LIST>" crates/engine/nie-data/src/` — **les modules portent des noms de concept, pas de format** |
| Crate, chemin du dépôt | `cargo metadata --no-deps` | ne jamais déduire un chemin de crate de son nom |

Tables de `var/niers.sqlite` (lecture seule, `SELECT` uniquement) : `function`, `coverage`,
`rtti_class`, `rtti_base`, `xref`, `str`, `pdata_func`, `hash_name`, `symbol`, `section`,
`format`, `format_field`, `anchor`, `glob`, `hypothesis`, `binary`, `meta`, `func_const`,
`func_str_ref`, et la famille `cam_*` (caméra).

## Comment répondre quand la vérification est impossible

Si la source est indisponible (VFS non monté, KB absente), **le dire** et donner la commande qui
trancherait — jamais une réponse plausible non vérifiée. « Je n'ai pas pu vérifier X, la commande
est `…` » est une bonne réponse ; un nom inventé n'en est pas une.

Quand une vérification contredit une hypothèse de départ, c'est la vérification qui gagne, y
compris contre `CLAUDE.md`, une mémoire ou un commentaire du code : ces textes ont été écrits à
une date, le VFS et la KB sont l'état présent.

## Pièges avérés

- **`.g4nv` a le magic `NAVM`** : l'extension ne prédit pas le magic. Vérifier les deux.
- **`G4MD` (métadonnées de modèle) ≠ `G4MG` (mesh)** : deux modules distincts.
- **Deux formats derrière `.cfg.bin`** : RDBN à listes (`cfgbin::is_rdbn` → `parse` +
  `read_values`) et T2B en arbre (`cfgbin::cfgbin_parse`, `CfgEntry`). Tout
  `common/property/**` est T2B. Se tromper de branche donne un parseur qui « marche » et rend
  des valeurs fausses.
- **Convention `/tex`** : la route model-serve remplace `.png` par `.g4tx` — écrire
  `…/x.g4tx.png` est l'erreur classique. En voie FFI le chemin garde son `.g4tx`.
- **`Vfs::init()` prend `<racine>/data`**, pas la racine — sinon « impossible d'ouvrir
  cpk_list.cfg.bin », un message qui n'indique pas la vraie cause.
- **`niers vfs extract -o` attend un fichier**, pas un dossier : sinon « Accès refusé (os
  error 5) », qui n'a rien à voir avec des permissions.
- **`nie.exe` est à la racine du dépôt**, pas dans `data/`. Base image `0x140000000`.
- **Un retour conditionnel n'est pas une constante** : porter un `sete al` / `found ? 1 : 0`
  comme `return 1` est une source classique de faux portages.

## Précision numérique

Adresses virtuelles en hexadécimal avec le préfixe `0x` et la base image réelle. Les colonnes
d'adresse de la KB (`vaddr`, `from_addr`, `to_addr`) sont déjà rendues en hexadécimal par les
outils MCP. Ne jamais arrondir ni « corriger » une adresse, un offset ou une taille : les
reproduire tels quels, ou ne pas les citer.
