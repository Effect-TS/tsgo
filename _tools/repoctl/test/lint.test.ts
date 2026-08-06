import assert from "node:assert/strict"
import test from "node:test"
import { filterDeadCodeFindings, parseDeadCodeAllowlist } from "../src/lint.ts"

test("parses documented dead-code exceptions", () => {
  assert.deepEqual(
    [...parseDeadCodeAllowlist("# reason\netsgoapi/\n\nOther\n")],
    ["etsgoapi/", "Other"]
  )
})

test("filters external APIs, tests, and allowed functions", () => {
  const findings = filterDeadCodeFindings([
    "etsgoapi/type_parser.go:1:1: unreachable func: PublicAPI",
    "etsgoapi\\type_parser.go:2:1: unreachable func: WindowsPublicAPI",
    "etsoxlintrunner/runner.go:1:1: unreachable func: RunRule",
    "internal/example/example_test.go:1:1: unreachable func: testHelper",
    "internal/example/example.go:1:1: unreachable func: Allowed",
    "internal/example/example.go:2:1: unreachable func: RemoveMe",
    ""
  ].join("\n"), new Set(["etsgoapi/", "etsoxlintrunner/", "Allowed"]))

  assert.deepEqual(findings, ["internal/example/example.go:2:1: unreachable func: RemoveMe"])
})
