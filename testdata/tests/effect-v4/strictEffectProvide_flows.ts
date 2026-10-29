// @effect-diagnostics strictEffectProvide:warning
import { Context, Effect, Layer, pipe } from "effect"
import { provide as provideIt } from "effect/Effect"

class MyService1 extends Context.Service<MyService1>()("MyService1", {
  make: Effect.succeed({ value: 1 })
}) {
  static Default = Layer.effect(this, this.make)
}

const program = Effect.gen(function*() {
  return (yield* MyService1).value
})

// Should report: curried Effect.provide applied to the program
export const curried = Effect.provide(MyService1.Default)(program)

// Should report: pipe function from effect
export const pipeFunction = pipe(program, Effect.provide(MyService1.Default))

// Should report once: Effect.fn pipeline argument
export const effectFn = Effect.fn(function*() {
  return (yield* MyService1).value
}, Effect.provide(MyService1.Default))

// Should report: aliased import of Effect.provide
export const aliased = program.pipe(provideIt(MyService1.Default))

// Should report once: nested chained pipes
export const chained = program.pipe(Effect.asVoid).pipe(Effect.provide(MyService1.Default))

// Should report: provide inside a runner call (no entry point exception)
export const runner = (signal: AbortSignal) =>
  Effect.runPromise(program.pipe(Effect.provide(MyService1.Default)), { signal })

// Should NOT report: standalone Effect.provide is not applied to an effect
export const withLive = Effect.provide(MyService1.Default)
export const appliedWithLive = program.pipe(withLive)

// Should report: data-first Effect.provide with options
export const dataFirstWithOptions = Effect.provide(program, MyService1.Default, { local: true })
