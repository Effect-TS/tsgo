// @effect-diagnostics *:off
// @effect-diagnostics catchTagToCatchReason:warning
import { Effect } from "effect"

class RetryReason {
  readonly _tag = "RetryReason"
}

class FatalReason {
  readonly _tag = "FatalReason"
}

class AppError {
  readonly _tag = "AppError"
  constructor(readonly reason: RetryReason | FatalReason) {}
}

declare const program: Effect.Effect<string, AppError>

export const fixable = program.pipe(
  Effect.catchTag("AppError", (error) => {
    if (error.reason._tag === "RetryReason") return Effect.succeed("retry")
    return Effect.fail(error)
  })
)
