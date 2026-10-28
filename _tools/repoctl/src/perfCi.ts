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

export const PerfSide = Schema.Literals(["base", "head"])
export type PerfSide = typeof PerfSide.Type

// baseStock is the stock compiler of the base TypeScript version, measured only when it differs from the head's.
export const PerfCompilerName = Schema.Literals(["stock", "baseStock", "base", "head"])
export type PerfCompilerName = typeof PerfCompilerName.Type

export const PerfSubject = Schema.Struct({
  commit: Schema.NonEmptyString,
  typescriptVersion: Schema.NonEmptyString,
  stockVersion: Schema.String,
  patchedVersion: Schema.String
})
export type PerfSubject = typeof PerfSubject.Type

// One compile of each compiler, in the order they ran.
export const PerfRound = Schema.Struct({
  order: Schema.Array(PerfCompilerName).check(Schema.isMinLength(1)),
  stock: PerfSample,
  baseStock: Schema.optionalKey(PerfSample),
  base: Schema.optionalKey(PerfSample),
  head: Schema.optionalKey(PerfSample)
})
export type PerfRound = typeof PerfRound.Type

// A channel measured on one runner. A channel present on one side only (new or removed in the PR) has just that
// subject.
export const PerfCell = Schema.Struct({
  schemaVersion: Schema.Literal(2),
  channel: Schema.NonEmptyString,
  workload: Schema.Struct({
    repository: Schema.String,
    revision: Schema.String,
    allRules: Schema.Boolean
  }),
  base: Schema.optionalKey(PerfSubject),
  head: Schema.optionalKey(PerfSubject),
  rounds: Schema.Array(PerfRound).check(Schema.isMinLength(1))
})
export type PerfCell = typeof PerfCell.Type

export const PerfCellFromString = Schema.fromJsonString(PerfCell)

export interface PerfCiSubjectOptions {
  readonly bin: string
  readonly commit: string
  readonly version: string
}

export interface PerfCiOptions {
  readonly channel: string
  readonly base?: PerfCiSubjectOptions | undefined
  readonly head?: PerfCiSubjectOptions | undefined
  readonly workloadDir: string
  readonly rounds: number
  readonly output: string
}

// A side is measured when all of its flags are passed.
export const perfCiSubject = (side: PerfSide, flags: {
  readonly bin: string | undefined
  readonly commit: string | undefined
  readonly version: string | undefined
}): Effect.Effect<PerfCiSubjectOptions | undefined, PerfError> => {
  const { bin, commit, version } = flags
  if (bin === undefined && commit === undefined && version === undefined) return Effect.succeed(undefined)
  if (bin !== undefined && commit !== undefined && version !== undefined) {
    return Effect.succeed({ bin, commit, version })
  }
  return Effect.fail(
    new PerfError({ reason: `--${side}-bin, --${side}-commit, and --${side}-version must be passed together` })
  )
}

// Round r runs the compilers rotated by r, reversing the base order every other cycle: each compiler takes each
// position equally often over every n rounds, and with three compilers every 2n rounds cover all orders.
export const roundOrder = <A>(compilers: ReadonlyArray<A>, round: number): ReadonlyArray<A> => {
  const count = compilers.length
  const cycle = Math.floor(round / count)
  const shift = round % count
  const ordered = cycle % 2 === 0 ? compilers : [...compilers].reverse()
  return [...ordered.slice(shift), ...ordered.slice(0, shift)]
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

interface NamedCompiler {
  readonly name: PerfCompilerName
  readonly compiler: Compiler
}

const measureCompile = Effect.fnUntraced(function*(
  workloadDir: string,
  logDirectory: string,
  { compiler, name }: NamedCompiler,
  round: number
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const label = `round ${round} ${name}`
  const logPath = path.join(logDirectory, `round-${round}-${name}.log`)
  yield* cleanTarget(workloadDir, label, path.join(logDirectory, `round-${round}-${name}.clean.log`))

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
  if (!Number.isInteger(options.rounds) || options.rounds < 1) {
    return yield* new PerfError({ reason: "--rounds must be a positive integer" })
  }
  if (!/^[\w.+-]+$/.test(options.channel)) {
    return yield* new PerfError({ reason: `Invalid channel name: ${options.channel}` })
  }
  if (options.base === undefined && options.head === undefined) {
    return yield* new PerfError({ reason: "at least one of the base and head compilers is required" })
  }
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const workloadDir = path.resolve(options.workloadDir)
  const output = path.resolve(options.output)
  const logDirectory = path.join(path.dirname(output), options.channel)
  const sides = (["base", "head"] as const).flatMap((side) => {
    const subject = options[side]
    return subject === undefined ? [] : [{ side, ...subject, bin: path.resolve(subject.bin) }]
  })
  for (const { bin, side } of sides) {
    if (!(yield* fs.exists(bin))) {
      return yield* new PerfError({ reason: `${side} binary does not exist: ${bin}` })
    }
  }
  yield* fs.makeDirectory(logDirectory, { recursive: true })

  yield* prepareWorkload(workloadDir)
  const resolved: Array<{ readonly side: PerfSide; readonly subject: PerfSubject; readonly compiler: NamedCompiler }> =
    []
  const stocks = new Map<string, Compiler>()
  for (const { bin, commit, side, version } of sides) {
    const { patched, stock } = yield* resolveCompilers(workloadDir, version, bin)
    if (!stocks.has(version)) stocks.set(version, stock)
    resolved.push({
      side,
      subject: { commit, typescriptVersion: version, stockVersion: stock.version, patchedVersion: patched.version },
      compiler: { name: side, compiler: patched }
    })
  }
  // The stock compiler is the head's TypeScript (the base's for a removed channel); the base's own stock runs too
  // when the PR changes the channel's TypeScript.
  const stockVersion = (options.head ?? options.base)!.version
  const compilers: Array<NamedCompiler> = [{ name: "stock", compiler: stocks.get(stockVersion)! }]
  if (options.base !== undefined && options.base.version !== stockVersion) {
    compilers.push({ name: "baseStock", compiler: stocks.get(options.base.version)! })
  }
  compilers.push(...resolved.map((entry) => entry.compiler))

  const rounds: Array<PerfRound> = []
  for (let round = 0; round < options.rounds; round++) {
    const order = roundOrder(compilers, round)
    const samples: Partial<Record<PerfCompilerName, PerfSample>> = {}
    for (const compiler of order) {
      samples[compiler.name] = yield* measureCompile(workloadDir, logDirectory, compiler, round + 1)
    }
    rounds.push({
      order: order.map((compiler) => compiler.name),
      stock: samples.stock!,
      ...(samples.baseStock === undefined ? {} : { baseStock: samples.baseStock }),
      ...(samples.base === undefined ? {} : { base: samples.base }),
      ...(samples.head === undefined ? {} : { head: samples.head })
    })
  }

  const base = resolved.find((entry) => entry.side === "base")?.subject
  const head = resolved.find((entry) => entry.side === "head")?.subject
  const cell: PerfCell = {
    schemaVersion: 2,
    channel: options.channel,
    workload: { ...perfWorkload, allRules: true },
    ...(base === undefined ? {} : { base }),
    ...(head === undefined ? {} : { head }),
    rounds
  }
  yield* fs.writeFileString(output, `${JSON.stringify(cell, null, 2)}\n`)
  yield* Console.log(`Perf cell written to: ${output}`)
})
