// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Data, Effect } from "effect"

class TaskError extends Data.TaggedError("TaskError")<{
  readonly message: string
}> {}

declare const fetchCount: Effect.Effect<number, string>

// MUST TRIGGER: adjacent Effect.map + Effect.mapError is a single Effect.mapBoth
export const isPositive = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapError((message) => new TaskError({ message }))
)
