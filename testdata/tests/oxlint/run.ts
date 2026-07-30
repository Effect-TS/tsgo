// PROTOTYPE: one-command launcher for the real Oxlint integration.
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"

const root = fileURLToPath(new URL("../../../", import.meta.url))
const config = fileURLToPath(new URL("./oxlintrc.json", import.meta.url))
const fixtures = [
  fileURLToPath(new URL("./floatingEffect.ts", import.meta.url)),
  fileURLToPath(new URL("./floatingEffect_stream.ts", import.meta.url)),
]

const build = spawnSync("pnpm", ["build:go"], { cwd: root, stdio: "inherit" })
if (build.status !== 0) process.exit(build.status ?? 1)

const lint = spawnSync(
  "pnpm",
  ["dlx", "oxlint@1.75.0", "--config", config, ...fixtures],
  {
    cwd: root,
    env: { EFFECT_TSGO_BRIDGE_TRACE: "1", ...process.env },
    stdio: "inherit",
  },
)

if (lint.status === 1) {
  console.log("\nPrototype completed: Oxlint reported the expected Effect diagnostics.")
  process.exit(0)
}
process.exit(lint.status ?? 1)
