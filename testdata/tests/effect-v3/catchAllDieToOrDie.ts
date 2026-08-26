// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics catchAllDieToOrDie:warning
import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"

declare const failable: Effect.Effect<number, Error>

// Should trigger: direct API reference.
export const pipeDirect = failable.pipe(Effect.catchAll(Effect.die))

// Should trigger: concise and block identity-forwarding functions.
export const pipeArrow = failable.pipe(
  Effect.catchAll((error) => Effect.die(error))
)
export const pipeArrowBlock = failable.pipe(
  Effect.catchAll((error) => {
    return Effect.die(error)
  })
)
export const pipeFunction = failable.pipe(
  Effect.catchAll(function(error) {
    return Effect.die(error)
  })
)

// Should trigger: Function.pipe and both dual application forms.
export const functionPipe = pipe(failable, Effect.catchAll(Effect.die))
export const dataFirst = Effect.catchAll(failable, Effect.die)
export const dataLast = Effect.catchAll(Effect.die)(failable)

// Should trigger: Effect.fn trailing transformations are piping flows too.
export const effectFn = Effect.fn(function*() {
  return yield* failable
}, Effect.catchAll(Effect.die))

// Should not trigger: the failure is transformed before becoming a defect.
export const wrapped = failable.pipe(
  Effect.catchAll((error) => Effect.die(new Error(String(error))))
)

// Should not trigger: this is not identity forwarding.
export const differentValue = failable.pipe(
  Effect.catchAll((_error) => Effect.die("different"))
)

// Should not trigger: Effect.orDie would itself be redundant.
export const unfailable = Effect.succeed(1).pipe(
  Effect.catchAll(Effect.die)
)

// Should not trigger: a local API with the same member names.
const LocalEffect = {
  catchAll: (_handler: (error: Error) => Effect.Effect<never>) =>
    <A, E, R>(self: Effect.Effect<A, E, R>) => self,
  die: Effect.die
}
export const localApi = failable.pipe(LocalEffect.catchAll(LocalEffect.die))
