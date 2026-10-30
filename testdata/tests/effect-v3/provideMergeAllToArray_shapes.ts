// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics provideMergeAllToArray:warning
import { Context, Effect, Layer } from "effect"

class A extends Context.Tag("A")<A, { readonly a: number }>() {}
class B extends Context.Tag("B")<B, { readonly b: number }>() {}

declare const ALayer: Layer.Layer<A>
declare const BLayer: Layer.Layer<B>
declare const program: Effect.Effect<void, never, A | B>
const café = "ü"

// MUST TRIGGER: a non-ASCII identifier before the call on the same line; the fix keeps the comments
// -> [café, program.pipe(Effect.provide(/* app */ [ALayer, BLayer /* last */]))]
export const nonAsciiPrefix = [café, program.pipe(Effect.provide(/* app */ Layer.mergeAll(ALayer, BLayer /* last */)))]

// MUST TRIGGER: pipeable argument of Effect.fn
export const effectFn = Effect.fn(function*() {
  yield* program
}, Effect.provide(Layer.mergeAll(ALayer, BLayer)))

// MUST TRIGGER: pipeable argument of Effect.fnUntraced
export const effectFnUntraced = Effect.fnUntraced(function*() {
  yield* program
}, Effect.provide(Layer.mergeAll(ALayer, BLayer)))

// MUST NOT TRIGGER: the curried Effect.provide is never applied to an effect
export const unapplied = Effect.provide(Layer.mergeAll(ALayer, BLayer))

// MUST NOT TRIGGER: explicit type arguments on Layer.mergeAll
export const explicitMergeAllTypeArguments = program.pipe(
  Effect.provide(Layer.mergeAll<[Layer.Layer<A>, Layer.Layer<B>]>(ALayer, BLayer))
)

// MUST NOT TRIGGER: explicit type arguments on Effect.provide
export const explicitProvideTypeArguments = program.pipe(Effect.provide<A, never, never>(Layer.mergeAll(ALayer)))

// MUST NOT TRIGGER: Layer.mergeAll without arguments
// @ts-expect-error mergeAll requires at least one layer
export const noArguments = program.pipe(Effect.provide(Layer.mergeAll()))

// MUST NOT TRIGGER: disabled by a next-line directive
// @effect-diagnostics-next-line provideMergeAllToArray:off
export const disabledNextLine = program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer)))
