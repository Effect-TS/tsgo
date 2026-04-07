---
"@effect/tsgo": minor
---

Add the `lazyPromiseInEffectSync` diagnostic for `Effect.sync` thunks that return the global `Promise<T>` type.

This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, baselines, and exact Promise detection via TypeScriptGo checker shims.
