// @effect-diagnostics *:off
// @effect-diagnostics abortControllerInEffect:suggestion
import { Effect } from "effect"

export const preview = Effect.gen(function*() {
  return new AbortController().signal
})
