import * as Schema from "effect/Schema"
import assert from "node:assert/strict"
import test from "node:test"
import { buildPerfMatrix } from "../src/matrix.ts"
import { type PerfCell, type PerfCompilerName, type PerfSample, roundOrder } from "../src/perfCi.ts"
import { PerfMatrix, PerfReportError, renderPerfReport, wilcoxonSignedRankTwoSided } from "../src/perfReport.ts"
import type { Upstream } from "../src/upstream.ts"

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

interface SideSpec {
  readonly version: string
  readonly overheads: ReadonlyArray<PatchedOverhead>
  readonly stock?: PerfSample
}

const commits = {
  base: "1111111111111111111111111111111111111111",
  head: "2222222222222222222222222222222222222222"
} as const

const patchedSample = (stockSample: PerfSample, overhead: PatchedOverhead): PerfSample => ({
  ...stockSample,
  errors: 5,
  warnings: 1,
  symbols: stockSample.symbols + overhead.symbols,
  types: stockSample.types + overhead.types,
  instantiations: stockSample.instantiations + overhead.instantiations,
  memoryAllocs: stockSample.memoryAllocs * (1 + overhead.allocsPercent / 100),
  checkTimeSeconds: stockSample.checkTimeSeconds * (1 + overhead.checkPercent / 100),
  totalTimeSeconds: stockSample.totalTimeSeconds * (1 + overhead.totalPercent / 100)
})

// Builds the cell perf ci writes: stock is the head's TypeScript (the base's for a removed channel), and baseStock
// runs when the base's TypeScript differs.
const makeCell = (channel: string, sides: { readonly base?: SideSpec; readonly head?: SideSpec }): PerfCell => {
  const reference = (sides.head ?? sides.base)!
  const baseStock = sides.base !== undefined && sides.head !== undefined && sides.base.version !== sides.head.version
    ? sides.base.stock ?? stock
    : undefined
  const subject = (side: "base" | "head") => {
    const spec = sides[side]
    return spec === undefined ? {} : {
      [side]: {
        commit: commits[side],
        typescriptVersion: spec.version,
        stockVersion: `Version ${spec.version}`,
        patchedVersion: "Version 7.1.0-dev+effect-tsgo.0.53.0"
      }
    }
  }
  const names: Array<PerfCompilerName> = [
    "stock",
    ...(baseStock === undefined ? [] : ["baseStock" as const]),
    ...(sides.base === undefined ? [] : ["base" as const]),
    ...(sides.head === undefined ? [] : ["head" as const])
  ]
  return {
    schemaVersion: 2,
    channel,
    workload: {
      repository: "https://github.com/Effect-TS/effect",
      revision: "c3c7647bdf9b5d19618f90b442c865492e10b1d9",
      allRules: true
    },
    ...subject("base"),
    ...subject("head"),
    rounds: reference.overheads.map((_, index) => ({
      order: roundOrder(names, index),
      stock: reference.stock ?? stock,
      ...(baseStock === undefined ? {} : { baseStock }),
      ...(sides.base === undefined ? {} : { base: patchedSample(baseStock ?? stock, sides.base.overheads[index]!) }),
      ...(sides.head === undefined
        ? {}
        : { head: patchedSample(reference.stock ?? stock, sides.head.overheads[index]!) })
    }))
  }
}

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

const row = (...cells: ReadonlyArray<string>) =>
  "<tr>" + cells.map((content, index) =>
    index === 0 || index === cells.length - 1
      ? `<td><sub>${content}</sub></td>`
      : `<td align=right><sub>${content}</sub></td>`
  ).join("") + "</tr>"

test("computes exact two-sided Wilcoxon signed-rank p-values", () => {
  assert.equal(wilcoxonSignedRankTwoSided([1, 2, 3, 4, 5, 6, 7, 8]), 2 / 256)
  assert.equal(wilcoxonSignedRankTwoSided([-1, -2, -3, -4, -5, -6]), 2 / 64)
  assert.equal(wilcoxonSignedRankTwoSided([1, -2, 3]), 0.75)
  assert.equal(wilcoxonSignedRankTwoSided([1, 1, 1]), 0.25)
  assert.equal(wilcoxonSignedRankTwoSided([0, 1, 1, 1]), 0.25)
  assert.equal(wilcoxonSignedRankTwoSided([0, 0]), 1)
  assert.equal(wilcoxonSignedRankTwoSided([]), 1)
})

test("renders no significant change when paired differences disagree", () => {
  const report = renderPerfReport([
    makeCell("next", {
      base: { version: "7.1.0-dev.20261014.1", overheads: [overhead(4447735, 79.6), overhead(4447737, 80.2)] },
      head: { version: "7.1.0-dev.20261014.1", overheads: [overhead(4447736, 79.8), overhead(4447734, 80)] }
    })
  ])

  assert.ok(report.startsWith("<!-- effect-tsgo-perf -->\n**Effect perf, PR vs base: no significant change**\n"))
  assert.ok(report.includes(
    "<thead><tr><th><sub>Metric</sub></th><th><sub>base vs stock</sub></th><th><sub>PR vs stock</sub></th>" +
      "<th><sub>PR vs base</sub></th><th><sub>Delta</sub></th><th><sub>p-value</sub></th></tr></thead>"
  ))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"6\"><sub>next · 7.1.0-dev.20261014.1 · effect (all rules)</sub></th></tr>"
  ))
  assert.ok(report.includes(row("Types", "+4,447,736 (±1)", "+4,447,735 (±1)", "-1 (±2)", "~", "p=1.000 n=2")))
  assert.ok(report.includes(
    row("Check time", "+79.9% (79.6 to 80.2)", "+79.9% (79.8 to 80.0)", "0.00% (-0.11 to 0.11)", "~", "p=1.000 n=2")
  ))
  assert.ok(report.includes(row("Errors", "5", "5", "0", "~", "count, n=2")))
  assert.ok(report.includes(
    "- Stock counters, next: `7.1.0-dev.20261014.1` symbols 12,849,728 · types 4,050,966 · " +
      "instantiations 14,151,659"
  ))
  assert.ok(report.includes(
    "- Workload: `https://github.com/Effect-TS/effect@c3c7647bdf9b5d19618f90b442c865492e10b1d9` " +
      "with every Effect rule executing"
  ))
  assert.ok(report.includes(
    "- Rounds per channel: 2, each compiling stock, base, and PR once on the same runner, rotating the order"
  ))
  assert.ok(report.includes(
    "- Commits: base `1111111111111111111111111111111111111111` · head `2222222222222222222222222222222222222222`"
  ))
  assert.ok(report.includes("Wilcoxon signed-rank p < 0.01, which needs at least 8 rounds"))
  assert.ok(report.endsWith("</details>\n"))
})

test("flags paired counter and time regressions and orders channels", () => {
  const checks = [70, 71, 72, 73, 74, 70, 71, 72, 73]
  const base = checks.map((check) => overhead(4447736, check))
  const head = checks.map((check) => overhead(4457736, check + 10, { instantiations: 1500000 }))
  const noisy = checks.map((check, index) => overhead(4447736, check + (index % 2 === 0 ? 1 : -1)))
  const report = renderPerfReport([
    makeCell("7.0.1", { base: { version: "7.0.1", overheads: base }, head: { version: "7.0.1", overheads: noisy } }),
    makeCell("next", {
      base: { version: "7.1.0-dev.20261014.1", overheads: base },
      head: { version: "7.1.0-dev.20261014.1", overheads: head }
    }),
    makeCell("latest", { head: { version: "7.0.2", overheads: base } })
  ])

  assert.ok(report.startsWith([
    "<!-- effect-tsgo-perf -->",
    "**Effect perf, PR vs base: 3 flagged rows**",
    "",
    "- next · Types · 🔻 +10,000 (+0.22%)",
    "- next · Instantiations · 🟩 -44,039 (-2.85%)",
    "- next · Check time · 🔻 +5.81%",
    ""
  ].join("\n")))
  assert.ok(report.includes(
    row(
      "Check time",
      "+72.0% (70.0 to 74.0)",
      "+82.0% (80.0 to 84.0)",
      "+5.81% (5.75 to 5.88)",
      "🔻 +5.81%",
      "p=0.004 n=9"
    )
  ))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"6\"><sub>latest · 7.0.2 · effect (all rules) · new in this PR</sub></th></tr>"
  ))
  assert.ok(report.includes(row("Symbols", "—", "+943,091", "—", "—", "n=9")))
  assert.ok(report.includes(row("Total time", "—", "+60.0% (60.0 to 60.0)", "—", "—", "n=9")))
  assert.ok(report.includes("- Commits: base `1111111111111111111111111111111111111111` · head"))
  const latest = report.indexOf("<sub>latest ·")
  const next = report.indexOf("<sub>next ·")
  const pinned = report.indexOf("<sub>7.0.1 ·")
  assert.ok(latest < next && next < pinned)
})

test("marks removed channels, compiler changes, and missing results", () => {
  const samples = [overhead(4447736, 80), overhead(4447736, 80)]
  const report = renderPerfReport([
    makeCell("latest", { base: { version: "7.0.2", overheads: samples } }),
    makeCell("next", {
      base: { version: "7.1.0-dev.20261007.1", overheads: samples, stock: { ...stock, symbols: 12000000 } },
      head: { version: "7.1.0-dev.20261014.1", overheads: samples }
    }),
    makeCell("6.9.0", { head: { version: "6.9.0", overheads: samples } })
  ], [
    { name: "latest", base: { version: "7.0.2" } },
    { name: "next", base: { version: "7.1.0-dev.20261007.1" }, head: { version: "7.1.0-dev.20261014.1" } },
    { name: "6.9.0", base: { version: "6.9.0" }, head: { version: "6.9.0" } },
    { name: "6.8.0", head: { version: "6.8.0" } }
  ])

  assert.ok(report.includes(
    "**Effect perf, PR vs base: no significant change**\n\n" +
      "⚠️ Missing benchmark results: 6.8.0, 6.9.0 · base\n"
  ))
  assert.ok(report.includes(
    "<tr><th align=left colspan=\"6\"><sub>6.8.0 · 6.8.0 · effect (all rules) · ⚠️ result missing</sub></th></tr>\n" +
      "<tr><th align=left colspan=\"6\"><sub>6.9.0 ·"
  ))
  assert.ok(report.includes("<sub>latest · 7.0.2 · effect (all rules) · removed in this PR</sub>"))
  assert.ok(report.includes(
    "<sub>next · 7.1.0-dev.20261007.1 → 7.1.0-dev.20261014.1 · effect (all rules) · compiler changed</sub>"
  ))
  assert.ok(report.includes("<sub>6.9.0 · 6.9.0 · effect (all rules) · ⚠️ base result missing</sub>"))
  // The base is measured against its own stock compiler; PR vs base compares the patched compilers directly, and two
  // rounds are too few to flag.
  assert.ok(report.includes(row("Symbols", "+943,091", "+943,091", "+849,728", "~", "p=0.500 n=2")))
  assert.ok(report.includes(
    "- Stock counters, next: `7.1.0-dev.20261014.1` symbols 12,849,728 · types 4,050,966 · " +
      "instantiations 14,151,659; `7.1.0-dev.20261007.1` symbols 12,000,000"
  ))
})

test("renders stock counter jitter as a half-range", () => {
  const report = renderPerfReport([
    makeCell("next", {
      base: { version: "7.1.0-dev.20261014.1", overheads: [overhead(4447736, 80), overhead(4447736, 80)] }
    }),
    makeCell("beta", {
      head: { version: "7.1.0-dev.20261014.1", overheads: [overhead(4447736, 80)], stock: { ...stock, types: 4050967 } }
    })
  ])
  assert.ok(report.includes(
    "- Stock counters, beta: `7.1.0-dev.20261014.1` symbols 12,849,728 · types 4,050,967 · " +
      "instantiations 14,151,659\n"
  ))
  assert.ok(report.includes("- Rounds per channel: 1–2, each compiling"))
})

test("rejects duplicate channels and rounds missing a side", () => {
  const cell = makeCell("next", { head: { version: "7.1.0-dev.20261014.1", overheads: [overhead(4447736, 80)] } })
  assert.throws(() => renderPerfReport([cell, cell]), PerfReportError)
  const { head: _, ...round } = cell.rounds[0]!
  assert.throws(() => renderPerfReport([{ ...cell, rounds: [round] }]), PerfReportError)
})

test("decodes the measure half of the perf matrix", () => {
  const upstream = (next: string): typeof Upstream.Type => ({
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next },
      "oxlint-tsgolint": { latest: "7.0.2005" },
      oxlint: { latest: "unused" }
    },
    components: {
      typescript: {
        "7.0.2": { gitHead: commits.base, provider: "typescript-go" },
        [next]: { gitHead: commits.base, provider: "typescript" }
      },
      "oxlint-tsgolint": {},
      oxlint: {}
    },
    profiles: []
  })
  const matrix = buildPerfMatrix(upstream("7.1.0-dev.20261007.1"), upstream("7.1.0-dev.20261014.1"), "a", "b")
  assert.deepEqual(Schema.decodeUnknownSync(PerfMatrix)(JSON.parse(JSON.stringify(matrix))).measure.include, [
    { name: "latest", base: { version: "7.0.2" }, head: { version: "7.0.2" } },
    { name: "next", base: { version: "7.1.0-dev.20261007.1" }, head: { version: "7.1.0-dev.20261014.1" } }
  ])
})
