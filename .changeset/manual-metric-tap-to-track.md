---
"@effect/tsgo": minor
---

Add `manualMetricTapToTrack`, an Effect v4 style suggestion with a quickfix that rewrites `Effect.tap` / `tapError` / `tapDefect` calls which only call `Metric.update` into `Effect.trackSuccesses` / `trackErrors` / `trackDefects`:

```ts
handleRequest.pipe(
  Effect.tap((size) => Metric.update(responseSize, size)),
  Effect.tapError(() => Metric.update(requestErrors, 1)),
  Effect.tap(Metric.update(rpcSuccesses, 1))
)
// becomes
handleRequest.pipe(
  Effect.trackSuccesses(responseSize),
  Effect.trackErrors(requestErrors, () => 1),
  Effect.trackSuccesses(Metric.withConstantInput(rpcSuccesses, 1))
)
```

Inside callbacks, only metrics that are stable at the call site (earlier `const` bindings, `declare const`, enum members) are moved, because `track*` evaluates the metric once while a tap callback re-evaluates it on every run. A computed input stays inside a mapper so it is still evaluated on every run.
