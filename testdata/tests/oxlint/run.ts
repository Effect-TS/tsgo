// PROTOTYPE: one-command launcher for the real Oxlint integration.
import { spawnSync } from "node:child_process"
import { readFileSync, unlinkSync, writeFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

import { type DiagnosticsParams, SyncApi } from "../../../_packages/tsgo/src/experimental/oxlint/sync-api.ts"

const root = fileURLToPath(new URL("../../../", import.meta.url))
const config = fileURLToPath(new URL("./oxlintrc.json", import.meta.url))
const fixtures = [
  fileURLToPath(new URL("./floatingEffect.ts", import.meta.url)),
  fileURLToPath(new URL("./floatingEffect_stream.ts", import.meta.url)),
  fileURLToPath(new URL("./quickfix.ts", import.meta.url)),
]
const quickfixInput = fixtures[2]
const quickfixOutput = fileURLToPath(new URL("./quickfix-output.ts", import.meta.url))

const build = spawnSync("pnpm", ["build:go"], { cwd: root, stdio: "inherit" })
if (build.status !== 0) process.exit(build.status ?? 1)

const protocol = new SyncApi({ cwd: root, executable: fileURLToPath(new URL("../../../tsgo", import.meta.url)) })
const protocolParams = {
  file: quickfixInput,
  text: readFileSync(quickfixInput, "utf8"),
  effectOptions: { diagnostics: true },
}
try {
  let missingRulesRejected = false
  try {
    protocol.diagnostics(protocolParams as DiagnosticsParams)
  } catch (error) {
    missingRulesRejected = error instanceof Error && error.message.includes("missing rules")
  }
  if (!missingRulesRejected) throw new Error("protocol accepted a diagnostics request without rules")

  const empty = protocol.diagnostics({ ...protocolParams, rules: [] })
  if (empty.diagnostics.length !== 0) throw new Error("protocol returned diagnostics for an empty rule set")

  const requestedRule = "missingStarInYieldEffectGen"
  const filtered = protocol.diagnostics({
    ...protocolParams,
    text: `// @effect-diagnostics-next-line floatingEffect:off\nconst unusedDirectiveTarget = "unused"\n${protocolParams.text}`,
    rules: [requestedRule],
  })
  if (filtered.diagnostics.length !== 1 || filtered.diagnostics.some((diagnostic) => diagnostic.ruleName !== requestedRule)) {
    throw new Error("protocol returned a synthetic or unrequested diagnostic")
  }
} finally {
  protocol.close()
}

const lint = spawnSync(
  "pnpm",
  ["dlx", "oxlint@1.75.0", "--config", config, ...fixtures],
  {
    cwd: root,
    env: { EFFECT_TSGO_BRIDGE_TRACE: "1", ...process.env },
    encoding: "utf8",
  },
)

process.stdout.write(lint.stdout)
process.stderr.write(lint.stderr)
const lintOutput = lint.stdout + lint.stderr
const floatingCount = lintOutput.match(/error effect\(floatingEffect\):/g)?.length ?? 0
const quickfixCount = lintOutput.match(/warning effect\(missingStarInYieldEffectGen\):/g)?.length ?? 0
const clientCount = lintOutput.match(/client-started/g)?.length ?? 0
const requestCount = lintOutput.match(/"diagnosticsRequests":1/g)?.length ?? 0
if (lint.status !== 1 || floatingCount !== 5 || quickfixCount !== 1 || clientCount !== 1 || requestCount !== fixtures.length) {
  console.error(
    `Prototype failed: expected 5 floating errors, 1 quick-fix warning, 1 client, and ${fixtures.length} requests; received ${floatingCount}, ${quickfixCount}, ${clientCount}, and ${requestCount}`,
  )
  process.exit(lint.status ?? 1)
}

const quickfixSource = readFileSync(quickfixInput, "utf8")
let quickfixFailure: string | undefined
writeFileSync(quickfixOutput, quickfixSource)
try {
  const safeFix = spawnSync(
    "pnpm",
    ["dlx", "oxlint@1.75.0", "--fix", "--config", config, quickfixOutput],
    { cwd: root, stdio: "inherit" },
  )
  if (safeFix.status !== 0 && safeFix.status !== 1) {
    quickfixFailure = `plain --fix exited with status ${safeFix.status}`
  } else if (readFileSync(quickfixOutput, "utf8") !== quickfixSource) {
    quickfixFailure = "plain --fix unexpectedly applied an Effect suggestion"
  }

  const fixed = spawnSync(
    "pnpm",
    ["dlx", "oxlint@1.75.0", "--fix-suggestions", "--config", config, quickfixOutput],
    { cwd: root, stdio: "inherit" },
  )
  if (fixed.status !== 0) {
    quickfixFailure ??= `--fix-suggestions exited with status ${fixed.status}`
  } else if (!readFileSync(quickfixOutput, "utf8").includes("return yield* Effect.succeed(1)")) {
    quickfixFailure ??= "Oxlint did not apply the Effect suggestion"
  }
} finally {
  unlinkSync(quickfixOutput)
}
if (quickfixFailure) {
  console.error(`Prototype failed: ${quickfixFailure}`)
  process.exit(1)
}

console.log("\nPrototype completed: Oxlint reported diagnostics and applied the Effect suggestion.")
