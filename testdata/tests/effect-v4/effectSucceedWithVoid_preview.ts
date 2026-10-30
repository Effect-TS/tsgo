// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithVoid:warning
import { Effect } from "effect"

export const preview = Effect.succeed<void>(void 0)
