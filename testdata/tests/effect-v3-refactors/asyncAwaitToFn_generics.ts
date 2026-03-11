// refactor: 5:28-5:29
// @effect-v3
import * as Effect from "effect/Effect"

export async function refactorMe<X>(arg: X) {
  return await Promise.resolve(arg)
}
