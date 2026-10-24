import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { gnuTime, parsePeakRss, peakRssFormat, resolveTsrsBinaries } from "./perfTsrs.ts"
import { runCommand, runCommandCaptureSplit } from "./process.ts"
import {
  readUpstream,
  resolveMaterializedTypeScriptSource,
  typeScriptSourceEntrypoint
} from "./upstream.ts"

export type PerfEngine = "tsgo" | "tsrs"

export interface PerfOptions {
  readonly target: string
  // tsgo (default): stock TypeScript-Go vs the patched Go binary. tsrs: stock tsrs (tsrs_cli built from the pinned
  // commit without _patches/tsrs) vs the patched Rust binary (ets_tsrs), both with this repository's release profile.
  readonly engine?: PerfEngine
  // tsrs only: TSRS_CHECKER_ASSIGNMENT (`--checkerAssignment`) for both binaries ("go" by default, deterministic; ""
  // to leave it unset).
  readonly checkerAssignment?: string
  readonly version?: string
  readonly latest: boolean
  readonly output?: string
  readonly runId?: string
  readonly patchedBin?: string
  readonly stockBin?: string
  readonly config: string
  readonly runs: number
  readonly diagnosticsFlag: string
}

export interface Compiler {
  readonly name: "stock" | "patched"
  readonly identity: string
  readonly command: string
  readonly args: ReadonlyArray<string>
  readonly version: string
}

export interface PerfMetrics {
  readonly projectsBuilt?: string
  readonly totalTime?: string
  readonly checkTime?: string
  readonly parseTime?: string
  readonly bindTime?: string
  readonly emitTime?: string
  readonly changesTime?: string
  readonly memoryUsed?: string
  readonly memoryAllocs?: string
  readonly symbols?: string
  readonly types?: string
  readonly instantiations?: string
}

interface PerfRun extends PerfMetrics {
  readonly name: Compiler["name"]
  readonly run: number
  readonly exitCode: number
  // Measured outside the compiler: wall time of the process, and its peak resident set size (GNU time, Linux only).
  readonly wallTimeSeconds: number
  readonly peakRssKilobytes?: number
}

export class PerfError extends Data.TaggedError("PerfError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Performance comparison failed: ${this.reason}`
  }
}

const metric = (diagnostics: string, label: string) => {
  const matches = [...diagnostics.matchAll(new RegExp(`^${label}:[ \\t]*(.+)$`, "gm"))]
  return matches.at(-1)?.[1]?.trim()
}

export const parsePerfMetrics = (diagnostics: string): PerfMetrics => ({
  projectsBuilt: metric(diagnostics, "Projects built"),
  totalTime: metric(diagnostics, "Aggregate Total time"),
  checkTime: metric(diagnostics, "Aggregate Check time"),
  parseTime: metric(diagnostics, "Aggregate Parse time"),
  bindTime: metric(diagnostics, "Aggregate Bind time"),
  emitTime: metric(diagnostics, "Aggregate Emit time"),
  changesTime: metric(diagnostics, "Aggregate Changes compute time"),
  memoryUsed: metric(diagnostics, "Aggregate Memory used"),
  memoryAllocs: metric(diagnostics, "Aggregate Memory allocs"),
  symbols: metric(diagnostics, "Aggregate Symbols"),
  types: metric(diagnostics, "Aggregate Types"),
  instantiations: metric(diagnostics, "Aggregate Instantiations")
})

export const median = (values: ReadonlyArray<number>) => {
  if (values.length === 0) return undefined
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1
    ? sorted[middle]
    : (sorted[middle - 1]! + sorted[middle]!) / 2
}

export const numericMetric = (value: string | undefined, suffix: string) => {
  if (value === undefined || !value.endsWith(suffix)) return undefined
  const digits = value.slice(0, value.length - suffix.length)
  if (digits === "") return undefined
  const parsed = Number(digits)
  return Number.isFinite(parsed) ? parsed : undefined
}

const formatRun = (run: PerfRun) => [
  `${run.name} run ${run.run}:`,
  `exit=${run.exitCode}`,
  `projects=${run.projectsBuilt ?? "n/a"}`,
  `total=${run.totalTime ?? "n/a"}`,
  `check=${run.checkTime ?? "n/a"}`,
  `parse=${run.parseTime ?? "n/a"}`,
  `bind=${run.bindTime ?? "n/a"}`,
  `emit=${run.emitTime ?? "n/a"}`,
  `changes=${run.changesTime ?? "n/a"}`,
  `memory=${run.memoryUsed ?? "n/a"}`,
  `allocs=${run.memoryAllocs ?? "n/a"}`,
  `wall=${run.wallTimeSeconds.toFixed(3)}s`,
  `peakRss=${run.peakRssKilobytes === undefined ? "n/a" : `${run.peakRssKilobytes}K`}`
].join(" ")

export const cleanTarget = Effect.fnUntraced(function*(target: string, label: string, logPath: string) {
  const fs = yield* FileSystem.FileSystem
  yield* Console.log(`[${label}] pnpm clean`)
  const clean = yield* runCommandCaptureSplit("pnpm", target, ["clean"])
  yield* fs.writeFileString(logPath, clean.stdout + clean.stderr)
  if (clean.exitCode !== 0) {
    return yield* new PerfError({ reason: `pnpm clean exited with code ${clean.exitCode}` })
  }
})

export const resolveCompilers = Effect.fnUntraced(function*(
  cwd: string,
  typescriptVersion: string,
  patchedBin: string,
  stockBin?: string
) {
  const path = yield* Path.Path
  const stockCommand = stockBin === undefined ? "pnpm" : path.resolve(stockBin)
  const stockArgs = stockBin === undefined
    ? ["--silent", "--package", `typescript@${typescriptVersion}`, "dlx", "tsc"]
    : []
  const stockIdentity = stockBin === undefined
    ? `pnpm --silent --package typescript@${typescriptVersion} dlx tsc`
    : stockCommand
  const [stockVersionResult, patchedVersionResult] = yield* Effect.all([
    runCommandCaptureSplit(stockCommand, cwd, [...stockArgs, "--version"]),
    runCommandCaptureSplit(patchedBin, cwd, ["--version"])
  ], { concurrency: "unbounded" })
  if (stockVersionResult.exitCode !== 0) {
    return yield* new PerfError({ reason: `Unable to read stock compiler version: ${stockVersionResult.stderr}` })
  }
  if (patchedVersionResult.exitCode !== 0) {
    return yield* new PerfError({ reason: `Unable to read patched compiler version: ${patchedVersionResult.stderr}` })
  }
  const stockVersion = stockVersionResult.stdout.trim()
  const patchedVersion = patchedVersionResult.stdout.trim()
  if (stockVersion.includes("+effect-tsgo.")) {
    return yield* new PerfError({ reason: `Resolved stock binary is Effect-patched: ${stockIdentity} (${stockVersion})` })
  }
  yield* Console.log(`Stock compiler: ${stockIdentity} (${stockVersion})`)
  yield* Console.log(`Patched compiler: ${patchedBin} (${patchedVersion})`)
  return {
    stock: {
      name: "stock",
      identity: stockIdentity,
      command: stockCommand,
      args: stockArgs,
      version: stockVersion
    },
    patched: {
      name: "patched",
      identity: patchedBin,
      command: patchedBin,
      args: [],
      version: patchedVersion
    }
  } satisfies Record<Compiler["name"], Compiler>
})

const runCompiler = Effect.fnUntraced(function*(
  target: string,
  runDirectory: string,
  compiler: Compiler,
  run: number,
  config: string,
  diagnosticsFlag: string,
  engine: PerfEngine,
  engineEnv: Readonly<Record<string, string>>
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const caseDirectory = path.join(runDirectory, compiler.name, `run-${run}`)
  const pprofDirectory = path.join(caseDirectory, "pprof")
  yield* fs.makeDirectory(pprofDirectory, { recursive: true })

  yield* cleanTarget(target, `${compiler.name} run ${run}`, path.join(caseDirectory, "clean.log"))

  // tsrs writes no pprof profiles.
  const compilerArgs = ["-b", config, diagnosticsFlag, ...(engine === "tsgo" ? ["--pprofDir", pprofDirectory] : [])]
  const envPrefix = Object.entries(engineEnv).map(([name, value]) => `${name}=${value} `).join("")
  yield* Console.log(`[${compiler.name} run ${run}] ${envPrefix}${compiler.identity} ${compilerArgs.join(" ")}`)
  const time = yield* gnuTime()
  const resourcesPath = path.join(caseDirectory, "resources.txt")
  const startedAt = performance.now()
  const result = time === undefined
    ? yield* runCommandCaptureSplit(compiler.command, target, [...compiler.args, ...compilerArgs], engineEnv)
    : yield* runCommandCaptureSplit(time, target, [
      "-f",
      peakRssFormat,
      "-o",
      resourcesPath,
      compiler.command,
      ...compiler.args,
      ...compilerArgs
    ], engineEnv)
  const wallTimeSeconds = (performance.now() - startedAt) / 1000
  const peakRssKilobytes = time === undefined ? undefined : parsePeakRss(yield* fs.readFileString(resourcesPath))
  const diagnostics = result.stdout + result.stderr
  yield* Effect.all([
    fs.writeFileString(path.join(caseDirectory, "stdout.log"), result.stdout),
    fs.writeFileString(path.join(caseDirectory, "stderr.log"), result.stderr),
    fs.writeFileString(path.join(caseDirectory, "diagnostics.log"), diagnostics),
    fs.writeFileString(path.join(caseDirectory, "meta.txt"), [
      `name=${compiler.name}`,
      `run=${run}`,
      `cwd=${target}`,
      `binary=${compiler.identity}`,
      `version=${compiler.version}`,
      `config=${config}`,
      `pprofDir=${pprofDirectory}`,
      `diagnosticsFlag=${diagnosticsFlag}`,
      `args=${compilerArgs.join(" ")}`,
      `env=${envPrefix.trim()}`,
      `exit=${result.exitCode}`,
      `wallTimeSeconds=${wallTimeSeconds.toFixed(3)}`,
      `peakRssKilobytes=${peakRssKilobytes ?? "n/a"}`,
      ""
    ].join("\n"))
  ])
  if (result.exitCode !== 0) {
    yield* Console.error(`[${compiler.name} run ${run}] failed with exit code ${result.exitCode}`)
  }
  return {
    name: compiler.name,
    run,
    exitCode: result.exitCode,
    wallTimeSeconds,
    ...(peakRssKilobytes === undefined ? {} : { peakRssKilobytes }),
    ...parsePerfMetrics(diagnostics)
  } satisfies PerfRun
})

const metricValues = (runs: ReadonlyArray<PerfRun>, select: (run: PerfRun) => string | undefined, suffix: string) =>
  runs.flatMap((run) => {
    const value = numericMetric(select(run), suffix)
    return value === undefined ? [] : [value]
  })

export interface PerfSummary {
  readonly name: Compiler["name"]
  readonly totalTimeSeconds?: number | undefined
  readonly checkTimeSeconds?: number | undefined
  readonly memoryUsedKilobytes?: number | undefined
  readonly wallTimeSeconds?: number | undefined
  readonly peakRssKilobytes?: number | undefined
}

const overheadMetrics = [
  ["wall", "wallTimeSeconds"],
  ["total", "totalTimeSeconds"],
  ["check", "checkTimeSeconds"],
  ["memory", "memoryUsedKilobytes"],
  ["peakRss", "peakRssKilobytes"]
] as const

// "overhead (patched vs stock medians): wall=+12.3% ..." from the stock and patched summaries.
export const overheadLines = (summaries: ReadonlyArray<PerfSummary>): ReadonlyArray<string> => {
  const stock = summaries.find((summary) => summary.name === "stock")
  const patched = summaries.find((summary) => summary.name === "patched")
  if (stock === undefined || patched === undefined) return []
  const parts = overheadMetrics.flatMap(([label, key]) => {
    const base = stock[key]
    const value = patched[key]
    if (base === undefined || value === undefined || base === 0) return []
    const percent = (value / base - 1) * 100
    return [`${label}=${percent >= 0 ? "+" : ""}${percent.toFixed(1)}%`]
  })
  return parts.length === 0 ? [] : [`overhead (patched vs stock medians): ${parts.join(" ")}`]
}

export const comparePerformance = Effect.fnUntraced(function*(repositoryRoot: string, options: PerfOptions) {
  if (!Number.isInteger(options.runs) || options.runs < 1) {
    return yield* new PerfError({ reason: "--runs must be a positive integer" })
  }
  if (options.latest && options.version !== undefined) {
    return yield* new PerfError({ reason: "--latest and --version cannot be used together" })
  }

  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const target = path.resolve(options.target)
  const output = path.resolve(options.output ?? path.join(repositoryRoot, "_tools", ".perf-out"))
  const runId = options.runId ?? new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z")
  const runDirectory = path.join(output, runId)
  const engine = options.engine ?? "tsgo"

  if (!(yield* fs.exists(target))) {
    return yield* new PerfError({ reason: `Target repository does not exist: ${target}` })
  }
  yield* fs.makeDirectory(runDirectory, { recursive: true })

  const upstream = yield* readUpstream(repositoryRoot)
  const typescriptVersion = options.version ?? upstream.tags.typescript[options.latest ? "latest" : "next"]
  if (upstream.components.typescript[typescriptVersion] === undefined) {
    return yield* new PerfError({ reason: `Unknown TypeScript component version: ${typescriptVersion}` })
  }

  let patchedBin: string
  let stockBin = options.stockBin
  if (engine === "tsrs") {
    const binaries = yield* resolveTsrsBinaries(
      repositoryRoot,
      options.patchedBin === undefined ? undefined : path.resolve(options.patchedBin),
      stockBin
    )
    patchedBin = binaries.patched
    stockBin = binaries.stock
  } else {
    patchedBin = path.resolve(options.patchedBin ?? path.join(repositoryRoot, "tsgo"))
    if (!(yield* fs.exists(patchedBin))) {
      const compiler = yield* resolveMaterializedTypeScriptSource(repositoryRoot)
      yield* Console.log(`Patched binary missing, building ${patchedBin}`)
      yield* runCommand("go", repositoryRoot, [
        "build",
        "-o",
        patchedBin,
        typeScriptSourceEntrypoint(compiler)
      ], false, { CGO_ENABLED: "0" })
    }
  }
  const checkerAssignment = options.checkerAssignment ?? "go"
  // tsrs's `--checkerAssignment` flag is not accepted with `-b`; its environment variable is.
  const engineEnv: Record<string, string> = engine === "tsrs" && checkerAssignment !== ""
    ? { TSRS_CHECKER_ASSIGNMENT: checkerAssignment }
    : {}

  const { patched, stock } = yield* resolveCompilers(repositoryRoot, typescriptVersion, patchedBin, stockBin)
  if (engine === "tsrs" && !patched.version.includes("+effect-tsgo.")) {
    return yield* new PerfError({ reason: `Patched tsrs binary is not Effect-patched: ${patchedBin} (${patched.version})` })
  }
  const compilers: ReadonlyArray<Compiler> = [stock, patched]

  const results: Array<PerfRun> = []
  for (let run = 1; run <= options.runs; run++) {
    for (const compiler of compilers) {
      results.push(yield* runCompiler(
        target,
        runDirectory,
        compiler,
        run,
        options.config,
        options.diagnosticsFlag,
        engine,
        engineEnv
      ))
    }
  }

  const summaries = compilers.map((compiler) => {
    const runs = results.filter((result) => result.name === compiler.name)
    return {
      name: compiler.name,
      totalTimeSeconds: median(metricValues(runs, (run) => run.totalTime, "s")),
      checkTimeSeconds: median(metricValues(runs, (run) => run.checkTime, "s")),
      memoryUsedKilobytes: median(metricValues(runs, (run) => run.memoryUsed, "K")),
      wallTimeSeconds: median(runs.map((run) => run.wallTimeSeconds)),
      peakRssKilobytes: median(runs.flatMap((run) => run.peakRssKilobytes === undefined ? [] : [run.peakRssKilobytes]))
    }
  })
  const summaryLines = [
    ...results.map(formatRun),
    ...summaries.map((summary) => summary.totalTimeSeconds === undefined ||
        summary.checkTimeSeconds === undefined ||
        summary.memoryUsedKilobytes === undefined
      ? `${summary.name} median: unavailable`
      : `${summary.name} median: total=${summary.totalTimeSeconds.toFixed(3)}s ` +
        `check=${summary.checkTimeSeconds.toFixed(3)}s memory=${summary.memoryUsedKilobytes.toFixed(0)}K` +
        ` wall=${summary.wallTimeSeconds?.toFixed(3) ?? "n/a"}s` +
        ` peakRss=${summary.peakRssKilobytes === undefined ? "n/a" : `${summary.peakRssKilobytes.toFixed(0)}K`}`),
    ...overheadLines(summaries)
  ]
  yield* Effect.all([
    fs.writeFileString(path.join(runDirectory, "summary.txt"), `${summaryLines.join("\n")}\n`),
    fs.writeFileString(path.join(runDirectory, "summary.json"), `${JSON.stringify({
      target,
      engine,
      typescriptVersion,
      compilers,
      runs: results,
      medians: summaries
    }, null, 2)}\n`)
  ])
  for (const line of summaryLines) yield* Console.log(line)
  yield* Console.log(`Artifacts written to: ${runDirectory}`)

  const failures = results.filter((result) => result.exitCode !== 0)
  if (failures.length > 0) {
    return yield* new PerfError({ reason: `${failures.length} compiler run(s) failed` })
  }
})
