---
"@effect/tsgo": minor
---

Add the `optionMatchToFromOption` diagnostic and quick fix for Option-to-Effect conversions that can use `Effect.fromOption`.

The rule recognizes supported `Option.match` call styles and `Option.isSome` / `Option.isNone` conditional expressions whose branches only wrap the value with `Effect.succeed` or produce `Effect.fail`. It suggests the one-argument `Effect.fromOption` form for the default `Cause.NoSuchElementError`, and preserves custom failures in a lazy callback.
