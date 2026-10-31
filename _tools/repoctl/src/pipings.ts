// `repoctl pipings`: prints the piping flows (and the diagnostics) the Go runner finds in a TypeScript snippet, so a
// rule author can check how a shape normalizes before matching it (docs/agents/piping-flows.md). The snippet runs as a
// fixture through internal/effecttest's TestScratchFixture, against the bundled Effect packages; the baselines it
// writes are removed afterwards.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { ChildProcessSpawner } from "effect/process"
import { text } from "node:stream/consumers"
import { runCommandCaptureSplit } from "./process.ts"

export class PipingsError extends Data.TaggedError("PipingsError")<{
  readonly reason: string
}> {
  get message(): string {
    return `repoctl pipings: ${this.reason}`
  }
}

export interface PipingsOptions {
  // A TypeScript file, or "-" for standard input. `// @filename:` units work as in testdata/tests fixtures.
  readonly file: string
  readonly effect: "v3" | "v4"
  readonly errors: boolean
}

export const runPipings = Effect.fnUntraced(function*(repositoryRoot: string, options: PipingsOptions) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const source = options.file === "-"
    ? yield* Effect.promise(() => text(process.stdin))
    : yield* fs.readFileString(path.resolve(options.file))
  // One name per run, so concurrent runs and real fixtures never share baselines.
  const name = `scratch_${process.pid}_${Date.now()}`
  const directory = yield* fs.makeTempDirectory({ prefix: "repoctl-pipings-" })
  const baselines = ["local", "reference"].map((kind) =>
    path.join(repositoryRoot, "testdata", "baselines", kind, `effect-${options.effect}`)
  )
  const cleanup = Effect.all([
    fs.remove(directory, { recursive: true, force: true }),
    ...baselines.flatMap((folder) =>
      ["errors", "pipings", "layers"].map((kind) => fs.remove(path.join(folder, `${name}.${kind}.txt`), { force: true }))
    )
  ]).pipe(Effect.ignore)
  yield* Effect.gen(function*() {
    const fixture = path.join(directory, `${name}.ts`)
    yield* fs.writeFileString(fixture, source)
    const result = yield* runCommandCaptureSplit("go", repositoryRoot, [
      "test",
      "./internal/effecttest/",
      "-run",
      "^TestScratchFixture$",
      "-count=1"
    ], { ETS_SCRATCH_FIXTURE: fixture, ETS_SCRATCH_EFFECT: options.effect })
    if (result.exitCode !== ChildProcessSpawner.ExitCode(0)) {
      yield* Console.error(result.stdout + result.stderr)
      return yield* new PipingsError({ reason: "the Go runner failed on the snippet (output above)" })
    }
    const read = (kind: string) => fs.readFileString(path.join(baselines[1], `${name}.${kind}.txt`))
    yield* Console.log(yield* read("pipings"))
    if (options.errors) {
      yield* Console.log(yield* read("errors"))
    }
  }).pipe(Effect.ensuring(cleanup))
})
