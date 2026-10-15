import assert from "node:assert/strict"
import test from "node:test"
import type { PerfCell, PerfSample } from "../src/perfCi.ts"
import { mannWhitneyUTwoSided, PerfReportError, renderPerfReport } from "../src/perfReport.ts"

const stock: PerfSample = {
  exitCode: 2,
  errors: 4,
  warnings: 0,
  symbols: 12849728,
  types: 4050966,
  instantiations: 14151659,
  memoryAllocs: 1_000_000_000,
  checkTimeSeconds: 20,
  totalTimeSeconds: 10
}

interface PatchedOverhead {
  readonly symbols: number
  readonly types: number
  readonly instantiations: number
  readonly allocsPercent: number
  readonly checkPercent: number
  readonly totalPercent: number
}

const makeCell = (
  channel: string,
  side: "base" | "head",
  typescriptVersion: string,
  overheads: ReadonlyArray<PatchedOverhead>,
  stockSample: PerfSample = stock
): PerfCell => ({
  schemaVersion: 1,
  channel,
  side,
  commit: side === "base" ? "1111111111111111111111111111111111111111" : "2222222222222222222222222222222222222222",
  typescriptVersion,
  workload: {
    repository: "https://github.com/Effect-TS/effect",
    revision: "c3c7647bdf9b5d19618f90b442c865492e10b1d9",
    allRules: true
  },
  stockVersion: `Version ${typescriptVersion}`,
  patchedVersion: "Version 7.1.0-dev+effect-tsgo.0.53.0",
  pairs: overheads.map((overhead, index) => ({
    order: index % 2 === 0 ? "stock-first" : "patched-first",
    stock: stockSample,
    patched: {
      ...stockSample,
      errors: 5,
      warnings: 1,
      symbols: stockSample.symbols + overhead.symbols,
      types: stockSample.types + overhead.types,
      instantiations: stockSample.instantiations + overhead.instantiations,
      memoryAllocs: stockSample.memoryAllocs * (1 + overhead.allocsPercent / 100),
      checkTimeSeconds: stockSample.checkTimeSeconds * (1 + overhead.checkPercent / 100),
      totalTimeSeconds: stockSample.totalTimeSeconds * (1 + overhead.totalPercent / 100)
    }
  }))
})

const overhead = (
  types: number,
  checkPercent: number,
  extra: Partial<PatchedOverhead> = {}
): PatchedOverhead => ({
  symbols: 943091,
  types,
  instantiations: 1544039,
  allocsPercent: 80,
  checkPercent,
  totalPercent: 60,
  ...extra
})

test("computes exact two-sided Mann-Whitney U p-values", () => {
  assert.equal(mannWhitneyUTwoSided([1, 2, 3], [4, 5, 6]), 0.1)
  assert.equal(mannWhitneyUTwoSided([6, 7, 8, 9, 10], [1, 2, 3, 4, 5]), 2 / 252)
  assert.equal(mannWhitneyUTwoSided([85.33, 92.37, 96.4], [89.95, 91.83, 95.72]), 1)
  assert.equal(mannWhitneyUTwoSided([79.55, 79.8, 80.17], [79.15, 79.28, 79.79]), 0.2)
  assert.equal(mannWhitneyUTwoSided([1, 1, 1], [1, 1, 1]), 1)
  assert.equal(mannWhitneyUTwoSided([], [1]), 1)
})

test("renders no significant change when ranges overlap", () => {
  const report = renderPerfReport([
    makeCell("next", "head", "7.1.0-dev.20261014.1", [overhead(4447736, 79.8), overhead(4447734, 80.2)]),
    makeCell("next", "base", "7.1.0-dev.20261014.1", [overhead(4447735, 79.6), overhead(4447737, 80)])
  ])

  assert.ok(report.startsWith("<!-- effect-tsgo-perf -->\n**Effect perf vs stock tsgo: no significant change**\n"))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"7\"><sub>next · 7.1.0-dev.20261014.1 · effect (all rules)</sub></th></tr>"
  ))
  assert.ok(report.includes(
    "<tr><td><sub>Types over stock</sub></td><td align=right><sub>+4,447,736 (±1)</sub></td>" +
      "<td align=right><sub>+4,447,735 (±1)</sub></td><td align=right><sub>~</sub></td>" +
      "<td align=right><sub>+4,447,734</sub></td><td align=right><sub>+4,447,736</sub></td>" +
      "<td><sub>counter, n=2</sub></td></tr>"
  ))
  assert.ok(report.includes(
    "<tr><td><sub>Check time over stock</sub></td><td align=right><sub>+79.8% (79.6 to 80.0)</sub></td>" +
      "<td align=right><sub>+80.0% (79.8 to 80.2)</sub></td><td align=right><sub>~</sub></td>" +
      "<td align=right><sub>+79.8%</sub></td><td align=right><sub>+80.2%</sub></td>" +
      "<td><sub>p=0.667 n=2</sub></td></tr>"
  ))
  assert.ok(report.includes(
    "<tr><td><sub>Errors</sub></td><td align=right><sub>5</sub></td><td align=right><sub>5</sub></td>" +
      "<td align=right><sub>~</sub></td>"
  ))
  assert.ok(report.includes(
    "- Stock counters, next: `7.1.0-dev.20261014.1` symbols 12,849,728 · types 4,050,966 · " +
      "instantiations 14,151,659"
  ))
  assert.ok(report.includes(
    "- Workload: `https://github.com/Effect-TS/effect@c3c7647bdf9b5d19618f90b442c865492e10b1d9` " +
      "with every Effect rule executing"
  ))
  assert.ok(report.includes("- Pairs per side: 2, alternating stock-first and patched-first"))
  assert.ok(report.includes(
    "- Commits: base `1111111111111111111111111111111111111111` · head `2222222222222222222222222222222222222222`"
  ))
  assert.ok(report.endsWith("</details>\n"))
})

test("flags counter and time regressions and orders channels", () => {
  const base = [70, 71, 72, 73, 74].map((check) => overhead(4447736, check))
  const head = [80, 81, 82, 83, 84].map((check) => overhead(4457736, check, { instantiations: 1500000 }))
  const report = renderPerfReport([
    makeCell("7.0.1", "head", "7.0.1", base),
    makeCell("next", "head", "7.1.0-dev.20261014.1", head),
    makeCell("next", "base", "7.1.0-dev.20261014.1", base),
    makeCell("latest", "head", "7.0.2", base)
  ])

  assert.ok(report.startsWith([
    "<!-- effect-tsgo-perf -->",
    "**Effect perf vs stock tsgo: 3 flagged rows**",
    "",
    "- next · Types over stock · 🔻 +10,000 (+0.22%)",
    "- next · Instantiations over stock · 🟩 -44,039 (-2.85%)",
    "- next · Check time over stock · 🔻 +10.0 pts",
    ""
  ].join("\n")))
  assert.ok(report.includes(
    "<tr><td><sub>Check time over stock</sub></td><td align=right><sub>+72.0% (70.0 to 74.0)</sub></td>" +
      "<td align=right><sub>+82.0% (80.0 to 84.0)</sub></td><td align=right><sub>🔻 +10.0 pts</sub></td>" +
      "<td align=right><sub>+80.0%</sub></td><td align=right><sub>+84.0%</sub></td>" +
      "<td><sub>p=0.008 n=5</sub></td></tr>"
  ))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"7\"><sub>latest · 7.0.2 · effect (all rules) · new in this PR</sub></th></tr>"
  ))
  assert.ok(report.includes(
    "<tr><td><sub>Symbols over stock</sub></td><td align=right><sub>—</sub></td>" +
      "<td align=right><sub>+943,091</sub></td><td align=right><sub>—</sub></td>"
  ))
  const latest = report.indexOf("<sub>latest ·")
  const next = report.indexOf("<sub>next ·")
  const pinned = report.indexOf("<sub>7.0.1 ·")
  assert.ok(latest < next && next < pinned)
})

test("marks removed channels, compiler changes, and missing results", () => {
  const samples = [overhead(4447736, 80), overhead(4447736, 80)]
  const report = renderPerfReport([
    makeCell("latest", "base", "7.0.2", samples),
    makeCell("next", "base", "7.1.0-dev.20261007.1", samples, { ...stock, symbols: 12000000 }),
    makeCell("next", "head", "7.1.0-dev.20261014.1", samples),
    makeCell("6.9.0", "head", "6.9.0", samples)
  ], [
    { name: "latest", side: "base", version: "7.0.2" },
    { name: "next", side: "base", version: "7.1.0-dev.20261007.1" },
    { name: "next", side: "head", version: "7.1.0-dev.20261014.1" },
    { name: "6.9.0", side: "base", version: "6.9.0" },
    { name: "6.9.0", side: "head", version: "6.9.0" },
    { name: "6.8.0", side: "head", version: "6.8.0" }
  ])

  assert.ok(report.includes(
    "**Effect perf vs stock tsgo: no significant change**\n\n" +
      "⚠️ Missing benchmark results: 6.8.0 · head, 6.9.0 · base\n"
  ))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"7\"><sub>6.8.0 · 6.8.0 · effect (all rules) · ⚠️ head result missing</sub></th></tr>\n" +
      "<tr><th align=left colspan=\"7\"><sub>6.9.0 ·"
  ))
  assert.ok(report.includes("<sub>latest · 7.0.2 · effect (all rules) · removed in this PR</sub>"))
  assert.ok(report.includes(
    "<sub>next · 7.1.0-dev.20261007.1 → 7.1.0-dev.20261014.1 · effect (all rules) · compiler changed</sub>"
  ))
  assert.ok(report.includes("<sub>6.9.0 · 6.9.0 · effect (all rules) · ⚠️ base result missing</sub>"))
  assert.ok(report.includes(
    "- Stock counters, next: `7.1.0-dev.20261007.1` symbols 12,000,000 · types 4,050,966 · " +
      "instantiations 14,151,659; `7.1.0-dev.20261014.1` symbols 12,849,728"
  ))
})

test("renders stock counter jitter as a half-range", () => {
  const report = renderPerfReport([
    makeCell("next", "base", "7.1.0-dev.20261014.1", [overhead(4447736, 80), overhead(4447736, 80)]),
    makeCell("next", "head", "7.1.0-dev.20261014.1", [overhead(4447736, 80)], { ...stock, types: 4050967 })
  ])
  assert.ok(report.includes(
    "- Stock counters, next: `7.1.0-dev.20261014.1` symbols 12,849,728 · types 4,050,966 (±1) · " +
      "instantiations 14,151,659\n"
  ))
})

test("rejects duplicate results for one channel side", () => {
  const samples = [overhead(4447736, 80)]
  assert.throws(
    () =>
      renderPerfReport([
        makeCell("next", "head", "7.1.0-dev.20261014.1", samples),
        makeCell("next", "head", "7.1.0-dev.20261014.1", samples)
      ]),
    PerfReportError
  )
})
