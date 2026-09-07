import { writeFileSync } from "fs";

const scriptContent = `#!/usr/bin/env bun
/**
 * Automation daemon for A2A synchronization in niers.
 * Usage: bun scripts/a2a-sync.ts [tick|status|heartbeat]
 */
import { existsSync, writeFileSync, appendFileSync } from "fs";

const command = process.argv[2] || "heartbeat";
const astraId = "astra@aphrody-code/niers";
const heartbeatFile = ".coord/heartbeat-astra.txt";
const now = new Date().toISOString();

if (command === "heartbeat") {
  writeFileSync(heartbeatFile, now + "\\n", "utf-8");
  console.log(\`[A2A] Heartbeat updated for \${astraId}: \${now}\`);
} else if (command === "tick") {
  const subject = process.argv[3] || "fact: automated tick";
  const body = process.argv[4] || "nominal";
  const proc = Bun.spawnSync([
    "aphrody", "a2a", "tick",
    "--iteration", "0",
    "--side", "astra",
    "--peer", "claude",
    "--kind", "fact",
    "--subject", subject,
    "--body", body
  ]);
  console.log(proc.stdout.toString().trim());
} else if (command === "status") {
  const proc = Bun.spawnSync(["aphrody", "doctor"]);
  console.log(proc.stdout.toString().trim());
}
`;

writeFileSync("scripts/a2a-sync.ts", scriptContent, "utf-8");
console.log("Created scripts/a2a-sync.ts");
