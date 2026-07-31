// PROTOTYPE: typed adapter over the copied synchronous child-process channel.
import {
  type CodeAction,
  type Diagnostic,
  type Method,
  type MethodMap,
  protocolVersion,
  type RunEffectDiagnosticsParams,
  type RunEffectDiagnosticsResult,
  type TextEdit,
  type WireResponse,
} from "./protocol.generated.ts"
import { SyncChannel } from "./sync-channel.ts"

export type { RunEffectDiagnosticsParams, RunEffectDiagnosticsResult }
export type EffectDiagnostic = Diagnostic
export type EffectCodeAction = CodeAction
export type EffectTextEdit = TextEdit

export class SyncApi {
  readonly #channel: SyncChannel
  #nextID = 1

  constructor(options: { readonly cwd: string; readonly executable: string }) {
    this.#channel = new SyncChannel(options.executable, ["--effect-js-api", "--cwd", options.cwd])
  }

  runEffectDiagnostics(params: RunEffectDiagnosticsParams): RunEffectDiagnosticsResult {
    return this.#request("runEffectDiagnostics", params)
  }

  #request<M extends Method>(method: M, params: MethodMap[M]["params"]): MethodMap[M]["result"] {
    const id = this.#nextID++
    const response = JSON.parse(this.#channel.request(JSON.stringify({
      version: protocolVersion,
      id,
      method,
      params,
    }))) as WireResponse<M>
    if (response.version !== protocolVersion || response.id !== id) {
      throw new Error(`Invalid Effect JavaScript API response for request ${id}`)
    }
    if (response.error) throw new Error(response.error.message)
    if (response.result == null) throw new Error(`Effect JavaScript API request ${id} returned no result`)
    return response.result
  }

  close(): void {
    this.#channel.close()
  }
}
