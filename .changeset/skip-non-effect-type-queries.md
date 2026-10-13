---
"@effect/tsgo": patch
---

Skip type queries for expressions that can never have an Effect library type, such as literals, functions and, on Effect v4, plain object literals. Diagnostics are unchanged, but they are cheaper to compute: on the Effect monorepo a `tsc -b` build is about 1.3s faster with the default rules and about 2.4s faster with every rule except `apiStabilityLeak` enabled.

For example, rules such as `effectInVoidSuccess` no longer ask the checker for the type of the array and object literal arguments here, because neither can be an Effect:

```ts
Effect.forEach([1, 2, 3], (n) => Effect.log(n), { concurrency: 2 })
```
