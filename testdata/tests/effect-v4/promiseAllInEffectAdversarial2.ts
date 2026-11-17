// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics promiseAllInEffect:warning
import { Effect } from "effect"

declare const tasks: ReadonlyArray<Promise<number>> | Array<Promise<string>>

// MUST TRIGGER: both members of this union are variable-length arrays.
export const unionAll = Effect.promise(() => Promise.all(tasks))

// MUST TRIGGER: allSettled has the same unbounded array fan-out.
export const unionSettled = Effect.promise(() => Promise.allSettled(tasks))
