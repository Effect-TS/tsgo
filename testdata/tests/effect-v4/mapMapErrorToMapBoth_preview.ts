// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:suggestion
import { Effect } from "effect"

declare const fetchCount: Effect.Effect<number, string>

export const preview = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapError((s) => s.length)
)
