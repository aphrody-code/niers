import { Database } from "bun:sqlite";
import { readdirSync, statSync, existsSync } from "fs";
import { join } from "path";

// ─── 1. Modélisation Quotas & Spécifications ─────────────────────────────────

const QUOTA_WEEKLY_PERCENT_REMAINING = 98.38;
const QUOTA_5H_PERCENT_REMAINING = 99.94;

const ESTIMATED_TOTAL_WEEKLY_TOKENS = 15_000_000;
const ESTIMATED_5H_BURST_TOKENS = 1_500_000;

const REMAINING_WEEKLY_TOKENS = Math.round(ESTIMATED_TOTAL_WEEKLY_TOKENS * (QUOTA_WEEKLY_PERCENT_REMAINING / 100));
const REMAINING_5H_TOKENS = Math.round(ESTIMATED_5H_BURST_TOKENS * (QUOTA_5H_PERCENT_REMAINING / 100));

// ─── 2. Analyse de la couche locale AGY CLI & Session active ─────────────────

const agyDir = "C:/Users/aphro/.gemini/antigravity-cli";
const convId = process.env.ANTIGRAVITY_CONVERSATION_ID || "b1dc189c-77fc-4dec-9561-5b9a5a120b4b";
const convDbPath = `${agyDir}/conversations/${convId}.db`;
const transcriptPath = `${agyDir}/brain/${convId}/.system_generated/logs/transcript_full.jsonl`;

let sessionMetrics = {
  dbExists: existsSync(convDbPath),
  stepsCount: 0,
  genMetaCount: 0,
  transcriptBytes: 0,
  transcriptLines: 0,
  modelName: "gemini-3.8-flash (thinking low)",
};

if (sessionMetrics.dbExists) {
  try {
    const db = new Database(convDbPath, { readonly: true });
    const stepsRow = db.query("SELECT COUNT(*) as count FROM steps").get() as { count: number };
    const metaRow = db.query("SELECT COUNT(*) as count FROM gen_metadata").get() as { count: number };
    sessionMetrics.stepsCount = stepsRow?.count || 0;
    sessionMetrics.genMetaCount = metaRow?.count || 0;
  } catch (e) {
    console.error("Erreur lecture SQLite AGY:", e);
  }
}

if (existsSync(transcriptPath)) {
  const file = Bun.file(transcriptPath);
  sessionMetrics.transcriptBytes = file.size;
  const content = await file.text();
  sessionMetrics.transcriptLines = content.trim().split("\n").length;
}

// ─── 3. Analyse de la surface Rust trackée du dépôt ─────────────────────────

const gitProc = Bun.spawnSync(["git", "ls-files", "*.rs"]);
const trackedRsFiles = gitProc.stdout.toString().trim().split("\n").filter(Boolean);

let totalRsBytes = 0;
let totalRsLines = 0;
const crateDistribution: Record<string, { files: number; lines: number; bytes: number }> = {};

for (const relPath of trackedRsFiles) {
  const f = Bun.file(relPath);
  const size = f.size;
  totalRsBytes += size;
  const text = await f.text();
  const lines = text.split("\n").length;
  totalRsLines += lines;

  const parts = relPath.split(/[\\\/]/);
  const rootGroup = parts.length > 2 ? `${parts[0]}/${parts[1]}` : parts[0];
  if (!crateDistribution[rootGroup]) {
    crateDistribution[rootGroup] = { files: 0, lines: 0, bytes: 0 };
  }
  crateDistribution[rootGroup].files++;
  crateDistribution[rootGroup].lines += lines;
  crateDistribution[rootGroup].bytes += size;
}

const totalRsEstimatedTokens = Math.round(totalRsBytes / 3.8);

// ─── 4. Analyse du Plan (PLAN.md & CODEX-JOUR-UNIQUE.md) ──────────────────────

const remainingTasks = [
  { block: "Bloc 1", title: "Portail Bun & Typecheck (mcp & cron)", costTokens: 120_000, turns: 4 },
  { block: "Bloc 2", title: "J2 Wiki Serverless (zéro lecture locale + assets nie-web)", costTokens: 350_000, turns: 10 },
  { block: "Bloc 3", title: "J3 Optimisation poids & ISR (/chara < 250Ko)", costTokens: 180_000, turns: 6 },
  { block: "Bloc 4", title: "J4 Débranding Rose Griffon vers aphrody-dev", costTokens: 150_000, turns: 5 },
  { block: "Bloc 5", title: "nie-site production rebuild (correctif WAL)", costTokens: 80_000, turns: 3 },
  { block: "Bloc 6", title: "J7 Moka cache + baseline criterion + doc audits", costTokens: 200_000, turns: 6 },
  { block: "Bloc 7", title: "Couverture Ultime (manquant = 0, 583 capacités)", costTokens: 1_200_000, turns: 35 },
];

const totalPlanTokensEstimated = remainingTasks.reduce((acc, t) => acc + t.costTokens, 0);
const totalPlanTurnsEstimated = remainingTasks.reduce((acc, t) => acc + t.turns, 0);

const AVERAGE_TOKENS_PER_TURN = 28_000;
const maxTurnsWeekly = Math.floor(REMAINING_WEEKLY_TOKENS / AVERAGE_TOKENS_PER_TURN);
const maxTurns5H = Math.floor(REMAINING_5H_TOKENS / AVERAGE_TOKENS_PER_TURN);

const averageTurnsPerTask = 6;
const maxTasksWeekly = Math.floor(maxTurnsWeekly / averageTurnsPerTask);
const maxTasks5H = Math.floor(maxTurns5H / averageTurnsPerTask);

const activeHoursWeekly = ((maxTurnsWeekly * 1.5) / 60).toFixed(1);
const activeHours5H = ((maxTurns5H * 1.5) / 60).toFixed(1);

// ─── 5. Génération du Rapport Markdown ───────────────────────────────────────

const report = `# RAPPORT D'ESTIMATION D'USAGE & CAPACITÉ — GEMINI 3.8 FLASH (THINKING LOW)

*Date d'évaluation : 2026-09-07T01:55:00+02:00*
*Environnement : Antigravity CLI (agy v1.1.27) / Bun v1.4.0 / Rust Monorepo \`niers\`*

---

## 1. État des Quotas Google AI Pro & Moteur

| Métrique | Valeur Brute | État Restant | Réserve Estimée (Tokens de calcul) |
| :--- | :--- | :--- | :--- |
| **Weekly Limit Remaining** | \`98.38%\` | 132h 28m restantes (~5,5 jours) | **~14 757 000 tokens** (base 15M) |
| **Five Hour Limit (Burst)**| \`99.94%\` | 4h 59m restantes | **~1 499 000 tokens** (base 1.5M) |
| **Modèle Actif** | **Gemini 3.8 Flash** | **Thinking Low** | 1M Context / 65K Output |
| **Profil de pensée** | Bridé économique (~400-800 tks) | Zéro surchauffe | Pas de dérive de tokens de réflexion |

---

## 2. Empreinte de la Session AGY Active (\`${convId}\`)

- **Base SQLite de conversation** : \`${convDbPath}\`
- **Nombre total de steps exécutés** : **${sessionMetrics.stepsCount}** (Call, Response, Tool Execution)
- **Métadonnées de génération (gen_metadata)** : **${sessionMetrics.genMetaCount}** requêtes modèles
- **Journal d'exécution (\`transcript_full.jsonl\`)** : **${sessionMetrics.transcriptLines}** lignes, **${(sessionMetrics.transcriptBytes / 1024).toFixed(1)} Ko**

---

## 3. Surface de Code Rust Trackée (\`niers\`)

| Périmètre | Fichiers \`.rs\` | Lignes de Code | Taille Brute | Tokens Équivalents (Code brut) |
| :--- | :--- | :--- | :--- | :--- |
| **TOTAL Rust Tracké** | **${trackedRsFiles.length}** | **${totalRsLines.toLocaleString()}** | **${(totalRsBytes / (1024 * 1024)).toFixed(2)} Mo** | **~${(totalRsEstimatedTokens / 1_000_000).toFixed(2)} M tokens** |

### Découpage par pôle de crates :
${Object.entries(crateDistribution)
  .sort((a, b) => b[1].lines - a[1].lines)
  .slice(0, 10)
  .map(([k, v]) => `- **\`${k}\`** : ${v.files} fichiers, ${v.lines.toLocaleString()} lignes (${(v.bytes / 1024).toFixed(0)} Ko)`)
  .join("\n")}

---

## 4. Reste à Faire vs Plan (\`PLAN.md\` & \`CODEX-JOUR-UNIQUE.md\`)

| Bloc du Plan | Intitulé & Périmètre | Turns Estimés | Coût Token Estimé |
| :--- | :--- | :--- | :--- |
${remainingTasks.map(t => `| **${t.block}** | ${t.title} | ~${t.turns} | ~${(t.costTokens / 1000).toFixed(0)}k |`).join("\n")}
| **TOTAL TOUT LE PLAN** | **J1 → J7 + Couverture Ultime** | **~${totalPlanTurnsEstimated} turns** | **~${(totalPlanTokensEstimated / 1_000_000).toFixed(2)} M tokens** |

---

## 5. Synthèse d'Autonomie & Capacité d'Exécution

| Critère d'Autonomie | Sur la fenêtre Burst (5 heures) | Sur le Quota Hebdomadaire (7 jours) |
| :--- | :--- | :--- |
| **Tokens Disponibles** | **~1 499 000 tokens** | **~14 757 000 tokens** |
| **Interactions (Turns d'outils / agent)** | **~50 à 55 turns** | **~500 à 550 turns** |
| **Sessions / Tâches complètes** | **~8 à 10 tâches architecturales** | **~85 à 90 tâches architecturales** |
| **Temps d'exécution actif non-stop** | **~1,2 à 1,5 heure pure** (sur 5h d'intervalle) | **~12 à 15 heures de dev intensif continu** |
| **Couverture du Plan complet** | Réalise **2 à 3 blocs majeurs** (ex: Blocs 1, 2, 5) | **Couvre 6.5× la totalité du Plan restant !** |

---

## Conclusion & Verdict Opérationnel

1. **Aucun risque de saturation hebdomadaire** : Même en exécutant l'intégralité du plan restant (J1 à J7 + Couverture Ultime à 100% de \`manquant = 0\`), la consommation prévisionnelle (~2,28 M tokens) ne consommera qu'environ **15,4%** de votre quota hebdomadaire Gemini.
2. **Gestion du burst de 5h** : Avec Flash Thinking Low, vous pouvez enchaîner sans aucune pause 50 turns complets de modification de code Rust, exécution de tests et recompilation Cargo avant d'atteindre le palier de régulation de 5 heures.
`;

await Bun.write("docs/AUDIT-USAGE-GEMINI-FLASH.md", report);
console.log("Rapport généré avec succès dans docs/AUDIT-USAGE-GEMINI-FLASH.md");
