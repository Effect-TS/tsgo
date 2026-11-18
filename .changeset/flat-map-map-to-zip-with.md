---
"@effect/tsgo": minor
---

Add the `flatMapMapToZipWith` style diagnostic. `Effect.flatMap(self, (a) => Effect.map(that, (b) => f(a, b)))`, where `that` does not use `a`, is reported, with a quick fix to `Effect.zipWith(self, that, (a, b) => f(a, b))`, or to `Effect.zip(self, that)` when the combiner is exactly `[a, b]`:

```ts
// before
const greeting = Effect.flatMap(getUser, (user) =>
  Effect.map(getConfig, (config) => `${user.name} (${config.locale})`))

// after
const greeting = Effect.zipWith(getUser, getConfig, (user, config) => `${user.name} (${config.locale})`)
```

The rule stays silent when the mapper returns the outer value unchanged (`Effect.map(that, () => a)`), because `flatMapToTap` covers that. The fix is offered only when `that` can be evaluated eagerly without changing behavior (a `const` declared earlier, or a `const` exported by a package such as `Clock.currentTimeMillis`).
