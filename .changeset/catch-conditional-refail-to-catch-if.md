---
"@effect/tsgo": minor
---

Add the `catchConditionalRefailToCatchIf` diagnostic for conditional `Effect.catch` and `Effect.catchCause` handlers that re-fail their untouched input, suggesting `Effect.catchIf`, `Effect.catchCauseIf`, or `Effect.catchTag` as appropriate.
