---
"@effect/tsgo": minor
---

Add the `sleepThenEffectToDelay` diagnostic (suggestion, style). Sequencing `Effect.sleep(d)` in front of an effect with `Effect.andThen` or `Effect.flatMap` (and `Effect.zipRight` on Effect v3) re-implements `Effect.delay`, which is defined exactly that way. The quick fix rewrites the pair to `Effect.delay`, keeping a continuation that builds its effect lazy with `Effect.suspend`:

```ts
Effect.sleep("1 second").pipe(Effect.andThen(sendRequest))
// -> sendRequest.pipe(Effect.delay("1 second"))

Effect.flatMap(Effect.sleep(retryAfter), () => loop(request))
// -> Effect.delay(Effect.suspend(() => loop(request)), retryAfter)
```

The diagnostic is always reported, but the fix is only offered when the rewrite cannot change behaviour:

- `Effect.delay(effect, d)` evaluates the effect expression before the duration, while the sleep-first forms evaluate and read the duration first. The fix requires that evaluating the continuation cannot change anything (a function, or a stable value such as a literal or const binding), that the duration is a stable primitive the continuation cannot change, or that the call is the curried `Effect.andThen(effect)(Effect.sleep(d))` form. A const binding of a tuple or `Duration` object does not count, since the continuation could mutate it.
- A continuation that builds its effect is passed to `Effect.suspend`, which calls it without the sleep result. The fix requires an inline arrow or a const binding initialized with an arrow, since any other function could observe the difference through `arguments` or `this`.

Standalone `yield* Effect.sleep(d)` statements in generators, `Effect.sleep` passed as a pipe stage (`pipe(d, Effect.sleep, Effect.andThen(next))`), pipes whose first stage after the sleep is not the sequencer, `tap`/`zip`/`as`/`map`, callbacks that read the sleep result, and v3 `zipRight` with options are not reported.

Data-first calls of Effect v3 combinators whose return type is a conditional type with `infer` binders, such as `Effect.andThen(self, f)` and `Effect.tap(self, f)`, are now recognized as piping flows, so rules that match them (for example `flatMapToTap`) also report the data-first form on Effect v3.

Quick fixes that rewrite a curried call such as `(Effect.andThen(next))(self)` now replace the whole application even when the factory call is parenthesized, instead of only the parenthesized factory call.
