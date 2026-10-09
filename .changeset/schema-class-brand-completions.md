---
"@effect/tsgo": minor
---

Include `{ readonly brand: unique symbol }` in Effect v4 Schema class completions for `Class`, `TaggedClass`, `Error`, and `TaggedError`. Add a branded completion for `Schema.Opaque`.

For example, the `Class<Person>` completion inserts `Schema.Class<Person, { readonly brand: unique symbol }>("Person")`. Effect v3 and `Model.Class` completions keep their existing type arguments.
