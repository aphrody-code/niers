import { readFileSync } from "fs";

const proc = Bun.spawnSync(["git", "ls-files", "*.md"]);
const files = proc.stdout.toString().trim().split("\n").filter(Boolean);

// Catégories
const categories = {
  ai: [] as { file: string; size: number }[],
  docs: [] as { file: string; size: number }[],
  packages: [] as { file: string; size: number }[],
  apps: [] as { file: string; size: number }[],
  crates: [] as { file: string; size: number }[],
  others: [] as { file: string; size: number }[],
};

for (const f of files) {
  const stat = Bun.file(f);
  const item = { file: f, size: stat.size };
  if (f.startsWith(".agents") || f.startsWith(".claude") || f.includes("AGENTS") || f.includes("CLAUDE") || f.includes("GEMINI") || f.includes("ai.json") || f.includes("A2A")) {
    categories.ai.push(item);
  } else if (f.startsWith("docs/")) {
    categories.docs.push(item);
  } else if (f.startsWith("packages/")) {
    categories.packages.push(item);
  } else if (f.startsWith("apps/")) {
    categories.apps.push(item);
  } else if (f.startsWith("crates/")) {
    categories.crates.push(item);
  } else {
    categories.others.push(item);
  }
}

console.log("AI files:", categories.ai.length);
console.log("Docs files:", categories.docs.length);
console.log("Packages files:", categories.packages.length);
console.log("Apps files:", categories.apps.length);
console.log("Crates files:", categories.crates.length);
console.log("Others files:", categories.others.length);

console.log("\n--- AI FILES ---");
categories.ai.forEach(x => console.log(`${x.size.toString().padStart(6)} B  ${x.file}`));
