---
"@effect/tsgo": patch
---

Move fix-only work out of rule analysis into the quick fixes, so it runs only when the editor asks for code actions instead of on every check. This covers 27 rules, among them `yieldInCallArgument`, `failCauseToConstructor`, `suspendFailToFailSync`, `catchTagToCatchReason`, `mapMapErrorToMapBoth` and `unnecessaryEffectGen`. Their diagnostics are unchanged.

Three rules now report explicitly typed calls that they used to skip, because those calls can still be rewritten by hand. They get no quick fix:

- `provideLayerSucceedToProvideService`: `Effect.provide(Layer.succeed<I, S>(Service, implementation))` (rewrite: `Effect.provideService<I, S>(Service, implementation)`), and the same for `Layer.effect<…>`
- `provideMergeAllToArray`: `Effect.provide<…>(Layer.mergeAll(...))` and `Effect.provide(Layer.mergeAll<…>(...))`
- `timeoutCatchTagToTimeoutOrElse`: explicit type arguments on the `Effect.timeout` step

```ts
// now reported, without a quick fix
program.pipe(Effect.provide(Layer.succeed<Service, { readonly value: number }>(Service, implementation)))
```
