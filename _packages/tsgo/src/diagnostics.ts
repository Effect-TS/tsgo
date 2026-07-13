import * as childProcess from "node:child_process"

export type DiagnosticsOutputFormat = "json" | "pretty" | "text" | "github-actions"

export interface DiagnosticsRequest {
  readonly cwd: string
  readonly file?: string
  readonly project?: string
  readonly format: DiagnosticsOutputFormat
  readonly strict: boolean
  readonly severity?: string
  readonly progress: boolean
  readonly lspconfig?: string
}

export interface DiagnosticsProcessResult {
  readonly status: number | null
  readonly signal: NodeJS.Signals | null
  readonly error?: Error
}

export type SpawnDiagnosticsProcess = (
  binaryPath: string,
  argv: ReadonlyArray<string>,
  options: { readonly stdio: "inherit" }
) => DiagnosticsProcessResult

export interface DiagnosticsParentProcess {
  readonly pid: number
  exitCode: number | string | null | undefined
  kill(pid: number, signal: NodeJS.Signals): boolean
}

export const runDiagnosticsBinary = (
  binaryPath: string,
  request: DiagnosticsRequest,
  spawn: SpawnDiagnosticsProcess = childProcess.spawnSync
): DiagnosticsProcessResult => {
  const result = spawn(binaryPath, ["--effect-cli-diagnostics", JSON.stringify(request)], { stdio: "inherit" })
  if (result.error !== undefined) {
    throw result.error
  }
  return result
}

export const propagateDiagnosticsExit = (
  result: DiagnosticsProcessResult,
  parent: DiagnosticsParentProcess = process
): void => {
  if (result.signal !== null) {
    parent.kill(parent.pid, result.signal)
    return
  }
  parent.exitCode = result.status ?? 1
}
