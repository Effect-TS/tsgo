---
"@effect/tsgo": minor
---

Add the `preferSucceedSomeOrNone` diagnostic and autofix for replacing `Effect.succeed(Option.none())` and `Effect.succeed(Option.some(value))`, including equivalent pipe forms, with `Effect.succeedNone` and `Effect.succeedSome(value)`.
