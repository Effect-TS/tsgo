---
"@effect/tsgo": patch
---

Fix false-positive `TS2683` diagnostics for `Effect.gen({ self: this }, ...)` by avoiding eager call-signature analysis in affected Effect contexts.

This includes nested `Effect.gen` generic calls plus related cases such as `this` in callees, `Effect.sync`/`Effect.tryPromise` callbacks, `.pipe()` chains, and curried wrappers.
