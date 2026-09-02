---
"@effect/tsgo": patch
---

Avoid reporting the `schemaNumber` diagnostic when `Schema.Number` is refined with the built-in `isFinite` or `isInt` checks, including pipe-style refinements.
