import assert from "node:assert/strict"
import test from "node:test"
import {
  buildOxlintVerifyCases,
  formatOxlintVerifySummary,
  oxlintChecks,
  uncoveredOxlintComponents
} from "../src/oxlintVerify.ts"
import type { Upstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

const upstream: typeof Upstream.Type = {
  schemaVersion: 5,
  tags: {
    typescript: { latest: "7.0.2", next: "7.1.0-beta" },
    oxlint: { latest: "1.94.0" },
    "oxlint-tsgolint": { latest: "7.0.2007" },
    tsrs: { latest: "0.25.0-ts7.1.0-beta" }
  },
  components: {
    typescript: {
      "7.0.2": { gitHead: revision, provider: "typescript-go" },
      "7.1.0-beta": { gitHead: revision, provider: "typescript" }
    },
    "oxlint-tsgolint": {
      "7.0.2006": { gitHead: revision, dependencies: { typescript: "7.0.2" } },
      "7.0.2007": { gitHead: revision, dependencies: { typescript: "7.0.2" } }
    },
    oxlint: { "1.93.0": { gitHead: revision }, "1.94.0": { gitHead: revision } },
    tsrs: {
      "0.25.0-ts7.1.0-beta": { gitHead: revision, typescript: { version: "7.1.0-beta", gitHead: revision } },
      "7.1.0-beta.tsrs2": { gitHead: revision, typescript: { version: "7.1.0-beta", gitHead: revision } }
    }
  },
  profiles: [
    { name: "vite-plus", description: "Vite+ 1.7.0", dependencies: { oxlint: "1.94.0", tsrs: "7.1.0-beta.tsrs2" } },
    { name: "oxlint@1.94.0", description: "", dependencies: { oxlint: "1.94.0", "oxlint-tsgolint": "7.0.2007" } },
    { name: "oxlint@1.93.0", description: "", dependencies: { oxlint: "1.93.0", "oxlint-tsgolint": "7.0.2007" } },
    { name: "vite-plus@1.7.0", description: "", dependencies: { oxlint: "1.94.0", tsrs: "7.1.0-beta.tsrs2" } },
    { name: "vite-plus@1.6.0", description: "", dependencies: { oxlint: "1.93.0", "oxlint-tsgolint": "7.0.2006" } }
  ]
}

test("verifies each runtime profile and each tsrs version once", () => {
  const cases = buildOxlintVerifyCases(upstream)
  assert.deepEqual(cases.map(({ name }) => name), [
    "oxlint@1.93.0 + oxlint-tsgolint@7.0.2007",
    "oxlint@1.94.0 + oxlint-tsgolint@7.0.2007",
    "oxlint@1.94.0 + tsrs@0.25.0-ts7.1.0-beta",
    "oxlint@1.94.0 + tsrs@7.1.0-beta.tsrs2",
    "vite-plus@1.6.0 (oxlint 1.93.0, tsgolint 7.0.2006)",
    "vite-plus@1.7.0 (oxlint 1.94.0, tsrs 7.1.0-beta.tsrs2)"
  ])
  // A vite-plus that pins tsrs runs plain `effect-tsgo vp`, which lints with the packaged build of the tsrs it installs.
  assert.deepEqual(cases.at(-1), {
    name: "vite-plus@1.7.0 (oxlint 1.94.0, tsrs 7.1.0-beta.tsrs2)",
    dependencies: { "vite-plus": "1.7.0" },
    runner: "vp",
    oxlint: "1.94.0",
    linter: { kind: "tsrs", version: "7.1.0-beta.tsrs2" }
  })
  // Each tsrs version runs on the latest oxlint, without oxlint-tsgolint.
  assert.deepEqual(cases[3], {
    name: "oxlint@1.94.0 + tsrs@7.1.0-beta.tsrs2",
    dependencies: { oxlint: "1.94.0" },
    runner: "oxlint",
    oxlint: "1.94.0",
    linter: { kind: "tsrs", version: "7.1.0-beta.tsrs2", flag: "--tsrs-lint=7.1.0-beta.tsrs2" }
  })
  assert.deepEqual(uncoveredOxlintComponents(upstream, cases), [])
})

test("reports the built artifacts that no case runs", () => {
  const cases = buildOxlintVerifyCases(upstream).filter(({ name }) => !name.startsWith("vite-plus@1.6.0"))
  assert.deepEqual(uncoveredOxlintComponents(upstream, cases), ["oxlint-tsgolint@7.0.2006"])
})

const result = (exitCode: number, stdout: string, stderr = "") => ({ exitCode, stdout, output: `${stdout}\n${stderr}` })

test("checks the proxy's output for the Effect diagnostics", () => {
  const floating = "diagnostic.ts:3:1: error effecttsgo(floating-effect): This Effect value is neither yielded"
  assert.equal(oxlintChecks.reports(result(1, floating)), undefined)
  assert.match(oxlintChecks.reports(result(0, floating)) ?? "", /exit code 0, expected 1/)
  assert.match(oxlintChecks.reports(result(1, "", "unknown rule")) ?? "", /no effecttsgo\(floating-effect\)/)
  assert.equal(oxlintChecks.silent(result(0, "Found 0 warnings")), undefined)
  assert.match(oxlintChecks.silent(result(0, floating)) ?? "", /unexpected/)

  assert.equal(oxlintChecks.rules(result(0, JSON.stringify([{ scope: "effecttsgo", value: "floating-effect" }]))), undefined)
  assert.match(oxlintChecks.rules(result(0, "[]")) ?? "", /does not list/)

  const label = { label: "Inside this Effect generator.", span: { offset: 67, length: 8, line: 3, column: 35 } }
  const related = (labels: ReadonlyArray<unknown>) =>
    result(1, JSON.stringify({ diagnostics: [{ code: "effecttsgo(missing-star-in-yield-effect-gen)", labels }] }))
  assert.equal(oxlintChecks.relatedLabel(related([label])), undefined)
  assert.match(oxlintChecks.relatedLabel(related([])) ?? "", /labeled range/)
  assert.match(oxlintChecks.relatedLabel(related([{ ...label, span: { offset: 0 } }])) ?? "", /label span/)

  const strict = (message: string, span: unknown) => ({
    code: "effecttsgo(strict-boolean-expressions)",
    message: `Unexpected \`${message}\` type in condition, expected strictly a boolean instead.`,
    labels: [{ span }]
  })
  const span = { offset: 1, length: 2 }
  assert.equal(
    oxlintChecks.strictBoolean(result(1, JSON.stringify({ diagnostics: [strict("undefined", span), strict("{ id: string; }", span)] }))),
    undefined
  )
  assert.match(
    oxlintChecks.strictBoolean(result(1, JSON.stringify({ diagnostics: [strict("undefined", span)] }))) ?? "",
    /1 strict-boolean-expressions diagnostics, expected 2/
  )

  assert.equal(
    oxlintChecks.traceRefused(result(1, "", "OXLINT_TSGOLINT_TRACE is not supported by the tsrs linter")),
    undefined
  )
  assert.match(oxlintChecks.traceRefused(result(0, "")) ?? "", /was accepted/)

  assert.equal(oxlintChecks.patchedTsrs(result(0, "Version 7.1.0-beta+effect-tsgo.0.71.0 (tsrs 7.1.0-beta.tsrs2)")), undefined)
  assert.match(oxlintChecks.patchedTsrs(result(0, "Version 7.1.0-beta (tsrs 7.1.0-beta.tsrs2)")) ?? "", /not the Effect-patched/)
})

test("summarizes the runtimes and details the failures", () => {
  const summary = formatOxlintVerifySummary([
    { name: "oxlint@1.94.0 + oxlint-tsgolint@7.0.2007", checks: [{ name: "reports floating-effect", failure: undefined }] },
    { name: "oxlint@1.94.0 + tsrs@7.1.0-beta.tsrs2", checks: [{ name: "reports floating-effect", failure: "exit code 0" }] }
  ])
  assert.match(summary, /\| oxlint@1\.94\.0 \+ oxlint-tsgolint@7\.0\.2007 \| 1 \| passed \|/)
  assert.match(summary, /\| oxlint@1\.94\.0 \+ tsrs@7\.1\.0-beta\.tsrs2 \| 1 \| 1 failed \|/)
  assert.match(summary, /\*\*oxlint@1\.94\.0 \+ tsrs@7\.1\.0-beta\.tsrs2: reports floating-effect\*\*\n\n```\nexit code 0\n```/)
})
