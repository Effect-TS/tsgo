---
"@effect/tsgo": patch
---

Cache object-literal contextual types per checker across experimental and unstable API diagnostics. Keep normal and inference-blocked queries separate, including nil results, to avoid repeating the same contextual-type queries.
