---
"@effect/tsgo": minor
---

Add `mapConstToAs`, a style suggestion for Effect v3 and v4 that rewrites `Effect.map` callbacks which ignore their input and return a constant into `Effect.as`:

```ts
saveUser("alice").pipe(Effect.map(() => "saved"))
// becomes
saveUser("alice").pipe(Effect.as("saved"))
```

Literals, negative numbers, enum members, earlier `const` bindings and `declare const` values are recognized; object/array literals, `let` bindings, property reads and computed expressions are not, since `Effect.as` captures its value once while a `map` callback re-evaluates it on every run. Callbacks with a return type annotation are also skipped, because the annotation fixes the success type and `Effect.as` would infer it from the value instead.

`syncToSucceed` and `flatMapIgnoredParamToAndThen` share the same constant detection, so they now also recognize `declare const` values, negative numeric literals and enum members.
