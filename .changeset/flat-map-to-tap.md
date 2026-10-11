---
"@effect/tsgo": minor
---

Add the `flatMapToTap` style suggestion and quick fix for Effect v3 and v4. Callbacks that run an effect and restore their unchanged parameter with terminal `Effect.as` or `Effect.map` can use `Effect.tap` instead. The fix preserves callback statements and earlier effect transformations.
