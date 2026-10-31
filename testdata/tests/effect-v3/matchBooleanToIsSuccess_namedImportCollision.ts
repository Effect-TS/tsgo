// @effect-v3
import { Effect } from "effect"
import { match } from "effect/Effect"

declare const saveUser: Effect.Effect<void, Error>
const isSuccess = 1

// MUST TRIGGER: a local isSuccess shadows the bare name -> falls back to Effect.isSuccess
export const saved = match(saveUser, { onFailure: () => false, onSuccess: () => true })
export const flag = isSuccess
