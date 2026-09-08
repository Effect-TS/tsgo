---
"@effect/tsgo": minor
---

Add the `timeoutCatchTagToTimeoutOrElse` diagnostic and quick fixes for Effect v4. Suggest `Effect.timeoutOrElse` for `Effect.timeout` followed by `Effect.catchTag("TimeoutError", ...)`, and `Effect.timeoutOption` for the corresponding Some/None pattern. Only suggest a rewrite when the input error channel excludes `TimeoutError` and the handler does not use the caught error.
