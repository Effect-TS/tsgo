---
"@effect/tsgo": minor
---

Add `failCauseToConstructor`, a style suggestion for Effect v3 and v4 that rewrites `Effect.failCause` / `Effect.failCauseSync` wrapping a single `Cause.fail` / `Cause.die` into the direct constructor:

```ts
Effect.failCause(Cause.die(new Error("boom")))   // -> Effect.die(new Error("boom"))
pipe(error, Cause.fail, Effect.failCause)        // -> Effect.fail(error)
Effect.failCauseSync(() => Cause.fail(error))    // -> Effect.failSync(() => error)
```

Piped spellings are detected through piping flows, and later steps are kept. On v3, a lazy `Cause.die` becomes `Effect.dieSync`; v4 has no `dieSync`, so it is not reported there. Forwarded, stored, composed or transformed causes, explicit `failCause<E>` type arguments, `as` assertions and annotated thunks are skipped. Lazy forms written as a `pipe(...)` subject or with a piped body are reported without a quick fix, as are lazy values whose error type would change once returned from a `failSync` thunk (such as `cond ? "A" : "B"`, which `Cause.fail` widens to `string`) and named-import calls where no value `Effect` module binding is in scope (type-only imports do not count).
