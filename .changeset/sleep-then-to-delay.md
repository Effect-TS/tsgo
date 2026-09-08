---
"@effect/tsgo": patch
---

Add `sleepThenEffectToDelay` diagnostic (`TS377133`) to suggest using `Effect.delay` instead of sequencing `Effect.sleep` before an effect.
