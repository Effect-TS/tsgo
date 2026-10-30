---
"@effect/tsgo": minor
---

Add the `emptyCallbackToNever` diagnostic (Effect v4 only). It reports `Effect.callback` calls whose register is an empty function literal. Such an effect never completes, and the rule offers a quickfix to `Effect.never`:

```ts
// before
Effect.log("worker started").pipe(Effect.andThen(Effect.callback<never>(() => {})))
// after
Effect.log("worker started").pipe(Effect.andThen(Effect.never))
```

Detection runs on piping flows, so a register piped into a bare `Effect.callback` step is reported too, and the fix keeps any later steps:

```ts
// before
pipe(() => {}, Effect.callback<never>, Effect.onInterrupt(() => Effect.log("stopped")))
// after
pipe(Effect.never, Effect.onInterrupt(() => Effect.log("stopped")))
```

The rule fires only when the call has no explicit type arguments or all of them are `never`, so the fix never narrows a declared channel. It does not report on Effect v3: there `Effect.never` keeps the process alive, so an empty `Effect.async` is not equivalent.
