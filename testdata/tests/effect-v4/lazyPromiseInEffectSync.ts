// @effect-diagnostics lazyPromiseInEffectSync:warning
import { Effect, pipe } from "effect"

declare const promiseValue: Promise<number>
declare const thenableValue: { then: (onFulfilled: (value: number) => unknown) => unknown }

// Should trigger - Promise returned from Effect.sync
export const fromPromiseResolve = Effect.sync(() => Promise.resolve(1))

// Should trigger - declared Promise type returned from Effect.sync
export const fromPromiseValue = Effect.sync(() => promiseValue)

// Should NOT trigger - sync value
export const fromValue = Effect.sync(() => 1)

// Should NOT trigger - thenable but not Promise
export const fromThenable = Effect.sync(() => thenableValue)

// Should NOT trigger - already using the async constructor
export const fromEffectPromise = Effect.promise(() => Promise.resolve(1))

// Should trigger - Effect.sync nested inside a pipeline callback
export const fromNestedSync = Effect.succeed(1).pipe(Effect.flatMap(() => Effect.sync(() => promiseValue)))

// Should trigger - point-free Effect.sync applied to a Promise thunk
export const fromPointFree = pipe(() => Promise.resolve(1), Effect.sync)

// Should trigger - point-free Effect.sync after an earlier step
export const fromPointFreeChain = pipe(1, (n) => () => Promise.resolve(n), Effect.sync)

// Should NOT trigger - point-free Effect.sync applied to a sync thunk
export const fromPointFreeValue = pipe(() => 1, Effect.sync)
