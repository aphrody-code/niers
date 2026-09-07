import { readFileSync } from "fs";

const proc = Bun.spawnSync(["git", "ls-files", "*.md"]);
const files = proc.stdout.toString().trim().split("\n").filter(Boolean);

// On extrait des n-grammes de phrases de 50+ caractères
const phraseMap = new Map<string, string[]>();

for (const f of files) {
  const text = readFileSync(f, "utf-8");
  const paragraphs = text.split(/\n\s*\n/).map(p => p.trim()).filter(p => p.length > 120);
  for (const p of paragraphs) {
    // normaliser un peu
    const clean = p.replace(/\s+/g, " ").slice(0, 150);
    if (!phraseMap.has(clean)) {
      phraseMap.set(clean, []);
    }
    const arr = phraseMap.get(clean)!;
    if (!arr.includes(f)) arr.push(f);
  }
}

const duplicates = Array.from(phraseMap.entries()).filter(([_, files]) => files.length > 1);
duplicates.sort((a, b) => b[1].length - a[1].length);

console.log(`Found ${duplicates.length} duplicate paragraphs across different md files:`);
for (const [snippet, flist] of duplicates.slice(0, 20)) {
  console.log(`\nSnippet: "${snippet.slice(0, 70)}..."`);
  console.log(`Present in ${flist.length} files:`, flist);
}
