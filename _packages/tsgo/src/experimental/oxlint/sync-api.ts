// PROTOTYPE: typed adapter over the copied synchronous child-process channel.
import { SyncChannel } from "./sync-channel.ts"

const protocolVersion = 2

interface WireResponse<T> {
  readonly version: number
  readonly id: number
  readonly result?: T
  readonly error?: { readonly message: string }
}

export interface EffectDiagnostic {
  readonly file: string
  readonly start: number
  readonly end: number
  readonly code: number
  readonly ruleName: string
  readonly message: string
  readonly actions?: ReadonlyArray<EffectCodeAction>
}

export interface EffectCodeAction {
  readonly title: string
  readonly edits: ReadonlyArray<EffectTextEdit>
}

export interface EffectTextEdit {
  readonly start: number
  readonly end: number
  readonly newText: string
}

export interface DiagnosticsParams {
  readonly file: string
  readonly text: string
  readonly project?: string
  readonly rules: ReadonlyArray<string>
  readonly effectOptions?: unknown
  readonly includeFixes?: boolean
}

export interface DiagnosticsResult {
  readonly diagnostics: ReadonlyArray<EffectDiagnostic>
  readonly optionsSource: "settings" | "tsconfig"
}

export class SyncApi {
  readonly #channel: SyncChannel
  #nextID = 1

  constructor(options: { readonly cwd: string; readonly executable: string }) {
    this.#channel = new SyncChannel(options.executable, ["--effect-js-api", "--cwd", options.cwd])
  }

  diagnostics(params: DiagnosticsParams): DiagnosticsResult {
    const id = this.#nextID++
    const response = JSON.parse(this.#channel.request(JSON.stringify({
      version: protocolVersion,
      id,
      method: "diagnostics",
      params,
    }))) as WireResponse<DiagnosticsResult>
    if (response.version !== protocolVersion || response.id !== id) {
      throw new Error(`Invalid Effect JavaScript API response for request ${id}`)
    }
    if (response.error) throw new Error(response.error.message)
    if (!response.result) throw new Error(`Effect JavaScript API request ${id} returned no result`)
    return response.result
  }

  close(): void {
    this.#channel.close()
  }
}
