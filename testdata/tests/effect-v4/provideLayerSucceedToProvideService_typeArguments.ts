// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics provideLayerSucceedToProvideService:warning
import { Context, Effect, Layer } from "effect"

class Service extends Context.Service<Service, { readonly value: number }>()("Service") {}

const implementation = { value: 1 }
const acquisition = Effect.succeed(implementation)
const program = Service.use((service) => Effect.succeed(service.value))

// MUST TRIGGER without a fix: Effect.provideService<Service, { readonly value: number }>(Service, implementation)
// can be written by hand, but the fix does not carry type arguments over
export const succeedTypeArguments = program.pipe(
  Effect.provide(Layer.succeed<Service, { readonly value: number }>(Service, implementation))
)

// MUST TRIGGER without a fix: the same for Layer.effect and Effect.provideServiceEffect
export const effectTypeArguments = program.pipe(
  Effect.provide(Layer.effect<Service, { readonly value: number }, never, never>(Service, acquisition))
)

// MUST TRIGGER with a fix: no type arguments
export const plain = program.pipe(Effect.provide(Layer.succeed(Service, implementation)))
