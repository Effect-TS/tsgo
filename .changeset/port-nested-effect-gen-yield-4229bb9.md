---
"@effect/tsgo": minor
---

Add the `nestedEffectGenYield` diagnostic for nested bare `yield* Effect.gen(...)` calls inside existing Effect generator contexts.

This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, generated metadata, schema entries, and reference baselines.
