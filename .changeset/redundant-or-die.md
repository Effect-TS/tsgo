---
"@effect/tsgo": minor
---

Add the `redundantOrDie` diagnostic, which suggests hoisting repeated trailing `Effect.orDie` calls from every yielded effect to the generator result.
