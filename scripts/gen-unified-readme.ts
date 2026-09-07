import { writeFileSync } from "fs";

const unifiedReadmeContent = `# Documentation niers — Architecture & Index Unifié

> **Dépôt :** \`aphrody-code/nie\` (projet \`niers\`)  
> **Mission :** Réécriture pixel-perfect et byte-exact de *Inazuma Eleven: Victory Road* (\`nie.exe\`) en Rust natif.  
> **Contrat Commercial :** Accord N° RG-L5-VR-2026-001 (Rose Griffon / LEVEL-5 Inc.).

---

## 1. Références Maîtresses & Règle Unique

Pour éviter la dispersion et les redondances entre agents (Claude, Codex, Astra/AGY), la documentation est désormais articulée autour de **trois documents consolidés** :

| Document | Rôle & Contenu |
| :--- | :--- |
| **[UNIFIED-AGENTS.md](UNIFIED-AGENTS.md)** | **Contrat opérationnel unique** : règles multi-agents, urgence/YOLO, convention de langue (EN identifiants / FR utilisateur), gates Cargo/Bun, protocole A2A, pièges de compilation et environnement Windows/Linux. |
| **[UNIFIED-PLAN.md](UNIFIED-PLAN.md)** | **Feuille de route & Plans unifiés** : synthèse du cap ultime (\`manquant = 0\`), des 7 blocs prioritaires actifs, de la bascule Vercel/Aphrody.com et de la forge byte-exacte. |
| **[AUDIT-USAGE-GEMINI-FLASH.md](AUDIT-USAGE-GEMINI-FLASH.md)** | **Analyse des quotas & capacité de travail** : mesure des 307 623 lignes Rust, calcul de l'overhead agy-cli et projection d'autonomie avec Gemini 3.8 Flash (Thinking Low). |

---

## 2. Cartographie Thématique du Dossier \`docs/\`

### 2.1 Moteur de Jeu & Rendu Graphique
- **[STACK.md](STACK.md)** : Architecture runtime du moteur, intégration Lua 5.2 et boucle principale.
- **[DESIGN.md](DESIGN.md)** & **[DESIGN-UI.md](DESIGN-UI.md)** : Rendu pixel-perfect des écrans Start, Menu et HUD (mesures directes sur les captures de \`data/menu/\`).
- **[AVATAR.md](AVATAR.md)** : Spécifications complètes de l'éditeur d'avatar (\`chara_edit\`).
- **[PLAN-SESSION-3D.md](PLAN-SESSION-3D.md)** : Pipeline de rendu 3D serveur et intégration \`nie-render3d\`.
- **[BENCHMARKS.md](BENCHMARKS.md)** : Mesures comparatives de performance (Rust vs C++ vs C#).

### 2.2 Reverse Engineering, Binaire & Formats
- **[FORGE.md](FORGE.md)** : Production de \`nie.exe\` byte-exact (atteint 74.00% du binaire et 92.24% de \`.text\`).
- **[RE.md](RE.md)** : Base de connaissances RE, ancrage des fonctions et structures décompilées.
- **[FORMATS.md](FORMATS.md)** & **[VFS.md](VFS.md)** : Spécifications des conteneurs CPK, RDBN, T2B, textures G4TX et VFS (255 308 fichiers indexés).
- **[modele-de-match.md](modele-de-match.md)** : Analyse de la simulation match et calculs de tirs/arrêts.

### 2.3 Applications, Wiki & Production Web
- **[AZALEE.md](AZALEE.md)** & **[MIGRATION-SUPABASE-CLOUD-ANALYSIS.md](MIGRATION-SUPABASE-CLOUD-ANALYSIS.md)** : Architecture serverless du wiki Azalée sur Vercel et pooler Supabase Cloud.
- **[MIGRATION-EXPLORATEUR.md](MIGRATION-EXPLORATEUR.md)** : Unification Inacord / Aphrody via \`packages/inacord-ui\`.
- **[FILTRES.md](FILTRES.md)** : Matrice des filtres et navigation du catalogue.
- **[EXPLOITATION.md](EXPLOITATION.md)** & **[SECURITE-BASCULE.md](SECURITE-BASCULE.md)** : Gestion de la production VPS, services systemd, nginx et remédiation sécurité.
- **[FUSION.md](FUSION.md)** : Justification du monorepo unifié pour l'écosystème Inazuma Eleven.

---

## 3. Règle d'Exécution & Invariant

1. **Aucun commit aveugle :** Une gate n'est validée que lorsqu'une commande a été jouée et a retourné un compte exact (lignes, liens, bytes, code de retour).
2. **Déploiement live :** Un service déployé n'est achevé que lorsqu'une requête en ligne sur son port/domaine a certifié son statut.
`;

writeFileSync("docs/README.md", unifiedReadmeContent, "utf-8");
console.log("Updated docs/README.md");
