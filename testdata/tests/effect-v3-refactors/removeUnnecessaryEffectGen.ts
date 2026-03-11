// refactor: 5:24, 9:24, 15:24, 20:24
// @effect-v3
import * as Effect from "effect/Effect"

export const test1 = Effect.gen(function*() {
  return yield* Effect.succeed(42)
})

export const test2 = Effect.gen(function*() {
  return yield* Effect.gen(function*() {
    return yield* Effect.succeed(42)
  })
})

export const test3 = Effect.gen(function*() {
  yield* Effect.succeed(true)
  return yield* Effect.succeed(42)
})

export const test4 = Effect.gen(function*() {
  yield* Effect.succeed(42)
})
