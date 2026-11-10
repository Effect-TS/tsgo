// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics timeoutCatchTagToTimeoutOrElse:warning
import { Cause, Effect } from "effect"

declare const task: Effect.Effect<string>

// MUST TRIGGER without a fix: Effect.timeoutOrElse<string, never, never, string, never, never>
// can be written by hand, but the fix does not carry type arguments over
export const timeoutTypeArguments = Effect.timeout<string, never, never>(task, 1).pipe(
  Effect.catchTag("TimeoutError", () => Effect.succeed("fallback"))
)

// MUST TRIGGER without a fix: the same for the Option variant
export const timeoutTypeArgumentsOption = Effect.timeout<string, never, never>(task, 1).pipe(
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: type arguments on a later step may have no timeoutOption equivalent
export const asSomeTypeArguments = Effect.asSome<string, Cause.TimeoutError, never>(Effect.timeout(task, 1)).pipe(
  Effect.catchTag("TimeoutError", () => Effect.succeedNone)
)
