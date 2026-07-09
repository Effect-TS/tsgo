// @effect-v3
import * as Effect from "effect/Effect"

export const program = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.succeed(42))
)
