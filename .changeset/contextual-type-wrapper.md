---
"@effect/tsgo": minor
---

Route Effect contextual-type queries through `TypeParser.GetContextualType` and enforce the wrapper with customlint. For example, use `tp.GetContextualType(node, checker.ContextFlagsNone)` instead of the checker method. Query behavior stays unchanged, with one place to add contextual-type safety guards in future changes.
