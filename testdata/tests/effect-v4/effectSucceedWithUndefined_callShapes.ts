// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:suggestion
import { Effect, pipe } from "effect"

// MUST TRIGGER: data-first argument -> Effect.andThen(Effect.void, Effect.undefined)
export const dataFirst = Effect.andThen(Effect.void, Effect.succeed(undefined))

// MUST TRIGGER: pipe subject -> pipe(Effect.undefined, Effect.as(1))
export const pipeSubject = pipe(Effect.succeed(undefined), Effect.as(1))

// MUST TRIGGER: inside an Effect.fn body -> Effect.undefined
export const fn = Effect.fn(function*() { return yield* Effect.succeed(undefined) })

// MUST TRIGGER: inside an Effect.fnUntraced body -> Effect.undefined
export const fnUntraced = Effect.fnUntraced(function*() { return yield* Effect.succeed(undefined) })

// MUST TRIGGER: point-free pipe as a data-first argument -> Effect.undefined
export const pointFreeArgument = Effect.andThen(Effect.void, pipe(undefined, Effect.succeed))

// MUST NOT TRIGGER: an undefined-typed parameter named `undefined` still shadows the global
export const shadowedUndefinedType = (undefined: undefined) => Effect.succeed(undefined)
