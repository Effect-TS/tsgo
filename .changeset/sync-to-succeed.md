---
"@effect/tsgo": minor
---

Add the `syncToSucceed` diagnostic and quick fix, which replaces `Effect.sync` thunks returning stable constant values with `Effect.succeed`.
