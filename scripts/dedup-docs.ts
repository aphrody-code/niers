import { writeFileSync } from "fs";

// 1. apps/azalee/AGENTS.md -> redirect cleanly
const azaleeAgents = `# Next.js Rules (Azalée)

See root [AGENTS.md](../../AGENTS.md) for master repository rules and architecture.

<!-- BEGIN:nextjs-agent-rules -->
## Next.js Framework Rules
This application uses Next.js 16 App Router. APIs, conventions, and file structures differ from older versions.
Refer to \`node_modules/next/dist/docs/\` for updated framework guidelines.
<!-- END:nextjs-agent-rules -->
`;
writeFileSync("apps/azalee/AGENTS.md", azaleeAgents, "utf-8");

// 2. packages/db/GEMINI.md -> streamlined and referencing AGENTS.md
const dbGemini = `# Instructions @rose-griffon/db

See root [AGENTS.md](../../AGENTS.md) for global monorepo rules.

## Package Architecture & Constraints
- Specific entrypoints: \`/browser\`, \`/server\`, \`/service\` to prevent bundling Node.js modules into client bundles.
- **NEVER** import \`@rose-griffon/db/service\` or \`@rose-griffon/db/server\` in client components.
- Run \`bun run types:gen\` with Supabase CLI to update \`src/types.gen.ts\`.
- Always use \`getAssetUrl(path)\` to generate dynamic image URLs.
`;
writeFileSync("packages/db/GEMINI.md", dbGemini, "utf-8");

// 3. apps/azalee/public/llm.md & llms.md deduplication
const llmsContent = `# Azalée — Wiki Inazuma Eleven: Victory Road

> Wiki francophone consacré à Inazuma Eleven: Victory Road (IEVR), maintenu par l'association Rose Griffon.
> Base de données complète : personnages, techniques, objets, auras, tactiques et actualités.

- Wiki : https://azalee.rosegriffon.fr
- Association : Rose Griffon — https://rosegriffon.fr
- Développeur : yoyo — https://x.com/yoyo__goat

## Sections principales
- [Personnages](https://azalee.rosegriffon.fr/chara) : fiches des joueurs et statistiques.
- [Techniques](https://azalee.rosegriffon.fr/skill) : techniques spéciales (tirs, dribbles, blocs, gardiens).
- [Objets](https://azalee.rosegriffon.fr/item) : équipements et consommables.
- [Auras](https://azalee.rosegriffon.fr/aura) : esprits guerriers, totems, miximax, éveils.
- [Passifs](https://azalee.rosegriffon.fr/passive) : compétences passives.
- [Tactiques](https://azalee.rosegriffon.fr/tactic) : tactiques d'équipe.
- [Actualités](https://azalee.rosegriffon.fr/news) : annonces et patch-notes.
- [Explorateur Inacord](https://aphrody.com) : outils de bureau et navigation de catalogue.

## Liens utiles
- [Sitemap](https://azalee.rosegriffon.fr/sitemap.xml)
- [robots.txt](https://azalee.rosegriffon.fr/robots.txt)
`;
writeFileSync("apps/azalee/public/llms.md", llmsContent, "utf-8");
writeFileSync("apps/azalee/public/llm.md", `# Redirect to llms.md\n\nSee [llms.md](llms.md) for the complete LLM knowledge map.\n`, "utf-8");

// 4. docs/mainmenu01-analyse-visuelle.md -> streamline by linking to docs/DESIGN.md
const mainmenuVisual = `# \`mainmenu01\` — Mesures & Analyse Visuelle

Pour l'implémentation complète du rendu pixel-perfect du moteur, voir [DESIGN.md](DESIGN.md).

## Synthèse des mesures d'angle et de géométrie (2026-09-06)
- **Angle des tuiles de la rangée** : pente mesurée à **dx/dy = -0,400** (angle exact -21,80°, R² = 1,000).
- **Panneau droit** : pente mesurée à **dx/dy = -0,546** (angle -28,63°, R² = 1,000).
- **Palette mesurée sur capture 2048×1159** :
  - Fond dominant (69,0%) : \`#F9FDF9\` (Oklch 0,990 0,007 145°)
  - Bleu bandeau (10,4%) : \`#93D3F0\` (Oklch 0,834 0,077 228°)
  - Bleu nuit tuiles (7,7%) : \`#2C497C\` (Oklch 0,409 0,093 261°)
  - Bleu icônes (7,1%) : \`#4B8DD5\` (Oklch 0,633 0,128 252°)

Script d'extraction : \`scripts/validation/mesurer-mainmenu.py\`.
Valeurs figées dans \`packages/inacord-ui/src/shell/geometrie-mainmenu.ts\`.
`;
writeFileSync("docs/mainmenu01-analyse-visuelle.md", mainmenuVisual, "utf-8");

// 5. docs/stack/security.md -> reference docs/SECURITE-BASCULE.md
const stackSecurity = `# Sécurité et Prérequis d'Exposition

Audit exhaustif et historique des tests d'intrusion : voir [../SECURITE-BASCULE.md](../SECURITE-BASCULE.md).

## Synthèse du Statut de Sécurité (J6)
- Les points critiques self-host (RPC anonyme, grants excessifs, JWT exposé) ont été traités dans la feuille de route J6.
- Sur Vercel, le wiki n'utilise que la clé publique \`anon\` sous RLS stricte.
- \`nie-model-serve\` est confiné derrière \`nie-site\` en réseau privé sans exposition publique directe.
`;
writeFileSync("docs/stack/security.md", stackSecurity, "utf-8");

console.log("Deduplicated target files successfully.");
