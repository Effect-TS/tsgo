---
"@effect/tsgo": minor
---

Add the `isTaggedOnTaggedUnion` diagnostic (anti-pattern, warning, Effect v4). It reports `Predicate.isTagged(x, "A")`, `Predicate.isTagged("A")(x)` and `Predicate.isTagged("A")` passed as a callback, when the tested value (for a callback, the parameter the callback receives) is already typed as a union whose members all have a string literal `_tag`. `isTagged` accepts `unknown`, so on such a value it does not check the tag against the union, it leaves a member whose only field is `_tag` in the union after a failed test, and as a callback it narrows elements to `{ _tag: "A" }`. A tag that names no member of the union gets its own message, since the test is always false.

The quick fix replaces `isTagged` with `Predicate.isTaggedStrict`, keeping the arguments and the qualifier it is written with. It is offered when the installed Effect exports `isTaggedStrict`. When the union is Effect's `Option`, `Exit` or `Result` and the module is imported, it uses the module's guard instead, such as `Option.isSome`. A tag outside the union gets no fix.

```ts
// reported, fix: Predicate.isTaggedStrict(size, "Auto")
Predicate.isTagged(size, "Auto")
// reported, fix: Array.filter(sizes, Predicate.isTaggedStrict("Px"))
Array.filter(sizes, Predicate.isTagged("Px"))
// reported, fix: Option.isSome(option)
Predicate.isTagged(option, "Some")
// reported without a fix: always false
Predicate.isTagged(size, "Atuo")
// not reported: the value is not typed as a tagged union
Predicate.isTagged(input as unknown, "Px")
```
