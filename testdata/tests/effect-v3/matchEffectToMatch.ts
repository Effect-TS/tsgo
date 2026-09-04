// @effect-diagnostics matchEffectToMatch:suggestion
// @effect-diagnostics-in-tsgo false
import { Effect } from "effect"

declare const effect: Effect.Effect<number, string>

export const pipeable = effect.pipe(Effect.matchEffect({
  onFailure: (error) => Effect.succeed(error.length),
  onSuccess: (value) => Effect.succeed(value + 1)
}))
