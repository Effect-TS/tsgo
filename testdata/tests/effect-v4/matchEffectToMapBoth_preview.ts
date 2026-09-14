// @effect-diagnostics *:off
import { Effect } from "effect"

declare const program: Effect.Effect<number, string>

program.pipe(Effect.matchEffect({
  onFailure: (error) => Effect.fail(error.length),
  onSuccess: (value) => Effect.succeed(value + 1)
}))
