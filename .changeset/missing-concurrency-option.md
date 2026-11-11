---
"@effect/tsgo": minor
---

Add the opt-in `missingConcurrencyOption` diagnostic in `style`. `Effect.all` and `Effect.forEach` run sequentially unless `concurrency` is set, which readers coming from `Promise.all` often do not expect. The rule reports calls whose options argument is missing or is an object literal without a `concurrency` key, in every spelling: data-first, data-last in a pipe, curried, and inside `Effect.gen` / `Effect.fn` bodies.

```ts
yield* Effect.forEach(items, save)                      // reported
yield* Effect.forEach(items, save, { concurrency: 1 })  // ok
yield* Effect.all(checks, { concurrency: 4 })           // ok
```

A quick fix adds `concurrency: 1`, which keeps the current behaviour: `Effect.forEach(items, save)` becomes `Effect.forEach(items, save, { concurrency: 1 })`, and `Effect.all(checks, { discard: true })` becomes `Effect.all(checks, { discard: true, concurrency: 1 })`. Options that are not an object literal, or that use a spread, are not reported. The rule defaults to off and supports Effect v3 and v4.

Piping flows now recognize Effect v4's `forEach` as a dual, so `Effect.forEach(items, f)` and `Effect.forEach(f)(items)` normalize like they do on v3.
