// refactor: 6:8, 13:8, 22:16, 30:8, 5:17, 12:21
// @effect-v3
import * as Eff from "effect/Effect"

export const program = () =>
  Eff.gen(function*() {
    const a = yield* Eff.succeed(1)
    const b = yield* Eff.succeed(2)
    return a + b
  })

export const programWithPipes = (fa: number, fb: number) =>
  Eff.gen(function*() {
    const a = yield* Eff.succeed(fa)
    const b = yield* Eff.succeed(fb)
    return a + b
  }).pipe(
    Eff.map((a) => a + 1)
  )

export function sampleReturns<A extends number, B extends number>(arg1: A, arg2: B) {
  return Eff.gen(function*() {
    const a = yield* Eff.succeed(arg1)
    const b = yield* Eff.succeed(arg2)
    return a + b
  })
}

export const sampleReturnsConciseBody = <A extends number, B extends number>(arg1: A, arg2: B) =>
  Eff.gen(function*() {
    const a = yield* Eff.succeed(arg1)
    const b = yield* Eff.succeed(arg2)
    return a + b
  })
