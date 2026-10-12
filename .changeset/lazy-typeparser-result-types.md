---
"@effect/tsgo": patch
---

Resolve the types carried by typeparser results only when a rule reads them, and reuse the resolved type on later reads. For example, a piping flow no longer queries the type of every transformation up front, so rules that filter flows by callee skip those queries. `effectInVoidSuccess` now checks the expected type before resolving the real type. Diagnostics are unchanged, and checking `.repos/effect` is about 2% faster.
