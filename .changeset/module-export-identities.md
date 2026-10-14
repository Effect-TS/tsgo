---
"@effect/tsgo": patch
---

Resolve Effect API references once per symbol. `IsNodeReferenceToModuleExport` used to answer "is this symbol exported as `gen`?" one member name at a time, caching each answer in a per-symbol map. It now computes, once per symbol, the names its declaring module exports it under, from a per-module index of exports keyed the way the checker compares symbols, and answers every member name from that list. `export =` modules keep the direct lookup.

For example, checking whether a callee is `Effect.gen`, `Effect.fn` or `Effect.sync` now reads one cached list (`["gen"]` for `Effect.gen`) instead of hashing three cache keys.

With 125 rules enabled on the Effect monorepo, wall time, CPU time and peak memory are unchanged within noise, and the reported diagnostics are unchanged.
