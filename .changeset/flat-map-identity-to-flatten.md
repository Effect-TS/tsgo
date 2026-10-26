---
"@effect/tsgo": minor
---

Add the `flatMapIdentityToFlatten` diagnostic (style, suggestion) and quickfix, which replaces `Effect.flatMap` by `Effect.flatten` when the callback is one of two forms:

- Effect's `identity` export referenced directly (`identity`, `Function.identity`, or an import renamed with `as`). Local variables holding `identity` are not followed.
- An arrow function without type annotations that returns its own parameter, such as `(inner) => inner`.

Annotated arrows and aliases of `identity` are not reported, because an annotation can change the resulting type.

```ts
// before
loadTask.pipe(Effect.flatMap((inner) => inner))
Effect.flatMap(loadTask, identity)
// after
loadTask.pipe(Effect.flatten)
Effect.flatten(loadTask)
```
