---
"@effect/tsgo": patch
---

Fix `effectFnOpportunity` conversions to preserve enclosing expressions, sibling declarations, comments, literal spelling, and source formatting. Avoid suggesting conversions for function declarations referenced before their declaration.

Wrap non-call pipe arguments in unary callbacks so they do not receive the converted function's arguments. For example, `.pipe(Effect.ignore)` becomes an `Effect.fn` pipeable `_ => Effect.ignore(_)`, while factory calls such as `Effect.map(f)` are retained.
