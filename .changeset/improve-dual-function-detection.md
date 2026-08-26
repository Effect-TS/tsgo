---
"@effect/tsgo": minor
---

Improve dual function detection by matching alpha-equivalent data-first and pipeable overload declarations without instantiating their generic signatures.

For example, data-first calls such as `Option.match(value, handlers)` now normalize to the same piping-flow representation as `value.pipe(Option.match(handlers))`, even when the overloads reorder generic parameters or return a generic union.
