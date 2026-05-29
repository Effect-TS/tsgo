import { Effect } from "effect"

export const program = Effect.fail("error").pipe(
  Effect.catch(() => Effect.succeed(42))
)
