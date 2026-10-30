---
"@effect/tsgo": minor
---

Add the `provideMergeAllToArray` style diagnostic (default severity `suggestion`, Effect v3 and v4). `Effect.provide` accepts an array of layers and merges them itself, so wrapping them in `Layer.mergeAll` adds nothing. The quick fix "Pass the layers as an array to Effect.provide" rewrites the argument to an array, keeping each layer's text, spreads, comments, and any v4 `options` argument. Calls whose layers depend on each other (the ones `layerMergeAllWithDependencies` reports) are skipped.

```ts
// before
program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer)))
Effect.provide(program, Layer.mergeAll(ALayer, ...BCLayers))

// after quick fix
program.pipe(Effect.provide([ALayer, BLayer, CLayer]))
Effect.provide(program, [ALayer, ...BCLayers])

// not reported
const AppLayer = Layer.mergeAll(ALayer, BLayer)
program.pipe(Effect.provide(AppLayer))
program.pipe(Effect.provide(Layer.mergeAll(DLayer, CRequiresDLayer)))
```
