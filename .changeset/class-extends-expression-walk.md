---
"@effect/tsgo": patch
---

Fix `preferEffectArray`, `preferEffectRecord`, `preferEffectString`, `promiseInEffectSuccess` and `anyUnknownInErrorContext` skipping everything inside a class `extends` expression. Their walks stop at type nodes, and the `extends` clause of a class is an `ExpressionWithTypeArguments`, which counts as one, so the `make` of a service class was never checked. The walks now visit the expression of a class `extends` clause; its type arguments, `implements` clauses and interface `extends` clauses are still skipped.

```ts
import { Context, Effect } from "effect"

const xs: ReadonlyArray<number> = [1, 2, 3]

export class Doubled extends Context.Service<Doubled, { readonly all: ReadonlyArray<number> }>()("Doubled", {
  make: Effect.gen(function*() {
    // now reported by preferEffectArray
    return { all: xs.map((n) => n * 2) }
  })
}) {}
```

`effectFnOpportunity` had the same blind spot when checking whether a function declaration is used before it is declared: a call inside an earlier class `extends` expression was ignored, so it offered to convert a hoisted function into a `const` that the class would then read before initialization. That opportunity is no longer offered.

```ts
export class Early extends Context.Service<Early, { readonly value: number }>()("Early", {
  make: makeEarly(1) // runs when the class is defined
}) {}

// no longer suggested as Effect.fn: the const would not be initialized yet
function makeEarly(n: number) {
  return Effect.gen(function*() {
    return { value: n }
  })
}
```
