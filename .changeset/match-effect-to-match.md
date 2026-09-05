---
"@effect/tsgo": minor
---

Add the `matchEffectToMatch` style diagnostic and quick fix for replacing `Effect.matchEffect` or `Effect.matchCauseEffect` whose handlers only return `Effect.succeed` with their non-effectful counterparts.

Make lazy-expression parsing synchronous and non-generator by default, with flags for callers that explicitly accept thunks, async functions, or generators.
