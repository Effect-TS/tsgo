---
"@effect/tsgo": minor
---

Add Effect v4 support for the `runEffectInsideEffect` diagnostic and quick fix.

Nested `Effect.run*` calls inside generators now suggest and apply `Effect.run*With` fixes using extracted services.
