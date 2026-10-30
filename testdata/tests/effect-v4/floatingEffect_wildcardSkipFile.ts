// @effect-diagnostics *:skip-file
// @effect-diagnostics floatingEffect:error
import { Effect } from "effect"

Effect.succeed("skip-file wins over later section directives")

export const program = Effect.gen(function*() {
  return 1
})
