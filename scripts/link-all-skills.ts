import { readdirSync, existsSync } from "fs";
import { join } from "path";

const src = "plugins/niers-plugin/skills";
const dest = ".agents/skills";

for (const s of readdirSync(src)) {
  const target = join(dest, s);
  if (!existsSync(target)) {
    const fullSrc = join("C:/Users/aphro/niers", src, s);
    Bun.spawnSync(["powershell", "-Command", `New-Item -ItemType Junction -Path "${target}" -Target "${fullSrc}"`]);
    console.log(`Linked skill: ${s}`);
  }
}
