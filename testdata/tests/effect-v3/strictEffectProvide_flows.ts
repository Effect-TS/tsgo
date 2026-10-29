// @effect-v3
// @effect-diagnostics strictEffectProvide:warning
import * as Effect from "effect/Effect"
import { provide as provideIt } from "effect/Effect"
import { pipe } from "effect/Function"

class MyService1 extends Effect.Service<MyService1>()("MyService1", {
  succeed: { value: 1 }
}) {}

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

// Should NOT report: the provided Effect goes straight into a runner
export const runner = (signal: AbortSignal) =>
  Effect.runPromise(program.pipe(Effect.provide(MyService1.Default)), { signal })

// Should NOT report: standalone Effect.provide is not applied to an effect
export const withLive = Effect.provide(MyService1.Default)
export const appliedWithLive = program.pipe(withLive)
