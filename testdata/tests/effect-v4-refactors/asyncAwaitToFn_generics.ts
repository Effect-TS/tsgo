// refactor: 4:28-4:29
import * as Effect from "effect/Effect"

export async function refactorMe<X>(arg: X) {
  return await Promise.resolve(arg)
}
