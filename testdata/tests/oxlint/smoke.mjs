import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const repositoryRoot = process.argv[2]
assert.ok(repositoryRoot, "repository root argument is required")

const fixture = dirname(fileURLToPath(import.meta.url))
const oxlint = join(repositoryRoot, "oxlint", "apps", "oxlint", "dist", "cli.js")
const env = {
  ...process.env,
  OXLINT_TSGOLINT_PATH: join(repositoryRoot, "_packages", "tsgo-linux-x64", "lib", "tsgolint")
}

const run = (...args) => spawnSync(process.execPath, [oxlint, ...args], {
  cwd: fixture,
  encoding: "utf8",
  env
})

const rules = run("--rules", "--format", "json")
assert.equal(rules.status, 0, rules.stderr)
const registeredRules = JSON.parse(rules.stdout)
assert.ok(registeredRules.some((rule) => rule.scope === "effecttsgo" && rule.value === "floating-effect"))

const diagnostic = run("--type-aware", "--config", ".oxlintrc.json", "diagnostic.ts")
assert.equal(diagnostic.status, 1, diagnostic.stderr)
assert.match(`${diagnostic.stdout}\n${diagnostic.stderr}`, /effecttsgo\(floating-effect\)/)

const disabled = run("--type-aware", "--config", ".oxlintrc.json", "disabled.ts")
assert.equal(disabled.status, 0, disabled.stderr)
assert.doesNotMatch(`${disabled.stdout}\n${disabled.stderr}`, /effecttsgo\(floating-effect\)/)

console.log("Oxlint profile smoke test passed")
