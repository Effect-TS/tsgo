// PROTOTYPE: typed adapter over the copied synchronous child-process channel.
import { SyncChannel } from "./sync-channel.ts"

const protocolVersion = 1

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
}

export interface LintParams {
  readonly file: string
  readonly text: string
  readonly project?: string
  readonly rules: ReadonlyArray<string>
  readonly effectOptions?: unknown
}

export interface LintResult {
  readonly diagnostics: ReadonlyArray<EffectDiagnostic>
  readonly optionsSource: "settings" | "tsconfig"
}

export class SyncApi {
  readonly #channel: SyncChannel
  #nextID = 1

  constructor(options: { readonly cwd: string; readonly executable: string }) {
    this.#channel = new SyncChannel(options.executable, ["--effect-oxlint", "--cwd", options.cwd])
  }

  lint(params: LintParams): LintResult {
    const id = this.#nextID++
    const response = JSON.parse(this.#channel.request(JSON.stringify({
      version: protocolVersion,
      id,
      method: "lint",
      params,
    }))) as WireResponse<LintResult>
    if (response.version !== protocolVersion || response.id !== id) {
      throw new Error(`Invalid Effect Oxlint response for request ${id}`)
    }
    if (response.error) throw new Error(response.error.message)
    if (!response.result) throw new Error(`Effect Oxlint request ${id} returned no result`)
    return response.result
  }

  close(): void {
    this.#channel.close()
  }
}
