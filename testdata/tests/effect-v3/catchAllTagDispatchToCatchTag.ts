// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics catchAllTagDispatchToCatchTag:suggestion
import { Effect } from "effect"

class NotFoundError {
  readonly _tag = "NotFoundError"
}

class TimeoutError {
  readonly _tag = "TimeoutError"
}

declare const program: Effect.Effect<string, NotFoundError | TimeoutError>

export const result = program.pipe(
  Effect.catchAll((error) =>
    error._tag === "NotFoundError"
      ? Effect.succeed("missing")
      : Effect.fail(error)
  )
)
