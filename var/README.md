# var/ — artefacts generes (gitignore, JAMAIS edites a la main)

Inventaire des artefacts derives. Regen groupee : `bash scripts/regen-var.sh --all`.

| Artefact | Taille | Regenerable | Commande de regen | Invalider quand |
|---|---|---|---|---|
| `niers.sqlite` | ~237 Mo | oui | `niers seed` + `niers rebuild` (ou `scripts/regen-var.sh --kb`) | l'exe RE change (nouvelle MAJ du jeu) |
| `niers.sqlite-wal` / `-shm` | var | oui | auto (WAL sqlite) | jamais a la main |
| `model-crc-manifest.ndjson` | ~3,5 Mo | oui | `niers uniform-map` (`--models`) | les CPK du jeu changent |
| `uniform-model-map.ndjson` | ~655 Ko | oui | derive inagle + uniform-map | donnees uniformes inagle changent |
| `g4tx-manifest.ndjson` | var | oui | `niers textures` (`--textures`) | les CPK changent |
| `g4tx-xval.ndjson`, `xval.ndjson` | ~166 Ko | oui | cross-check validation g4tx | regression decodeur g4tx |
| `model-cache/` | ~variable | oui (a la volee) | servi par nie-model-serve | MAJ du jeu → purger |
| `zukan/` | ~272 Mo | semi | `niers wiki` / ingesteur zukan | re-scrape zukan voulu |
| `rag/` | var | oui | `scripts/rag-build.py` (service e5 sur :8799) | corpus RE change |
| `re-heartbeat.log` | append | n/a | cron `/tmp/niers-re-heartbeat.sh` | CASSE si `target/release/niers` absent → rebuild |
| `ghidra-decompile/`, `ghidra-scripts/` | var | oui | skill `ghidra-headless` | re-export Ghidra |

Notes :
- Apres une MAJ du jeu/exe : `scripts/regen-var.sh --all` (regenere KB + manifestes), puis penser a purger `model-cache/` et redemarrer `nie-model-serve`.
- Redis (hors var/) : db0 = frontiere RE + wiki ; db3 = index fichiers CPK + textures. NE PAS exporter `NIERS_REDIS` pour `niers textures`/`menu-predecode` (ecraserait db3 par db0).
