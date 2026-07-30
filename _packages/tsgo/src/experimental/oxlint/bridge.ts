// PROTOTYPE: synchronous broker for testing the Oxlint/TypeScript-Go integration seam.
import { fileURLToPath } from "node:url"

import { type EffectDiagnostic, SyncApi } from "./sync-api.ts"

const root = fileURLToPath(new URL("../../../../../", import.meta.url))
const tsgo = fileURLToPath(new URL("../../../../../tsgo", import.meta.url))
const traceEnabled = process.env.EFFECT_TSGO_BRIDGE_TRACE !== "0"

interface OxlintContext {
  readonly filename: string
  readonly sourceCode: { readonly text: string }
  readonly settings: Readonly<Record<string, unknown>>
}

interface Frame {
  readonly file: string
  readonly text: string
  readonly rules: Set<string>
  readonly effectOptions?: unknown
  exitsRemaining: number
  results?: Map<string, ReadonlyArray<EffectDiagnostic>>
}

let api: SyncApi | undefined
let frame: Frame | undefined

function trace(event: string, state: object): void {
  if (traceEnabled) console.error(`[effect-tsgo bridge prototype] ${event} ${JSON.stringify(state)}`)
}

function getApi(): SyncApi {
  if (!api) {
    api = new SyncApi({ cwd: root, executable: tsgo })
    trace("client-started", {
      cwd: root,
      executable: tsgo,
      transport: process.platform === "win32" ? "sync-content-length-json-named-pipe" : "sync-content-length-json-stdio",
    })
  }
  return api
}

function computeResults(current: Frame): void {
  const start = performance.now()
  const api = getApi()
  const requestsBefore = api.requestCount
  const response = api.diagnostics({
    file: current.file,
    text: current.text,
    rules: [...current.rules],
    effectOptions: current.effectOptions,
    includeFixes: true,
  })
  const byRule = new Map<string, Array<EffectDiagnostic>>()
  for (const diagnostic of response.diagnostics) {
    const bucket = byRule.get(diagnostic.ruleName) ?? []
    bucket.push(diagnostic)
    byRule.set(diagnostic.ruleName, bucket)
  }
  current.results = byRule
  trace("file-checked", {
    file: current.file,
    enabledRules: [...current.rules],
    optionsSource: response.optionsSource,
    diagnosticsRequests: api.requestCount - requestsBefore,
    effectDiagnostics: response.diagnostics.length,
    elapsedMs: Number((performance.now() - start).toFixed(2)),
  })
}

export function register(ruleName: string, context: OxlintContext): void {
  const file = context.filename
  if (!frame) {
    frame = {
      file,
      text: context.sourceCode.text,
      rules: new Set(),
      ...(Object.hasOwn(context.settings, "effect-tsgo")
        ? { effectOptions: context.settings["effect-tsgo"] }
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
}

process.once("exit", close)
