---
"@effect/tsgo": minor
---

Add the `catchToOrElseSucceed` diagnostic, which suggests `Effect.orElseSucceed` for `Effect.catch(() => Effect.succeed(value))` in Effect v4 and `Effect.catchAll(() => Effect.succeed(value))` in Effect v3.
