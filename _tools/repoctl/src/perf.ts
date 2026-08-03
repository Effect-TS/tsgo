import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandCaptureSplit } from "./process.ts"
import { getProfile, readUpstream, type ProfileName } from "./upstream.ts"

export interface PerfOptions {
  readonly target: string
  readonly profile: ProfileName
  readonly output?: string
  readonly runId?: string
  readonly patchedBin?: string
  readonly stockBin?: string
  readonly config: string
  readonly runs: number
  readonly diagnosticsFlag: string
}

interface Compiler {
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
}

interface PerfRun extends PerfMetrics {
  readonly name: Compiler["name"]
  readonly run: number
  readonly exitCode: number
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
  memoryAllocs: metric(diagnostics, "Aggregate Memory allocs")
})

export const median = (values: ReadonlyArray<number>) => {
  if (values.length === 0) return undefined
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1
    ? sorted[middle]
    : (sorted[middle - 1]! + sorted[middle]!) / 2
}

const numericMetric = (value: string | undefined, suffix: string) => {
  if (value === undefined || !value.endsWith(suffix)) return undefined
  const parsed = Number(value.slice(0, -suffix.length))
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
  `allocs=${run.memoryAllocs ?? "n/a"}`
].join(" ")

const runCompiler = Effect.fnUntraced(function*(
  target: string,
  runDirectory: string,
  compiler: Compiler,
  run: number,
  config: string,
  diagnosticsFlag: string
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const caseDirectory = path.join(runDirectory, compiler.name, `run-${run}`)
  const pprofDirectory = path.join(caseDirectory, "pprof")
  yield* fs.makeDirectory(pprofDirectory, { recursive: true })

  yield* Console.log(`[${compiler.name} run ${run}] pnpm clean`)
  const clean = yield* runCommandCaptureSplit("pnpm", target, ["clean"])
  yield* fs.writeFileString(path.join(caseDirectory, "clean.log"), clean.stdout + clean.stderr)
  if (clean.exitCode !== 0) {
    return yield* new PerfError({ reason: `pnpm clean exited with code ${clean.exitCode}` })
  }

  yield* Console.log(
    `[${compiler.name} run ${run}] ${compiler.identity} -b ${config} ${diagnosticsFlag} --pprofDir ${pprofDirectory}`
  )
  const result = yield* runCommandCaptureSplit(compiler.command, target, [
    ...compiler.args,
    "-b",
    config,
    diagnosticsFlag,
    "--pprofDir",
    pprofDirectory
  ])
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
      `exit=${result.exitCode}`,
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
    ...parsePerfMetrics(diagnostics)
  } satisfies PerfRun
})

const metricValues = (runs: ReadonlyArray<PerfRun>, select: (run: PerfRun) => string | undefined, suffix: string) =>
  runs.flatMap((run) => {
    const value = numericMetric(select(run), suffix)
    return value === undefined ? [] : [value]
  })

export const comparePerformance = Effect.fnUntraced(function*(repositoryRoot: string, options: PerfOptions) {
  if (!Number.isInteger(options.runs) || options.runs < 1) {
    return yield* new PerfError({ reason: "--runs must be a positive integer" })
  }

  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const target = path.resolve(options.target)
  const output = path.resolve(options.output ?? path.join(repositoryRoot, "_tools", ".perf-out"))
  const runId = options.runId ?? new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z")
  const runDirectory = path.join(output, runId)
  const patchedBin = path.resolve(options.patchedBin ?? path.join(repositoryRoot, "tsgo"))

  if (!(yield* fs.exists(target))) {
    return yield* new PerfError({ reason: `Target repository does not exist: ${target}` })
  }
  yield* fs.makeDirectory(runDirectory, { recursive: true })

  if (!(yield* fs.exists(patchedBin))) {
    yield* Console.log(`Patched binary missing, building ${patchedBin}`)
    yield* runCommand("go", repositoryRoot, [
      "build",
      "-o",
      patchedBin,
      "./typescript-go/cmd/tsgo"
    ], false, { CGO_ENABLED: "0" })
  }

  const upstream = yield* readUpstream(repositoryRoot)
  const profile = yield* getProfile(upstream, options.profile)
  const stockCommand = options.stockBin === undefined ? "pnpm" : path.resolve(options.stockBin)
  const stockArgs = options.stockBin === undefined
    ? ["--silent", "--package", `typescript@${profile.ts.npmVersion}`, "dlx", "tsc"]
    : []
  const stockIdentity = options.stockBin === undefined
    ? `pnpm --silent --package typescript@${profile.ts.npmVersion} dlx tsc`
    : stockCommand
  const [stockVersionResult, patchedVersionResult] = yield* Effect.all([
    runCommandCaptureSplit(stockCommand, repositoryRoot, [...stockArgs, "--version"]),
    runCommandCaptureSplit(patchedBin, repositoryRoot, ["--version"])
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

  const compilers: ReadonlyArray<Compiler> = [
    {
      name: "stock",
      identity: stockIdentity,
      command: stockCommand,
      args: stockArgs,
      version: stockVersion
    },
    {
      name: "patched",
      identity: patchedBin,
      command: patchedBin,
      args: [],
      version: patchedVersion
    }
  ]
  yield* Console.log(`Stock compiler: ${stockIdentity} (${stockVersion})`)
  yield* Console.log(`Patched compiler: ${patchedBin} (${patchedVersion})`)

  const results: Array<PerfRun> = []
  for (let run = 1; run <= options.runs; run++) {
    for (const compiler of compilers) {
      results.push(yield* runCompiler(
        target,
        runDirectory,
        compiler,
        run,
        options.config,
        options.diagnosticsFlag
      ))
    }
  }

  const summaries = compilers.map((compiler) => {
    const runs = results.filter((result) => result.name === compiler.name)
    return {
      name: compiler.name,
      totalTimeSeconds: median(metricValues(runs, (run) => run.totalTime, "s")),
      checkTimeSeconds: median(metricValues(runs, (run) => run.checkTime, "s")),
      memoryUsedKilobytes: median(metricValues(runs, (run) => run.memoryUsed, "K"))
    }
  })
  const summaryLines = [
    ...results.map(formatRun),
    ...summaries.map((summary) => summary.totalTimeSeconds === undefined ||
        summary.checkTimeSeconds === undefined ||
        summary.memoryUsedKilobytes === undefined
      ? `${summary.name} median: unavailable`
      : `${summary.name} median: total=${summary.totalTimeSeconds.toFixed(3)}s ` +
        `check=${summary.checkTimeSeconds.toFixed(3)}s memory=${summary.memoryUsedKilobytes.toFixed(0)}K`)
  ]
  yield* Effect.all([
    fs.writeFileString(path.join(runDirectory, "summary.txt"), `${summaryLines.join("\n")}\n`),
    fs.writeFileString(path.join(runDirectory, "summary.json"), `${JSON.stringify({
      target,
      profile: options.profile,
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
