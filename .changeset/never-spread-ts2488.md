---
"@effect/tsgo": patch
---

Prevent a false TS2488 when a rule such as `promiseInEffectSuccess` asks for the type of a spread over a `never`-typed value. TypeScript checks a spread through its enclosing array literal or call, where `never` counts as array-like, but checking the spread element on its own reported that `never` has no `[Symbol.iterator]()` method.

```ts
export function f() {
  const rows = [1] as never
  return [...rows] // no longer reports TS2488
}
```
