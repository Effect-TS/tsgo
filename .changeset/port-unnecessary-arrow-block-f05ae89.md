---
"@effect/tsgo": minor
---

Add the `unnecessaryArrowBlock` diagnostic and quick fix for arrow functions whose block body only returns an expression.

This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, quickfix baselines, and generated metadata/schema documentation.
