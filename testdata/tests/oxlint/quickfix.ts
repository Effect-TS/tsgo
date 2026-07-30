import * as Effect from "effect/Effect"

const marker = "😀"

export const value = Effect.gen(function*() {
  return yield Effect.succeed(1)
})

void marker
