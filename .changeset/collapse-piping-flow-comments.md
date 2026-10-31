---
"@effect/tsgo": patch
---

Quick fixes that collapse several piping flow steps into one (`timeoutCatchTagToTimeoutOrElse`, `mapSomeCatchNoSuchElement`) now keep source comments. Comments on kept steps stay with them, and comments on the replaced steps move to the replacement.

```ts
// before
lookup.pipe(
  // wrap successes
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)
// after the fix, the comment is kept
lookup.pipe(
  // wrap successes
  Effect.catchNoSuchElement
)
```
