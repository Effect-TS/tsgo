// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics timeoutCatchTagToTimeoutOrElse:warning
import { Effect } from "effect"

declare const fetchQuote: Effect.Effect<string>

export const quote = fetchQuote.pipe(
  Effect.timeout("5 seconds"),
  Effect.catchTag("TimeoutError", () => Effect.succeed("cached quote"))
)

export const maybeQuote = fetchQuote.pipe(
  Effect.timeout("5 seconds"),
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Effect.succeedNone)
)
