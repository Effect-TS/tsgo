---
"@effect/tsgo": minor
---

Add the `flatMapIdentityToFlatten` diagnostic (style, suggestion) and quickfix, which replaces `Effect.flatMap` with an identity callback by `Effect.flatten`.

```ts
// before
loadTask.pipe(Effect.flatMap((inner) => inner))
Effect.flatMap(loadTask, identity)
// after
loadTask.pipe(Effect.flatten)
Effect.flatten(loadTask)
```
