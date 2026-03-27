---
"@effect/tsgo": patch
---

Refactor typeparser and duplicate-package caching to keep checker-local cache state on `EffectLinks` instead of using process-global cache variables.

This removes manual cache reset hooks and simplifies repeated package and type lookups without changing diagnostics behavior.
