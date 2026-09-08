// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:suggestion
import { Effect } from "effect"

declare const sendRequest: Effect.Effect<void>

export const throttled = Effect.sleep("1 second").pipe(
  Effect.andThen(sendRequest)
)
