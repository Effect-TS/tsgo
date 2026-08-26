---
"@effect/tsgo": minor
---

Add the `mapSomeToAsSome` diagnostic and quick fix, which replaces `Effect.map(Option.some)` and its exact eta-expanded form with `Effect.asSome` in pipeable and data-first flows.
