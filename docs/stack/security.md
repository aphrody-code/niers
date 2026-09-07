# Sécurité et Prérequis d'Exposition

Audit exhaustif et historique des tests d'intrusion : voir [../SECURITE-BASCULE.md](../SECURITE-BASCULE.md).

## Synthèse du Statut de Sécurité (J6)
- Les points critiques self-host (RPC anonyme, grants excessifs, JWT exposé) ont été traités dans la feuille de route J6.
- Sur Vercel, le wiki n'utilise que la clé publique `anon` sous RLS stricte.
- `nie-model-serve` est confiné derrière `nie-site` en réseau privé sans exposition publique directe.
