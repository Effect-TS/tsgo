// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics timeoutCatchTagToTimeoutOrElse:warning
import { Cause, Effect } from "effect"

declare const task: Effect.Effect<string>
declare const innerTimeout: Effect.Effect<string, Cause.TimeoutError>

// Both occurrences in the same flow should be reported.
export const repeated = task.pipe(
  Effect.timeout(1),
  Effect.catchTag("TimeoutError", () => Effect.succeed("first")),
  Effect.timeout(2),
  Effect.catchTag("TimeoutError", () => Effect.succeed("second"))
)

// Rejecting the first sequence must not hide later matches. The two remaining
// patterns should be reported in source order, with the Option pattern first.
export const rejectedThenMixed = innerTimeout.pipe(
  Effect.timeout(1),
  Effect.catchTag("TimeoutError", () => Effect.succeed("inner fallback")),
  Effect.timeout(2),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Effect.succeedNone),
  Effect.timeout(3),
  Effect.catchTag("TimeoutError", () => Effect.succeedNone)
)
