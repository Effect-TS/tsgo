---
"@effect/tsgo": minor
---

Add the `resultMatchToFromResult` style diagnostic (Effect v4). It suggests `Effect.fromResult` when `Result.match` or a `Result.isSuccess`/`Result.isFailure` ternary only lifts the success into `Effect.succeed` and the failure into `Effect.fail`, and offers a quick fix.

```ts
// before
const port = Result.match(parsePort("8080"), {
  onFailure: (error) => Effect.fail(error),
  onSuccess: (value) => Effect.succeed(value)
})
const port2 = Result.isSuccess(result) ? Effect.succeed(result.success) : Effect.fail(result.failure)

// after
const port = Effect.fromResult(parsePort("8080"))
const port2 = Effect.fromResult(result)
```

Handlers and branches may also be written as pipe flows, such as `(value) => pipe(value, Effect.succeed)` or `pipe(result.success, Effect.succeed)`, and `Result.match` is found in every piping form: data-first, data-last, `pipe`, `.pipe`, as a data-first value argument, and inside `Effect.fn` bodies.

The rule does not trigger when a handler changes the value or the error, uses explicit type arguments, recovers or swaps the channels, or when the handlers object isn't an inline literal.
