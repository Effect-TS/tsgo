---
"@effect/tsgo": patch
---

Fix `schemaNumber` false positives for `isInt()` and `isFinite()` checks wrapped in `.abort()` or combined with `.and(...)`. `.abort()` keeps the filter's predicate and `X.and(Y)` runs both filters, so the schema still rejects `NaN`, `Infinity` and `-Infinity`, like the bare filter and the `.annotate(...)` form, which was already accepted. A finite filter on either side of `.and(...)` counts, and the wrappers can be chained in any order, in `.check(...)` and in piped `Schema.check(...)`.

```ts
Schema.Number.check(Schema.isInt().abort())                              // no longer reported
Schema.Number.check(Schema.isFinite().abort(), Schema.isBetween({ minimum: 1, maximum: 24 })) // no longer reported
Schema.Number.check(Schema.isInt().and(Schema.isGreaterThan(0)))         // no longer reported
Schema.Number.check(Schema.isGreaterThan(0).and(Schema.isInt()))         // no longer reported
Schema.Number.check(Schema.isGreaterThan(0).abort())                     // still reported
```
