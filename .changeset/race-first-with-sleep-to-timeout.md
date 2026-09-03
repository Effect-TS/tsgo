---
"@effect/tsgo": minor
---

Add the Effect v4 `raceFirstWithSleepToTimeout` diagnostic, which suggests `Effect.timeoutOrElse` when a first-completion race has exactly one `Effect.sleep`- or `Effect.delay`-based timer arm.
