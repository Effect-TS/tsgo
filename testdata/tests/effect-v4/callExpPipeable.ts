import * as Effect from "effect/Effect"
import * as Runtime from "effect/Runtime"

export const result1 = Effect.gen(function*() {
  yield* Effect.as(42)(Effect.log("Hello, world!"))

  yield* Effect.as(42)(Effect.map((x: string) => x.length)(Effect.succeed("Hello, world!")))
})
