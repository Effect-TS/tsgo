---
"@effect/tsgo": patch
---

`catchDieToOrDie`, `optionMatchToFromOption` and `mapSomeToAsSome` now recognize identity-forwarding handlers written as a pipe flow, not only as a direct call.

```ts
// now reported, like (error) => Effect.die(error)
failable.pipe(Effect.catch((error) => pipe(error, Effect.die)))
numberEffect.pipe(Effect.map((value) => pipe(value, Option.some)))
Option.match(option, { onNone: () => Effect.fail(new MissingValue()), onSome: (value) => pipe(value, Effect.succeed) })
```
