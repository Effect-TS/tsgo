// @effect-v4
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const objects: ReadonlyArray<{ readonly value?: number }>
declare const pairs: ReadonlyArray<readonly [number?]>
declare const processItem: (item: number) => Effect.Effect<string>

// ---------------------------------------------------------------------------
// MUST TRIGGER, with a fix that terminates the declaration when the next
// statement would otherwise continue it
// ---------------------------------------------------------------------------

// MUST TRIGGER: the next statement starts with `[`.
// fix: const results = yield* Effect.forEach(items, processItem);
export const nextStartsWithBracket = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  [1, 2].forEach((n) => n)
  return results
})

// MUST TRIGGER: the next statement starts with `(`.
// fix: const results = yield* Effect.forEach(items, processItem);
export const nextStartsWithParen = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  (results as Array<string>).join(",")
  return results
})

// MUST TRIGGER: the next statement starts with a template literal.
// fix: const results = yield* Effect.forEach(items, processItem);
export const nextStartsWithTemplate = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  `${results.length}`.trim()
  return results
})

// MUST TRIGGER: the next statement is on the same line as the block body.
// fix: const results = yield* Effect.forEach(items, processItem); yield* Effect.log(results.length)
export const sameLineAfterBlock = Effect.gen(function*() {
  const results = []
  for (const item of items) { results.push(yield* processItem(item)) } yield* Effect.log(results.length)
  return results
})

// MUST TRIGGER: the statement body's semicolon is kept, also before a
// statement on the same line.
// fix: const results = yield* Effect.forEach(items, processItem); yield* Effect.log(results.length)
export const sameLineAfterSemicolon = Effect.gen(function*() {
  const results = []
  for (const item of items) results.push(yield* processItem(item)); yield* Effect.log(results.length)
  return results
})

// MUST TRIGGER: the statement body's semicolon is kept at the end of the line.
// fix: const results = yield* Effect.forEach(items, processItem);
export const semicolonStyle = Effect.gen(function*() {
  const results = [];
  for (const item of items) results.push(yield* processItem(item));
  return results;
})

// MUST TRIGGER: the yielded expression starts with `{`, so the arrow body is
// parenthesized instead of parsing as a block.
// fix: const results = yield* Effect.forEach(items, (item) => ({ effect: processItem(item) }.effect))
export const bodyStartsWithBrace = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* { effect: processItem(item) }.effect)
  }
  return results
})

// ---------------------------------------------------------------------------
// MUST TRIGGER, without quick fix (the loop binding contains a yield, which
// cannot move into the arrow's parameter list)
// ---------------------------------------------------------------------------

// MUST TRIGGER (no fix): object pattern default yields.
export const objectDefaultYields = Effect.gen(function*() {
  const results = []
  for (const { value = yield* Effect.succeed(1) } of objects) {
    results.push(yield* processItem(value))
  }
  return results
})

// MUST TRIGGER (no fix): array pattern default yields.
export const arrayDefaultYields = Effect.gen(function*() {
  const results = []
  for (const [value = yield* Effect.succeed(1)] of pairs) {
    results.push(yield* processItem(value))
  }
  return results
})

// MUST TRIGGER: a yield inside a nested function of the binding belongs to
// that function, so the fix is still offered.
// fix: const results = yield* Effect.forEach(objects, ({ value = [...(function*() { yield 1 })()][0] }) => processItem(value))
export const nestedGeneratorInBinding = Effect.gen(function*() {
  const results = []
  for (const { value = [...(function*() { yield 1 })()][0] } of objects) {
    results.push(yield* processItem(value))
  }
  return results
})
