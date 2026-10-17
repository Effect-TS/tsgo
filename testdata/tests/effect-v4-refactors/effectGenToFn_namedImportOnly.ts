// refactor: 3:27
import { gen } from "effect/Effect"
export const foo = () => gen(function*() { return 1 })
