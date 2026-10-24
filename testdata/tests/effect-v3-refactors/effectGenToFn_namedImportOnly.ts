// refactor: 4:27
// @effect-v3
import { gen } from "effect/Effect"
export const foo = () => gen(function*() { return 1 })
