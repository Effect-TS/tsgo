// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics promiseAllInEffect:warning
import { Effect } from "effect"

declare const tasks: ReadonlyArray<Promise<number>>

// MUST TRIGGER: literal element access calls the same global Promise.all.
export const bracket = Effect.promise(() => Promise["all"](tasks))

// MUST TRIGGER: the global Promise constructor is never nullish.
export const optionalReceiver = Effect.promise(() => Promise?.all(tasks))

// MUST TRIGGER: the global Promise.all method is never nullish.
export const optionalMethod = Effect.promise(() => Promise.all?.(tasks))
