// @effect-diagnostics matchBooleanToIsSuccess:off
import { Effect } from "effect"

declare const saveUser: Effect.Effect<void, Error>

// MUST NOT TRIGGER: the rule is off for the whole file
export const saved = Effect.match(saveUser, { onFailure: () => false, onSuccess: () => true })
