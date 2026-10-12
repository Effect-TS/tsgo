// refactor: 6:38, 10:21, 13:46
// @effect-v3
import * as Effect from "effect/Effect"

export const inCatch = () => {
  try {} catch ({ task = async () => await Promise.resolve(1) }) {}
}

export const inObject = {
  task: async () => await Promise.resolve(1)
}

export const first = 1, second = async () => await Promise.resolve(first)
