# Configuration et données

| Format | Magic | Extension | Module | Contenu |
|---|---|---|---|---|
| cfg.bin | RDBN, ou en-tête T2B | `.cfg.bin` | `cfgbin.rs` | **Deux formats distincts** — voir ci-dessous |
| OBJBIN | `OBJB` | `.objbin` | `objbin.rs` | Définitions d'objets-menu |
| MEVBIN | — | `.mevbin` | `mevbin.rs` | Motion Event Binary |

## Le piège central : deux formats derrière `.cfg.bin`

```rust
if cfgbin::is_rdbn(data) {
    let doc = cfgbin::parse(data)?;      // RDBN : listes
    let vals = cfgbin::read_values(&doc, ...);
} else {
    let tree = cfgbin::cfgbin_parse(data)?;  // T2B : arbre de CfgEntry
}
```

- **RDBN** — structure à listes. `is_rdbn` → `parse` + `read_values`.
- **T2B** — arbre de `CfgEntry`. `cfgbin_parse`. **Tout `common/property/**` est T2B.**

Se tromper de branche ne lève pas d'erreur : le décodage rend des valeurs plausibles et fausses.
Toujours passer par `is_rdbn`, jamais deviner d'après le chemin.

## Explorer une famille

```bash
target/debug/examples/probe_rdbn <prefix>     # RDBN
target/debug/examples/probe_t2b  <prefix>     # T2B
```

`NIE_GAME_DIR` doit être posée, sauf sur l'installation Steam où le VFS est le répertoire
courant.

Décodage sans écrire de code : outil MCP `asset_get` avec `decode: "cfg"` — rend le JSON,
`source: "ffi"` quand les CPK sont montés localement.

## Portage vers `nie-data`

Les modules de `crates/engine/nie-data/src/` sont nommés **par concept**, pas par format ni par
nom de fichier. Chercher par marqueur avant d'en créer un :

```bash
grep -rl "<MARKER_LIST>" crates/engine/nie-data/src/
```

Une famille portée se valide par un golden : `cargo test -p nie-data --test <fam>_golden`.
