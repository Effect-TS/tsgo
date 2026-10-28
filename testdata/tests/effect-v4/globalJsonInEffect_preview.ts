// @effect-diagnostics *:off
// @effect-diagnostics globalJsonInEffect:warning
import { Effect } from "effect"

export const preview = Effect.gen(function*() {
  const text = yield* Effect.succeed('{"ok":true}')
  return JSON.parse(text)
})
