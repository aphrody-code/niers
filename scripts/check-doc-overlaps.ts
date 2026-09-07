import { readFileSync, existsSync } from "fs";

const proc = Bun.spawnSync(["git", "ls-files", "*.md"]);
const files = proc.stdout.toString().trim().split("\n").filter(Boolean);

// On teste les intersections de tokens (mots-clés rares) entre les gros documents docs/
const docs = files.filter(f => f.startsWith("docs/") && !f.includes("game-data") && !f.includes("vfs"));

function getKeywords(text: string): Set<string> {
  const words = text.toLowerCase().match(/[a-z0-9_\-\.]{5,}/g) || [];
  const set = new Set<string>();
  for (const w of words) {
    if (!["https", "fichier", "fichiers", "commit", "valeur", "donnees", "projet", "command", "return"].includes(w)) {
      set.add(w);
    }
  }
  return set;
}

const docKeywords = docs.map(f => ({
  file: f,
  keywords: getKeywords(readFileSync(f, "utf-8"))
}));

console.log("Checking conceptual overlaps across main docs:");
for (let i = 0; i < docKeywords.length; i++) {
  for (let j = i + 1; j < docKeywords.length; j++) {
    const a = docKeywords[i];
    const b = docKeywords[j];
    let common = 0;
    for (const k of a.keywords) {
      if (b.keywords.has(k)) common++;
    }
    const similarity = common / Math.min(a.keywords.size, b.keywords.size);
    if (similarity > 0.45) {
      console.log(`High overlap (${(similarity * 100).toFixed(1)}%): ${a.file} <--> ${b.file}`);
    }
  }
}
