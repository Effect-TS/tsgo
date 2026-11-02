---
"@effect/tsgo": patch
---

Fix `schemaNumber` false positives when the finite check is annotated. Annotating a filter keeps its validation predicate, so `isInt()` and `isFinite()` wrapped in `.annotate(...)` (any number of times) now count as finite checks, in both `.check(...)` and `Schema.check(...)` inside a pipe.

```ts
// no longer reported
Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" }))
Schema.Number.check(Schema.isFinite().annotate({ expected: "a finite number" }))
Schema.Number.pipe(Schema.check(Schema.isInt().annotate({ expected: "an integer" })))
```
