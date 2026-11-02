// @effect-diagnostics *:off
// @effect-diagnostics matchBooleanToIsSuccess:warning
import { Effect } from "effect"

declare const saveUser: Effect.Effect<void, Error>

export const saved = Effect.match(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})
