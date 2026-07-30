// PROTOTYPE: synchronous broker for testing the Oxlint/TypeScript-Go integration seam.
import getExePath from "#getExePath"

import { type EffectDiagnostic, type RunEffectDiagnosticsParams, SyncApi } from "../api/sync-api.ts"

interface OxlintContext {
  readonly cwd: string
  readonly filename: string
  readonly sourceCode: { readonly text: string }
  readonly settings: Readonly<Record<string, unknown>>
}

interface Frame {
  readonly cwd: string
  readonly file: string
  readonly text: string
  readonly rules: Set<string>
  readonly overrideEffectOptions?: RunEffectDiagnosticsParams["overrideEffectOptions"]
  exitsRemaining: number
  results?: Map<string, ReadonlyArray<EffectDiagnostic>>
}

let api: SyncApi | undefined
let apiCwd: string | undefined
let frame: Frame | undefined

function getApi(cwd: string): SyncApi {
  if (apiCwd !== cwd) {
    api?.close()
    api = new SyncApi({ cwd, executable: getExePath() })
    apiCwd = cwd
  }
  return api!
}

function computeResults(current: Frame): void {
  const response = getApi(current.cwd).runEffectDiagnostics({
    targetFilePath: current.file,
    overrideSourceText: current.text,
    onlyRules: [...current.rules],
    overrideEffectOptions: current.overrideEffectOptions,
    includeFixes: true,
  })
  const byRule = new Map<string, Array<EffectDiagnostic>>()
  for (const diagnostic of response.diagnostics) {
    const bucket = byRule.get(diagnostic.ruleName) ?? []
    bucket.push(diagnostic)
    byRule.set(diagnostic.ruleName, bucket)
  }
  current.results = byRule
}

export function register(ruleName: string, context: OxlintContext): void {
  const file = context.filename
  if (!frame) {
    frame = {
      cwd: context.cwd,
      file,
      text: context.sourceCode.text,
      rules: new Set(),
      ...(Object.hasOwn(context.settings, "effect-tsgo")
        ? { overrideEffectOptions: context.settings["effect-tsgo"] as RunEffectDiagnosticsParams["overrideEffectOptions"] }
        : {}),
      exitsRemaining: 0,
    }
  } else if (frame.file !== file) {
    throw new Error(`Overlapping Oxlint file frames: ${frame.file} and ${file}`)
  }

  frame.rules.add(ruleName)
  frame.exitsRemaining++
}

export function resultsFor(ruleName: string): ReadonlyArray<EffectDiagnostic> {
  if (!frame) throw new Error("No active Oxlint file frame")
  if (!frame.results) computeResults(frame)
  return frame.results?.get(ruleName) ?? []
}

export function finish(): void {
  if (!frame) return
  frame.exitsRemaining--
  if (frame.exitsRemaining === 0) frame = undefined
}

export function abort(): void {
  frame = undefined
}

export function close(): void {
  api?.close()
  api = undefined
  apiCwd = undefined
}

process.once("exit", close)
