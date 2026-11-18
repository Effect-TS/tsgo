// @effect-v3
import { Effect } from "effect"

const first = Effect.succeed(10)
const second = Effect.succeed(20)

// MUST TRIGGER: the generic outer callback still has exactly one plain parameter,
// the inner effect is independent, and its mapper uses the outer result.
// A fix may be skipped if the generic parameter cannot be preserved safely.
export const genericOuter = Effect.flatMap(first, <A extends number>(a: A) =>
  Effect.map(second, (b) => a + b))

// MUST TRIGGER: an inline generic mapper still has one parameter and combines
// both independent Effect results. A fix may be skipped to preserve its binder.
export const genericInner = Effect.flatMap(first, (a) =>
  Effect.map(second, <B extends number>(b: B) => a + b))
