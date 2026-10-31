// @effect-v3
import { Effect, Either, pipe } from "effect"

declare const items: ReadonlyArray<number>
declare const groups: ReadonlyArray<Array<string>>
declare const processItem: (item: number) => Effect.Effect<string>
declare const processIndexed: (item: number, index: number) => Effect.Effect<string>
declare const offset: Effect.Effect<number>
declare function overloaded(item: number): Effect.Effect<string>
declare function overloaded(item: string): Effect.Effect<string>

function processDeclared(item: number): Effect.Effect<string> {
  return Effect.succeed(String(item))
}

const processFn = Effect.fn("processFn")(function*(item: number) {
  return String(item)
})

// ---------------------------------------------------------------------------
// MUST TRIGGER, with a fix that keeps an arrow (the callee is not a plain
// single-parameter function called with the loop variable)
// ---------------------------------------------------------------------------

// MUST TRIGGER: method pipe on the yielded Effect.
// fix: const results = yield* Effect.forEach(items, (item) => processItem(item).pipe(Effect.orDie))
export const methodPipe = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item).pipe(Effect.orDie))
  }
  return results
})

// MUST TRIGGER: pipe function call.
// fix: const lengths = yield* Effect.forEach(items, (item) => pipe(processItem(item), Effect.map((s) => s.length)))
export const pipeCall = Effect.gen(function*() {
  const lengths = []
  for (const item of items) {
    lengths.push(yield* pipe(processItem(item), Effect.map((s) => s.length)))
  }
  return lengths
})

// MUST TRIGGER: data-first Effect.map.
// fix: const lengths = yield* Effect.forEach(items, (item) => Effect.map(processItem(item), (s) => s.length))
export const dataFirstMap = Effect.gen(function*() {
  const lengths = []
  for (const item of items) {
    lengths.push(yield* Effect.map(processItem(item), (s) => s.length))
  }
  return lengths
})

// MUST TRIGGER: data-last Effect.map applied to the Effect.
// fix: const lengths = yield* Effect.forEach(items, (item) => Effect.map((s: string) => s.length)(processItem(item)))
export const dataLastMap = Effect.gen(function*() {
  const lengths = []
  for (const item of items) {
    lengths.push(yield* Effect.map((s: string) => s.length)(processItem(item)))
  }
  return lengths
})

// MUST TRIGGER: an Effect.fn callee has a rest parameter.
// fix: const results = yield* Effect.forEach(items, (item) => processFn(item))
export const effectFnCallee = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processFn(item))
  }
  return results
})

// MUST TRIGGER: an overloaded callee has several call signatures.
// fix: const results = yield* Effect.forEach(items, (item) => overloaded(item))
export const overloadedCallee = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* overloaded(item))
  }
  return results
})

// ---------------------------------------------------------------------------
// MUST TRIGGER, with a fix that passes the callee directly
// ---------------------------------------------------------------------------

// MUST TRIGGER: a function declaration with a single parameter.
// fix: const results = yield* Effect.forEach(items, processDeclared)
export const declaredCallee = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processDeclared(item))
  }
  return results
})

// MUST TRIGGER: Either is an Effect subtype in v3, so Effect.forEach accepts it.
// fix: const values = yield* Effect.forEach(items, (item) => Either.right(item))
export const yieldedEither = Effect.gen(function*() {
  const values = []
  for (const item of items) {
    values.push(yield* Either.right(item))
  }
  return values
})

// ---------------------------------------------------------------------------
// MUST TRIGGER, without quick fix
// ---------------------------------------------------------------------------

// MUST TRIGGER (no fix): the yielded expression contains another yield, which
// cannot move into an arrow function.
export const nestedYield = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processIndexed(item, yield* offset))
  }
  return results
})

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: the loop binding shadows the accumulator, so push targets
// the loop variable.
export const shadowedAccumulator = Effect.gen(function*() {
  const out: Array<string> = []
  for (const out of groups) {
    out.push(yield* processItem(out.length))
  }
  return out
})

// MUST NOT TRIGGER: a declaration in between captures the accumulator through
// a shorthand property.
export const shorthandCapture = Effect.gen(function*() {
  const out: Array<string> = []
  const box = { out }
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  return box
})

// MUST NOT TRIGGER: labelled loop.
export const labelledLoop = Effect.gen(function*() {
  const out = []
  outer: for (const item of items) {
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: optional push call.
export const optionalPush = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out?.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: the iterable references the accumulator.
export const iterableReferencesAccumulator = Effect.gen(function*() {
  const out: Array<string> = []
  for (const item of items.slice(out.length)) {
    out.push(yield* processItem(item))
  }
  return out
})
