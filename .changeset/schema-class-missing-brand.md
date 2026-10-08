---
"@effect/tsgo": minor
---

Add `schemaClassMissingBrand`, a correctness warning enabled by default for Effect v4 schema classes, errors, and opaque classes.

The diagnostic warns when the second brand type argument is omitted. For example, `Schema.Class<Person>("Person")` warns, while `Schema.Class<Person, { readonly brand: unique symbol }>("Person")` supplies a nominal brand. An explicit `{}` keeps structural typing and suppresses the warning. Effect v3 is excluded.
