---
"@effect/tsgo": minor
---

Add `effectSleepZeroToYieldNow`, a style suggestion with a quick fix for zero-duration `Effect.sleep` calls.

For example, `Effect.sleep(0)` becomes `Effect.yieldNow()` in Effect v3 and `Effect.yieldNow` in Effect v4. The rule also recognizes safe zero-duration strings, tuples, and Duration constructors. The replacement yields through the fiber scheduler instead of the Clock service.
