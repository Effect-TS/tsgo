// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics raceFirstWithSleepToTimeout:warning
import { Effect } from "effect"

declare const request: Effect.Effect<string, Error>

export const result = Effect.raceFirst(
  request,
  Effect.sleep("5 seconds").pipe(
    Effect.andThen(Effect.fail(new Error("request timed out")))
  )
)
