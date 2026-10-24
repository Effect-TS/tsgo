import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Schema from "effect/Schema"
import {
  cleanTarget,
  type Compiler,
  numericMetric,
  parsePerfMetrics,
  PerfError,
  resolveCompilers
} from "./perf.ts"
import { runCommand, runCommandCaptureSplit, runCommandString } from "./process.ts"

export const perfWorkload = {
  repository: "https://github.com/Effect-TS/effect",
  revision: "7f524e25cfb49a71ac39b0f44278d08d40dda5de"
} as const

export const PerfSample = Schema.Struct({
  exitCode: Schema.Finite,
  errors: Schema.Finite,
  warnings: Schema.Finite,
  symbols: Schema.Finite,
  types: Schema.Finite,
  instantiations: Schema.Finite,
  memoryAllocs: Schema.Finite,
  checkTimeSeconds: Schema.Finite,
  totalTimeSeconds: Schema.Finite
})
export type PerfSample = typeof PerfSample.Type

export const PerfPair = Schema.Struct({
  order: Schema.Literals(["stock-first", "patched-first"]),
  stock: PerfSample,
  patched: PerfSample
})
export type PerfPair = typeof PerfPair.Type

export const PerfSide = Schema.Literals(["base", "head"])
export type PerfSide = typeof PerfSide.Type

export const PerfCell = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  channel: Schema.NonEmptyString,
  side: PerfSide,
  commit: Schema.NonEmptyString,
  typescriptVersion: Schema.NonEmptyString,
  workload: Schema.Struct({
    repository: Schema.String,
    revision: Schema.String,
    allRules: Schema.Boolean
  }),
  stockVersion: Schema.String,
  patchedVersion: Schema.String,
  pairs: Schema.Array(PerfPair).check(Schema.isMinLength(1))
})
export type PerfCell = typeof PerfCell.Type

export const PerfCellFromString = Schema.fromJsonString(PerfCell)

export interface PerfCiOptions {
  readonly channel: string
  readonly side: PerfSide
  readonly commit: string
  readonly version: string
  readonly patchedBin: string
  readonly workloadDir: string
  readonly pairs: number
  readonly output: string
}

export const countDiagnostics = (output: string, category: "error" | "warning") =>
  output.match(new RegExp(`^(?:.+\\(\\d+,\\d+\\): )?${category} TS\\d+:`, "gm"))?.length ?? 0

export const parsePerfSample = (exitCode: number, output: string): PerfSample | undefined => {
  const metrics = parsePerfMetrics(output)
  const symbols = numericMetric(metrics.symbols, "")
  const types = numericMetric(metrics.types, "")
  const instantiations = numericMetric(metrics.instantiations, "")
  const memoryAllocs = numericMetric(metrics.memoryAllocs, "")
  const checkTimeSeconds = numericMetric(metrics.checkTime, "s")
  const totalTimeSeconds = numericMetric(metrics.totalTime, "s")
  if (
    symbols === undefined || types === undefined || instantiations === undefined || memoryAllocs === undefined ||
    checkTimeSeconds === undefined || totalTimeSeconds === undefined
  ) {
    return undefined
  }
  return {
    exitCode,
    errors: countDiagnostics(output, "error"),
    warnings: countDiagnostics(output, "warning"),
    symbols,
    types,
    instantiations,
    memoryAllocs,
    checkTimeSeconds,
    totalTimeSeconds
  }
}

const languageServicePluginName = /"name"\s*:\s*"@effect\/language-service"(\s*,)?/g

// Textual edit because the workload tsconfigs are JSONC with comments; returns undefined when the
// file does not configure the Effect plugin.
export const enableAllRules = (text: string): string | undefined => {
  if (text.search(languageServicePluginName) === -1) return undefined
  if (text.includes(`"skipDisabledOptimization": true`)) return text
  return text.replace(
    languageServicePluginName,
    (match, comma: string | undefined) =>
      comma === undefined
        ? `${match}, "skipDisabledOptimization": true`
        : `${match} "skipDisabledOptimization": true,`
  )
}

export const prepareWorkload = Effect.fnUntraced(function*(workloadDir: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  if (!(yield* fs.exists(workloadDir))) {
    yield* Console.log(`Fetching ${perfWorkload.repository}@${perfWorkload.revision} into ${workloadDir}`)
    yield* fs.makeDirectory(workloadDir, { recursive: true })
    yield* runCommand("git", workloadDir, ["init", "--quiet"])
    yield* runCommand("git", workloadDir, ["remote", "add", "origin", perfWorkload.repository])
    yield* runCommand("git", workloadDir, ["fetch", "--quiet", "--depth", "1", "origin", perfWorkload.revision])
    yield* runCommand("git", workloadDir, ["checkout", "--quiet", "FETCH_HEAD"])
  }
  const head = (yield* runCommandString("git", workloadDir, ["rev-parse", "HEAD"])).trim()
  if (head !== perfWorkload.revision) {
    return yield* new PerfError({
      reason: `Workload ${workloadDir} is at ${head}, expected ${perfWorkload.revision}`
    })
  }
  yield* runCommand("pnpm", workloadDir, ["install", "--frozen-lockfile", "--ignore-scripts"])

  const tsconfigs = (yield* runCommandString("git", workloadDir, ["ls-files", "-z"]))
    .split("\0")
    .filter((file) => /^tsconfig.*\.json$/.test(path.basename(file)))
  const edited: Array<string> = []
  for (const file of tsconfigs) {
    const absolute = path.join(workloadDir, file)
    const text = yield* fs.readFileString(absolute)
    const updated = enableAllRules(text)
    if (updated === undefined) continue
    if (updated !== text) yield* fs.writeFileString(absolute, updated)
    edited.push(file)
  }
  if (edited.length === 0) {
    return yield* new PerfError({ reason: `No tsconfig in ${workloadDir} configures @effect/language-service` })
  }
  yield* Console.log(`All Effect rules enabled in ${edited.length} tsconfig(s): ${edited.join(", ")}`)
})

const measureCompile = Effect.fnUntraced(function*(
  workloadDir: string,
  logDirectory: string,
  compiler: Compiler,
  pair: number
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const label = `pair ${pair} ${compiler.name}`
  const logPath = path.join(logDirectory, `pair-${pair}-${compiler.name}.log`)
  yield* cleanTarget(workloadDir, label, path.join(logDirectory, `pair-${pair}-${compiler.name}.clean.log`))

  const args = [...compiler.args, "-b", "tsconfig.json", "--extendedDiagnostics"]
  yield* Console.log(`[${label}] ${compiler.identity} ${args.slice(compiler.args.length).join(" ")}`)
  const result = yield* runCommandCaptureSplit(compiler.command, workloadDir, args)
  const output = result.stdout + result.stderr
  yield* fs.writeFileString(logPath, output)
  const sample = parsePerfSample(Number(result.exitCode), output)
  if (sample === undefined) {
    return yield* new PerfError({
      reason: `${label} exited with code ${result.exitCode} without aggregate diagnostics; see ${logPath}`
    })
  }
  yield* Console.log(
    `[${label}] exit=${sample.exitCode} errors=${sample.errors} warnings=${sample.warnings} ` +
      `symbols=${sample.symbols} types=${sample.types} instantiations=${sample.instantiations} ` +
      `allocs=${sample.memoryAllocs} check=${sample.checkTimeSeconds}s total=${sample.totalTimeSeconds}s`
  )
  return sample
})

export const runPerfCi = Effect.fnUntraced(function*(options: PerfCiOptions) {
  if (!Number.isInteger(options.pairs) || options.pairs < 1) {
    return yield* new PerfError({ reason: "--pairs must be a positive integer" })
  }
  if (!/^[\w.+-]+$/.test(options.channel)) {
    return yield* new PerfError({ reason: `Invalid channel name: ${options.channel}` })
  }
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const workloadDir = path.resolve(options.workloadDir)
  const patchedBin = path.resolve(options.patchedBin)
  const output = path.resolve(options.output)
  const logDirectory = path.join(path.dirname(output), `${options.channel}-${options.side}`)
  if (!(yield* fs.exists(patchedBin))) {
    return yield* new PerfError({ reason: `Patched binary does not exist: ${patchedBin}` })
  }
  yield* fs.makeDirectory(logDirectory, { recursive: true })

  yield* prepareWorkload(workloadDir)
  const { patched, stock } = yield* resolveCompilers(workloadDir, options.version, patchedBin)

  const pairs: Array<PerfPair> = []
  for (let pair = 1; pair <= options.pairs; pair++) {
    const order = pair % 2 === 1 ? "stock-first" : "patched-first"
    const [first, second] = order === "stock-first" ? [stock, patched] : [patched, stock]
    const firstSample = yield* measureCompile(workloadDir, logDirectory, first, pair)
    const secondSample = yield* measureCompile(workloadDir, logDirectory, second, pair)
    pairs.push(
      order === "stock-first"
        ? { order, stock: firstSample, patched: secondSample }
        : { order, stock: secondSample, patched: firstSample }
    )
  }

  const cell: PerfCell = {
    schemaVersion: 1,
    channel: options.channel,
    side: options.side,
    commit: options.commit,
    typescriptVersion: options.version,
    workload: { ...perfWorkload, allRules: true },
    stockVersion: stock.version,
    patchedVersion: patched.version,
    pairs
  }
  yield* fs.writeFileString(output, `${JSON.stringify(cell, null, 2)}\n`)
  yield* Console.log(`Perf cell written to: ${output}`)
})
