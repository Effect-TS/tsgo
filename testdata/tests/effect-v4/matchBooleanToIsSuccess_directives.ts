import { Effect } from "effect"

declare const saveUser: Effect.Effect<void, Error>

// MUST NOT TRIGGER: disabled for the next line
// @effect-diagnostics-next-line matchBooleanToIsSuccess:off
export const saved = Effect.match(saveUser, { onFailure: () => false, onSuccess: () => true })

// MUST TRIGGER: the directive only covers the line after it -> Effect.isFailure
export const failed = Effect.match(saveUser, { onFailure: () => true, onSuccess: () => false })
