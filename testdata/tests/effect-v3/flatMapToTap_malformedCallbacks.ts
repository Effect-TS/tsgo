// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics flatMapToTap:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number>
declare const inner: unknown
declare const loose: any

// MUST TRIGGER: the well-formed callback
export const valid = source.pipe(Effect.flatMap((n) => Effect.as(Effect.log(n), n)))

// MUST NOT TRIGGER: async callback
// @ts-expect-error
export const asyncCallback = source.pipe(Effect.flatMap(async (n) => Effect.as(Effect.log(n), n)))

// MUST NOT TRIGGER: generator callback
// @ts-expect-error
export const generatorCallback = source.pipe(Effect.flatMap(function*(n) { return Effect.as(Effect.log(n), n) }))

// MUST NOT TRIGGER: callback with a second parameter
// @ts-expect-error
export const twoParameters = source.pipe(Effect.flatMap((n, extra) => Effect.as(Effect.log(n), n)))

// MUST NOT TRIGGER: generic callback
export const genericCallback = source.pipe(Effect.flatMap(<A>(n: A) => Effect.as(Effect.log(n), n)))

// MUST NOT TRIGGER: the inner value is not an Effect
// @ts-expect-error
export const unknownInner = source.pipe(Effect.flatMap((n) => Effect.as(inner, n)))

// MUST NOT TRIGGER: the inner value is any
export const looseInner = source.pipe(Effect.flatMap((n) => loose.pipe(Effect.as(n))))

// MUST NOT TRIGGER: the outer value is any
export const looseOuter = loose.pipe(Effect.flatMap((n: number) => Effect.as(Effect.log(n), n)))
