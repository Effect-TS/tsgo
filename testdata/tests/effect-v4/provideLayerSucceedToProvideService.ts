import { Context, Effect, Layer, pipe } from "effect"

class Service extends Context.Service<Service, { readonly value: number }>()("Service") {}

const implementation = { value: 1 }
const acquisition = Effect.succeed(implementation)
const program = Service.use((service) => Effect.succeed(service.value))

// Should trigger: pipeable, function-pipe, data-first, and curried layer constructors.
export const pipeableSucceed = program.pipe(Effect.provide(Layer.succeed(Service, implementation)))
export const functionPipeEffect = pipe(program, Effect.provide(Layer.effect(Service, acquisition)))
export const dataFirstSucceed = Effect.provide(program, Layer.succeed(Service)(implementation))
export const curriedEffect = Effect.provide(Layer.effect(Service)(acquisition))(program)

// Should preserve the arguments' multiline shape in the replacement.
export const multiline = program.pipe(
  Effect.provide(Layer.succeed(
    Service,
    {
      value: 2
    }
  ))
)

// Should not trigger: named layers may intentionally be shared and memoized.
const sharedLayer = Layer.succeed(Service, implementation)
export const namedLayer = program.pipe(Effect.provide(sharedLayer))

// Should not trigger: layer-to-layer provision is a different operation.
export const layerProvision = Layer.empty.pipe(Layer.provide(Layer.succeed(Service, implementation)))

// Should not trigger: unrelated APIs with the same method names.
const unrelated = {
  provide: (value: unknown) => value,
  succeed: (...args: ReadonlyArray<unknown>) => args
}
export const unrelatedCall = unrelated.provide(unrelated.succeed(Service, implementation))
