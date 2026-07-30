// PROTOTYPE: one-command launcher for the real Oxlint integration.
import { spawnSync } from "node:child_process"
import { readFileSync, unlinkSync, writeFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

import { type RunEffectDiagnosticsParams, SyncApi } from "../../../_packages/tsgo/src/experimental/api/sync-api.ts"

const root = fileURLToPath(new URL("../../../", import.meta.url))
const config = fileURLToPath(new URL("./oxlintrc.json", import.meta.url))
const fixtures = [
  fileURLToPath(new URL("./floatingEffect.ts", import.meta.url)),
  fileURLToPath(new URL("./floatingEffect_stream.ts", import.meta.url)),
  fileURLToPath(new URL("./quickfix.ts", import.meta.url)),
]
const quickfixInput = fixtures[2]
const quickfixOutput = fileURLToPath(new URL("./quickfix-output.ts", import.meta.url))
const quickfixSource = readFileSync(quickfixInput, "utf8")

const build = spawnSync("pnpm", ["build:go"], { cwd: root, stdio: "inherit" })
if (build.status !== 0) process.exit(build.status ?? 1)

const protocol = new SyncApi({ cwd: root, executable: fileURLToPath(new URL("../../../tsgo", import.meta.url)) })
const protocolParams = {
  targetFilePath: quickfixInput,
  overrideEffectOptions: { diagnostics: true },
}
try {
  const configured = protocol.runEffectDiagnostics(protocolParams)
  if (configured.diagnostics.length !== 1) throw new Error("protocol did not run configured diagnostics when onlyRules was omitted")

  const empty = protocol.runEffectDiagnostics({ ...protocolParams, onlyRules: [] })
  if (empty.diagnostics.length !== 0) throw new Error("protocol returned diagnostics for an empty rule set")

  const requestedRule = "missingStarInYieldEffectGen"
  const filtered = protocol.runEffectDiagnostics({
    ...protocolParams,
    overrideSourceText: `// @effect-diagnostics-next-line floatingEffect:off\nconst unusedDirectiveTarget = "unused"\n${quickfixSource}`,
    onlyRules: [requestedRule],
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
    encoding: "utf8",
  },
)

process.stdout.write(lint.stdout)
process.stderr.write(lint.stderr)
const lintOutput = lint.stdout + lint.stderr
const floatingCount = lintOutput.match(/error effect\(floatingEffect\):/g)?.length ?? 0
const quickfixCount = lintOutput.match(/warning effect\(missingStarInYieldEffectGen\):/g)?.length ?? 0
if (lint.status !== 1 || floatingCount !== 5 || quickfixCount !== 1) {
  console.error(
    `Prototype failed: expected 5 floating errors and 1 quick-fix warning; received ${floatingCount} and ${quickfixCount}`,
  )
  process.exit(lint.status ?? 1)
}

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
