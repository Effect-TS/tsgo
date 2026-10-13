---
"@effect/tsgo": patch
---

Store the Effect caches that cover most nodes of a file in paged arrays indexed by node id, the layout the TypeScript 7.1 checker uses for its own hottest node caches. A lookup now indexes an array instead of hashing a node pointer. For example, `tsc -b` on the Effect monorepo runs about 2% faster with identical diagnostics. On TypeScript 7.0.2, which lacks block-allocated node ids, these caches stay hash maps.
