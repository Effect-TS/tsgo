---
"@effect/tsgo": minor
---

Add the v4-only `catchRefailToTapError` diagnostic. It suggests `Effect.tapError` when an `Effect.catch` handler sequences an effect with `Effect.andThen` or a zero-argument `Effect.flatMap` callback and then fails with the original, unmodified error. Generator handlers and selective catches are excluded.
