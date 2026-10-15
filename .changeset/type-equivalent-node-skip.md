---
"@effect/tsgo": patch
---

Report each problem once in `effectInFailure`, `anyUnknownInErrorContext` and `promiseInEffectSuccess`. These rules check the type of every node in a file, and the checker gives some pairs of nodes the same type. A declaration and its name share a type, and so do `a.b` and its name `b`. The rules now check one node of each pair.

A variable whose initializer has the problem used to get two warnings, one on the variable name and one on the initializer. Now only the initializer is reported:

```ts
// before: warnings on `shouldReport` and on `Effect.try(...)`
// after: one warning on `Effect.try(...)`
export const shouldReport = Effect.try({
  try: () => JSON.parse("{"),
  catch: (e) => Effect.log(e)
})
```

A property access is now reported over the whole expression `promiseEffectHolder.current` instead of the name `current`.

`anyUnknownInErrorContext` keeps only the innermost warning when nested expressions report the same problem. It used to drop an outer warning whenever any warning inside it reported something. Now it drops the outer warning only when an inner warning reports every channel the outer one does, with the same `any` or `unknown`. An outer `unknown` error channel that comes from an explicit type argument is still reported when the inner expressions only have `any`:

```ts
declare const anyFailure: Effect.Effect<number, any>
// warnings on `Effect.withFiber(...)` (unknown in the error channel) and on `anyFailure` (any)
export const keepsDifferentKind = Effect.withFiber<number, unknown>(() => anyFailure)
```
