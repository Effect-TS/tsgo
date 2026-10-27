// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import type { Effect } from "effect"
import { map as m, mapError as me, succeed } from "effect/Effect"

declare const fetchCount: Effect.Effect<number, string>

// MUST TRIGGER (no quickfix): only named imports of map and mapError are in scope;
// no binding reaches Effect.mapBoth, and the type-only Effect import is not a value
export const namedOnly = succeed(1).pipe(m((n) => n + 1), me(String))

// MUST TRIGGER (no quickfix): same, on a declared effect
export const namedOnlyReverse = fetchCount.pipe(
  me((message) => message.length),
  m((n) => n > 0)
)

// MUST TRIGGER (no quickfix): a local value named Effect is not the Effect module
export const shadowedByLocal = (() => {
  const Effect = { mapBoth: (options: unknown) => options }
  void Effect
  return fetchCount.pipe(m((n) => n + 1), me((message) => message.length))
})()
