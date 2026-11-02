import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { ChildProcess } from "effect/process"
import { ChildProcessSpawner } from "effect/process"
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

const windowsCommandShims = new Set(["corepack", "npm", "pnpm"])

export const isWindowsCommandShim = (command: string, platform: NodeJS.Platform = process.platform): boolean =>
  platform === "win32" && windowsCommandShims.has(command)

export const resolveCommand = (command: string, platform: NodeJS.Platform = process.platform): string =>
  isWindowsCommandShim(command, platform) ? `${command}.cmd` : command

// Windows .cmd shims must run through a shell, and the spawner rejects `shell` combined with an argument list,
// so the arguments are joined into the command line (the same line Node built from them for `shell: true`).
export const makeCommand = (
  command: string,
  args: ReadonlyArray<string>,
  options: ChildProcess.CommandOptions,
  platform: NodeJS.Platform = process.platform
): ChildProcess.Command =>
  isWindowsCommandShim(command, platform)
    ? ChildProcess.make([resolveCommand(command, platform), ...args].join(" "), [], { ...options, shell: true })
    : ChildProcess.make(command, args, options)

export const runCommand = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>,
  quiet = false,
  env?: Readonly<Record<string, string>>
) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  const exitCode = yield* spawner.exitCode(makeCommand(command, args, {
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

// Like runCommand with the output on stderr only, but returns the exit code instead of failing on a nonzero one
// (compiler runs, whose exit codes 1 and 2 report diagnostics).
export const runCommandExitCode = Effect.fnUntraced(function*(
  command: string,
  cwd: string,
  args: ReadonlyArray<string>,
  env?: Readonly<Record<string, string>>
) {
  const spawner = yield* ChildProcessSpawner.ChildProcessSpawner
  return Number(
    yield* spawner.exitCode(makeCommand(command, args, {
      cwd,
      stdin: "inherit",
      stdout: "ignore",
      stderr: "inherit",
      env,
      extendEnv: true
    }))
  )
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
    const handle = yield* spawner.spawn(makeCommand(command, args, {
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
    const handle = yield* spawner.spawn(makeCommand(command, args, {
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
