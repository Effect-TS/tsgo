// @effect-v3
import * as Effect from "effect/Effect"

export const shouldWarn = Effect.gen(function*() {
  return Effect.fail("error")
})
