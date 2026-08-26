---
"@effect/tsgo": minor
---

Refactor the `mapSomeToAsSome` mapper detection onto the shared `UnwrapIdentityForwarder` helper. The diagnostic and quick fix now also recognize identity-forwarding function expressions:

```ts
// now triggers, alongside Effect.map(Option.some) and Effect.map((v) => Option.some(v))
numberEffect.pipe(
  Effect.map(function (value) {
    return Option.some(value)
  })
)
```

Annotated or optional mapper parameters and explicit type arguments on the inner `Option.some` call still do not trigger, since they can intentionally widen the resulting `Option` type.
