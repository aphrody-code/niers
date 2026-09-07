import { readFileSync, existsSync } from "fs";

const proc = Bun.spawnSync(["git", "ls-files", "*.md"]);
const files = proc.stdout.toString().trim().split("\n").filter(Boolean);

interface DocAnalysis {
  file: string;
  size: number;
  lines: number;
  firstLine: string;
}

const list: DocAnalysis[] = [];
for (const f of files) {
  if (!f.startsWith("docs/")) continue;
  const content = readFileSync(f, "utf-8");
  const lines = content.split("\n");
  list.push({
    file: f,
    size: content.length,
    lines: lines.length,
    firstLine: lines[0]?.slice(0, 80) || ""
  });
}

list.sort((a, b) => b.size - a.size);
console.log(`Found ${list.length} docs markdown files. Top 20 by size:`);
for (const d of list.slice(0, 20)) {
  console.log(`${d.size.toString().padStart(6)} B | ${d.lines.toString().padStart(4)} L | ${d.file}`);
}
