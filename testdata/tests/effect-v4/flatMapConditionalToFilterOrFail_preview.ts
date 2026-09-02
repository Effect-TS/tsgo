import { Effect } from "effect"

declare const program: Effect.Effect<number>

program.pipe(
  Effect.flatMap((value) =>
    value > 0 ? Effect.succeed(value) : Effect.fail("non-positive")
  )
)
