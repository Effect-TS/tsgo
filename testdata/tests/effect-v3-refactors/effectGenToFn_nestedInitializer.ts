// refactor: 6:32, 10:15, 13:40
// @effect-v3
import * as Effect from "effect/Effect"

export const outer = () => {
  try {} catch ({ task = () => Effect.gen(function*() { return 1 }) }) {}
}

export const nested = {
  task: () => Effect.gen(function*() { return 1 })
}

export const first = 1, second = () => Effect.gen(function*() { return first })
