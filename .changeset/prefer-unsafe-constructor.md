---
"@effect/tsgo": minor
---

Add the `preferUnsafeConstructor` diagnostic and quickfix: `Effect.runSync` applied directly to a pure effect-package constructor call (e.g. `Effect.runSync(Scope.make())`) is reported when the same module exports a type-equivalent synchronous `*Unsafe` sibling, with a fix rewriting to `Scope.makeUnsafe()` while preserving arguments and type arguments.
