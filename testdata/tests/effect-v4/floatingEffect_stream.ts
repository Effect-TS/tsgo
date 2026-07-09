import { Effect, Stream } from "effect"

export const shouldWarn = Effect.gen(function*() {
  Stream.succeed(42)
})
