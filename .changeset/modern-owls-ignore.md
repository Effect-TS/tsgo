---
"@effect/tsgo": minor
---

Add the `catchToIgnore` diagnostic, which suggests `Effect.ignore` or `Effect.ignoreCause` when `Effect.catch` or `Effect.catchCause` returns `Effect.void` on a void success channel.
