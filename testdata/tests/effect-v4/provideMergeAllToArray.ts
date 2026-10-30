// @effect-diagnostics *:off
// @effect-diagnostics provideMergeAllToArray:warning
import { Context, Effect, Layer, pipe } from "effect"
import * as E from "effect/Effect"
import * as L from "effect/Layer"
import { provide as provideEffect } from "effect/Effect"
import { mergeAll as mergeLayers } from "effect/Layer"

class A extends Context.Service<A, { readonly a: number }>()("A") {}
class B extends Context.Service<B, { readonly b: number }>()("B") {}
class C extends Context.Service<C, { readonly c: number }>()("C") {}
class D extends Context.Service<D, { readonly d: number }>()("D") {}

declare const program: Effect.Effect<void, never, A | B | C>
declare const ALayer: Layer.Layer<A>
declare const BLayer: Layer.Layer<B>
declare const CLayer: Layer.Layer<C>
declare const DLayer: Layer.Layer<D>
declare const CRequiresDLayer: Layer.Layer<C, never, D>
declare const BCLayers: readonly [Layer.Layer<B>, Layer.Layer<C>]
declare const ABCLayers: readonly [Layer.Layer<A>, Layer.Layer<B>, Layer.Layer<C>]
declare const moreLayers: Array<Layer.Layer<B> | Layer.Layer<C>>
declare const condition: boolean

const provide = Effect.provide
const mergeAll = Layer.mergeAll

// Location: every MUST TRIGGER case is reported on the Layer.mergeAll(...) call written as the
// provide argument (the node the quickfix replaces), not on the whole Effect.provide call.

// MUST TRIGGER: pipeable form, the issue's first example
// quickfix: Effect.provide([ALayer, BLayer, CLayer])
export const pipeable = program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer)))

// MUST TRIGGER: data-first form, the issue's second example
// quickfix: Effect.provide(program, [ALayer, BLayer, CLayer])
export const dataFirst = Effect.provide(program, Layer.mergeAll(ALayer, BLayer, CLayer))

// MUST TRIGGER: function pipe form
export const functionPipe = pipe(program, Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer)))

// MUST TRIGGER: curried form applied immediately
export const curried = Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer))(program)

// MUST TRIGGER: the options argument is kept; arrays accept the same options in v4
// quickfix: Effect.provide([ALayer, BLayer, CLayer], { local: true })
export const pipeableWithOptions = program.pipe(
  Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer), { local: true })
)

// MUST TRIGGER: data-first form with options
export const dataFirstWithOptions = Effect.provide(program, Layer.mergeAll(ALayer, BLayer, CLayer), {
  local: false
})

// MUST TRIGGER: namespace imports under other names (resolved through types, not identifier text)
export const namespaceAliases = program.pipe(E.provide(L.mergeAll(ALayer, BLayer, CLayer)))

// MUST TRIGGER: renamed named imports
export const renamedImports = program.pipe(provideEffect(mergeLayers(ALayer, BLayer, CLayer)))

// MUST TRIGGER: constant aliases of Effect.provide and Layer.mergeAll
export const constantAliases = program.pipe(provide(mergeAll(ALayer, BLayer, CLayer)))

// MUST TRIGGER: a single layer; the fix still produces an array
// quickfix: Effect.provide([ALayer])
export const singleLayer = Effect.succeed(1).pipe(Effect.provide(Layer.mergeAll(ALayer)))

// MUST TRIGGER: layer expressions are kept as written, including inline constructors
// quickfix: Effect.provide([ALayer, Layer.succeed(B, { b: 1 }), CLayer])
export const inlineConstructors = program.pipe(
  Effect.provide(Layer.mergeAll(ALayer, Layer.succeed(B, { b: 1 }), CLayer))
)

// MUST TRIGGER: only the mergeAll written directly as the provide argument is reported;
// the nested mergeAll is a mergeAll argument and is kept as written
// quickfix: Effect.provide([ALayer, Layer.mergeAll(BLayer, CLayer)])
export const nestedMergeAll = program.pipe(Effect.provide(Layer.mergeAll(ALayer, Layer.mergeAll(BLayer, CLayer))))

// MUST TRIGGER: multiline arguments with comments and a trailing comma keep their layout
export const multiline = program.pipe(
  Effect.provide(
    Layer.mergeAll(
      ALayer, // the A service
      BLayer,
      /* the C service */ CLayer,
    )
  )
)

// MUST TRIGGER: a spread argument is kept inside the array
// quickfix: Effect.provide([ALayer, ...BCLayers])
export const spreadTuple = program.pipe(Effect.provide(Layer.mergeAll(ALayer, ...BCLayers)))

// MUST TRIGGER: a spread of a non-empty array type after a leading layer
// quickfix: Effect.provide([ALayer, ...moreLayers])
export const spreadArray = program.pipe(Effect.provide(Layer.mergeAll(ALayer, ...moreLayers)))

// MUST TRIGGER: the only argument is a spread of a non-empty tuple; the array literal stays a
// non-empty tuple, so the rewritten call type-checks with the same type
// quickfix: Effect.provide([...ABCLayers])
export const spreadOnlyTuple = program.pipe(Effect.provide(Layer.mergeAll(...ABCLayers)))

// MUST TRIGGER: a parenthesized mergeAll argument is still the provide argument
// quickfix: Effect.provide([ALayer, BLayer, CLayer])
export const parenthesized = program.pipe(Effect.provide((Layer.mergeAll(ALayer, BLayer, CLayer))))

// MUST TRIGGER: inside Effect.gen
export const insideGen = Effect.gen(function*() {
  return yield* Effect.provide(program, Layer.mergeAll(ALayer, BLayer, CLayer))
})

// MUST NOT TRIGGER: already an array
export const alreadyArray = program.pipe(Effect.provide([ALayer, BLayer, CLayer]))

// MUST NOT TRIGGER: the merged layer is stored in a variable and reused
const AppLayer = Layer.mergeAll(ALayer, BLayer, CLayer)
export const storedInVariable = program.pipe(Effect.provide(AppLayer))
export const storedInVariableAgain = Effect.provide(program, AppLayer)

// MUST NOT TRIGGER: the merged layer is composed further before reaching provide
export const composedFurther = program.pipe(
  Effect.provide(Layer.mergeAll(ALayer, BLayer, CRequiresDLayer).pipe(Layer.provide(DLayer)))
)

// MUST NOT TRIGGER: the merged layer is composed through a data-first Layer combinator
export const composedDataFirst = program.pipe(
  Effect.provide(Layer.provide(Layer.mergeAll(ALayer, BLayer, CRequiresDLayer), DLayer))
)

// MUST NOT TRIGGER: the mergeAll call is cast, so it is not written directly as the argument
export const castArgument = program.pipe(
  Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer) as Layer.Layer<A | B | C>)
)

// MUST NOT TRIGGER: the mergeAll call is only one branch of the argument
export const conditionalArgument = program.pipe(
  Effect.provide(condition ? Layer.mergeAll(ALayer, BLayer, CLayer) : Layer.mergeAll(CLayer, BLayer, ALayer))
)

// MUST NOT TRIGGER: Layer.provide / Layer.provideMerge build layers, they are not Effect.provide
export const layerProvide = CRequiresDLayer.pipe(Layer.provide(Layer.mergeAll(DLayer, ALayer)))
export const layerProvideMerge = CRequiresDLayer.pipe(Layer.provideMerge(Layer.mergeAll(DLayer, ALayer)))

// MUST NOT TRIGGER: Layer.merge is not Layer.mergeAll
export const binaryMerge = program.pipe(Effect.provide(Layer.merge(ALayer, BLayer)), Effect.provide(CLayer))

// MUST TRIGGER: the layers depend on each other; layerMergeAllWithDependencies also reports this call
// quickfix: Effect.provide([DLayer, CRequiresDLayer])
export const interdependentLayers = Effect.void.pipe(
  Effect.provide(Layer.mergeAll(DLayer, CRequiresDLayer))
)

// MUST NOT TRIGGER: unrelated APIs with the same method names
const unrelated = {
  provide: (value: unknown) => value,
  mergeAll: (...values: ReadonlyArray<unknown>) => values
}
export const unrelatedCall = unrelated.provide(unrelated.mergeAll(ALayer, BLayer))
