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
// `tsrs`: the linter is the patched tsrs, which the binding runs as `tsrs --lint headless` when
// EFFECT_TSGO_TSGOLINT_MODE=tsrs comes with EFFECT_TSGO_TSGOLINT_PATH, as `effect-tsgo --tsrs-lint` sets them.
const mode = process.argv[4] ?? "tsgolint"
assert.ok(mode === "tsgolint" || mode === "tsrs", `unknown linter mode: ${mode}`)

const fixture = dirname(fileURLToPath(import.meta.url))
const oxlint = join(repositoryRoot, "oxlint", "apps", "oxlint", "dist", "cli.js")
const packageDirectory = join(repositoryRoot, "_packages", "tsgo")
const linterEnv = mode === "tsrs"
  ? { EFFECT_TSGO_TSGOLINT_PATH: tsgolint, EFFECT_TSGO_TSGOLINT_MODE: "tsrs" }
  : { OXLINT_TSGOLINT_PATH: tsgolint }
const env = {
  ...process.env,
  ...linterEnv
}

const run = (...args) => spawnSync(process.execPath, [oxlint, ...args], {
  cwd: fixture,
  encoding: "utf8",
  env
})

const packagePreset = spawnSync(process.execPath, [
  "--input-type=module",
  "--eval",
  `import { recommended, strict } from "@effect/tsgo/oxlint-presets";
   import recommendedJson from "@effect/tsgo/oxlint-presets/recommended.json" with { type: "json" };
   import strictJson from "@effect/tsgo/oxlint-presets/strict.json" with { type: "json" };
   if (recommended.rules["effecttsgo/floating-effect"] !== "error" || recommendedJson.rules["effecttsgo/floating-effect"] !== "error") process.exit(1);
   if (recommended.rules["effecttsgo/global-date"] !== undefined || recommendedJson.rules["effecttsgo/global-date"] !== undefined) process.exit(1);
   if (strict.rules["effecttsgo/catch-unfailable-effect"] !== "error" || strictJson.rules["effecttsgo/catch-unfailable-effect"] !== "error") process.exit(1);`
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

const strictBoolean = run("--type-aware", "--format", "json", "--config", ".oxlintrc.json", "strict-boolean-expressions-duplicates.ts")
assert.equal(strictBoolean.status, 1, strictBoolean.stderr)
const strictBooleanDiagnostics = JSON.parse(strictBoolean.stdout).diagnostics.filter(
  (item) => item.code === "effecttsgo(strict-boolean-expressions)"
)
assert.equal(strictBooleanDiagnostics.length, 2, "one diagnostic per distinct non-boolean type")
assert.deepEqual(
  strictBooleanDiagnostics.map((item) => item.message).sort(),
  [
    "Unexpected `undefined` type in condition, expected strictly a boolean instead.",
    "Unexpected `{ id: string; }` type in condition, expected strictly a boolean instead."
  ].sort()
)
assert.deepEqual(strictBooleanDiagnostics[0].labels[0].span, strictBooleanDiagnostics[1].labels[0].span)

const disabled = run("--type-aware", "--config", ".oxlintrc.json", "disabled.ts")
assert.equal(disabled.status, 0, disabled.stderr)
assert.doesNotMatch(`${disabled.stdout}\n${disabled.stderr}`, /effecttsgo\(floating-effect\)/)

const recommended = run("--config", ".oxlintrc-recommended.json", "diagnostic.ts")
assert.equal(recommended.status, 1, recommended.stderr)
assert.match(`${recommended.stdout}\n${recommended.stderr}`, /effecttsgo\(floating-effect\)/)

const recommendedOverride = run("--config", ".oxlintrc-recommended-override.json", "diagnostic.ts")
assert.equal(recommendedOverride.status, 0, recommendedOverride.stderr)
assert.doesNotMatch(`${recommendedOverride.stdout}\n${recommendedOverride.stderr}`, /effecttsgo\(floating-effect\)/)

// The effect-tsgo proxies select the patched tsgolint with EFFECT_TSGO_TSGOLINT_PATH, which wins over the
// OXLINT_TSGOLINT_PATH that vite-plus sets.
const proxied = spawnSync(process.execPath, [oxlint, "--type-aware", "--config", ".oxlintrc.json", "diagnostic.ts"], {
  cwd: fixture,
  encoding: "utf8",
  env: {
    ...process.env,
    EFFECT_TSGO_TSGOLINT_PATH: tsgolint,
    ...(mode === "tsrs" ? { EFFECT_TSGO_TSGOLINT_MODE: "tsrs" } : {}),
    OXLINT_TSGOLINT_PATH: join(fixture, "missing-tsgolint")
  }
})
assert.equal(proxied.status, 1, proxied.stderr)
assert.match(`${proxied.stdout}\n${proxied.stderr}`, /effecttsgo\(floating-effect\)/)

if (mode === "tsrs") {
  // tsrs's linter has no Go profiling flags: the binding refuses them instead of passing them on.
  const profiled = spawnSync(process.execPath, [oxlint, "--type-aware", "--config", ".oxlintrc.json", "diagnostic.ts"], {
    cwd: fixture,
    encoding: "utf8",
    env: { ...env, OXLINT_TSGOLINT_TRACE: join(fixture, "trace.out") }
  })
  assert.notEqual(profiled.status, 0, profiled.stderr)
  assert.match(`${profiled.stdout}\n${profiled.stderr}`, /OXLINT_TSGOLINT_TRACE is not supported by the tsrs linter/)
}

console.log(`Oxlint profile smoke test passed (${mode})`)
