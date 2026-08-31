import { Data, Effect, pipe } from "effect"

class InvalidValue extends Data.TaggedError("InvalidValue")<{ readonly value: number }> {}

declare const source: Effect.Effect<number>
declare const fallback: (value: number) => Effect.Effect<number, InvalidValue>

// Should trigger filterOrFail: pipeable conditional expression.
export const pipeableFail = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value) : Effect.fail(new InvalidValue({ value })))
)

// Should trigger filterOrElse and negate the predicate: identity is the false arm.
export const invertedElse = pipe(
  source,
  Effect.flatMap((value) => value <= 0 ? fallback(value) : Effect.succeed(value))
)

// Should trigger filterOrFail: block-bodied if/else.
export const blockFail = source.pipe(
  Effect.flatMap(function(value) {
    if (value > 0) {
      return Effect.succeed(value)
    } else {
      return Effect.fail(new InvalidValue({ value }))
    }
  })
)

// Should trigger filterOrElse: data-first and defect fallback.
export const dataFirst = Effect.flatMap(
  source,
  (value) => value > 0 ? Effect.succeed(value) : Effect.die("invalid")
)

// Should trigger filterOrElse: v4 tagged errors are yieldable Effect values.
export const yieldableTaggedError = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value) : new InvalidValue({ value }))
)

// Should trigger filterOrFail: direct curried data-last form.
export const dataLast = Effect.flatMap(
  (value: number) => value > 0 ? Effect.succeed(value) : Effect.fail(new InvalidValue({ value }))
)(source)

const flatMapAlias = Effect.flatMap

// Should trigger through API-reference symbol identity (without a quick fix).
export const aliasedFlatMap = source.pipe(
  flatMapAlias((value) => value > 0 ? Effect.succeed(value) : Effect.fail(new InvalidValue({ value })))
)

// Should NOT trigger: the success arm transforms the callback parameter.
export const transformedSuccess = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value + 1) : Effect.fail(new InvalidValue({ value })))
)

// Should NOT trigger: the success arm refers to a shadowed value, not the callback parameter.
const other = 1
export const differentIdentity = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(other) : Effect.fail(new InvalidValue({ value })))
)

// Should NOT trigger: both branches are identity succeeds.
export const bothSucceed = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value) : Effect.succeed(value))
)

// Should NOT trigger: the other branch is not known to be Effect-typed.
declare const unknownFallback: (value: number) => any
export const nonEffectFallback = source.pipe(
  Effect.flatMap((value) => value > 0 ? Effect.succeed(value) : unknownFallback(value))
)

// Should NOT trigger: unrelated APIs with the same property names.
const unrelated = {
  flatMap: <A, B>(self: A, f: (value: A) => B): B => f(self),
  succeed: <A>(value: A): A => value,
  fail: <E>(error: E): E => error
}
export const unrelatedCall = unrelated.flatMap(1, (value) =>
  value > 0 ? unrelated.succeed(value) : unrelated.fail("invalid")
)
