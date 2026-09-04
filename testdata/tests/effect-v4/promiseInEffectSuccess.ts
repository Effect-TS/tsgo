// @effect-diagnostics promiseInEffectSuccess:warning

import { Effect } from "effect"
import * as EffectModule from "effect/Effect"

declare const base: Effect.Effect<number>
declare const other: Effect.Effect<string>
declare const callableOther: Effect.Effect<string> & (() => void)
declare const promiseValue: Promise<number>
declare const promiseOrNumber: Promise<number> | number
declare const promiseLikeValue: PromiseLike<number>
declare const thenableValue: { then: (onFulfilled: (value: number) => unknown) => unknown }
declare const promiseCallback: (value: number) => Promise<number>
declare const unionCallback: (value: number) => Promise<number> | number
declare function overloadedCallback(value: string): Promise<string>
declare function overloadedCallback(value: number): number
declare const customPromiseEffect: Effect.Effect<Promise<number>>
declare const makePromiseEffect: () => Effect.Effect<Promise<number>>
declare const preserveEffect: <A, E, R>(effect: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
declare const genericPromiseEffect: <A>(value: A) => Effect.Effect<Promise<number>>
declare const promiseEffectHolder: { readonly current: Effect.Effect<Promise<number>> }
declare const promiseEffects: ReadonlyArray<Effect.Effect<Promise<number>>>
declare const independentPromiseEffect: (ignored: unknown) => Effect.Effect<Promise<number>>

Effect.succeed(promiseValue)
Effect.succeed(promiseOrNumber)
EffectModule.succeed(promiseValue)

base.pipe(Effect.as(promiseValue))
Effect.as(base, promiseValue)

base.pipe(Effect.map(promiseCallback))
Effect.map(base, (value) => Promise.resolve(value))
Effect.map(base, unionCallback)

base.pipe(Effect.zipWith(other, (_left, right) => Promise.resolve(right)))
base.pipe(Effect.zipWith(other, (_left, right) => Promise.resolve(right), { concurrent: true }))
Effect.zipWith(base, other, (_left, right) => Promise.resolve(right))
Effect.zipWith(base, callableOther, (_left, right) => Promise.resolve(right))

customPromiseEffect
makePromiseEffect()
preserveEffect(customPromiseEffect)
genericPromiseEffect<number>(1)
promiseEffectHolder.current
promiseEffects[0]
Effect.map(base, () => {
  Effect.succeed<Promise<number>>(promiseValue)
  return Promise.resolve(1)
})
Effect.map(base, () => {
  Effect.sync(() => promiseValue)
  return Promise.resolve(1)
})
Effect.map(base, () => {
  customPromiseEffect
  return Promise.resolve(1)
})
independentPromiseEffect(Effect.succeed<Promise<number>>(promiseValue))

Effect.succeed(1)
Effect.succeed(promiseLikeValue)
Effect.succeed(thenableValue)
Effect.promise(() => promiseValue)
Effect.sync(() => promiseValue)
base.pipe(Effect.as(1))
base.pipe(Effect.map((value) => value + 1))
base.pipe(Effect.map(async (value) => value + 1))
base.pipe(Effect.zipWith(other, (left, right) => left + right.length))
Effect.map(base, overloadedCallback)

Effect.succeed<Promise<number>>(promiseValue)
Effect.as<Promise<number>>(promiseValue)
Effect.map<number, Promise<number>>(promiseCallback)
base.pipe(Effect.as<Promise<number>>(promiseValue), Effect.as(promiseValue))
base.pipe(Effect.as(promiseValue), Effect.as<Promise<number>>(promiseValue))
Effect.succeed<Promise<number>>(promiseValue).pipe(Effect.map((x) => x))
base.pipe((Effect.as<Promise<number>>(promiseValue)))
Effect.succeed(promiseValue) as Effect.Effect<Promise<number>>
Effect.succeed(promiseValue) satisfies Effect.Effect<Promise<number>>

export const intentionallyCarried: Effect.Effect<Promise<number>> = Effect.succeed(promiseValue)
export const intentionallyReplaced: Effect.Effect<Promise<number>> = base.pipe(Effect.as(promiseValue))
export const intentionallyMapped: Effect.Effect<Promise<number>> = base.pipe(Effect.map(promiseCallback))
export const intentionallyCombined: Effect.Effect<Promise<string>> = base.pipe(
  Effect.zipWith(other, (_left, right) => Promise.resolve(right))
)
export function intentionallyReturned(): Effect.Effect<Promise<number>> {
  return Effect.succeed(promiseValue)
}
export const intentionallyArrow = (): Effect.Effect<Promise<number>> => Effect.succeed(promiseValue)
export const intentionallyContextual: () => Effect.Effect<Promise<number>> = () => Effect.succeed(promiseValue)
export const intentionallyReturnedWithUnrelated: () => Effect.Effect<Promise<number>> = () => {
  customPromiseEffect
  return Effect.succeed(promiseValue)
}

export const inferredReturn = () => Effect.succeed(promiseValue)

const local = {
  succeed: <A>(value: A) => value,
  map: <A, B>(_value: A, f: (value: A) => B) => f(_value)
}

local.succeed(promiseValue)
local.map(1, promiseCallback)
