---
"@effect/tsgo": patch
---

`missedPipeableOpportunity` no longer pulls React hooks into a pipe when the program references React (a package named `react` or `@types/react`, or any package depending on `react`). A callee named like a hook (`use` followed by an uppercase letter or a digit) ends the pipeable run, so the calls before it can still be reported:

```ts
// before: Effect.succeed(1).pipe(Effect.asVoid, Effect.asVoid, useEffectValue)
// after:
useEffectValue(Effect.succeed(1).pipe(Effect.asVoid, Effect.asVoid))
```
