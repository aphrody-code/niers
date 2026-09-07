import { readFileSync, writeFileSync, readdirSync, existsSync, mkdirSync } from "fs";
import { join } from "path";

const sources = [
  "C:/Users/aphro/.claude/projects/C--Program-Files--x86--Steam-steamapps-common-INAZUMA-ELEVEN-Victory-Road/memory",
  "C:/Users/aphro/.claude/projects/C--Users-aphro-nie/memory",
  "C:/Users/aphro/.claude/projects/C--Users-aphro/memory",
  "C:/Users/aphro/.claude/projects/C--Users-aphro-aphrody/memory"
];

let allMemories: { file: string; content: string }[] = [];
for (const dir of sources) {
  if (!existsSync(dir)) continue;
  for (const f of readdirSync(dir)) {
    if (!f.endsWith(".md") || f === "MEMORY.md") continue;
    const content = readFileSync(join(dir, f), "utf-8");
    allMemories.push({ file: f, content });
  }
}

console.log("Total memory modules found:", allMemories.length);

const outDir = ".agents/rules";
if (!existsSync(outDir)) mkdirSync(outDir, { recursive: true });

let combined = "# MEMOIRE GLOBALE & REGLES OPERATIONNELLES NIERS (CLAUDE / CODEX / AGY)\n\n";
combined += "> Synchronisation dynamique des regles fondamentales et retours d'experience du projet `niers` (aphrody-code/nie) pour Antigravity CLI.\n\n";

for (const m of allMemories) {
  combined += "## Module: " + m.file + "\n\n" + m.content.trim() + "\n\n---\n\n";
}

writeFileSync(outDir + "/project-memory.md", combined, "utf-8");
console.log("Memory rule written to .agents/rules/project-memory.md, length:", combined.length);
