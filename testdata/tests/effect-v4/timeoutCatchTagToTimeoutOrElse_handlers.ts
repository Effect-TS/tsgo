// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics timeoutCatchTagToTimeoutOrElse:warning
import { Effect, Option, pipe } from "effect"

declare const task: Effect.Effect<string>

// Should trigger: the complete returned flow constructs a successful None.
export const functionPipe = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => pipe(Option.none(), Effect.succeed))
)
export const methodPipe = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Option.none().pipe(Effect.succeed))
)
export const parenthesizedBlock = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => {
    return (pipe((Option.none()), Effect.succeed))
  })
)

// Must not change: additional transformations could have observable effects.
export const transformedSucceedNone = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Effect.succeedNone.pipe(Effect.delay(1)))
)
export const transformedSucceed = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => pipe(Option.none(), Effect.succeed, Effect.delay(1)))
)
export const transformedInput = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => pipe(Option.none(), Option.orElse(() => Option.some("fallback")), Effect.succeed))
)
export const someInput = task.pipe(
  Effect.timeout(1),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => pipe(Option.some("fallback"), Effect.succeed))
)
