// refactor: 6:27, 9:27
// @effect-v3
import { gen } from "effect/Effect"
import * as Effect from "effect/Effect"

export const foo = () => gen(function*() { return 1 })

const localGen = Effect.gen
export const bar = () => localGen(function*() { return 2 })
