import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { existsSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const repositoryRoot = process.argv[2]
assert.ok(repositoryRoot, "repository root argument is required")
const tsgolint = process.argv[3]
assert.ok(tsgolint, "tsgolint path argument is required")
assert.ok(existsSync(tsgolint), `tsgolint executable does not exist: ${tsgolint}`)

const fixture = dirname(fileURLToPath(import.meta.url))
const oxlint = join(repositoryRoot, "oxlint", "apps", "oxlint", "dist", "cli.js")
const packageDirectory = join(repositoryRoot, "_packages", "tsgo")
const env = {
  ...process.env,
  OXLINT_TSGOLINT_PATH: tsgolint
}

const run = (...args) => spawnSync(process.execPath, [oxlint, ...args], {
  cwd: fixture,
  encoding: "utf8",
  env
})

const packagePreset = spawnSync(process.execPath, [
  "--input-type=module",
  "--eval",
  `import { recommended } from "@effect/tsgo/oxlint-presets";
   import recommendedJson from "@effect/tsgo/oxlint-presets/recommended.json" with { type: "json" };
   if (recommended.rules["effecttsgo/global-date"] !== "warn" || recommendedJson.rules["effecttsgo/global-date"] !== "warn") process.exit(1);`
], {
  cwd: packageDirectory,
  encoding: "utf8"
})
assert.equal(packagePreset.status, 0, packagePreset.stderr)

const rules = run("--rules", "--format", "json")
assert.equal(rules.status, 0, rules.stderr)
const registeredRules = JSON.parse(rules.stdout)
assert.ok(registeredRules.some((rule) => rule.scope === "effecttsgo" && rule.value === "floating-effect"))

const diagnostic = run("--type-aware", "--config", ".oxlintrc.json", "diagnostic.ts")
assert.equal(diagnostic.status, 1, diagnostic.stderr)
assert.match(`${diagnostic.stdout}\n${diagnostic.stderr}`, /effecttsgo\(floating-effect\)/)

const relatedDiagnostic = run("--type-aware", "--format", "json", "--config", ".oxlintrc.json", "related-diagnostic.ts")
assert.equal(relatedDiagnostic.status, 1, relatedDiagnostic.stderr)
const relatedDiagnosticOutput = JSON.parse(relatedDiagnostic.stdout)
const missingStarDiagnostic = relatedDiagnosticOutput.diagnostics.find(
  (item) => item.code === "effecttsgo(missing-star-in-yield-effect-gen)"
)
assert.ok(missingStarDiagnostic, "missing-star-in-yield-effect-gen diagnostic was not reported")
const relatedLabel = missingStarDiagnostic.labels.find(
  (label) => label.label === "Inside this Effect generator."
)
assert.ok(relatedLabel, "related diagnostic was not converted to a labeled range")
assert.deepEqual(relatedLabel.span, { offset: 67, length: 8, line: 3, column: 35 })

const disabled = run("--type-aware", "--config", ".oxlintrc.json", "disabled.ts")
assert.equal(disabled.status, 0, disabled.stderr)
assert.doesNotMatch(`${disabled.stdout}\n${disabled.stderr}`, /effecttsgo\(floating-effect\)/)

const recommended = run("--config", ".oxlintrc-recommended.json", "global-date.ts")
assert.equal(recommended.status, 0, recommended.stderr)
assert.match(`${recommended.stdout}\n${recommended.stderr}`, /effecttsgo\(global-date\)/)

const recommendedOverride = run("--config", ".oxlintrc-recommended-override.json", "global-date.ts")
assert.equal(recommendedOverride.status, 0, recommendedOverride.stderr)
assert.doesNotMatch(`${recommendedOverride.stdout}\n${recommendedOverride.stderr}`, /effecttsgo\(global-date\)/)

console.log("Oxlint profile smoke test passed")
