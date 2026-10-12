---
"@effect/tsgo": patch
---

Resolve the types carried by typeparser results only when a rule reads them, and reuse the resolved type on later reads. For example, a piping flow no longer queries the type of every transformation up front, so a rule that filters flows by callee skips those queries. Diagnostics are unchanged.
