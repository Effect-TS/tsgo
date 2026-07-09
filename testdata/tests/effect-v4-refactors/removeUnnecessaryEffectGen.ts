// refactor: 4:24, 8:24, 14:24, 19:24
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
