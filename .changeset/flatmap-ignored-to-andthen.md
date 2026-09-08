---
"@effect/tsgo": minor
---

Add `flatMapIgnoredParamToAndThen` diagnostic (`TS377131`) to suggest using `Effect.andThen` instead of `Effect.flatMap` when the callback ignores its parameter and returns a pre-existing effect.
