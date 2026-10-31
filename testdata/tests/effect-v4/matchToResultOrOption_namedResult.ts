// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import type { Effect } from "effect"
import { Result } from "effect"
import { match as fold, result as reify } from "effect/Effect"

declare const source: Effect.Effect<number, string>

// MUST TRIGGER: renamed named import of Effect.match; the fix reuses the renamed Effect.result binding
export const value = fold(source, { onFailure: Result.fail, onSuccess: Result.succeed })
export const kept = reify
