// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics scopeInLayerEffect:warning
// @effect-diagnostics effectMapVoid:warning
import * as X from "effect"

class MyService extends X.Context.Tag("MyService")<MyService, {
  a: number
}>() {}

// The fix builds X.Layer.scoped from the barrel namespace import
export const myLayer = X.Layer.effect(
  MyService,
  X.Effect.gen(function*() {
    yield* X.Effect.addFinalizer(() => X.Effect.log("finalizer"))
    return { a: 1 }
  })
)

// The fix builds X.Effect.asVoid from the barrel namespace import
export const mapVoid = X.Effect.succeed(1).pipe(
  X.Effect.map(() => undefined)
)
