import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import { median } from "./perf.ts"
import { type PerfCell, PerfCellFromString, type PerfPair, PerfSide } from "./perfCi.ts"

export const perfReportMarker = "<!-- effect-tsgo-perf -->"

const maxReportBytes = 60000
const significanceLevel = 0.01

export class PerfReportError extends Data.TaggedError("PerfReportError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Performance report failed: ${this.reason}`
  }
}

export const PerfMatrix = Schema.Struct({
  include: Schema.Array(Schema.Struct({
    name: Schema.NonEmptyString,
    side: PerfSide,
    version: Schema.NonEmptyString
  }))
})
export type PerfMatrixEntry = (typeof PerfMatrix.Type)["include"][number]

const PerfMatrixFromString = Schema.fromJsonString(PerfMatrix)

export const mannWhitneyUTwoSided = (a: ReadonlyArray<number>, b: ReadonlyArray<number>): number => {
  if (a.length === 0 || b.length === 0) return 1
  const combined = [
    ...a.map((value) => ({ value, fromA: true })),
    ...b.map((value) => ({ value, fromA: false }))
  ].sort((left, right) => left.value - right.value)
  // Midranks are doubled so that tied ranks stay integral in the rank-sum DP below.
  const doubledRanks: Array<number> = []
  for (let start = 0; start < combined.length;) {
    let end = start
    while (end + 1 < combined.length && combined[end + 1]!.value === combined[start]!.value) end++
    for (let index = start; index <= end; index++) doubledRanks[index] = start + end + 2
    start = end + 1
  }
  const observed = combined.reduce((sum, entry, index) => entry.fromA ? sum + doubledRanks[index]! : sum, 0)
  const maxSum = doubledRanks.reduce((sum, rank) => sum + rank, 0)
  const ways = Array.from({ length: a.length + 1 }, () => new Array<number>(maxSum + 1).fill(0))
  ways[0]![0] = 1
  for (const rank of doubledRanks) {
    for (let size = a.length; size >= 1; size--) {
      const previous = ways[size - 1]!
      const current = ways[size]!
      for (let sum = maxSum; sum >= rank; sum--) current[sum]! += previous[sum - rank]!
    }
  }
  const distribution = ways[a.length]!
  let total = 0
  let lower = 0
  let upper = 0
  distribution.forEach((count, sum) => {
    total += count
    if (sum <= observed) lower += count
    if (sum >= observed) upper += count
  })
  return Math.min(1, 2 * Math.min(lower, upper) / total)
}

type MetricKind = "count" | "counter" | "ratio"

interface MetricDefinition {
  readonly label: string
  readonly kind: MetricKind
  readonly value: (pair: PerfPair) => number
}

const relativePercent = (patched: number, stock: number) => 100 * (patched / stock - 1)

const metrics: ReadonlyArray<MetricDefinition> = [
  { label: "Errors", kind: "count", value: (pair) => pair.patched.errors },
  { label: "Warnings", kind: "count", value: (pair) => pair.patched.warnings },
  { label: "Symbols over stock", kind: "counter", value: (pair) => pair.patched.symbols - pair.stock.symbols },
  { label: "Types over stock", kind: "counter", value: (pair) => pair.patched.types - pair.stock.types },
  {
    label: "Instantiations over stock",
    kind: "counter",
    value: (pair) => pair.patched.instantiations - pair.stock.instantiations
  },
  {
    label: "Memory allocs over stock",
    kind: "ratio",
    value: (pair) => relativePercent(pair.patched.memoryAllocs, pair.stock.memoryAllocs)
  },
  {
    label: "Check time over stock",
    kind: "ratio",
    value: (pair) => relativePercent(pair.patched.checkTimeSeconds, pair.stock.checkTimeSeconds)
  },
  {
    label: "Total time over stock",
    kind: "ratio",
    value: (pair) => relativePercent(pair.patched.totalTimeSeconds, pair.stock.totalTimeSeconds)
  }
]

interface Summary {
  readonly values: ReadonlyArray<number>
  readonly median: number
  readonly min: number
  readonly max: number
}

const summarize = (values: ReadonlyArray<number>): Summary => ({
  values,
  median: median(values)!,
  min: Math.min(...values),
  max: Math.max(...values)
})

const countFormat = new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 })

const formatCount = (value: number) => countFormat.format(value)
const formatSignedCount = (value: number) => `${value > 0 ? "+" : ""}${formatCount(value)}`
const fixed = (value: number, digits: number) => {
  const text = value.toFixed(digits)
  return Number(text) === 0 ? (0).toFixed(digits) : text
}
const formatSignedFixed = (value: number, digits: number) => {
  const text = fixed(value, digits)
  return Number(text) > 0 ? `+${text}` : text
}

const escapeHtml = (text: string) =>
  text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll("\"", "&quot;")

const formatSummary = (kind: MetricKind, summary: Summary) => {
  if (kind === "ratio") {
    return `${formatSignedFixed(summary.median, 1)}% (${fixed(summary.min, 1)} to ${fixed(summary.max, 1)})`
  }
  const spread = Math.ceil((summary.max - summary.min) / 2)
  const value = kind === "counter" ? formatSignedCount(summary.median) : formatCount(summary.median)
  return spread === 0 ? value : `${value} (±${formatCount(spread)})`
}

const formatExtreme = (kind: MetricKind, value: number) =>
  kind === "ratio" ? `${formatSignedFixed(value, 1)}%` : kind === "counter" ? formatSignedCount(value) : formatCount(value)

const direction = (delta: number) => delta < 0 ? "🟩" : "🔻"

interface Delta {
  readonly text: string
  readonly flagged: boolean
  readonly pValue?: number
}

const compareSummaries = (kind: MetricKind, base: Summary, head: Summary): Delta => {
  const delta = head.median - base.median
  if (kind === "ratio") {
    const pValue = mannWhitneyUTwoSided(head.values, base.values)
    return pValue < significanceLevel
      ? { text: `${direction(delta)} ${formatSignedFixed(delta, 1)} pts`, flagged: true, pValue }
      : { text: "~", flagged: false, pValue }
  }
  if (head.min <= base.max && base.min <= head.max) return { text: "~", flagged: false }
  const percent = base.median === 0 ? "" : ` (${formatSignedFixed(delta / Math.abs(base.median) * 100, 2)}%)`
  return kind === "counter"
    ? { text: `${direction(delta)} ${formatSignedCount(delta)}${percent}`, flagged: true }
    : { text: `${formatSignedCount(delta)}${percent}`, flagged: false }
}

const sampleCount = (base: PerfCell | undefined, head: PerfCell | undefined) => {
  const counts = [base, head].flatMap((cell) => cell === undefined ? [] : [cell.pairs.length])
  return counts.length === 2 && counts[0] !== counts[1] ? counts.join("/") : String(counts[0])
}

const assertStableStockCounters = (channel: string, base: PerfCell | undefined, head: PerfCell | undefined) => {
  const groups = base !== undefined && head !== undefined && base.typescriptVersion === head.typescriptVersion
    ? [[base, head]]
    : [base, head].flatMap((cell) => cell === undefined ? [] : [[cell]])
  for (const group of groups) {
    const samples = group.flatMap((cell) => cell.pairs.map((pair) => pair.stock))
    const drifted = (["symbols", "types", "instantiations"] as const).flatMap((counter) => {
      const values = [...new Set(samples.map((sample) => sample[counter]))]
      return values.length > 1 ? [`${counter} ${values.join(" vs ")}`] : []
    })
    if (drifted.length > 0) {
      throw new PerfReportError({
        reason: `stock counters drifted for ${channel} (${group[0]!.typescriptVersion}): ${drifted.join(", ")}; ` +
          "the workload or the stock compiler is not deterministic"
      })
    }
  }
}

const channelOrder = (channel: string) => channel === "latest" ? 0 : channel === "next" ? 1 : 2

const sortChannels = (channels: Iterable<string>) =>
  [...new Set(channels)].sort((left, right) =>
    channelOrder(left) - channelOrder(right) || (left < right ? -1 : left > right ? 1 : 0)
  )

const cell = (content: string, align?: "right") =>
  `<td${align === undefined ? "" : ` align=${align}`}><sub>${content}</sub></td>`

interface FlaggedRow {
  readonly channel: string
  readonly metric: string
  readonly delta: string
}

const renderChannel = (
  channel: string,
  base: PerfCell | undefined,
  head: PerfCell | undefined,
  expected: ReadonlyMap<string, string> | undefined,
  flagged: Array<FlaggedRow>,
  missing: Array<string>
) => {
  const notes: Array<string> = []
  const isExpected = (side: PerfSide) => expected?.has(`${channel}\0${side}`) ?? false
  for (const [side, value] of [["base", base], ["head", head]] as const) {
    if (value === undefined && isExpected(side)) {
      missing.push(`${channel} · ${side}`)
      notes.push(`⚠️ ${side} result missing`)
    }
  }
  if (base === undefined && head !== undefined && !isExpected("base")) notes.push("new in this PR")
  if (head === undefined && base !== undefined && !isExpected("head")) notes.push("removed in this PR")
  const version = base !== undefined && head !== undefined && base.typescriptVersion !== head.typescriptVersion
    ? `${base.typescriptVersion} → ${head.typescriptVersion}`
    : (head ?? base)?.typescriptVersion ?? expected?.get(`${channel}\0head`) ?? expected?.get(`${channel}\0base`) ??
      "unknown version"
  if (base !== undefined && head !== undefined && base.typescriptVersion !== head.typescriptVersion) {
    notes.push("compiler changed")
  }
  const title = [escapeHtml(channel), escapeHtml(version), "effect (all rules)", ...notes].join(" · ")
  const rows = [`<tr><th align=left colspan="7"><sub>${title}</sub></th></tr>`]
  if (base === undefined && head === undefined) return rows

  const count = sampleCount(base, head)
  for (const metric of metrics) {
    const baseSummary = base === undefined ? undefined : summarize(base.pairs.map(metric.value))
    const headSummary = head === undefined ? undefined : summarize(head.pairs.map(metric.value))
    const delta = baseSummary === undefined || headSummary === undefined
      ? undefined
      : compareSummaries(metric.kind, baseSummary, headSummary)
    if (delta?.flagged === true) flagged.push({ channel, metric: metric.label, delta: delta.text })
    const pValue = metric.kind === "ratio"
      ? `${delta?.pValue === undefined ? "" : `p=${delta.pValue.toFixed(3)} `}n=${count}`
      : `counter, n=${count}`
    rows.push([
      "<tr>",
      cell(metric.label),
      cell(baseSummary === undefined ? "—" : formatSummary(metric.kind, baseSummary), "right"),
      cell(headSummary === undefined ? "—" : formatSummary(metric.kind, headSummary), "right"),
      cell(delta?.text ?? "—", "right"),
      cell(headSummary === undefined ? "—" : formatExtreme(metric.kind, headSummary.min), "right"),
      cell(headSummary === undefined ? "—" : formatExtreme(metric.kind, headSummary.max), "right"),
      cell(pValue),
      "</tr>"
    ].join(""))
  }
  return rows
}

const formatStockCounters = (cells: ReadonlyArray<PerfCell>) =>
  [...new Map(cells.map((cell) => [cell.typescriptVersion, cell.pairs[0]!.stock])).entries()]
    .map(([version, stock]) =>
      `\`${version}\` symbols ${formatCount(stock.symbols)} · types ${formatCount(stock.types)} · ` +
      `instantiations ${formatCount(stock.instantiations)}`
    )
    .join("; ")

const distinct = (values: ReadonlyArray<string>) => [...new Set(values)]

export const renderPerfReport = (
  cells: ReadonlyArray<PerfCell>,
  expected?: ReadonlyArray<PerfMatrixEntry>
): string => {
  const bySide = new Map<string, PerfCell>()
  for (const entry of cells) {
    const key = `${entry.channel}\0${entry.side}`
    if (bySide.has(key)) {
      throw new PerfReportError({ reason: `duplicate ${entry.side} result for channel ${entry.channel}` })
    }
    bySide.set(key, entry)
  }
  const expectedKeys = expected === undefined
    ? undefined
    : new Map(expected.map((entry) => [`${entry.name}\0${entry.side}`, entry.version]))
  const channels = sortChannels([...cells.map((entry) => entry.channel), ...(expected ?? []).map((entry) => entry.name)])
  if (channels.length === 0) {
    throw new PerfReportError({ reason: "no perf results to report" })
  }

  const flagged: Array<FlaggedRow> = []
  const missing: Array<string> = []
  const rows: Array<string> = []
  const stockLines: Array<string> = []
  for (const channel of channels) {
    const base = bySide.get(`${channel}\0base`)
    const head = bySide.get(`${channel}\0head`)
    assertStableStockCounters(channel, base, head)
    rows.push(...renderChannel(channel, base, head, expectedKeys, flagged, missing))
    const measured = [base, head].flatMap((entry) => entry === undefined ? [] : [entry])
    if (measured.length > 0) stockLines.push(`- Stock counters, ${channel}: ${formatStockCounters(measured)}`)
  }

  const verdict = flagged.length === 0
    ? "**Effect perf vs stock tsgo: no significant change**"
    : `**Effect perf vs stock tsgo: ${flagged.length} flagged row${flagged.length === 1 ? "" : "s"}**`
  const summary = [
    perfReportMarker,
    verdict,
    ...(flagged.length === 0 ? [] : ["", ...flagged.map((row) => `- ${row.channel} · ${row.metric} · ${row.delta}`)]),
    ...(missing.length === 0 ? [] : ["", `⚠️ Missing benchmark results: ${missing.join(", ")}`]),
    ""
  ]

  const pairCounts = cells.map((entry) => entry.pairs.length)
  const minPairs = Math.min(...pairCounts)
  const maxPairs = Math.max(...pairCounts)
  const commits = (side: PerfSide) =>
    distinct(cells.filter((entry) => entry.side === side).map((entry) => `\`${entry.commit}\``)).join(", ") || "—"
  const workloads = distinct(cells.map((entry) =>
    `\`${entry.workload.repository}@${entry.workload.revision}\`${
      entry.workload.allRules ? " with every Effect rule executing (`skipDisabledOptimization: true`)" : ""
    }`
  ))
  const footer = [
    ...stockLines,
    `- Workload: ${workloads.join(", ")}`,
    `- Pairs per side: ${minPairs === maxPairs ? minPairs : `${minPairs}–${maxPairs}`}, alternating stock-first and patched-first`,
    `- Commits: base ${commits("base")} · head ${commits("head")}`,
    "- Counters are patched − stock per pair (median ±half-range); allocs and times are patched / stock − 1 per pair, " +
    `median (min to max). Time and alloc deltas are flagged only when the exact two-sided Mann-Whitney U p < ${significanceLevel}.`
  ]

  const details = [
    "<details><summary>Details</summary>",
    "",
    "<table>",
    "<thead><tr><th><sub>Metric</sub></th><th><sub>base</sub></th><th><sub>pr</sub></th><th><sub>Delta</sub></th>" +
    "<th><sub>Best</sub></th><th><sub>Worst</sub></th><th><sub>p-value</sub></th></tr></thead>",
    "<tbody>",
    ...rows,
    "</tbody>",
    "</table>",
    "",
    ...footer,
    "",
    "</details>",
    ""
  ]
  const report = [...summary, ...details].join("\n")
  if (Buffer.byteLength(report) <= maxReportBytes) return report
  return [...summary, `Details omitted: the report exceeded ${maxReportBytes} bytes.`, ""].join("\n")
}

export interface PerfReportOptions {
  readonly input: string
  readonly output: string
  readonly matrix?: string
}

const decodeFile = <S extends Schema.Decoder<unknown>>(schema: S, file: string) =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const text = yield* fs.readFileString(file)
    return yield* Schema.decodeUnknownEffect(schema)(text).pipe(
      Effect.mapError((error) => new PerfReportError({ reason: `${file}: ${error.message}` }))
    )
  })

export const writePerfReport = Effect.fnUntraced(function*(options: PerfReportOptions) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const input = path.resolve(options.input)
  const output = path.resolve(options.output)
  const files = (yield* fs.readDirectory(input, { recursive: true }))
    .filter((file) => file.endsWith(".json"))
    .sort()
  const cells: Array<PerfCell> = []
  for (const file of files) {
    cells.push(yield* decodeFile(PerfCellFromString, path.join(input, file)))
  }
  const expected = options.matrix === undefined
    ? undefined
    : (yield* decodeFile(PerfMatrixFromString, path.resolve(options.matrix))).include
  const report = yield* Effect.try({
    try: () => renderPerfReport(cells, expected),
    catch: (error) => error instanceof PerfReportError ? error : new PerfReportError({ reason: String(error) })
  })
  yield* fs.makeDirectory(path.dirname(output), { recursive: true })
  yield* fs.writeFileString(output, report)
  yield* Console.log(report)
})
