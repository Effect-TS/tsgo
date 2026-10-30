import { Effect } from "effect"

export const program = Effect.gen(function*() {
  // @effect-diagnostics-next-line *:off
  yield* Effect.log("wildcard next-line directives are never reported as unused")
  // @effect-diagnostics-next-line floatingEffect:off
  yield* Effect.log("unused rule next-line directive")
  // @effect-diagnostics-next-line floatingEffect:off
  Effect.log("used rule next-line directive")
})
