---
"@effect/tsgo": patch
---

Cache module export checks per symbol. Rules check whether most identifiers and callees refer to a module export, such as `gen`, `fn`, or `map` from the `effect` package's `Effect` module. Each symbol now gets a slot in a paged array indexed by symbol id, the layout the TypeScript 7.1 checker uses for its own symbol caches. The slot holds a small map of answers keyed by module and member name. Before, every answer shared one large hash map. For example, `tsc -b` on the Effect monorepo with every rule enabled runs about 1% faster with identical diagnostics. On TypeScript 7.0.2, which lacks block-allocated symbol ids, the per-symbol slots stay in a hash map.
