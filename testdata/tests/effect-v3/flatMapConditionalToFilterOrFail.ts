// @effect-v3
import { Data, Effect, pipe } from "effect"

class InvalidValue extends Data.TaggedError("InvalidValue")<{ readonly value: number }> {}

declare const source: Effect.Effect<number>
declare const fallback: (value: number) => Effect.Effect<number, InvalidValue>

export const pipeableFail = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value) : Effect.fail(new InvalidValue({ value })))
)

export const invertedElse = pipe(
  source,
  Effect.flatMap((value) => value <= 0 ? fallback(value) : Effect.succeed(value))
)

export const blockFail = source.pipe(
  Effect.flatMap(function(value) {
    if (value > 0) return Effect.succeed(value)
    return Effect.fail(new InvalidValue({ value }))
  })
)

export const dataFirst = Effect.flatMap(
  source,
  (value) => value > 0 ? Effect.succeed(value) : Effect.die("invalid")
)

export const transformedSuccess = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value + 1) : Effect.fail(new InvalidValue({ value })))
)
