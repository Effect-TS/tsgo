---
"@effect/tsgo": minor
---

Add `schemaClassMissingBrand`, a correctness suggestion enabled by default for Effect v4 schema classes, errors, and opaque classes.

The diagnostic suggests adding a brand when the second brand type argument is omitted. For example, `Schema.Class<Person>("Person")` triggers a suggestion, while `Schema.Class<Person, { readonly brand: unique symbol }>("Person")` supplies a nominal brand. An explicit `{}` keeps structural typing and suppresses the suggestion. Effect v3 is excluded.
