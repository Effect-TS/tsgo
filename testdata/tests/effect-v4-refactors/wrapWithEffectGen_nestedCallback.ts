// refactor: 3:21-3:22
import * as Effect from "effect/Effect"

export const task = Effect.promise(() => {
  if (Math.random() > 0.5) {
    return Promise.resolve(1)
  }
  if (Math.random() > 0.2) {
    return Promise.resolve(2)
  }
  return Promise.resolve(3)
})
