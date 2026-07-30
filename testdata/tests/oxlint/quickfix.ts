import * as Effect from "effect/Effect"

export const value = Effect.gen(function*() {
  return yield Effect.succeed(1)
})
