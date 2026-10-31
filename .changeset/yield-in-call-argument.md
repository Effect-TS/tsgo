---
"@effect/tsgo": minor
---

Add the `yieldInCallArgument` diagnostic (style, off by default). It reports a `yield*` passed directly as an argument of a call or `new` expression inside `Effect.gen`, `Effect.fn` or `Effect.fnUntraced`, and offers a quick fix that hoists the yield into a const:

```ts
// before
const result = transform(yield* load)
// after
const loaded = yield* load
const result = transform(loaded)
```

The fix is offered only when nothing evaluated before the `yield*` has side effects (identifiers, literals and property accesses count as safe), and not in conditional positions or loop conditions, optional calls or unbraced bodies. In those cases the diagnostic is still reported.
