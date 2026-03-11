// refactor: 5:28-5:29
// @effect-v3
import * as T from "effect/Effect"

export async function refactorMe(arg: string) {
  for (let i = 0; i < 10; i++) {
    await Promise.resolve(i)
  }
  return await Promise.resolve(arg)
}
