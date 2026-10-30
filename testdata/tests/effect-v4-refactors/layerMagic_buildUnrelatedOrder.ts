// refactor: 21:14-21:28
import { Effect, Layer, Context } from "effect"

class External extends Context.Service<External>()("External", {
  make: Effect.succeed({})
}) {}
class OnlyProvides extends Context.Service<OnlyProvides>()("OnlyProvides", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
class ProvidesAndRequires extends Context.Service<ProvidesAndRequires>()("ProvidesAndRequires", {
  make: Effect.as(External, {})
}) {
  static Default = Layer.effect(this, this.make)
}

const NeitherLayer = Layer.effectDiscard(Effect.void)
const OnlyRequiresLayer = Layer.effectDiscard(External)

export const unrelatedOrder = [
  NeitherLayer,
  OnlyRequiresLayer,
  OnlyProvides.Default,
  ProvidesAndRequires.Default
] as any as Layer.Layer<OnlyProvides | ProvidesAndRequires>
