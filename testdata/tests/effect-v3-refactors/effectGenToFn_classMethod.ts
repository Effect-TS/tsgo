// refactor: 7:17
// @effect-v3
import * as Eff from "effect/Effect"

class Test {
  methodReturnsEffect(arg1: number, arg2: number) {
    return Eff.gen(function*() {
      const a = yield* Eff.succeed(arg1)
      const b = yield* Eff.succeed(arg2)
      return a + b
    })
  }
}
