// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import { Effect, Result } from "effect"

declare const source: Effect.Effect<number, string>

// MUST NOT TRIGGER: disabled for the next line only
// @effect-diagnostics-next-line matchToResultOrOption:off
export const disabledLine = Effect.match(source, { onFailure: Result.fail, onSuccess: Result.succeed })

// MUST TRIGGER: the next-line directive above does not reach this fold
export const afterDisabledLine = Effect.match(source, { onFailure: Result.fail, onSuccess: Result.succeed })

// @effect-diagnostics matchToResultOrOption:off
// MUST NOT TRIGGER: turned off from here on by the directive above
export const disabledSection = Effect.match(source, { onFailure: Result.fail, onSuccess: Result.succeed })

// @effect-diagnostics matchToResultOrOption:error
// MUST TRIGGER: turned back on as an error by the directive above
export const errorSection = Effect.match(source, { onFailure: Result.fail, onSuccess: Result.succeed })
