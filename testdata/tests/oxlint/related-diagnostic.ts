import { Effect } from "effect"

export const program = Effect.gen(function*() {
  yield Effect.succeed(1)
})
