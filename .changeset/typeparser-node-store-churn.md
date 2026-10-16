---
"@effect/tsgo": patch
---

Make the Effect diagnostics write less to the checker's per-node caches. Rules run on the checker goroutine right after each file is checked, and on the Effect monorepo most of their cost was cache traffic (map misses, nil inserts, map growth) and the garbage collection it caused, not type computation.

- Reference checks such as "is this node `Effect.gen`?" no longer probe or fill the symbol cache for statements, blocks, clauses and tokens, which can never resolve to a symbol.
- The analysis that tracks whether code runs inside `Effect.gen`, `Effect.fn` or an Effect thunk stores an entry only where the answer changes, about 2% of nodes, instead of two entries for every node of every file.
- Type lookups reject nodes that can never have a type before touching the cache, the strict Effect type check caches only types named `Effect`, and whether a file has been checked is asked once per file instead of on every query.

Reported diagnostics are unchanged.
