import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as ChildProcessSpawner from "effect/unstable/process/ChildProcessSpawner"
import { CommandError, runCommand, runCommandCaptureSplit } from "./process.ts"

const deadcodeVersion = "v0.48.0"

export class DeadCodeError extends Data.TaggedError("DeadCodeError")<{
  readonly findings: ReadonlyArray<string>
}> {
  get message(): string {
    return [
      ...this.findings,
      "",
      "deadcode: the functions above are unreachable from the tsgo executable and all tests.",
      "Delete them, or add the function name or package path prefix to _tools/deadcode-allow.txt with a reason."
    ].join("\n")
  }
}

export const parseDeadCodeAllowlist = (content: string): ReadonlySet<string> =>
  new Set(content.split("\n").map((line) => line.trim()).filter((line) => line !== "" && !line.startsWith("#")))

export const filterDeadCodeFindings = (
  output: string,
  allowlist: ReadonlySet<string>
): ReadonlyArray<string> =>
  output.split("\n").map((line) => line.trim()).filter((line) => {
    const normalizedLine = line.replaceAll("\\", "/")
    if (normalizedLine === "" || normalizedLine.includes("_test.go:")) {
      return false
    }
    if ([...allowlist].some((entry) => entry.endsWith("/") && normalizedLine.startsWith(entry))) {
      return false
    }
    const marker = "unreachable func: "
    const markerIndex = normalizedLine.indexOf(marker)
    return markerIndex === -1 || !allowlist.has(normalizedLine.slice(markerIndex + marker.length))
  })

export const runDeadCode = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const allowlist = parseDeadCodeAllowlist(
    yield* fs.readFileString(path.join(repositoryRoot, "_tools", "deadcode-allow.txt"))
  )
  const args = [
    "run",
    `golang.org/x/tools/cmd/deadcode@${deadcodeVersion}`,
    "-test",
    "-filter=^github.com/effect-ts/tsgo($|/)",
    "./typescript-go/cmd/tsgo",
    "./..."
  ]
  const result = yield* runCommandCaptureSplit("go", repositoryRoot, args, { CGO_ENABLED: "0" })
  if (result.exitCode !== ChildProcessSpawner.ExitCode(0)) {
    return yield* new CommandError({
      command: "go",
      args,
      exitCode: result.exitCode,
      stderr: result.stderr
    })
  }

  const findings = filterDeadCodeFindings(result.stdout, allowlist)
  if (findings.length > 0) {
    return yield* new DeadCodeError({ findings })
  }
  yield* Console.log("deadcode: clean")
})

export const runLint = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* runCommand("golangci-lint", repositoryRoot, ["run", "./..."], false, { CGO_ENABLED: "0" })
  yield* runDeadCode(repositoryRoot)
})
