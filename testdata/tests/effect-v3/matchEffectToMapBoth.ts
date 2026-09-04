// @effect-diagnostics matchEffectToMapBoth:suggestion
// @effect-diagnostics-in-tsgo false
import { Effect } from "effect"

declare const effect: Effect.Effect<number, string>

export const pipeable = effect.pipe(Effect.matchEffect({
  onFailure: (error) => Effect.fail(error.length),
  onSuccess: (value) => Effect.succeed(value + 1)
}))
