// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Cause, Effect, pipe } from "effect"

declare const error: Error
declare const defect: unknown
declare const wrap: (e: Error) => Error

// MUST TRIGGER: Cause.fail and Effect.failCause at the end of a longer flow -> Effect.fail
export const longerPipe = pipe(error, wrap, Cause.fail, Effect.failCause)

// MUST TRIGGER: trailing Effect.fn pipeline steps -> Effect.fail
export const effectFnTrailing = Effect.fn(function*() {
  return yield* Effect.succeed(error)
}, Cause.fail, Effect.failCause)

// MUST TRIGGER: explicit type argument on the lazy inner constructor -> Effect.failSync
export const lazyTypeArgument = Effect.failCauseSync(() => Cause.fail<Error | string>(error))

// MUST TRIGGER: lazy defect -> Effect.dieSync(() => defect)
export const lazyDie = Effect.failCauseSync(() => Cause.die(defect))

// MUST NOT TRIGGER: async thunk
export const asyncThunk = Effect.failCauseSync(async () => Cause.fail(error))

// MUST NOT TRIGGER: generator thunk
export const generatorThunk = Effect.failCauseSync(function*() {
  return Cause.fail(error)
} as any)

// MUST NOT TRIGGER: thunk with a parameter
export const parameterThunk = Effect.failCauseSync(((_?: unknown) => Cause.fail(error)) as () => Cause.Cause<Error>)
