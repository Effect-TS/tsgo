---
"@effect/tsgo": patch
---

Fix `effectFnImplicitAny` so it only checks the primary `Effect.fn` callback body instead of reporting helper callback parameters that are contextually typed by the `Effect.fn` result.

This avoids false positives for secondary callbacks such as `Effect.fn(function* (...) { ... }, (effect, ...args) => ...)`.
