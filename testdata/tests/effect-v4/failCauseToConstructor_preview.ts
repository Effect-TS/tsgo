// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Cause, Effect } from "effect"

export const crashed = Effect.failCause(Cause.die(new Error("boom")))
