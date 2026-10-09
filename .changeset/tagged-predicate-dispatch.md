---
"@effect/tsgo": minor
---

Recognize `Predicate.isTagged(value, "Foo")` and `Predicate.isTagged("Foo")(value)` as tag comparisons in dispatch analysis. Existing catch-tag diagnostics and quick fixes now handle both forms in Effect v3 and v4.
