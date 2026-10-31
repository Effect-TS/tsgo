// @effect-diagnostics *:off
// @effect-diagnostics effectTryWithoutCatch:warning
import { Effect } from "effect"

declare const text: string
export const parsed = Effect.try(() => JSON.parse(text))
