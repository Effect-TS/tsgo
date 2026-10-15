---
"@effect/tsgo": patch
---

Skip Effect type queries for more expressions that always produce a primitive or a `RegExp`. `NodeCannotBeEffectLibraryType` now also rules out `true`, `false`, `null`, regular expression literals, prefix unary expressions such as `!x` and `-x`, `void x` and `typeof x`. The `undefined` identifier still goes through the checker, because a local binding can shadow it.

For example, for `return true` inside `Effect.gen`, `returnEffectInGen` no longer asks the checker for the type of `true`.

Reported diagnostics are unchanged.
