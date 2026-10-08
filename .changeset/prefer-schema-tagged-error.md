---
"@effect/tsgo": minor
---

Add the opt-in `preferSchemaTaggedError` diagnostic for Effect v3 and v4. It suggests `Schema.TaggedError` for class declarations that extend `Data.TaggedError`, including anonymous default exports. For example, `class NotFound extends Data.TaggedError("NotFound")<{ id: string }> {}` reports a warning when the rule is enabled.
