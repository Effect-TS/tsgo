---
"@effect/tsgo": patch
---

Fix the `catchToOrElseSucceed` and `flatMapToMap` quick fixes emitting code that does not parse when `Effect.succeed` receives an object literal inside a concise arrow body. The value is now parenthesized, including when the object literal is followed by `as const` or `satisfies T`:

```ts
// before
Effect.catch(() => Effect.succeed({ ok: false } as const))
Effect.flatMap((n) => Effect.succeed({ value: n }))

// after the fix (previously `() => { ok: false } as const` and `(n) => { value: n }`)
Effect.orElseSucceed(() => ({ ok: false } as const))
Effect.map((n) => ({ value: n }))
```
