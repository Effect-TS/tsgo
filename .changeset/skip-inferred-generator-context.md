---
"@effect/tsgo": patch
---

Prevent false TypeScript iterator and object-spread diagnostics when experimental or unstable API checks query generic constraints inside inferred generator function expressions. Skip inference-blocked contextual queries in these expressions while retaining normal contextual lookups. This conservative fallback can omit constraint-based property warnings inside inferred generators.
