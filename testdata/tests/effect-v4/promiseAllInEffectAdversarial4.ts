// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics promiseAllInEffect:warning
import { Effect } from "effect"

declare const tasks: ReadonlyArray<Promise<number>> & { readonly batchId: string }

// MUST TRIGGER, no quick fix: adding metadata does not bound an array's length.
export const brandedAll = Effect.promise(() => Promise.all(tasks))

// MUST TRIGGER, no quick fix: this is still an unbounded readonly array.
export const brandedSettled = Effect.promise(() => Promise.allSettled(tasks))
