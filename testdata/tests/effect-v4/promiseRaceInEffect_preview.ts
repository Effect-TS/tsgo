// @effect-diagnostics *:off
// @effect-diagnostics promiseRaceInEffect:warning
import { Effect } from "effect"

declare const mirrors: ReadonlyArray<string>
declare const download: (url: string) => Promise<Uint8Array>

export const preview = Effect.tryPromise(() => Promise.any(mirrors.map((url) => download(url))))
