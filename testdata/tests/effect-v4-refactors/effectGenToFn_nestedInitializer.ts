// refactor: 5:32, 9:15, 12:40, 14:44, 16:31, 19:15
import * as Effect from "effect/Effect"

export const inCatch = () => {
  try {} catch ({ task = () => Effect.gen(function*() { return 1 }) }) {}
}

export const inObject = {
  task: () => Effect.gen(function*() { return 1 })
}

export const first = 1, second = () => Effect.gen(function*() { return first })

export const inCall = Effect.succeed(() => Effect.gen(function*() { return 1 }))

export const inArray = [() => Effect.gen(function*() { return 1 })]

export const InClass = class {
  run = () => Effect.gen(function*() { return 1 })
}
