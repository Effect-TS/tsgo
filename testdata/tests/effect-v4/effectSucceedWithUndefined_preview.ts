// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:warning
import { Effect } from "effect"

export const preview = Effect.succeed(undefined)
