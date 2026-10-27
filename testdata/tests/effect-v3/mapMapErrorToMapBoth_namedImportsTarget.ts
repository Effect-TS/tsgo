// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import type { Effect } from "effect"
import { map as m, mapBoth as both, mapError as me } from "effect/Effect"

declare const fetchCount: Effect.Effect<number, string>

// MUST TRIGGER: the quickfix uses the imported mapBoth binding `both`
export const namedWithTarget = fetchCount.pipe(m((n) => n + 1), me((message) => message.length))

// MUST NOT TRIGGER: already merged through the imported binding
export const alreadyMerged = fetchCount.pipe(both({ onSuccess: (n) => n + 1, onFailure: (message) => message.length }))
