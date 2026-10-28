import * as Effect from "effect/Effect"
import * as Exit from "effect/Exit"
import assert from "node:assert/strict"
import test from "node:test"
import { countDiagnostics, enableAllRules, parsePerfSample, perfCiSubject, roundOrder } from "../src/perfCi.ts"

test("parses a perf sample from extended diagnostics", () => {
  const output = [
    "packages/a.ts(372,5): error TS2322: Type 'A' is not assignable to type 'B'.",
    "packages/b.ts(56,11): warning TS377108: The Effect success channel contains a Promise.",
    "error TS5083: Cannot read file 'tsconfig.json'.",
    "  Types of property 'error TS1' are incompatible.",
    "Aggregate Symbols:   12849728",
    "Aggregate Types:   4050966",
    "Aggregate Instantiations:   14151659",
    "Aggregate Memory allocs:   1259683982",
    "Aggregate Check time:   22.368s",
    "Aggregate Total time:   18.519s"
  ].join("\n")

  assert.equal(countDiagnostics(output, "error"), 2)
  assert.equal(countDiagnostics(output, "warning"), 1)
  assert.deepEqual(parsePerfSample(2, output), {
    exitCode: 2,
    errors: 2,
    warnings: 1,
    symbols: 12849728,
    types: 4050966,
    instantiations: 14151659,
    memoryAllocs: 1259683982,
    checkTimeSeconds: 22.368,
    totalTimeSeconds: 18.519
  })
  assert.equal(parsePerfSample(1, "panic: runtime error"), undefined)
})

test("enables every Effect rule with an idempotent textual edit", () => {
  const withComma = [
    "{",
    "  // comment",
    "  \"plugins\": [{",
    "    \"name\": \"@effect/language-service\",",
    "    \"namespaceImportPackages\": []",
    "  }]",
    "}"
  ].join("\n")
  const enabled = enableAllRules(withComma)
  assert.equal(
    enabled,
    withComma.replace(
      "\"name\": \"@effect/language-service\",",
      "\"name\": \"@effect/language-service\", \"skipDisabledOptimization\": true,"
    )
  )
  assert.equal(enableAllRules(enabled!), enabled)
  assert.equal(
    enableAllRules("{ \"plugins\": [{ \"name\": \"@effect/language-service\" }] }"),
    "{ \"plugins\": [{ \"name\": \"@effect/language-service\", \"skipDisabledOptimization\": true }] }"
  )
  assert.equal(enableAllRules("{ \"compilerOptions\": {} }"), undefined)
})

test("rotates the compile order so each compiler takes each position equally often", () => {
  const compilers = ["stock", "base", "head"] as const
  const orders = Array.from({ length: 6 }, (_, round) => roundOrder(compilers, round).join(" "))
  assert.deepEqual(orders, [
    "stock base head",
    "base head stock",
    "head stock base",
    "head base stock",
    "base stock head",
    "stock head base"
  ])
  for (const window of [orders.slice(0, 3), orders.slice(3)]) {
    for (let position = 0; position < compilers.length; position++) {
      assert.deepEqual(new Set(window.map((order) => order.split(" ")[position])), new Set(compilers))
    }
  }
  assert.deepEqual(roundOrder(["head"], 3), ["head"])
})

test("requires every flag of a measured side", () => {
  const none = { bin: undefined, commit: undefined, version: undefined }
  assert.equal(Effect.runSync(perfCiSubject("base", none)), undefined)
  assert.deepEqual(
    Effect.runSync(perfCiSubject("head", { bin: "tsgo", commit: "abc", version: "7.0.2" })),
    { bin: "tsgo", commit: "abc", version: "7.0.2" }
  )
  const exit = Effect.runSyncExit(perfCiSubject("base", { ...none, bin: "tsgo" }))
  assert.ok(Exit.isFailure(exit))
})
