// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics catchDieToOrDie:warning
import { Effect, pipe } from "effect"

declare const failable: Effect.Effect<number, Error>

// Should trigger: direct API reference.
export const pipeDirect = failable.pipe(Effect.catch(Effect.die))

// Should trigger: concise and block identity-forwarding functions.
export const pipeArrow = failable.pipe(
  Effect.catch((error) => Effect.die(error))
)
export const pipeArrowBlock = failable.pipe(
  Effect.catch((error) => {
    return Effect.die(error)
  })
)
export const pipeFunction = failable.pipe(
  Effect.catch(function(error) {
    return Effect.die(error)
  })
)

// Should trigger: Function.pipe and both dual application forms.
export const functionPipe = pipe(failable, Effect.catch(Effect.die))
export const dataFirst = Effect.catch(failable, Effect.die)
export const dataLast = Effect.catch(Effect.die)(failable)

// Should trigger: Effect.fn trailing transformations are piping flows too.
export const effectFn = Effect.fn(function*() {
  return yield* failable
}, Effect.catch(Effect.die))

// Should not trigger: the failure is transformed before becoming a defect.
export const wrapped = failable.pipe(
  Effect.catch((error) => Effect.die(new Error(String(error))))
)

// Should not trigger: this is not identity forwarding.
export const differentValue = failable.pipe(
  Effect.catch((_error) => Effect.die("different"))
)

// Should not trigger: Effect.orDie would itself be redundant.
export const unfailable = Effect.succeed(1).pipe(
  Effect.catch(Effect.die)
)

// Should not trigger: partial catches are not equivalent to Effect.orDie.
class TaggedError {
  readonly _tag = "TaggedError"
}
declare const tagged: Effect.Effect<number, TaggedError>
export const partial = tagged.pipe(
  Effect.catchTag("TaggedError", Effect.die)
)

// Should not trigger: a local API with the same member names.
const LocalEffect = {
  catch: (_handler: (error: Error) => Effect.Effect<never>) =>
    <A, E, R>(self: Effect.Effect<A, E, R>) => self,
  die: Effect.die
}
export const localApi = failable.pipe(LocalEffect.catch(LocalEffect.die))
