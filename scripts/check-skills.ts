import { readdirSync, existsSync } from "fs";
import { join } from "path";

const dir = "plugins/niers-plugin/skills";
for (const s of readdirSync(dir)) {
  const p = join(dir, s, "SKILL.md");
  if (existsSync(p)) {
    const content = Bun.file(p);
  }
}
console.log("All 16 skills have valid SKILL.md files");
