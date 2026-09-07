import { existsSync, readFileSync } from "node:fs";
import { execSync } from "node:child_process";

console.log("⚡ [YOLO 2026] Initialisation de l'analyseur de projet autonome...");

try {
  const status = execSync("git status --short", { encoding: "utf8" });
  console.log("📊 Git status:\n" + (status || "Arborescence propre."));
} catch (e) {
  console.log("⚠️ Aucun dépôt git détecté.");
}

const planCandidates = ["docs/PLAN.md", "UNIFIED-PLAN.md", "docs/UNIFIED-PLAN.md", "PLAN.md", "TODO.md"];
let detectedPlan = null;
for (const p of planCandidates) {
  if (existsSync(p)) {
    detectedPlan = p;
    break;
  }
}

if (detectedPlan) {
  console.log(`📋 Plan identifié : ${detectedPlan}`);
  const content = readFileSync(detectedPlan, "utf8");
  const openTasks = content.split("\n").filter(l => l.includes("- [ ]") || l.includes("⏳"));
  console.log(`🎯 Tâches ouvertes détectées : ${openTasks.length}`);
  openTasks.slice(0, 5).forEach((t, i) => console.log(`   ${i+1}. ${t.trim()}`));
} else {
  console.log("ℹ️ Aucun fichier plan canonique trouvé.");
}

if (existsSync("Cargo.toml")) console.log("🦀 Environnement Rust détecté.");
if (existsSync("package.json")) console.log("🥟 Environnement JS/TS détecté.");
if (existsSync("pyproject.toml")) console.log("🐍 Environnement Python détecté.");
