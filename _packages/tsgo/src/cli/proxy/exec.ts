import * as childProcess from "node:child_process"
import { type DiagnosticsParentProcess, propagateDiagnosticsExit } from "../diagnostics.js"

export interface ProxyCommand {
  readonly file: string
  readonly args: ReadonlyArray<string>
  readonly env: NodeJS.ProcessEnv
}

export interface ProxyChildProcess {
  kill(signal: NodeJS.Signals): boolean
  on(event: "error", listener: (error: Error) => void): unknown
  on(event: "exit", listener: (code: number | null, signal: NodeJS.Signals | null) => void): unknown
}

export type SpawnProxyProcess = (
  file: string,
  args: ReadonlyArray<string>,
  options: { readonly stdio: "inherit"; readonly env: NodeJS.ProcessEnv }
) => ProxyChildProcess

export type ExecveProcess = (file: string, args: ReadonlyArray<string>, env: NodeJS.ProcessEnv) => never

export interface ProxyParentProcess extends DiagnosticsParentProcess {
  on(signal: NodeJS.Signals, listener: (signal: NodeJS.Signals) => void): unknown
  off(signal: NodeJS.Signals, listener: (signal: NodeJS.Signals) => void): unknown
}

export interface ExecHost {
  readonly platform: NodeJS.Platform
  // process.execve, available on Node >= 22.15 outside Windows.
  readonly execve: ExecveProcess | undefined
  readonly spawn: SpawnProxyProcess
  readonly parent: ProxyParentProcess
  readonly reportError: (message: string) => void
}

export const nodeExecHost: ExecHost = {
  platform: process.platform,
  execve: typeof process.execve === "function" ? (file, args, env) => process.execve!(file, args, env) : undefined,
  spawn: (file, args, options) => childProcess.spawn(file, args, options),
  parent: process,
  reportError: (message) => process.stderr.write(`${message}\n`)
}

// The signals a terminal or supervisor sends to stop the proxied tool; Windows can only listen to some of them.
export const forwardedSignals = (platform: NodeJS.Platform): ReadonlyArray<NodeJS.Signals> =>
  platform === "win32" ? ["SIGINT", "SIGTERM", "SIGHUP", "SIGBREAK"] : ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"]

// Runs a tool in place of this process, like the upstream typescript and tsrs launchers: process.execve replaces the
// process image when available, otherwise the tool runs as a child that receives this process's signals and whose exit
// code or terminating signal becomes this process's own.
export const execProxyCommand = (command: ProxyCommand, host: ExecHost = nodeExecHost): void => {
  if (host.platform !== "win32" && host.execve !== undefined) {
    try {
      host.execve(command.file, [command.file, ...command.args], command.env)
    } catch {
      // execve may be unavailable or refused; fall back to a child process.
    }
  }

  const signals = forwardedSignals(host.platform)
  const listening: Array<NodeJS.Signals> = []
  let settled = false
  let child: ProxyChildProcess | undefined
  const forward = (signal: NodeJS.Signals) => {
    try {
      child?.kill(signal)
    } catch {
      // The child already exited; its exit is reported separately.
    }
  }
  const settle = (onSettled: () => void) => {
    if (settled) return
    settled = true
    for (const signal of listening) host.parent.off(signal, forward)
    onSettled()
  }
  const fail = (error: unknown) => settle(() => {
    host.reportError(`Failed to run ${command.file}: ${error instanceof Error ? error.message : String(error)}`)
    host.parent.exitCode = 1
  })

  try {
    child = host.spawn(command.file, command.args, { stdio: "inherit", env: command.env })
  } catch (error) {
    fail(error)
    return
  }
  child.on("error", fail)
  child.on("exit", (status, signal) => settle(() => propagateDiagnosticsExit({ status, signal }, host.parent)))
  for (const signal of signals) {
    if (settled) break
    try {
      host.parent.on(signal, forward)
      listening.push(signal)
    } catch {
      // Not every signal can be listened to on every platform.
    }
  }
}
