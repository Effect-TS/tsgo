---
"@effect/tsgo": patch
---

Detect `lazyPromiseInEffectSync` and `scopeInLayerEffect` through piping flows instead of a hand-rolled AST walk, so both rules now also report point-free, pipeable and curried forms.

`lazyPromiseInEffectSync` now reports `Effect.sync` used point-free in a pipe:

```ts
// now reported, in addition to Effect.sync(() => Promise.resolve(1))
export const fromPointFree = pipe(() => Promise.resolve(1), Effect.sync)
export const fromPointFreeChain = pipe(1, (n) => () => Promise.resolve(n), Effect.sync)
```

`scopeInLayerEffect` now reports the pipeable and curried `Layer.effect` forms, and its "Use scoped for Layer creation" quick fix rewrites them to `Layer.scoped`:

```ts
// now reported, in addition to Layer.effect(MyService, eff)
export const pipeableLayer = eff.pipe(Layer.effect(MyService))
export const curriedLayer = Layer.effect(MyService)(eff)
```
