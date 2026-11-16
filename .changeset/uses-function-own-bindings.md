---
"@effect/tsgo": patch
---

`manualMetricTapToTrack` now decides more precisely whether a function-expression callback can become an arrow mapper. It looks for the callback's own `this`, `super`, `new.target`, `arguments` and name the way the compiler resolves them, through the new typeparser helper `UsesFunctionOwnBindings`, which `flatMapProvideServiceToProvideServiceEffect` now uses too.

The rule now reports callbacks it used to skip, since none of these read a binding of the callback:

```ts
Effect.tap(function(size) { return Metric.update(responseSize, size + sizes.arguments) })       // a property named `arguments`
Effect.tap(function(size) { return Metric.update(responseSize, size + import.meta.url.length) }) // `import.meta`
Effect.tap(function(size) {                                                                     // a class that uses its own `this` and `arguments`
  return Metric.update(responseSize, size + new (class { readonly n = this; m() { return arguments.length } })().m())
})
```

It no longer reports a named callback that reads its own name through a shorthand property, which the fix would have left unbound:

```ts
Effect.tap(function record(size) { return Metric.update(responseSize, size + Object.keys({ record }).length) })
```

Class expressions are still skipped when their `extends` clause, a computed member name or a member decorator reads the callback's bindings.
