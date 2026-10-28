import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import { median } from "./perf.ts"
import { type PerfCell, PerfCellFromString, type PerfSample, type PerfSide } from "./perfCi.ts"

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

const PerfMatrixSubject = Schema.Struct({ version: Schema.NonEmptyString })

// The measure half of `repoctl matrix perf`: one entry per channel, with the sides it builds.
export const PerfMatrix = Schema.Struct({
  measure: Schema.Struct({
    include: Schema.Array(Schema.Struct({
      name: Schema.NonEmptyString,
      base: Schema.optionalKey(PerfMatrixSubject),
      head: Schema.optionalKey(PerfMatrixSubject)
    }))
  })
})
export type PerfMatrixEntry = (typeof PerfMatrix.Type)["measure"]["include"][number]

const PerfMatrixFromString = Schema.fromJsonString(PerfMatrix)

// Exact two-sided Wilcoxon signed-rank test of paired differences against zero; zero differences are dropped and
// tied magnitudes get midranks.
export const wilcoxonSignedRankTwoSided = (differences: ReadonlyArray<number>): number => {
  const nonZero = differences
    .filter((difference) => difference !== 0)
    .map((difference) => ({ magnitude: Math.abs(difference), positive: difference > 0 }))
    .sort((left, right) => left.magnitude - right.magnitude)
  if (nonZero.length === 0) return 1
  // Midranks are doubled so that tied ranks stay integral in the rank-sum DP below.
  const doubledRanks: Array<number> = []
  for (let start = 0; start < nonZero.length;) {
    let end = start
    while (end + 1 < nonZero.length && nonZero[end + 1]!.magnitude === nonZero[start]!.magnitude) end++
    for (let index = start; index <= end; index++) doubledRanks[index] = start + end + 2
    start = end + 1
  }
  const observed = nonZero.reduce((sum, entry, index) => entry.positive ? sum + doubledRanks[index]! : sum, 0)
  const maxSum = doubledRanks.reduce((sum, rank) => sum + rank, 0)
  // Under the null hypothesis each rank is positive with probability 1/2: count the sign assignments per sum.
  const ways = new Array<number>(maxSum + 1).fill(0)
  ways[0] = 1
  for (const rank of doubledRanks) {
    for (let sum = maxSum; sum >= rank; sum--) ways[sum]! += ways[sum - rank]!
  }
  let total = 0
  let lower = 0
  let upper = 0
  ways.forEach((count, sum) => {
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
  readonly value: (sample: PerfSample) => number
}

const metrics: ReadonlyArray<MetricDefinition> = [
  { label: "Errors", kind: "count", value: (sample) => sample.errors },
  { label: "Warnings", kind: "count", value: (sample) => sample.warnings },
  { label: "Symbols", kind: "counter", value: (sample) => sample.symbols },
  { label: "Types", kind: "counter", value: (sample) => sample.types },
  { label: "Instantiations", kind: "counter", value: (sample) => sample.instantiations },
  { label: "Memory allocs", kind: "ratio", value: (sample) => sample.memoryAllocs },
  { label: "Check time", kind: "ratio", value: (sample) => sample.checkTimeSeconds },
  { label: "Total time", kind: "ratio", value: (sample) => sample.totalTimeSeconds }
]

// Counts are the subject's own; counters subject − reference; allocs and times subject / reference − 1 in percent.
const compare = (metric: MetricDefinition, subject: PerfSample, reference: PerfSample) => {
  const value = metric.value(subject)
  if (metric.kind === "count") return value
  const referenceValue = metric.value(reference)
  return metric.kind === "counter" ? value - referenceValue : 100 * (value / referenceValue - 1)
}

const difference = (metric: MetricDefinition, head: PerfSample, base: PerfSample) =>
  metric.kind === "ratio" ? compare(metric, head, base) : metric.value(head) - metric.value(base)

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

// Counts are absolute and counters relative, both median ±half-range; ratios are percentages, median (min to max).
const formatSummary = (kind: MetricKind, summary: Summary, digits: number) => {
  if (kind === "ratio") {
    return `${formatSignedFixed(summary.median, digits)}% (${fixed(summary.min, digits)} to ${
      fixed(summary.max, digits)
    })`
  }
  const spread = Math.ceil((summary.max - summary.min) / 2)
  const value = kind === "counter" ? formatSignedCount(summary.median) : formatCount(summary.median)
  return spread === 0 ? value : `${value} (±${formatCount(spread)})`
}

const direction = (delta: number) => delta < 0 ? "🟩" : "🔻"

interface Delta {
  readonly text: string
  readonly flagged: boolean
  readonly pValue?: number
}

// Counters, allocs, and times are flagged on the paired test (counters jitter by a few symbols between runs of one
// binary); counts are shown when every round moves the same way, never flagged.
const judge = (kind: MetricKind, differences: Summary, baseOverhead: Summary | undefined): Delta => {
  const delta = differences.median
  if (kind === "count") {
    return differences.min <= 0 && differences.max >= 0
      ? { text: "~", flagged: false }
      : { text: formatSignedCount(delta), flagged: false }
  }
  const pValue = wilcoxonSignedRankTwoSided(differences.values)
  if (pValue >= significanceLevel) return { text: "~", flagged: false, pValue }
  if (kind === "ratio") return { text: `${direction(delta)} ${formatSignedFixed(delta, 2)}%`, flagged: true, pValue }
  const percent = baseOverhead === undefined || baseOverhead.median === 0
    ? ""
    : ` (${formatSignedFixed(delta / Math.abs(baseOverhead.median) * 100, 2)}%)`
  return { text: `${direction(delta)} ${formatSignedCount(delta)}${percent}`, flagged: true, pValue }
}

const channelOrder = (channel: string) => channel === "latest" ? 0 : channel === "beta" ? 1 : channel === "next" ? 2 : 3

const sortChannels = (channels: Iterable<string>) =>
  [...new Set(channels)].sort((left, right) =>
    channelOrder(left) - channelOrder(right) || (left < right ? -1 : left > right ? 1 : 0)
  )

const cell = (content: string, align?: "right") =>
  `<td${align === undefined ? "" : ` align=${align}`}><sub>${content}</sub></td>`

const columns = 6

interface FlaggedRow {
  readonly channel: string
  readonly metric: string
  readonly delta: string
}

// The patched sample of a side and the stock sample of its own TypeScript version, per round.
const sideSamples = (entry: PerfCell, side: PerfSide) => {
  if (entry[side] === undefined) return undefined
  return entry.rounds.map((round, index) => {
    const patched = round[side]
    if (patched === undefined) {
      throw new PerfReportError({ reason: `channel ${entry.channel} round ${index + 1} has no ${side} sample` })
    }
    return { patched, stock: side === "base" ? round.baseStock ?? round.stock : round.stock }
  })
}

const renderChannel = (
  channel: string,
  entry: PerfCell | undefined,
  expected: PerfMatrixEntry | undefined,
  flagged: Array<FlaggedRow>,
  missing: Array<string>
) => {
  const notes: Array<string> = []
  if (entry === undefined && expected !== undefined) {
    missing.push(channel)
    notes.push("⚠️ result missing")
  }
  if (entry !== undefined) {
    for (const side of ["base", "head"] as const) {
      if (entry[side] === undefined && expected?.[side] !== undefined) {
        missing.push(`${channel} · ${side}`)
        notes.push(`⚠️ ${side} result missing`)
      }
    }
    if (entry.base === undefined && expected?.base === undefined) notes.push("new in this PR")
    if (entry.head === undefined && expected?.head === undefined) notes.push("removed in this PR")
  }
  const changed = entry?.base !== undefined && entry.head !== undefined &&
    entry.base.typescriptVersion !== entry.head.typescriptVersion
  const version = changed
    ? `${entry.base!.typescriptVersion} → ${entry.head!.typescriptVersion}`
    : (entry?.head ?? entry?.base)?.typescriptVersion ?? expected?.head?.version ?? expected?.base?.version ??
      "unknown version"
  if (changed) notes.push("compiler changed")
  const title = [escapeHtml(channel), escapeHtml(version), "effect (all rules)", ...notes].join(" · ")
  const rows = [`<tr><th align=left colspan="${columns}"><sub>${title}</sub></th></tr>`]
  if (entry === undefined) return rows

  const base = sideSamples(entry, "base")
  const head = sideSamples(entry, "head")
  const count = entry.rounds.length
  for (const metric of metrics) {
    const overhead = (samples: typeof base) =>
      samples === undefined
        ? undefined
        : summarize(samples.map(({ patched, stock }) => compare(metric, patched, stock)))
    const baseOverhead = overhead(base)
    const headOverhead = overhead(head)
    const differences = base === undefined || head === undefined
      ? undefined
      : summarize(head.map((sample, index) => difference(metric, sample.patched, base[index]!.patched)))
    const delta = differences === undefined ? undefined : judge(metric.kind, differences, baseOverhead)
    if (delta?.flagged === true) flagged.push({ channel, metric: metric.label, delta: delta.text })
    const pValue = metric.kind === "count"
      ? `count, n=${count}`
      : `${delta?.pValue === undefined ? "" : `p=${delta.pValue.toFixed(3)} `}n=${count}`
    rows.push([
      "<tr>",
      cell(metric.label),
      cell(baseOverhead === undefined ? "—" : formatSummary(metric.kind, baseOverhead, 1), "right"),
      cell(headOverhead === undefined ? "—" : formatSummary(metric.kind, headOverhead, 1), "right"),
      cell(
        differences === undefined
          ? "—"
          : formatSummary(metric.kind === "count" ? "counter" : metric.kind, differences, 2),
        "right"
      ),
      cell(delta?.text ?? "—", "right"),
      cell(pValue),
      "</tr>"
    ].join(""))
  }
  return rows
}

const formatStockCounters = (entry: PerfCell) => {
  const samplesByVersion = new Map<string, Array<PerfSample>>()
  const add = (version: string, samples: ReadonlyArray<PerfSample>) =>
    samplesByVersion.set(version, [...(samplesByVersion.get(version) ?? []), ...samples])
  add((entry.head ?? entry.base)!.typescriptVersion, entry.rounds.map((round) => round.stock))
  const baseStock = entry.rounds.flatMap((round) => round.baseStock === undefined ? [] : [round.baseStock])
  if (entry.base !== undefined && baseStock.length > 0) add(entry.base.typescriptVersion, baseStock)
  return [...samplesByVersion.entries()]
    .map(([version, samples]) => {
      const counter = (name: "symbols" | "types" | "instantiations") =>
        formatSummary("count", summarize(samples.map((sample) => sample[name])), 1)
      return `\`${version}\` symbols ${counter("symbols")} · types ${counter("types")} · ` +
        `instantiations ${counter("instantiations")}`
    })
    .join("; ")
}

const distinct = (values: ReadonlyArray<string>) => [...new Set(values)]

export const renderPerfReport = (
  cells: ReadonlyArray<PerfCell>,
  expected?: ReadonlyArray<PerfMatrixEntry>
): string => {
  const byChannel = new Map<string, PerfCell>()
  for (const entry of cells) {
    if (entry.base === undefined && entry.head === undefined) {
      throw new PerfReportError({ reason: `channel ${entry.channel} has neither a base nor a head result` })
    }
    if (byChannel.has(entry.channel)) {
      throw new PerfReportError({ reason: `duplicate result for channel ${entry.channel}` })
    }
    byChannel.set(entry.channel, entry)
  }
  const expectedByChannel = new Map((expected ?? []).map((entry) => [entry.name, entry]))
  const channels = sortChannels([...byChannel.keys(), ...expectedByChannel.keys()])
  if (channels.length === 0) {
    throw new PerfReportError({ reason: "no perf results to report" })
  }

  const flagged: Array<FlaggedRow> = []
  const missing: Array<string> = []
  const rows: Array<string> = []
  const stockLines: Array<string> = []
  for (const channel of channels) {
    const entry = byChannel.get(channel)
    rows.push(...renderChannel(channel, entry, expectedByChannel.get(channel), flagged, missing))
    if (entry !== undefined) stockLines.push(`- Stock counters, ${channel}: ${formatStockCounters(entry)}`)
  }

  const verdict = flagged.length === 0
    ? "**Effect perf, PR vs base: no significant change**"
    : `**Effect perf, PR vs base: ${flagged.length} flagged row${flagged.length === 1 ? "" : "s"}**`
  const summary = [
    perfReportMarker,
    verdict,
    ...(flagged.length === 0 ? [] : ["", ...flagged.map((row) => `- ${row.channel} · ${row.metric} · ${row.delta}`)]),
    ...(missing.length === 0 ? [] : ["", `⚠️ Missing benchmark results: ${missing.join(", ")}`]),
    ""
  ]

  const roundCounts = cells.map((entry) => entry.rounds.length)
  const minRounds = Math.min(...roundCounts)
  const maxRounds = Math.max(...roundCounts)
  const commits = (side: PerfSide) =>
    distinct(cells.flatMap((entry) => entry[side] === undefined ? [] : [`\`${entry[side].commit}\``])).join(", ") ||
    "—"
  const workloads = distinct(cells.map((entry) =>
    `\`${entry.workload.repository}@${entry.workload.revision}\`${
      entry.workload.allRules ? " with every Effect rule executing (`skipDisabledOptimization: true`)" : ""
    }`
  ))
  const footer = [
    ...stockLines,
    `- Workload: ${workloads.join(", ")}`,
    ...(cells.length === 0 ? [] : [
      `- Rounds per channel: ${minRounds === maxRounds ? minRounds : `${minRounds}–${maxRounds}`}, each compiling ` +
      "stock, base, and PR once on the same runner, rotating the order so each compiler runs in each position " +
      "equally often (a channel whose TypeScript changed also compiles the base's stock)"
    ]),
    `- Commits: base ${commits("base")} · head ${commits("head")}`,
    "- PR vs base is paired per round: PR − base for counts and counters (median ±half-range), PR / base − 1 for " +
    "allocs and times (median (min to max)). Counter, alloc, and time deltas are flagged only when the exact " +
    `two-sided Wilcoxon signed-rank p < ${significanceLevel}, which needs at least 8 rounds.`,
    "- The vs stock columns give context, each patched compiler against the stock compile of the same round: " +
    "errors and warnings are the patched compiler's own counts, counters patched − stock (median ±half-range), " +
    "allocs and times patched / stock − 1 (median (min to max))."
  ]

  const details = [
    "<details><summary>Details</summary>",
    "",
    "<table>",
    "<thead><tr><th><sub>Metric</sub></th><th><sub>base vs stock</sub></th><th><sub>PR vs stock</sub></th>" +
    "<th><sub>PR vs base</sub></th><th><sub>Delta</sub></th><th><sub>p-value</sub></th></tr></thead>",
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
    : (yield* decodeFile(PerfMatrixFromString, path.resolve(options.matrix))).measure.include
  const report = yield* Effect.try({
    try: () => renderPerfReport(cells, expected),
    catch: (error) => error instanceof PerfReportError ? error : new PerfReportError({ reason: String(error) })
  })
  yield* fs.makeDirectory(path.dirname(output), { recursive: true })
  yield* fs.writeFileString(output, report)
  yield* Console.log(report)
})
