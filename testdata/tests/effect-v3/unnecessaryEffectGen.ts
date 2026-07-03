// @effect-v3
import * as Effect from "effect/Effect"

export const shouldNotRant = Effect.gen(function*() {
  yield* Effect.succeed(true)
  return yield* Effect.succeed(42)
})

export const shouldNotRaiseForNonEffect = Effect.gen(function*() {
  return 42
})

const foo = () => Effect.succeed(100)
const bar = (x: number) => Effect.succeed(x + x)

export const shouldNotRaiseForNestedYield = Effect.gen(function*() {
  return yield* bar(yield* foo())
})

export const shouldRaiseForSingle = Effect.gen(function*() {
  return yield* Effect.succeed(42)
})
