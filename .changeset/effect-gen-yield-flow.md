---
"@effect/tsgo": minor
---

Improve execution-flow graphing for `Effect.gen` and generator-based `Effect.fn` calls by modeling `yield*` operands as yieldable links and preserving the generator result type through piped calls.

This updates flow baselines for cases like `yield* Effect.succeed(1)` and `yield* Effect.fail(error)`, and exports the TypeScript-Go `forEachYieldExpression` helper through the checker shim for reuse.
