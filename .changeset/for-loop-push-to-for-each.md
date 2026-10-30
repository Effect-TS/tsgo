---
"@effect/tsgo": minor
---

Add the `forLoopPushToForEach` diagnostic (style, suggestion) and quickfix. Inside an Effect generator, a `for (const x of iterable)` loop whose only statement is `acc.push(yield* effect)`, with `acc` a `const acc = []` declared just before the loop, is reported. The fix replaces it with `Effect.forEach`. The fix is not offered when the accumulator is mutated after the loop. The callback is passed directly only when it is a plain single-parameter function applied to the loop variable, whether as `f(x)`, `pipe(x, f)` or `x.pipe(f)`; otherwise an arrow is kept.

```ts
// before
const results = []
for (const item of items) {
  results.push(yield* processItem(item))
}
// after
const results = yield* Effect.forEach(items, processItem)
```
