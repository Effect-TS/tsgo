import { Context, Effect, Layer } from "effect"

class Service extends Context.Service<Service, { readonly value: number }>()("Service") {}

const program = Service.use((service) => Effect.succeed(service.value)).pipe(
  Effect.provide(Layer.succeed(Service, { value: 1 }))
)
