// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics missingConcurrencyOption:warning
import { Effect, Option, pipe } from "effect"
import * as Eff from "effect/Effect"
import { all as allNamed, forEach as forEachNamed } from "effect/Effect"

declare const items: ReadonlyArray<number>
declare const save: (n: number) => Effect.Effect<void>
declare const check: Effect.Effect<boolean>
declare const checks: ReadonlyArray<Effect.Effect<boolean>>
declare const limit: number
declare const runtimeOptions: { readonly concurrency?: number; readonly discard?: false }
declare const discardOnly: { readonly discard: true }

// ---------------------------------------------------------------------------
// MUST TRIGGER: Effect.forEach without an options argument.
// ---------------------------------------------------------------------------

// MUST TRIGGER: data-first, no options (quick fix appends `{ concurrency: 1 }`).
export const forEachNoOptions = Effect.forEach(items, save)

// MUST TRIGGER: data-last inside pipe, no options (quick fix appends `{ concurrency: 1 }`).
export const forEachDataLastPipe = pipe(items, Effect.forEach(save))

// MUST TRIGGER: data-last inside a pipeable chain.
export const forEachDataLastPipeable = Effect.succeed(items).pipe(
  Effect.flatMap(Effect.forEach(save))
)

// MUST TRIGGER: data-last applied immediately (curried call).
export const forEachCurried = Effect.forEach(save)(items)

// MUST TRIGGER: inline callback.
export const forEachInlineCallback = Effect.forEach(items, (n) => Effect.log(n))

// ---------------------------------------------------------------------------
// MUST TRIGGER: Effect.forEach with an object literal options lacking `concurrency`.
// ---------------------------------------------------------------------------

// MUST TRIGGER: only `discard` (quick fix inserts `concurrency: 1` into the literal).
export const forEachDiscardOnly = Effect.forEach(items, save, { discard: true })

// MUST TRIGGER: empty object literal.
export const forEachEmptyOptions = Effect.forEach(items, save, {})

// MUST TRIGGER: multi-line object literal without `concurrency`.
export const forEachMultiLineOptions = Effect.forEach(items, save, {
  discard: true
})

// MUST TRIGGER: data-last with an object literal lacking `concurrency`.
export const forEachDataLastDiscard = pipe(items, Effect.forEach(save, { discard: true }))

// MUST TRIGGER: parenthesized object literal is still an object literal.
export const forEachParenthesizedOptions = Effect.forEach(items, save, ({ discard: true }))

// MUST TRIGGER: a key that only looks like `concurrency` is not `concurrency`.
// @ts-expect-error `concurrent` is not a valid option
export const forEachTypoKey = Effect.forEach(items, save, { concurrent: 4 })

// ---------------------------------------------------------------------------
// MUST TRIGGER: Effect.all without an options argument / without `concurrency`.
// ---------------------------------------------------------------------------

// MUST TRIGGER: iterable form, no options.
export const allTupleNoOptions = Effect.all([check, check])

// MUST TRIGGER: array variable, no options.
export const allArrayNoOptions = Effect.all(checks)

// MUST TRIGGER: struct form, no options.
export const allStructNoOptions = Effect.all({ a: check, b: check })

// MUST TRIGGER: object literal options without `concurrency`.
export const allDiscardOnly = Effect.all(checks, { discard: true })

// MUST TRIGGER: `mode` without `concurrency`.
export const allResultMode = Effect.all(checks, { mode: "either" })

// MUST TRIGGER: first step of a longer flow; the fix keeps the `.pipe(...)` that follows.
export const allThenMap = Effect.all(checks).pipe(Effect.map((xs) => xs.length))

// ---------------------------------------------------------------------------
// MUST TRIGGER: import and alias spellings of the same APIs.
// ---------------------------------------------------------------------------

// MUST TRIGGER: namespace import from "effect/Effect".
export const namespaceForEach = Eff.forEach(items, save)

// MUST TRIGGER: named imports from "effect/Effect".
export const namedForEach = forEachNamed(items, save)
export const namedAll = allNamed(checks)

// MUST TRIGGER: const alias of Effect.all / Effect.forEach.
const allAlias = Effect.all
const forEachAlias = Effect.forEach
export const aliasedAll = allAlias(checks)
export const aliasedForEach = forEachAlias(items, save)

// ---------------------------------------------------------------------------
// MUST TRIGGER: inside generators (the issue's example).
// ---------------------------------------------------------------------------

export const inGenerator = Effect.gen(function*() {
  // MUST TRIGGER
  yield* Effect.forEach(items, save)
  // MUST TRIGGER
  const results = yield* Effect.all(checks)
  // MUST NOT TRIGGER: sequential intent stated explicitly.
  yield* Effect.forEach(items, save, { concurrency: 1 })
  // MUST NOT TRIGGER: explicit bounded concurrency.
  yield* Effect.all(checks, { concurrency: 4 })
  return results
})

export const inEffectFn = Effect.fn("inEffectFn")(function*() {
  // MUST TRIGGER
  yield* Effect.forEach(items, save)
  // MUST TRIGGER
  yield* Effect.all(checks, { discard: true })
  // MUST NOT TRIGGER: explicit concurrency.
  yield* Effect.forEach(items, save, { concurrency: 2 })
})

// MUST TRIGGER: nested call inside another Effect.forEach callback; both calls are reported.
export const nested = Effect.forEach(items, (n) => Effect.all([save(n), save(n + 1)]))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER: an explicit `concurrency` key in an object literal.
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: sequential intent, numeric.
export const forEachSequential = Effect.forEach(items, save, { concurrency: 1 })

// MUST NOT TRIGGER: bounded.
export const forEachBounded = Effect.forEach(items, save, { concurrency: 4, discard: true })

// MUST NOT TRIGGER: "unbounded" is explicit (the rule is about intent, not the value).
export const forEachUnbounded = Effect.forEach(items, save, { concurrency: "unbounded" })

// MUST NOT TRIGGER: value from a variable.
export const allFromVariable = Effect.all(checks, { concurrency: limit })

// MUST NOT TRIGGER: shorthand property.
const concurrency = 2
export const forEachShorthand = Effect.forEach(items, save, { concurrency })

// MUST NOT TRIGGER: quoted key.
export const forEachQuotedKey = Effect.forEach(items, save, { "concurrency": 2 })

// MUST NOT TRIGGER: computed keys that name `concurrency`, statically or through a constant.
export const forEachComputedTemplateKey = Effect.forEach(items, save, { [`concurrency`]: 2 })
const concurrencyKey = "concurrency"
export const allComputedConstantKey = Effect.all(checks, { [concurrencyKey]: 2 })

// MUST NOT TRIGGER: key present with `undefined`; the key is written, so intent is stated.
export const forEachExplicitUndefined = Effect.forEach(items, save, { concurrency: undefined })

// MUST NOT TRIGGER: `as const` around a literal that has `concurrency`.
export const allAsConst = Effect.all(checks, { concurrency: 2 } as const)

// MUST NOT TRIGGER: data-last with explicit concurrency.
export const forEachDataLastExplicit = pipe(items, Effect.forEach(save, { concurrency: 2 }))
export const forEachCurriedExplicit = Effect.forEach(save, { concurrency: 1 })(items)

// MUST NOT TRIGGER: the issue's examples.
export const issueForEach = Effect.forEach(items, save, { concurrency: 1 })
export const issueAll = Effect.all(checks, { concurrency: 4 })

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER: options argument is present but not an object literal.
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: identifier (may or may not carry concurrency; not inspected).
export const forEachOptionsVariable = Effect.forEach(items, save, runtimeOptions)

// MUST NOT TRIGGER: identifier whose type has no concurrency is still not an object literal.
export const forEachDiscardVariable = Effect.forEach(items, save, discardOnly)

// MUST NOT TRIGGER: conditional expression.
declare const parallel: boolean
export const allConditionalOptions = Effect.all(
  checks,
  parallel ? { concurrency: "unbounded" } : { concurrency: 1 }
)

// MUST NOT TRIGGER: object literal with a spread may supply `concurrency`; not inspected.
export const forEachSpreadOptions = Effect.forEach(items, save, { ...runtimeOptions })
export const forEachSpreadDiscard = Effect.forEach(items, save, { ...discardOnly, discard: true })

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER: near misses that are not a call to Effect.all / Effect.forEach.
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: data-last Effect.all as a bare reference; there is no call and no
// options argument to add one to.
export const allBareReference = pipe(checks, Effect.all)

// MUST NOT TRIGGER: other concurrency-aware Effect APIs are out of scope.
export const partitioned = Effect.partition(items, save)
export const filtered = Effect.filter(items, (n) => Effect.succeed(n > 0))

// MUST NOT TRIGGER: same-named API in another Effect module.
export const optionAll = Option.all([Option.some(1), Option.some(2)])

// MUST NOT TRIGGER: native Array#forEach and Promise.all.
items.forEach((n) => n + 1)
export const promises = Promise.all([Promise.resolve(1)])

// MUST NOT TRIGGER: local functions that happen to be named all / forEach.
const local = {
  all: <A>(values: ReadonlyArray<A>) => values,
  forEach: <A>(values: ReadonlyArray<A>, f: (a: A) => void) => values.forEach(f)
}
export const localAll = local.all(items)
local.forEach(items, (n) => n)

// MUST NOT TRIGGER: a reference (not a call) passed around.
export const forEachReference = Effect.forEach

// ---------------------------------------------------------------------------
// Effect v3 only.
// ---------------------------------------------------------------------------

// MUST TRIGGER: v3-only options (`batching`, `concurrentFinalizers`) without `concurrency`.
export const forEachBatchingOnly = Effect.forEach(items, save, { batching: true })
export const allConcurrentFinalizersOnly = Effect.all(checks, { concurrentFinalizers: true })

// MUST TRIGGER: v3 `mode: "validate"` without `concurrency`.
export const allValidateMode = Effect.all(checks, { mode: "validate" })

// MUST NOT TRIGGER: "inherit" is an explicit concurrency value in v3.
export const allInherit = Effect.all(checks, { concurrency: "inherit" })
export const forEachInherit = Effect.forEach(items, save, { concurrency: "inherit", batching: true })

// MUST NOT TRIGGER: Effect.allWith is a different API (data-last options factory); out of scope.
export const allWithNoConcurrency = pipe(checks, Effect.allWith({ discard: true }))
