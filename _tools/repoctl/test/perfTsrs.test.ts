import assert from "node:assert/strict"
import test from "node:test"
import { overheadLines, parsePerfMetrics } from "../src/perf.ts"
import { parsePeakRss, releaseProfileOverrides } from "../src/perfTsrs.ts"

test("parses tsrs's aggregate --extendedDiagnostics table", () => {
  // tsrs_execute statistics.rs: "Memory used" is the resident set size at the end, allocations are not tracked.
  const metrics = parsePerfMetrics([
    "Projects in scope:                     25",
    "Projects built:                        25",
    "Timestamps only updates:                0",
    "Aggregate Files:                    12345",
    "Aggregate Lines:                  1234567",
    "Aggregate Identifiers:            2345678",
    "Aggregate Symbols:                3456789",
    "Aggregate Types:                   456789",
    "Aggregate Instantiations:         5678901",
    "Aggregate Memory used:           2345678K",
    "Aggregate Memory allocs:                0",
    "Aggregate Config time:             0.012s",
    "Aggregate Parse time:              1.234s",
    "Aggregate Bind time:               0.345s",
    "Aggregate Check time:              6.789s",
    "Aggregate Emit time:               0.456s",
    "Aggregate Total time:              9.012s"
  ].join("\n"))

  assert.deepEqual(metrics, {
    projectsBuilt: "25",
    totalTime: "9.012s",
    checkTime: "6.789s",
    parseTime: "1.234s",
    bindTime: "0.345s",
    emitTime: "0.456s",
    changesTime: undefined,
    memoryUsed: "2345678K",
    memoryAllocs: "0",
    symbols: "3456789",
    types: "456789",
    instantiations: "5678901"
  })
})

test("reads the release profile as cargo --config overrides", () => {
  const overrides = releaseProfileOverrides([
    "[profile.dev]",
    "opt-level = 3",
    "",
    "[profile.release]",
    "# Same as tsrs's release profile",
    "opt-level = 3",
    "lto = \"fat\"",
    "codegen-units = 1",
    "strip = \"symbols\" # set to `false` for debug information",
    "debug = false",
    "panic = \"unwind\"",
    "",
    "[workspace.lints.rust]",
    "non_upper_case_globals = \"allow\""
  ].join("\n"))

  assert.deepEqual(overrides, [
    "profile.release.opt-level=3",
    "profile.release.lto=\"fat\"",
    "profile.release.codegen-units=1",
    "profile.release.strip=\"symbols\"",
    "profile.release.debug=false",
    "profile.release.panic=\"unwind\""
  ])
  assert.deepEqual(releaseProfileOverrides("[profile.dev]\nopt-level = 3\n"), [])
})

test("parses the peak resident set size GNU time reports", () => {
  assert.equal(parsePeakRss("perf-resources maxrss=2345678\n"), 2345678)
  assert.equal(parsePeakRss("Command exited with non-zero status 2\nperf-resources maxrss=1024\n"), 1024)
  assert.equal(parsePeakRss(""), undefined)
})

test("reports the patched overhead over stock medians", () => {
  assert.deepEqual(
    overheadLines([
      { name: "stock", wallTimeSeconds: 10, checkTimeSeconds: 8, peakRssKilobytes: 1000, totalTimeSeconds: 0 },
      { name: "patched", wallTimeSeconds: 12.5, checkTimeSeconds: 7.6, peakRssKilobytes: 1100, totalTimeSeconds: 1 }
    ]),
    ["overhead (patched vs stock medians): wall=+25.0% check=-5.0% peakRss=+10.0%"]
  )
  assert.deepEqual(overheadLines([{ name: "stock", wallTimeSeconds: 1 }]), [])
})
