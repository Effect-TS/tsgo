// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:warning
import { Effect } from "effect"

declare const program: Effect.Effect<number, string>
declare const next: Effect.Effect<string>
declare const log: (value: number) => Effect.Effect<void>

// PIPINGS: a parenthesized curried application is a single `call` step on `program`,
// with `next` as the factory argument
export const parenthesizedCurried = ((Effect.andThen(next)))(program)
export const curried = Effect.andThen(next)(program)

// PIPINGS: Effect v3 andThen and tap return conditional types whose `infer` binders each
// overload declares separately; the data-first calls are still single `dataFirst` steps
// on `program`, like on Effect v4
export const dataFirstAndThen = Effect.andThen(program, next)
export const dataFirstAndThenFunction = Effect.andThen(program, (n) => log(n))
export const dataFirstTap = Effect.tap(program, (n) => log(n))

interface Box<A, E> {
  readonly a: A
  readonly e: E
}
declare const box: Box<number, string>

declare function same<A, X>(
  f: (a: A) => X
): <E>(self: Box<A, E>) => [X] extends [Box<infer A1, infer E1>] ? Box<A1, E | E1> : Box<X, E>
declare function same<A, E, X>(
  self: Box<A, E>,
  f: (a: A) => X
): [X] extends [Box<infer A1, infer E1>] ? Box<A1, E | E1> : Box<X, E>

declare function swapped<A, X>(
  f: (a: A) => X
): <E>(self: Box<A, E>) => [X] extends [Box<infer A1, infer E1>] ? Box<A1, E | E1> : Box<X, E>
declare function swapped<A, E, X>(
  self: Box<A, E>,
  f: (a: A) => X
): [X] extends [Box<infer A1, infer E1>] ? Box<E1, E | A1> : Box<X, E>

// PIPINGS: overloads whose infer binders correspond by position form a `dataFirst` step on `box`
export const sameBinders = same(box, (a) => a)

// PIPINGS: no flow, the data-first overload uses its infer binders differently from the
// pipeable one, so it is not its data-first counterpart
export const swappedBinders = swapped(box, (a) => a)
