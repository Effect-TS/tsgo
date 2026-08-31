---
"@effect/tsgo": minor
---

Add the `flatMapConditionalToFilterOrFail` diagnostic and quick fix, which replaces identity-succeed `Effect.flatMap` conditionals with `Effect.filterOrFail` or `Effect.filterOrElse`.
