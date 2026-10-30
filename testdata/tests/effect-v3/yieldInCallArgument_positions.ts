// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics yieldInCallArgument:warning
import { Effect } from "effect"

declare const load: Effect.Effect<number>
declare const fetchUser: Effect.Effect<number>
declare const services: { readonly buildReport: Effect.Effect<number>; readonly delete: Effect.Effect<number> }
declare const make: (n: number) => Effect.Effect<number>
declare const cond: boolean
declare const config: { readonly prefix: string }
declare const handlers: Array<(n: number) => string>
declare const store: { a?: { b: { f(n: number): string } } }
declare const counts: Record<string, number>

declare function transform(n: number): string
declare function consume(n: number): void
declare function combine(...values: Array<unknown>): string
declare function curried(n: number): (s: string) => string
declare function lookup(n: number): Record<string, number>
declare function sideEffect(): string
declare function key(): string

// ---------------------------------------------------------------------------
// Reported, with quick fix
// ---------------------------------------------------------------------------

export const nestedYields = Effect.gen(function*() {
  // MUST TRIGGER twice (fix each): the outer and the inner yield* are hoisted independently
  return transform(yield* make(yield* load))
})

export const verbNames = Effect.gen(function*() {
  // MUST TRIGGER (fix): `fetchUser` -> `fetchedUser`
  consume(yield* fetchUser)
  // MUST TRIGGER (fix): `services.buildReport` -> `builtReport`
  consume(yield* services.buildReport)
  // MUST TRIGGER (fix): `delete` is a reserved word, falls back to `value`
  consume(yield* services.delete)
  // MUST TRIGGER (fix): the name comes from the expression inside `as`, `loaded`
  consume(yield* (load as Effect.Effect<number>))
})

export const literalPrecedingArgs = Effect.gen(function*() {
  // MUST TRIGGER (fix): negative numbers, bigints, templates and regexes are side-effect free
  return combine(-1, 1n, `t`, /r/, config.prefix, yield* load)
})

export const parenthesizedStatement = Effect.gen(function*() {
  // MUST TRIGGER (fix): parenthesized argument of an expression statement
  consume((yield* load))
})

export const calledResult = Effect.gen(function*() {
  // MUST TRIGGER (fix): the call holding the yield* is the callee, `sideEffect()` runs after it
  return curried(yield* load)(sideEffect())
})

export const indexedResult = Effect.gen(function*() {
  // MUST TRIGGER (fix): the call holding the yield* is the indexed object, `key()` runs after it
  return lookup(yield* load)[key()]
})

export const leftOperand = Effect.gen(function*() {
  // MUST TRIGGER (fix): the left operand runs first, `sideEffect()` runs after it
  return transform(yield* load) + sideEffect()
})

export const assertedResult = Effect.gen(function*() {
  // MUST TRIGGER (fix): `as` and `!` around the call are transparent
  return (transform(yield* load) as string)!
})

export const conditionalCondition = Effect.gen(function*() {
  // MUST TRIGGER (fix): the condition of a ternary always runs, before either branch
  return transform(yield* load) ? sideEffect() : key()
})

// ---------------------------------------------------------------------------
// Reported, without quick fix
// ---------------------------------------------------------------------------

export const elementAccessCallee = Effect.gen(function*() {
  // MUST TRIGGER (no fix): the element access `handlers[0]` is not a side-effect free callee
  return handlers[0](yield* load)
})

export const optionalChainCallee = Effect.gen(function*() {
  // MUST TRIGGER (no fix): with `?.` earlier in the callee the argument only runs when `store.a` is defined
  return store.a?.b.f(yield* load)
})

export const computedAssignmentTarget = Effect.gen(function*() {
  // MUST TRIGGER (no fix): `key()` in the assignment target runs before the effect
  counts[key()] = transform(yield* load).length
})

export const orAssignment = Effect.gen(function*() {
  let s = ""
  // MUST TRIGGER (no fix): `||=` only evaluates its right side when `s` is falsy
  s ||= transform(yield* load)
  return s
})

export const ternaryElseBranch = Effect.gen(function*() {
  // MUST TRIGGER (no fix): the else branch of a ternary only runs when `cond` is false
  return cond ? "" : transform(yield* load)
})

export const incrementOperand = Effect.gen(function*() {
  // MUST TRIGGER (no fix): `++` is not an allowed position
  return ++counts[transform(yield* load)]
})

export const arrayElement = Effect.gen(function*() {
  // MUST TRIGGER (no fix): an array literal is not an allowed position
  return [transform(yield* load)]
})

export const objectProperty = Effect.gen(function*() {
  // MUST TRIGGER (no fix): an object literal is not an allowed position
  return { a: transform(yield* load) }
})

export const labeledStatement = Effect.gen(function*() {
  // MUST TRIGGER (no fix): the enclosing statement is the body of a label
  label: consume(yield* load)
})
