// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:suggestion
import { Effect } from "effect"

declare const sendRequest: Effect.Effect<void>

// Negative control: GOOD forms only
export const goodDelayPipe = sendRequest.pipe(Effect.delay("1 second"))
export const goodDelayDataFirst = Effect.delay(sendRequest, "100 millis")

export const goodGenStandalone = Effect.gen(function* () {
  yield* Effect.sleep("1 second")
  return yield* sendRequest
})

export const goodParamUsingCallback = Effect.sleep("1 second").pipe(
  Effect.flatMap((val) => Effect.succeed(val))
)
