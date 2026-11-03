---
"@effect/tsgo": minor
---

Add the `isTaggedOnTaggedUnion` diagnostic (anti-pattern, warning). It reports `Predicate.isTagged(x, "A")`, `Predicate.isTagged("A")(x)` and `Predicate.isTagged("A")` passed as a callback, when the tested value (for a callback, the parameter of its contextual signature) is already typed as a union whose members all have a string literal `_tag`. `isTagged` accepts `unknown`, so on such a value it does not check the tag against the union, it leaves a member whose only field is `_tag` in the union after a failed test, and as a callback it narrows elements to `{ _tag: "A" }`. A tag that names no member of the union gets its own message, since the test is always false.

The quick fix writes `x._tag === "A"` (`x._tag !== "A"` for `!isTagged`), and `value => value._tag === "A"` for a callback or a later pipe step. When the union is Effect's `Option`, `Exit`, `Result` or `Either` and the module is imported, it uses the module's guard instead, such as `Option.isSome`. A tag outside the union gets no fix.

```ts
// reported, fix: size._tag === "Auto"
Predicate.isTagged(size, "Auto")
// reported, fix: sizes.filter(value => value._tag === "Px")
sizes.filter(Predicate.isTagged("Px"))
// reported, fix: Option.isSome(option)
Predicate.isTagged(option, "Some")
// reported without a fix: always false
Predicate.isTagged(size, "Atuo")
// not reported: the value is not typed as a tagged union
Predicate.isTagged(input as unknown, "Px")
```
