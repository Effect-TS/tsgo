---
"@effect/tsgo": patch
---

Catch-handler rules now find every use of the caught error parameter in the same way, through a shared typeparser scan. A shorthand property `{ error }` now counts as a use of `error` in `catchConditionalRefailToCatchIf`, `catchTagToCatchReason` and the `catchAllTagDispatchToCatchTag` quick fix, as it already did in `timeoutCatchTagToTimeoutOrElse`. When a branch result reads the error only through `{ error }`, the `catchAllTagDispatchToCatchTag` fix now keeps the handler parameter, and `catchTagToCatchReason` counts the branch as using the parameter.

```ts
program.pipe(
  // now reported by catchConditionalRefailToCatchIf: the predicate reads `error` through `{ error }`
  Effect.catch((error) => isNotFound({ error }) ? Effect.succeed("guest") : Effect.fail(error))
)
```
