// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:warning
import { Effect } from "effect"

declare const sendRequest: Effect.Effect<void>

export const throttled = Effect.sleep("1 second").pipe(Effect.andThen(sendRequest))
