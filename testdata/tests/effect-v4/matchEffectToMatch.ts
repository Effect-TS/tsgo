// @effect-diagnostics matchEffectToMatch:suggestion
// @effect-diagnostics matchEffectToMapBoth:off
// @effect-diagnostics-in-tsgo false
import { Effect, pipe } from "effect"

declare const effect: Effect.Effect<number, string>

export const dataFirst = Effect.matchEffect(effect, {
  onFailure: (error) => Effect.succeed(error.length),
  onSuccess: (value) => Effect.succeed(value + 1)
})

export const pipeable = effect.pipe(Effect.matchCauseEffect({
  onFailure: (cause) => Effect.succeed(String(cause)),
  onSuccess: (value) => pipe(value + 1, Effect.succeed)
}))

export const quotedProperties = effect.pipe(Effect.matchEffect({
  "onFailure": (error) => Effect.succeed(error.length),
  "onSuccess": (value) => Effect.succeed(value + 1)
}))

export const effectful = effect.pipe(Effect.matchEffect({
  onFailure: (error) => Effect.fail(error),
  onSuccess: (value) => Effect.succeed(value + 1)
}))

export const extraLogic = effect.pipe(Effect.matchEffect({
  onFailure: (error) => {
    console.log(error)
    return Effect.succeed(error.length)
  },
  onSuccess: (value) => Effect.succeed(value + 1)
}))

const unrelated = { succeed: <A>(value: A) => Effect.succeed(value) }
export const unrelatedSucceed = effect.pipe(Effect.matchEffect({
  onFailure: (error) => unrelated.succeed(error.length),
  onSuccess: (value) => unrelated.succeed(value + 1)
}))
