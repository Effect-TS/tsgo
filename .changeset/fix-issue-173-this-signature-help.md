---
"@effect/tsgo": patch
---

Fix a false-positive `TS2683` when using `this` inside directly yielded expressions in `Effect.gen({ self: this })`.

This avoids losing contextual `this` typing during data-first call analysis for Effect generator code such as `yield* Scope.close(this.#scope, Exit.void)`.
