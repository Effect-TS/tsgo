// @effect-diagnostics *:off
// @effect-diagnostics catchIfTagToCatchTag:suggestion
import { Effect } from "effect"

class NotFoundError {
  readonly _tag = "NotFoundError"
}

class TimeoutError {
  readonly _tag = "TimeoutError"
}

declare const program: Effect.Effect<string, NotFoundError | TimeoutError>

export const recovered = program.pipe(
  Effect.catchIf(
    (error) => error._tag === "NotFoundError",
    () => Effect.succeed("missing")
  )
)

// Should trigger with a fallback fix: catchIf is hidden behind a const alias.
const catchIfAlias = Effect.catchIf
export const recoveredAlias = program.pipe(
  catchIfAlias((error) => error._tag === "NotFoundError", () => Effect.succeed("missing"))
)
