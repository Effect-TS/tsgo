import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as ChildProcess from "effect/unstable/process/ChildProcess"
import * as ChildProcessSpawner from "effect/unstable/process/ChildProcessSpawner"
import * as Stream from "effect/Stream"

export class CommandError extends Data.TaggedError("CommandError")<{
  readonly command: string
  readonly args: ReadonlyArray<string>
  readonly exitCode: number
  readonly stderr?: string
}> {
  get message(): string {
    const detail = this.stderr?.trim()
    return `${this.command} ${this.args.join(" ")} exited with code ${this.exitCode}` +
      (detail === undefined || detail === "" ? "" : `:\n${detail}`)
  }
}

export const runCommand = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>,
  quiet = false,
  env?: Readonly<Record<string, string>>
) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  const exitCode = yield* spawner.exitCode(ChildProcess.make(command, args, {
    cwd,
    stdin: "inherit",
    stdout: quiet ? "ignore" : "inherit",
    stderr: quiet ? "ignore" : "inherit",
    env,
    extendEnv: true
  }))
  if (exitCode !== ChildProcessSpawner.ExitCode(0)) {
    return yield* new CommandError({ command, args, exitCode })
  }
})

export const runCommandString = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>,
  env?: Readonly<Record<string, string>>
) {
  const result = yield* runCommandCaptureSplit(command, cwd, args, env)
  if (result.exitCode !== ChildProcessSpawner.ExitCode(0)) {
    return yield* new CommandError({ command, args, exitCode: result.exitCode, stderr: result.stderr })
  }
  return result.stdout
})

export const runCommandCapture = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>
) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  return yield* Effect.scoped(Effect.gen(function*() {
    const handle = yield* spawner.spawn(ChildProcess.make(command, args, {
      cwd,
      stdin: "inherit"
    }))
    const [output, exitCode] = yield* Effect.all([
      Stream.mkString(Stream.decodeText(handle.all)),
      handle.exitCode
    ], { concurrency: "unbounded" })
    return { exitCode, output }
  }))
})

export const runCommandCaptureSplit = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>,
  env?: Readonly<Record<string, string>>
) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  return yield* Effect.scoped(Effect.gen(function*() {
    const handle = yield* spawner.spawn(ChildProcess.make(command, args, {
      cwd,
      stdin: "inherit",
      env,
      extendEnv: true
    }))
    const [stdout, stderr, exitCode] = yield* Effect.all([
      Stream.mkString(Stream.decodeText(handle.stdout)),
      Stream.mkString(Stream.decodeText(handle.stderr)),
      handle.exitCode
    ], { concurrency: "unbounded" })
    return { exitCode, stderr, stdout }
  }))
})
