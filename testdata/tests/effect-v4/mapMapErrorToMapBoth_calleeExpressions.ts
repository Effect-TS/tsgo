// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number, string>
declare const getEffect: () => typeof Effect

// MUST TRIGGER (no quickfix): the first callee calls getEffect(); removing the step would drop that call
export const computedFirstCallee = source.pipe(
  getEffect().map((n) => n + 1),
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER (no quickfix): the second callee calls getEffect(); replacing it would drop that call
export const computedSecondCallee = source.pipe(
  Effect.map((n) => n + 1),
  getEffect().mapError((message) => message.length)
)

// MUST TRIGGER (no quickfix): both callees call getEffect()
export const computedBothCallees = getEffect().mapError(
  getEffect().map(source, (n) => n + 1),
  (message) => message.length
)

// MUST NOT TRIGGER: an element-access callee is not resolved as an Effect API, so the pair
// is not detected (and it would not be an entity name for the quickfix either)
export const elementAccessCallee = source.pipe(
  Effect["map"]((n) => n + 1),
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: identifier and dotted-name callees; the quickfix is offered
export const entityNameCallees = source.pipe(
  Effect.map((n) => n + 1),
  Effect.mapError((message) => message.length)
)
