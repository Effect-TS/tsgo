---
"@effect/tsgo": minor
---

Remove the AST location parameter from type-only `TypeParser` operations. Effect, Layer, Stream, Schema, service, Context.Tag, Scope, and related type predicates now derive instantiated property types directly from the supplied checker type.
