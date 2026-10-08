---
"@effect/tsgo": patch
---

Report experimental and unstable properties supplied in contextually typed object literals, including shorthand properties. For example, `x({ test: 42 })` and `x({ test })` now warn when the parameter type marks `test` with `@stability experimental`, even if the function and interface are stable.
