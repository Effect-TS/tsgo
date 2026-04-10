---
"@effect/tsgo": patch
---

Port the Effect context tracking refactor from the TypeScript reference implementation so diagnostics also recognize Effect constructor thunks such as `Effect.sync`, `Effect.promise`, `Effect.try`, and `Effect.tryPromise`.

This updates related metadata and baselines and adds thunk-focused test coverage for both Effect v3 and v4 fixtures.
