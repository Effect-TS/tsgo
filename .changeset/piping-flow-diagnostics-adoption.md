---
"@effect/tsgo": minor
---

Adopt the piping flow parser in more diagnostics.

- `promiseInEffectSuccess`: an explicit promise-success annotation (type arguments on `Effect.succeed`/`as`/`map`/`zipWith`) now suppresses the diagnostic from any position in the surrounding pipe, not only the last argument. `base.pipe(Effect.as<Promise<number>>(promiseValue), Effect.as(promiseValue))` no longer reports, matching the reversed order that was already accepted.
- `allOfMapToForEach`: now also detects the data-last form expressed through piping flows, e.g. `pipe(values.map(effectful), Effect.all)` and `pipe(values.map(effectful), Effect.all, Effect.asVoid)`, which were previously invisible to the call-expression walk. These matches are diagnostic-only: the existing quick fix remains limited to the standalone `Effect.all(xs.map(f), options?)` call it can safely rewrite.
- The piping flow parser now keeps the type arguments of parenthesized pipe arguments, e.g. `pipe(x, (Effect.as<...>(v)))`.
- The piping flow parser now normalizes curried pipeable applications, so `Effect.catch(handler)(effect)` has `effect` as its subject, `Effect.catch` as its callee, and `handler` as its transformation argument. The normalization is limited to calls whose signatures verify that they are the pipeable counterpart of a data-first overload of the same combinator, leaving unrelated curried APIs such as `Effect.fn("name")(body)` unchanged.
